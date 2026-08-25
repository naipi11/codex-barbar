//! Codex local-log cost aggregation helpers.

#[cfg(test)]
use chrono::Local;
use chrono::{Duration, NaiveDate};
use std::path::Path;

use crate::core::{CodexUsageRecord, CostUsageDayRange, CostUsagePricing, JsonlScanner};
use crate::cost_scanner::{CostAggregate, CostSummary, ModelTokenCounts};
use crate::pricing::{CostResolution, Currency, PricingResolver};

pub(crate) fn codex_period_start(today: NaiveDate, days: u32) -> NaiveDate {
    today - Duration::days(days.saturating_sub(1) as i64)
}

pub(crate) fn codex_scan_dates(range: &CostUsageDayRange) -> Vec<NaiveDate> {
    let Some(mut date) = CostUsageDayRange::parse_day_key(&range.scan_since_key) else {
        return Vec::new();
    };
    let Some(until) = CostUsageDayRange::parse_day_key(&range.scan_until_key) else {
        return Vec::new();
    };
    let mut dates = Vec::new();
    while date <= until {
        dates.push(date);
        date += Duration::days(1);
    }
    dates
}

pub(crate) fn add_codex_records_to_summary(
    summary: &mut CostSummary,
    records: &[CodexUsageRecord],
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> bool {
    let mut has_tokens = false;

    for record in records.iter().filter(|record| {
        CostUsageDayRange::is_in_range(&record.day_key, &range.since_key, &range.until_key)
    }) {
        let tokens = CodexTokenCounts::from_values(record.input, record.cached, record.output);
        if add_codex_tokens_to_summary(summary, &record.model, tokens, pricing).is_some() {
            has_tokens = true;
        }
    }

    has_tokens
}

/// Merge billable records into a day→model→`[input,cached,output]` map.
pub(crate) fn merge_codex_records_into_days(
    days: &mut std::collections::HashMap<String, std::collections::HashMap<String, Vec<i32>>>,
    records: &[CodexUsageRecord],
) {
    for record in records {
        let models = days.entry(record.day_key.clone()).or_default();
        let packed = models
            .entry(record.model.clone())
            .or_insert_with(|| vec![0, 0, 0]);
        if packed.len() < 3 {
            packed.resize(3, 0);
        }
        packed[0] = packed[0].saturating_add(record.input.max(0));
        packed[1] = packed[1].saturating_add(record.cached.max(0));
        packed[2] = packed[2].saturating_add(record.output.max(0));
    }
}

/// Apply one packed `[input, cached, output]` triple to a summary.
pub(crate) fn add_codex_packed_tokens_to_summary(
    summary: &mut CostSummary,
    model: &str,
    packed: &[i32],
    pricing: &dyn PricingResolver,
) -> Option<CostResolution> {
    let input = packed.first().copied().unwrap_or(0);
    let cached = packed.get(1).copied().unwrap_or(0);
    let output = packed.get(2).copied().unwrap_or(0);
    add_codex_tokens_to_summary(
        summary,
        model,
        CodexTokenCounts::from_values(input, cached, output),
        pricing,
    )
}

/// Fold day→model→packed token maps into a cost summary (range-filtered).
pub(crate) fn add_codex_days_map_to_summary(
    summary: &mut CostSummary,
    days: &std::collections::HashMap<String, std::collections::HashMap<String, Vec<i32>>>,
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> bool {
    let mut has_tokens = false;
    for (day_key, models) in days {
        if !CostUsageDayRange::is_in_range(day_key, &range.since_key, &range.until_key) {
            continue;
        }
        for (model, packed) in models {
            if add_codex_packed_tokens_to_summary(summary, model, packed, pricing).is_some() {
                has_tokens = true;
            }
        }
    }
    has_tokens
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn scan_codex_file_cost_for_range(
    path: &Path,
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> Option<CostAggregate> {
    let parse_result = match JsonlScanner::parse_codex_file(path, range, 0, None, None) {
        Ok(result) => result,
        Err(_) => return None,
    };

    Some(codex_records_cost(&parse_result.records, range, pricing))
}

#[cfg(test)]
pub(crate) fn scan_codex_file_cost(
    path: &Path,
    pricing: &dyn PricingResolver,
) -> Option<CostAggregate> {
    let today = Local::now().date_naive();
    let range = CostUsageDayRange::new(codex_period_start(today, 30), today);
    scan_codex_file_cost_for_range(path, &range, pricing)
}

#[derive(Clone, Copy)]
struct CodexTokenCounts {
    input: u64,
    cached: u64,
    output: u64,
}

impl CodexTokenCounts {
    fn from_values(input: i32, cached: i32, output: i32) -> Self {
        let input = input.max(0) as u64;
        Self {
            input,
            cached: (cached.max(0) as u64).min(input),
            output: output.max(0) as u64,
        }
    }

    fn is_empty(self) -> bool {
        self.input == 0 && self.cached == 0 && self.output == 0
    }
}

fn add_tokens(summary: &mut ModelTokenCounts, tokens: CodexTokenCounts) {
    summary.input_tokens += tokens.input;
    summary.output_tokens += tokens.output;
    summary.cached_tokens += tokens.cached;
}

fn add_codex_tokens_to_summary(
    summary: &mut CostSummary,
    model: &str,
    tokens: CodexTokenCounts,
    pricing: &dyn PricingResolver,
) -> Option<CostResolution> {
    if tokens.is_empty() {
        return None;
    }

    let model_key = if CostUsagePricing::is_codex_unattributed_model(model) {
        CostUsagePricing::CODEX_UNATTRIBUTED_MODEL.to_string()
    } else {
        model.to_string()
    };

    summary.input_tokens += tokens.input;
    summary.cached_tokens += tokens.cached;
    summary.output_tokens += tokens.output;
    let speed_bucket = codex_speed_bucket(&model_key);
    add_tokens(
        summary
            .by_model_tokens
            .entry(model_key.clone())
            .or_default(),
        tokens,
    );
    add_tokens(
        summary
            .by_speed_tokens
            .entry(speed_bucket.to_string())
            .or_default(),
        tokens,
    );

    let resolution = CostUsagePricing::resolve(
        pricing,
        &model_key,
        tokens.input,
        tokens.cached,
        tokens.output,
    );
    summary.total_cost.record_resolution(&resolution);
    summary
        .by_model
        .entry(model_key.clone())
        .or_default()
        .record_resolution(&resolution);
    summary
        .by_speed
        .entry(speed_bucket.to_string())
        .or_default()
        .record_resolution(&resolution);
    if !resolution_is_priced_usd(&resolution)
        && !CostUsagePricing::is_codex_unattributed_model(&model_key)
    {
        summary.unknown_models.insert(model_key);
    }
    Some(resolution)
}

#[cfg_attr(not(test), allow(dead_code))]
fn codex_records_cost(
    records: &[CodexUsageRecord],
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> CostAggregate {
    let mut total_cost = CostAggregate::default();

    for record in records.iter().filter(|record| {
        CostUsageDayRange::is_in_range(&record.day_key, &range.since_key, &range.until_key)
    }) {
        let tokens = CodexTokenCounts::from_values(record.input, record.cached, record.output);
        if !tokens.is_empty() {
            let resolution = CostUsagePricing::resolve(
                pricing,
                &record.model,
                tokens.input,
                tokens.cached,
                tokens.output,
            );
            total_cost.record_resolution(&resolution);
        }
    }

    total_cost
}

fn codex_speed_bucket(model: &str) -> &'static str {
    let normalized = model.to_ascii_lowercase();
    if normalized.contains("fast")
        || normalized.contains("priority")
        || normalized.contains("spark")
        || normalized.contains("smoke")
    {
        "fast"
    } else {
        "standard"
    }
}

fn resolution_is_priced_usd(resolution: &CostResolution) -> bool {
    resolution.currency == Currency::Usd && resolution.amount.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CodexUsageRecord;
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
            ModelAliasResolver::default(),
        )
        .unwrap()
    }

    #[test]
    fn codex_summary_uses_the_supplied_dynamic_catalog() {
        let target = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let range = CostUsageDayRange::new(target, target);
        let records = vec![
            CodexUsageRecord {
                day_key: "2026-05-31".to_string(),
                model: "gpt-test".to_string(),
                input: 200_000,
                cached: 0,
                output: 0,
            },
            CodexUsageRecord {
                day_key: "2026-05-31".to_string(),
                model: "gpt-test".to_string(),
                input: 200_000,
                cached: 0,
                output: 0,
            },
            CodexUsageRecord {
                day_key: "2026-05-30".to_string(),
                model: "gpt-test".to_string(),
                input: 200_000,
                cached: 0,
                output: 0,
            },
        ];
        let mut summary = CostSummary::default();
        let catalog = fixture_catalog("gpt-test");

        let has_tokens = add_codex_records_to_summary(&mut summary, &records, &range, &catalog);

        assert!(has_tokens);
        assert_eq!(summary.input_tokens, 400_000);
        assert_eq!(summary.total_cost.total_micros().unwrap().micros(), 800_000);
    }

    #[test]
    fn model_less_codex_usage_is_visible_but_unpriced() {
        let target = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let range = CostUsageDayRange::new(target, target);
        let records = vec![CodexUsageRecord {
            day_key: "2026-05-31".to_string(),
            model: CostUsagePricing::CODEX_UNATTRIBUTED_MODEL.to_string(),
            input: 55_000_000,
            cached: 0,
            output: 0,
        }];
        let mut summary = CostSummary::default();
        let catalog = fixture_catalog(CostUsagePricing::CODEX_UNATTRIBUTED_MODEL);

        let has_tokens = add_codex_records_to_summary(&mut summary, &records, &range, &catalog);

        assert!(has_tokens);
        assert_eq!(summary.input_tokens, 55_000_000);
        assert_eq!(
            summary.total_cost.completeness(),
            crate::cost_scanner::CostCompleteness::Unpriced
        );
        assert_eq!(summary.total_cost.total_usd(), None);
        assert!(summary.unknown_models.is_empty());
    }

    #[test]
    fn unknown_codex_model_is_unpriced_without_a_fallback_cost() {
        let target = NaiveDate::from_ymd_opt(2026, 5, 31).unwrap();
        let range = CostUsageDayRange::new(target, target);
        let records = vec![CodexUsageRecord {
            day_key: "2026-05-31".to_string(),
            model: "gpt-mystery".to_string(),
            input: 1_000_000,
            cached: 0,
            output: 1_000_000,
        }];
        let mut summary = CostSummary::default();

        let has_tokens =
            add_codex_records_to_summary(&mut summary, &records, &range, &PricingCatalog::empty());

        assert!(has_tokens);
        assert_eq!(summary.total_cost.total_usd(), None);
        assert_eq!(
            summary.by_model["gpt-mystery"].completeness(),
            crate::cost_scanner::CostCompleteness::Unpriced
        );
        assert!(summary.unknown_models.contains("gpt-mystery"));
    }

    #[test]
    fn test_codex_speed_bucket() {
        assert_eq!(codex_speed_bucket("gpt-5.5-fast"), "fast");
        assert_eq!(codex_speed_bucket("gpt-5.3-codex-spark"), "fast");
        assert_eq!(codex_speed_bucket("gpt-5-codex"), "standard");
    }
}
