use crate::pricing::catalog::{ContextTier, Currency, TokenRates};
use crate::pricing::source::{
    PricingSourceAdapter, PricingSourceError, PricingSourceId, SourceSnapshot, catalog_entry,
    finish_snapshot, money_from_json,
};
use chrono::Utc;
use serde_json::Value;

pub struct ModelsDevAdapter;

impl PricingSourceAdapter for ModelsDevAdapter {
    fn id(&self) -> PricingSourceId {
        PricingSourceId::ModelsDev
    }

    fn source_url(&self) -> &'static str {
        "https://models.dev/api.json"
    }

    fn parser_revision(&self) -> &'static str {
        "models-dev-supplemental-v1"
    }

    fn parse(&self, body: &str) -> Result<SourceSnapshot, PricingSourceError> {
        let root =
            serde_json::from_str::<Value>(body).map_err(|_| PricingSourceError::InvalidShape)?;
        let providers = match root {
            Value::Object(map) if map.contains_key("providers") => map
                .get("providers")
                .and_then(Value::as_object)
                .cloned()
                .ok_or(PricingSourceError::InvalidShape)?,
            Value::Object(map) => map,
            _ => return Err(PricingSourceError::InvalidShape),
        };

        let mut entries = Vec::new();
        for (_provider_key, provider) in providers {
            let Some(models) = provider.get("models").and_then(Value::as_object) else {
                continue;
            };
            for (_model_key, model) in models {
                if let Some(entry) = supplemental_model_entry(self, model)? {
                    entries.push(entry);
                }
            }
        }
        finish_snapshot(self, Utc::now(), entries)
    }
}

fn supplemental_model_entry(
    adapter: &ModelsDevAdapter,
    model: &Value,
) -> Result<Option<crate::pricing::catalog::CatalogEntry>, PricingSourceError> {
    let object = model.as_object().ok_or(PricingSourceError::InvalidShape)?;
    let raw_id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(PricingSourceError::InvalidShape)?;
    let canonical_model = raw_id.rsplit('/').next().unwrap_or(raw_id);
    let Some(cost) = object.get("cost").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(input) = money_from_json(cost.get("input"))? else {
        return Ok(None);
    };
    let Some(output) = money_from_json(cost.get("output"))? else {
        return Ok(None);
    };
    let cached = money_from_json(cost.get("cache_read"))?.unwrap_or(input);
    let mut context_tiers = Vec::new();
    if let Some(above) = cost.get("context_over_200k").and_then(Value::as_object) {
        let Some(tier_input) = money_from_json(above.get("input"))? else {
            return Err(PricingSourceError::InvalidShape);
        };
        let Some(tier_output) = money_from_json(above.get("output"))? else {
            return Err(PricingSourceError::InvalidShape);
        };
        let tier_cached = money_from_json(above.get("cache_read"))?.unwrap_or(tier_input);
        context_tiers.push(ContextTier {
            above_input_tokens: 200_000,
            input_per_million: tier_input,
            cached_input_per_million: tier_cached,
            output_per_million: tier_output,
        });
    }
    Ok(Some(catalog_entry(
        adapter,
        canonical_model,
        TokenRates {
            currency: Currency::Usd,
            input_per_million: input,
            cached_input_per_million: cached,
            output_per_million: output,
            context_tiers,
        },
        Utc::now(),
    )?))
}
