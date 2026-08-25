use super::model_alias::{AliasResolution, ModelAliasResolver, normalize_model_id};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use thiserror::Error;

const TOKENS_PER_MILLION: i128 = 1_000_000;
const MICROS_PER_MAJOR_UNIT: i64 = 1_000_000;
pub const PRICING_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Usd,
    Cny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PriceProvenance {
    ExactObserved,
    OfficialLive,
    OfficialCached,
    SupplementalCatalog,
    OfficialEquivalent,
    Unpriced,
}

/// A fixed-point amount in one-millionth of the associated currency unit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MoneyMicros(i64);

impl MoneyMicros {
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    pub const fn micros(self) -> i64 {
        self.0
    }

    pub fn to_major_units_f64(self) -> f64 {
        self.0 as f64 / MICROS_PER_MAJOR_UNIT as f64
    }

    fn is_non_negative(self) -> bool {
        self.0 >= 0
    }
}

/// A complete set of replacement rates used once input exceeds a threshold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTier {
    pub above_input_tokens: u64,
    pub input_per_million: MoneyMicros,
    pub cached_input_per_million: MoneyMicros,
    pub output_per_million: MoneyMicros,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRates {
    pub currency: Currency,
    pub input_per_million: MoneyMicros,
    pub cached_input_per_million: MoneyMicros,
    pub output_per_million: MoneyMicros,
    #[serde(default)]
    pub context_tiers: Vec<ContextTier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub canonical_model: String,
    pub vendor: String,
    pub rates: TokenRates,
    pub source_url: String,
    pub fetched_at: DateTime<Utc>,
    pub parser_revision: String,
    pub provenance: PriceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingCatalog {
    pub schema_version: u32,
    pub entries: Vec<CatalogEntry>,
    #[serde(default)]
    pub aliases: ModelAliasResolver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostResolution {
    pub amount: Option<MoneyMicros>,
    pub currency: Currency,
    pub provenance: PriceProvenance,
    pub canonical_model: Option<String>,
    pub source_updated_at: Option<DateTime<Utc>>,
}

pub trait PricingResolver: Send + Sync {
    fn resolve(&self, model: &str, input: u64, cached: u64, output: u64) -> CostResolution;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogValidationError {
    #[error("unsupported pricing catalog schema version {0}")]
    UnsupportedSchema(u32),
    #[error("pricing catalog contains no entries")]
    Empty,
    #[error("pricing catalog entry {index} has an empty {field}")]
    EmptyField { index: usize, field: &'static str },
    #[error("pricing catalog entry {index} has an invalid public source URL")]
    InvalidSourceUrl { index: usize },
    #[error("pricing catalog entry {index} has a non-normalized canonical model")]
    InvalidCanonicalModel { index: usize },
    #[error("pricing catalog entry {index} uses invalid provenance")]
    InvalidProvenance { index: usize },
    #[error("pricing catalog entry {index} contains a negative rate")]
    NegativeRate { index: usize },
    #[error("pricing catalog entry {index} has invalid context tiers")]
    InvalidContextTiers { index: usize },
    #[error("pricing catalog contains duplicate model {0}")]
    DuplicateModel(String),
    #[error("pricing catalog alias points to missing model {0}")]
    MissingAliasTarget(String),
    #[error("pricing catalog alias collides with canonical model {0}")]
    AliasCanonicalCollision(String),
    #[error("pricing catalog alias registry is invalid")]
    InvalidAliases,
}

impl PricingCatalog {
    pub fn empty() -> Self {
        Self {
            schema_version: PRICING_CATALOG_SCHEMA_VERSION,
            entries: Vec::new(),
            aliases: ModelAliasResolver::empty(),
        }
    }

    pub fn new(
        mut entries: Vec<CatalogEntry>,
        aliases: ModelAliasResolver,
    ) -> Result<Self, CatalogValidationError> {
        for entry in &mut entries {
            entry.canonical_model = normalize_model_id(&entry.canonical_model);
        }
        let catalog = Self {
            schema_version: PRICING_CATALOG_SCHEMA_VERSION,
            entries,
            aliases,
        };
        catalog.validate_entries(false)?;
        Ok(catalog)
    }

    pub fn validate_complete(&self) -> Result<(), CatalogValidationError> {
        self.validate_entries(true)
    }

    fn validate_entries(&self, require_entries: bool) -> Result<(), CatalogValidationError> {
        if self.schema_version != PRICING_CATALOG_SCHEMA_VERSION {
            return Err(CatalogValidationError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        if require_entries && self.entries.is_empty() {
            return Err(CatalogValidationError::Empty);
        }
        if !self.aliases.is_valid() {
            return Err(CatalogValidationError::InvalidAliases);
        }

        let mut models = BTreeSet::new();
        for (index, entry) in self.entries.iter().enumerate() {
            validate_entry(entry, index)?;
            let normalized = normalize_model_id(&entry.canonical_model);
            if normalized.is_empty() {
                return Err(CatalogValidationError::EmptyField {
                    index,
                    field: "canonical model",
                });
            }
            if !models.insert(normalized.clone()) {
                return Err(CatalogValidationError::DuplicateModel(normalized));
            }
        }

        for canonical in self.aliases.exact_targets() {
            if !models.contains(canonical) {
                return Err(CatalogValidationError::MissingAliasTarget(
                    canonical.to_string(),
                ));
            }
        }
        for alias in self.aliases.keys() {
            if models.contains(alias) {
                return Err(CatalogValidationError::AliasCanonicalCollision(
                    alias.to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn lookup(&self, model: &str) -> Option<&CatalogEntry> {
        let normalized = normalize_model_id(model);
        self.entries
            .iter()
            .find(|entry| entry.canonical_model == normalized)
    }

    fn resolved_entry(&self, model: &str) -> Option<(&CatalogEntry, bool)> {
        match self.aliases.resolve_alias(model) {
            AliasResolution::Ambiguous => return None,
            AliasResolution::Exact(canonical) => {
                return self.lookup(&canonical).map(|entry| (entry, true));
            }
            AliasResolution::None => {}
        }
        if let Some(entry) = self.lookup(model) {
            return Some((entry, false));
        }
        None
    }
}

impl PricingResolver for PricingCatalog {
    fn resolve(&self, model: &str, input: u64, cached: u64, output: u64) -> CostResolution {
        if self.validate_entries(false).is_err() {
            return CostResolution::unpriced();
        }
        let Some((entry, is_alias)) = self.resolved_entry(model) else {
            return CostResolution::unpriced();
        };
        CostResolution::from_entry(entry, is_alias, input, cached, output)
    }
}

impl CostResolution {
    pub fn unpriced() -> Self {
        Self {
            amount: None,
            currency: Currency::Usd,
            provenance: PriceProvenance::Unpriced,
            canonical_model: None,
            source_updated_at: None,
        }
    }

    pub fn from_rates(entry: &CatalogEntry, input: u64, cached: u64, output: u64) -> Self {
        if validate_entry(entry, 0).is_err() {
            return Self::unpriced();
        }
        Self::from_entry(entry, false, input, cached, output)
    }

    fn from_entry(
        entry: &CatalogEntry,
        is_alias: bool,
        input: u64,
        cached: u64,
        output: u64,
    ) -> Self {
        let rates = rates_for_input(&entry.rates, input);
        let amount = calculate_amount(rates, input, cached, output);
        Self {
            amount,
            currency: entry.rates.currency,
            provenance: if amount.is_some() {
                if is_alias
                    && matches!(
                        entry.provenance,
                        PriceProvenance::OfficialLive | PriceProvenance::OfficialCached
                    )
                {
                    PriceProvenance::OfficialEquivalent
                } else {
                    entry.provenance
                }
            } else {
                PriceProvenance::Unpriced
            },
            canonical_model: Some(entry.canonical_model.clone()),
            source_updated_at: Some(entry.fetched_at),
        }
    }
}

pub(crate) fn validate_catalog_entry(
    entry: &CatalogEntry,
    index: usize,
) -> Result<(), CatalogValidationError> {
    validate_entry(entry, index)
}

fn validate_entry(entry: &CatalogEntry, index: usize) -> Result<(), CatalogValidationError> {
    for (field, value) in [
        ("canonical model", entry.canonical_model.as_str()),
        ("vendor", entry.vendor.as_str()),
        ("parser revision", entry.parser_revision.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(CatalogValidationError::EmptyField { index, field });
        }
    }
    if normalize_model_id(&entry.canonical_model) != entry.canonical_model {
        return Err(CatalogValidationError::InvalidCanonicalModel { index });
    }
    if !valid_public_source_url(&entry.source_url) {
        return Err(CatalogValidationError::InvalidSourceUrl { index });
    }
    if !matches!(
        entry.provenance,
        PriceProvenance::OfficialLive
            | PriceProvenance::OfficialCached
            | PriceProvenance::SupplementalCatalog
    ) {
        return Err(CatalogValidationError::InvalidProvenance { index });
    }
    if !rates_are_non_negative(&entry.rates) {
        return Err(CatalogValidationError::NegativeRate { index });
    }

    let mut previous_threshold = None;
    for tier in &entry.rates.context_tiers {
        if previous_threshold.is_some_and(|previous| tier.above_input_tokens <= previous)
            || !tier.input_per_million.is_non_negative()
            || !tier.cached_input_per_million.is_non_negative()
            || !tier.output_per_million.is_non_negative()
        {
            return Err(CatalogValidationError::InvalidContextTiers { index });
        }
        previous_threshold = Some(tier.above_input_tokens);
    }
    Ok(())
}

pub(crate) fn valid_public_source_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let normalized_host = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized_host.is_empty()
        || normalized_host == "localhost"
        || normalized_host.ends_with(".localhost")
        || normalized_host.ends_with(".local")
    {
        return false;
    }
    match normalized_host.parse::<std::net::IpAddr>() {
        Ok(address) => is_public_source_ip(address),
        Err(_) => normalized_host.contains('.'),
    }
}

fn is_public_source_ip(address: std::net::IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_source_ipv4(address),
        IpAddr::V6(address) => is_public_source_ipv6(address),
    }
}

fn is_public_source_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _d] = ip.octets();
    !(a == 0
        || ip.is_private()
        || (a == 100 && b & 0b1100_0000 == 0b0100_0000)
        || ip.is_loopback()
        || ip.is_link_local()
        || (a == 192 && b == 0 && c == 0)
        || ip.is_documentation()
        || (a == 192 && b == 31 && c == 196)
        || (a == 192 && b == 52 && c == 193)
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 175 && c == 48)
        || (a == 198 && (b == 18 || b == 19))
        || a >= 224)
}

fn is_public_source_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    if segments[0] & 0xe000 != 0x2000 {
        return false;
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.to_ipv4_mapped().is_some()
        || matches!(segments, [0x64, 0xff9b, 0, 0, 0, 0, _, _])
        || matches!(segments, [0x64, 0xff9b, 1, _, _, _, _, _])
        || matches!(segments, [0x100, 0, 0, 0, _, _, _, _])
        || matches!(segments, [0x2001, b, _, _, _, _, _, _] if b < 0x200)
        || matches!(segments, [0x2002, _, _, _, _, _, _, _])
        || matches!(segments, [0x2001, 0xdb8, _, _, _, _, _, _])
        || segments[0] & 0xfff0 == 0x3ff0
        || matches!(segments, [0x5f00, ..])
        || matches!(segments, [0x2620, 0x4f, 0x8000, _, _, _, _, _])
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || (segments[0] & 0xffc0 == 0xfec0)
        || ip.is_multicast())
}

fn rates_are_non_negative(rates: &TokenRates) -> bool {
    rates.input_per_million.is_non_negative()
        && rates.cached_input_per_million.is_non_negative()
        && rates.output_per_million.is_non_negative()
}

fn rates_for_input(rates: &TokenRates, input: u64) -> RateDimensions {
    let selected = rates
        .context_tiers
        .iter()
        .rev()
        .find(|tier| input > tier.above_input_tokens);
    match selected {
        Some(tier) => RateDimensions {
            input: tier.input_per_million,
            cached: tier.cached_input_per_million,
            output: tier.output_per_million,
        },
        None => RateDimensions {
            input: rates.input_per_million,
            cached: rates.cached_input_per_million,
            output: rates.output_per_million,
        },
    }
}

#[derive(Clone, Copy)]
struct RateDimensions {
    input: MoneyMicros,
    cached: MoneyMicros,
    output: MoneyMicros,
}

fn calculate_amount(
    rates: RateDimensions,
    input: u64,
    cached: u64,
    output: u64,
) -> Option<MoneyMicros> {
    let cached = cached.min(input);
    let uncached = input.saturating_sub(cached);
    let uncached_cost = i128::from(uncached).checked_mul(i128::from(rates.input.micros()))?;
    let cached_cost = i128::from(cached).checked_mul(i128::from(rates.cached.micros()))?;
    let output_cost = i128::from(output).checked_mul(i128::from(rates.output.micros()))?;
    let numerator = uncached_cost
        .checked_add(cached_cost)?
        .checked_add(output_cost)?;
    let micros = numerator.checked_div(TOKENS_PER_MILLION)?;
    i64::try_from(micros).ok().map(MoneyMicros::from_micros)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::model_alias::ModelAliasResolver;
    use chrono::{DateTime, Utc};

    fn fetched_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-24T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn usd_rates(input: i64, cached: i64, output: i64) -> TokenRates {
        TokenRates {
            currency: Currency::Usd,
            input_per_million: MoneyMicros::from_micros(input * 1_000_000),
            cached_input_per_million: MoneyMicros::from_micros(cached * 1_000_000),
            output_per_million: MoneyMicros::from_micros(output * 1_000_000),
            context_tiers: Vec::new(),
        }
    }

    fn entry(model: &str, rates: TokenRates) -> CatalogEntry {
        CatalogEntry {
            canonical_model: model.to_string(),
            vendor: "test-vendor".to_string(),
            rates,
            source_url: "https://pricing.example.test/catalog".to_string(),
            fetched_at: fetched_at(),
            parser_revision: "fixture-v1".to_string(),
            provenance: PriceProvenance::OfficialCached,
        }
    }

    fn catalog_with(entry: CatalogEntry) -> PricingCatalog {
        PricingCatalog::new(vec![entry], ModelAliasResolver::empty()).unwrap()
    }

    #[test]
    fn direct_catalog_rate_uses_uncached_cached_and_output_dimensions() {
        let catalog = catalog_with(entry("gpt-test", usd_rates(2, 1, 8)));

        let result = catalog.resolve("gpt-test", 1_000_000, 250_000, 1_000_000);

        assert_eq!(result.provenance, PriceProvenance::OfficialCached);
        assert_eq!(result.amount.unwrap().micros(), 9_750_000);
        assert_eq!(result.currency, Currency::Usd);
        assert_eq!(result.canonical_model.as_deref(), Some("gpt-test"));
        assert_eq!(result.source_updated_at, Some(fetched_at()));
    }

    #[test]
    fn direct_rate_constructor_uses_catalog_entry_provenance() {
        let catalog = catalog_with(entry("gpt-test", usd_rates(2, 1, 8)));
        let entry = catalog.lookup("gpt-test").unwrap();

        let result = CostResolution::from_rates(entry, 1_000_000, 250_000, 1_000_000);

        assert_eq!(result.amount.unwrap().micros(), 9_750_000);
        assert_eq!(result.provenance, PriceProvenance::OfficialCached);
    }

    #[test]
    fn direct_rate_constructor_rejects_mutated_negative_rates() {
        let mut invalid = entry("gpt-test", usd_rates(2, 1, 8));
        invalid.rates.input_per_million = MoneyMicros::from_micros(-1);

        let result = CostResolution::from_rates(&invalid, 1_000_000, 0, 0);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
    }

    #[test]
    fn direct_rate_constructor_rejects_non_normalized_model_ids() {
        let mut invalid = entry("gpt-test", usd_rates(2, 1, 8));
        invalid.canonical_model = " GPT-Test ".to_string();

        let result = CostResolution::from_rates(&invalid, 1_000_000, 0, 0);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
    }

    #[test]
    fn unknown_model_is_unpriced_not_zero() {
        let result = PricingCatalog::empty().resolve("mystery-model", 9, 0, 3);

        assert_eq!(result.provenance, PriceProvenance::Unpriced);
        assert_eq!(result.amount, None);
        assert_eq!(result.canonical_model, None);
    }

    #[test]
    fn explicit_alias_is_labelled_official_equivalent() {
        let aliases = ModelAliasResolver::from_mappings([("gateway/gpt-test", "gpt-test")]);
        let catalog =
            PricingCatalog::new(vec![entry("gpt-test", usd_rates(2, 1, 8))], aliases).unwrap();

        let result = catalog.resolve("gateway/gpt-test", 1_000_000, 0, 0);

        assert_eq!(result.provenance, PriceProvenance::OfficialEquivalent);
        assert_eq!(result.amount.unwrap().micros(), 2_000_000);
        assert_eq!(result.canonical_model.as_deref(), Some("gpt-test"));
    }

    #[test]
    fn supplemental_alias_is_not_promoted_to_official_equivalent() {
        let aliases = ModelAliasResolver::from_mappings([("gateway/gpt-test", "gpt-test")]);
        let mut supplemental = entry("gpt-test", usd_rates(2, 1, 8));
        supplemental.provenance = PriceProvenance::SupplementalCatalog;
        let catalog = PricingCatalog::new(vec![supplemental], aliases).unwrap();

        let result = catalog.resolve("gateway/gpt-test", 1_000_000, 0, 0);

        assert_eq!(result.provenance, PriceProvenance::SupplementalCatalog);
    }

    #[test]
    fn ambiguous_alias_key_cannot_collide_with_a_canonical_model() {
        let aliases = ModelAliasResolver::from_mappings([
            ("shared-test", "gpt-test"),
            ("shared-test", "other-test"),
        ]);
        let result = PricingCatalog::new(
            vec![
                entry("shared-test", usd_rates(1, 1, 1)),
                entry("gpt-test", usd_rates(2, 1, 8)),
                entry("other-test", usd_rates(3, 1, 9)),
            ],
            aliases,
        );

        assert!(matches!(
            result,
            Err(CatalogValidationError::AliasCanonicalCollision(model))
                if model == "shared-test"
        ));
    }

    #[test]
    fn mutated_ambiguous_alias_is_unpriced_even_if_it_matches_a_canonical_model() {
        let mut catalog = catalog_with(entry("shared-test", usd_rates(1, 1, 1)));
        catalog.aliases = ModelAliasResolver::from_mappings([
            ("shared-test", "gpt-test"),
            ("shared-test", "other-test"),
        ]);

        let result = catalog.resolve("shared-test", 1_000_000, 0, 0);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
        assert_eq!(result.canonical_model, None);
    }

    #[test]
    fn context_tier_replaces_all_rate_dimensions_above_its_threshold() {
        let mut rates = usd_rates(1, 1, 2);
        rates.context_tiers.push(ContextTier {
            above_input_tokens: 100,
            input_per_million: MoneyMicros::from_micros(3_000_000),
            cached_input_per_million: MoneyMicros::from_micros(2_000_000),
            output_per_million: MoneyMicros::from_micros(4_000_000),
        });
        let catalog = catalog_with(entry("tier-test", rates));

        let at_threshold = catalog.resolve("tier-test", 100, 25, 10);
        let above_threshold = catalog.resolve("tier-test", 101, 25, 10);

        assert_eq!(at_threshold.amount.unwrap().micros(), 120);
        assert_eq!(above_threshold.amount.unwrap().micros(), 318);
    }

    #[test]
    fn arithmetic_overflow_is_unpriced_instead_of_saturating() {
        let catalog = catalog_with(entry(
            "overflow-test",
            TokenRates {
                currency: Currency::Usd,
                input_per_million: MoneyMicros::from_micros(i64::MAX),
                cached_input_per_million: MoneyMicros::from_micros(i64::MAX),
                output_per_million: MoneyMicros::from_micros(i64::MAX),
                context_tiers: Vec::new(),
            },
        ));

        let result = catalog.resolve("overflow-test", u64::MAX, 0, u64::MAX);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
    }

    #[test]
    fn resolver_rejects_a_catalog_mutated_to_contain_negative_rates() {
        let mut catalog = catalog_with(entry("gpt-test", usd_rates(2, 1, 8)));
        catalog.entries[0].rates.input_per_million = MoneyMicros::from_micros(-1);

        let result = catalog.resolve("gpt-test", 1_000_000, 0, 0);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
    }

    #[test]
    fn catalog_rejects_non_public_or_malformed_source_urls() {
        for source_url in [
            "https://localhost/pricing",
            "https://pricing.local/catalog",
            "https://192.168.1.10/catalog",
            "https://127.0.0.1/catalog",
            "https://[::1]/catalog",
            "https://[::808:808]/catalog",
            "https://[4000::1]/catalog",
            "https://example.com:bad/catalog",
            "https://user:pass@example.com/catalog",
            "https://pricing.example.com/catalog?token=not-public",
            "https://pricing.example.com/catalog#private-fragment",
        ] {
            let mut invalid = entry("gpt-test", usd_rates(2, 1, 8));
            invalid.source_url = source_url.to_string();

            assert!(
                matches!(
                    PricingCatalog::new(vec![invalid], ModelAliasResolver::empty()),
                    Err(CatalogValidationError::InvalidSourceUrl { .. })
                ),
                "accepted invalid source URL {source_url}"
            );
        }
    }
}
