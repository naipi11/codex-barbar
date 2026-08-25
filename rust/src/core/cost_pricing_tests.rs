use super::*;
use crate::pricing::{
    CatalogEntry, Currency, ModelAliasResolver, MoneyMicros, PriceProvenance, PricingCatalog,
    TokenRates,
};
use chrono::{DateTime, Utc};

fn fixture_catalog(model: &str) -> PricingCatalog {
    PricingCatalog::new(
        vec![CatalogEntry {
            canonical_model: model.to_string(),
            vendor: "test-vendor".to_string(),
            rates: TokenRates {
                currency: Currency::Usd,
                input_per_million: MoneyMicros::from_micros(2_000_000),
                cached_input_per_million: MoneyMicros::from_micros(1_000_000),
                output_per_million: MoneyMicros::from_micros(8_000_000),
                context_tiers: Vec::new(),
            },
            source_url: "https://pricing.example.test/catalog".to_string(),
            fetched_at: DateTime::parse_from_rfc3339("2026-08-24T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            parser_revision: "fixture-v1".to_string(),
            provenance: PriceProvenance::OfficialCached,
        }],
        ModelAliasResolver::empty(),
    )
    .unwrap()
}

#[test]
fn compatibility_facade_delegates_to_the_supplied_resolver() {
    let catalog = fixture_catalog("gpt-test");

    let result = CostUsagePricing::resolve(&catalog, "gpt-test", 1_000_000, 250_000, 1_000_000);

    assert_eq!(result.amount.unwrap().micros(), 9_750_000);
    assert_eq!(result.provenance, PriceProvenance::OfficialCached);
}

#[test]
fn empty_catalog_returns_unpriced_for_every_named_model() {
    let result = CostUsagePricing::resolve(&PricingCatalog::empty(), "gpt-test", 1, 0, 1);

    assert_eq!(result.amount, None);
    assert_eq!(result.provenance, PriceProvenance::Unpriced);
}

#[test]
fn unattributed_usage_stays_unpriced_even_if_a_catalog_contains_that_key() {
    let catalog = fixture_catalog(CostUsagePricing::CODEX_UNATTRIBUTED_MODEL);

    let result = CostUsagePricing::resolve(
        &catalog,
        CostUsagePricing::CODEX_UNATTRIBUTED_MODEL,
        1_000,
        0,
        500,
    );

    assert_eq!(result.amount, None);
    assert_eq!(result.provenance, PriceProvenance::Unpriced);
}

#[test]
fn normalization_does_not_invent_catalog_aliases() {
    assert_eq!(
        CostUsagePricing::normalize_codex_model("gpt-test"),
        "gpt-test"
    );
    assert_eq!(
        CostUsagePricing::normalize_codex_model(" openai/GPT-Test "),
        "openai/gpt-test"
    );
    assert_eq!(
        CostUsagePricing::normalize_codex_model("gpt-test-codex"),
        "gpt-test-codex"
    );
    assert_eq!(
        CostUsagePricing::normalize_codex_model(""),
        CostUsagePricing::CODEX_UNATTRIBUTED_MODEL
    );
}

#[test]
fn claude_normalization_is_syntax_only() {
    assert_eq!(
        CostUsagePricing::normalize_claude_model(" Anthropic.Claude-Test-v1:0 "),
        "anthropic.claude-test-v1:0"
    );
}

#[test]
fn format_model_name_remains_a_display_only_helper() {
    assert_eq!(
        CostUsagePricing::format_model_name("claude-3.5-sonnet"),
        "Sonnet 3.5"
    );
    assert_eq!(
        CostUsagePricing::format_model_name("claude-opus-4"),
        "Opus 4"
    );
    assert_eq!(CostUsagePricing::format_model_name("gpt-5"), "GPT-5");
}
