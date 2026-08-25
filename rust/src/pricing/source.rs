//! Official and supplemental pricing source adapters.

use super::catalog::{
    CatalogEntry, CatalogValidationError, ContextTier, Currency, MoneyMicros, PriceProvenance,
    TokenRates, valid_public_source_url,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;

pub use super::sources::{
    DeepSeekAdapter, KimiAdapter, ModelsDevAdapter, OpenAiAdapter, QwenAdapter, XaiAdapter,
};

const TOKENS_UNIT_MILLION: &str = "million";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingSourceId {
    OpenAi,
    DeepSeek,
    Xai,
    Kimi,
    Qwen,
    ModelsDev,
}

impl PricingSourceId {
    pub fn vendor_name(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::DeepSeek => "deepseek",
            Self::Xai => "xai",
            Self::Kimi => "kimi",
            Self::Qwen => "qwen",
            Self::ModelsDev => "models.dev",
        }
    }

    pub fn provenance(self) -> PriceProvenance {
        match self {
            Self::ModelsDev => PriceProvenance::SupplementalCatalog,
            _ => PriceProvenance::OfficialLive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub source_id: PricingSourceId,
    pub source_url: String,
    pub fetched_at: DateTime<Utc>,
    pub parser_revision: String,
    pub entries: Vec<CatalogEntry>,
}

#[derive(Debug, Error)]
pub enum PricingSourceError {
    #[error("failed to fetch pricing source: {0}")]
    Fetch(String),
    #[error("pricing source response did not match the expected schema")]
    InvalidShape,
    #[error("pricing source contained no usable model rates")]
    Empty,
    #[error("pricing source URL is not a public HTTPS document")]
    InvalidSourceUrl,
    #[error(transparent)]
    InvalidCatalog(#[from] CatalogValidationError),
}

pub trait PricingSourceAdapter {
    fn id(&self) -> PricingSourceId;
    fn source_url(&self) -> &'static str;
    fn parser_revision(&self) -> &'static str;
    fn parse(&self, body: &str) -> Result<SourceSnapshot, PricingSourceError>;
    fn fetch<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> impl std::future::Future<Output = Result<SourceSnapshot, PricingSourceError>> + Send + 'a
    where
        Self: Sync,
    {
        async move {
            let response = client
                .get(self.source_url())
                .send()
                .await
                .map_err(|error| PricingSourceError::Fetch(error.to_string()))?;
            let body = response
                .text()
                .await
                .map_err(|error| PricingSourceError::Fetch(error.to_string()))?;
            self.parse(&body)
        }
    }
}

pub(crate) fn snapshot_from_official_document(
    adapter: &impl PricingSourceAdapter,
    body: &str,
    fetched_at: DateTime<Utc>,
    input_key: &str,
    cached_key: &str,
    output_key: &str,
) -> Result<SourceSnapshot, PricingSourceError> {
    let root = serde_json::from_str::<Value>(body).map_err(|_| PricingSourceError::InvalidShape)?;
    let object = root.as_object().ok_or(PricingSourceError::InvalidShape)?;
    let currency = parse_currency(object.get("currency"))?;
    require_million_token_unit(object.get("tokenUnit"))?;
    let models = object
        .get("models")
        .and_then(Value::as_array)
        .ok_or(PricingSourceError::InvalidShape)?;
    if models.is_empty() {
        return Err(PricingSourceError::Empty);
    }

    let mut entries = Vec::new();
    for model in models {
        let Some(entry) = official_model_entry(
            adapter, model, currency, fetched_at, input_key, cached_key, output_key,
        )?
        else {
            continue;
        };
        entries.push(entry);
    }
    finish_snapshot(adapter, fetched_at, entries)
}

pub(crate) fn finish_snapshot(
    adapter: &impl PricingSourceAdapter,
    fetched_at: DateTime<Utc>,
    entries: Vec<CatalogEntry>,
) -> Result<SourceSnapshot, PricingSourceError> {
    if entries.is_empty() {
        return Err(PricingSourceError::Empty);
    }
    if !valid_public_source_url(adapter.source_url()) {
        return Err(PricingSourceError::InvalidSourceUrl);
    }
    Ok(SourceSnapshot {
        source_id: adapter.id(),
        source_url: adapter.source_url().to_string(),
        fetched_at,
        parser_revision: adapter.parser_revision().to_string(),
        entries,
    })
}

fn official_model_entry(
    adapter: &impl PricingSourceAdapter,
    model: &Value,
    currency: Currency,
    fetched_at: DateTime<Utc>,
    input_key: &str,
    cached_key: &str,
    output_key: &str,
) -> Result<Option<CatalogEntry>, PricingSourceError> {
    let object = model.as_object().ok_or(PricingSourceError::InvalidShape)?;
    let canonical_model = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(PricingSourceError::InvalidShape)?;
    let Some(input) = money_from_json(object.get(input_key))? else {
        return Ok(None);
    };
    let cached = money_from_json(object.get(cached_key))?.unwrap_or(input);
    let Some(output) = money_from_json(object.get(output_key))? else {
        return Ok(None);
    };
    let context_tiers = parse_context_tiers(object.get("contextTiers"))?;
    Ok(Some(catalog_entry(
        adapter,
        canonical_model,
        TokenRates {
            currency,
            input_per_million: input,
            cached_input_per_million: cached,
            output_per_million: output,
            context_tiers,
        },
        fetched_at,
    )?))
}

pub(crate) fn catalog_entry(
    adapter: &impl PricingSourceAdapter,
    canonical_model: &str,
    rates: TokenRates,
    fetched_at: DateTime<Utc>,
) -> Result<CatalogEntry, PricingSourceError> {
    let entry = CatalogEntry {
        canonical_model: canonical_model.trim().to_ascii_lowercase(),
        vendor: adapter.id().vendor_name().to_string(),
        rates,
        source_url: adapter.source_url().to_string(),
        fetched_at,
        parser_revision: adapter.parser_revision().to_string(),
        provenance: adapter.id().provenance(),
    };
    crate::pricing::catalog::validate_catalog_entry(&entry, 0)?;
    Ok(entry)
}

pub(crate) fn parse_currency(value: Option<&Value>) -> Result<Currency, PricingSourceError> {
    match value.and_then(Value::as_str).map(str::trim) {
        Some("USD") => Ok(Currency::Usd),
        Some("CNY") => Ok(Currency::Cny),
        _ => Err(PricingSourceError::InvalidShape),
    }
}

pub(crate) fn require_million_token_unit(value: Option<&Value>) -> Result<(), PricingSourceError> {
    match value.and_then(Value::as_str).map(str::trim) {
        Some(TOKENS_UNIT_MILLION) => Ok(()),
        _ => Err(PricingSourceError::InvalidShape),
    }
}

fn parse_context_tiers(value: Option<&Value>) -> Result<Vec<ContextTier>, PricingSourceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value.as_array().ok_or(PricingSourceError::InvalidShape)?;
    let mut tiers = Vec::new();
    for item in array {
        let object = item.as_object().ok_or(PricingSourceError::InvalidShape)?;
        let above_input_tokens = object
            .get("aboveInputTokens")
            .and_then(Value::as_u64)
            .ok_or(PricingSourceError::InvalidShape)?;
        let Some(input) = money_from_json(object.get("inputPerMillion"))? else {
            return Err(PricingSourceError::InvalidShape);
        };
        let Some(cached) = money_from_json(object.get("cachedInputPerMillion"))? else {
            return Err(PricingSourceError::InvalidShape);
        };
        let Some(output) = money_from_json(object.get("outputPerMillion"))? else {
            return Err(PricingSourceError::InvalidShape);
        };
        tiers.push(ContextTier {
            above_input_tokens,
            input_per_million: input,
            cached_input_per_million: cached,
            output_per_million: output,
        });
    }
    Ok(tiers)
}

pub(crate) fn money_from_json(
    value: Option<&Value>,
) -> Result<Option<MoneyMicros>, PricingSourceError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        _ => return Err(PricingSourceError::InvalidShape),
    };
    Ok(Some(
        parse_major_units_to_micros(&text).ok_or(PricingSourceError::InvalidShape)?,
    ))
}

fn parse_major_units_to_micros(text: &str) -> Option<MoneyMicros> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('+') || text.starts_with('-') {
        return None;
    }
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (text, ""),
    };
    let whole = if whole.is_empty() { "0" } else { whole };
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    if fraction.len() > 6 && fraction.bytes().skip(6).any(|byte| byte != b'0') {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let mut padded = fraction.as_bytes().to_vec();
    padded.resize(6, b'0');
    let fraction: i64 = std::str::from_utf8(&padded[..6]).ok()?.parse().ok()?;
    let micros = whole.checked_mul(1_000_000)?.checked_add(fraction)?;
    Some(MoneyMicros::from_micros(micros))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::model_alias::{AliasResolution, ModelAliasResolver};

    #[test]
    fn deepseek_fixture_preserves_native_cny_and_cache_dimensions() {
        let snapshot = DeepSeekAdapter
            .parse(include_str!("fixtures/deepseek.json"))
            .unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.canonical_model == "deepseek-v4-flash")
            .unwrap();
        assert_eq!(entry.rates.currency, Currency::Cny);
        assert!(entry.rates.cached_input_per_million < entry.rates.input_per_million);
        assert_eq!(entry.provenance, PriceProvenance::OfficialLive);
        assert_eq!(entry.source_url, DeepSeekAdapter.source_url());
    }

    #[test]
    fn openai_fixture_keeps_context_tiers_and_cached_input() {
        let snapshot = OpenAiAdapter
            .parse(include_str!("fixtures/openai.json"))
            .unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.canonical_model == "gpt-5.6-sol")
            .unwrap();
        assert_eq!(entry.rates.currency, Currency::Usd);
        assert_eq!(entry.rates.context_tiers.len(), 1);
        assert_eq!(entry.rates.context_tiers[0].above_input_tokens, 200_000);
        assert!(entry.rates.cached_input_per_million < entry.rates.input_per_million);
    }

    #[test]
    fn models_dev_fixture_is_supplemental_and_keeps_cache_dimensions() {
        let snapshot = ModelsDevAdapter
            .parse(include_str!("fixtures/models-dev.json"))
            .unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.canonical_model == "gpt-fresh")
            .unwrap();
        assert_eq!(entry.provenance, PriceProvenance::SupplementalCatalog);
        assert_eq!(entry.vendor, "models.dev");
        assert!(entry.rates.cached_input_per_million < entry.rates.input_per_million);
        assert_eq!(entry.rates.context_tiers.len(), 1);
    }

    #[test]
    fn official_parser_rejects_a_changed_document_shape() {
        let body = r#"{"currency":"USD","models":[{"id":"gpt-5.6-sol"}]}"#;
        assert!(matches!(
            OpenAiAdapter.parse(body),
            Err(PricingSourceError::InvalidShape)
        ));
    }

    #[test]
    fn gateway_alias_maps_only_an_exact_model_suffix() {
        assert_eq!(
            ModelAliasResolver::default().resolve_alias("4sapi-gpt/gpt-5.6-sol"),
            AliasResolution::Exact("gpt-5.6-sol".into())
        );
        assert_eq!(
            ModelAliasResolver::default().resolve_alias("4sapi-gpt/gpt-5.6"),
            AliasResolution::None
        );
        assert_eq!(
            ModelAliasResolver::default().resolve_alias("xai/grok-4.6"),
            AliasResolution::Exact("grok-4.6".into())
        );
        assert_eq!(
            ModelAliasResolver::default().resolve_alias("openai/gpt-5.6-sol"),
            AliasResolution::Exact("gpt-5.6-sol".into())
        );
    }
}
