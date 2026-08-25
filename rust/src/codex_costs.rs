//! Codex local-log cost aggregation helpers.

#[cfg(test)]
use chrono::Local;
use chrono::{Duration, NaiveDate};
use std::path::Path;

use crate::core::{CodexUsageRecord, CostUsageDayRange, CostUsagePricing, JsonlScanner};
use crate::cost_scanner::{CostSummary, ModelTokenCounts};
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
) -> (f64, bool) {
    let mut total_cost = 0.0;
    let mut has_tokens = false;

    for record in records.iter().filter(|record| {
        CostUsageDayRange::is_in_range(&record.day_key, &range.since_key, &range.until_key)
    }) {
        let tokens = CodexTokenCounts::from_values(record.input, record.cached, record.output);
        if let Some(resolution) =
            add_codex_tokens_to_summary(summary, &record.model, tokens, pricing)
        {
            if let Some(cost) = usd_cost(&resolution) {
                total_cost += cost;
            }
            has_tokens = true;
        }
    }

    (total_cost, has_tokens)
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
/// Returns `(session_cost, has_tokens)` — caller adds cost to `total_cost_usd`.
pub(crate) fn add_codex_days_map_to_summary(
    summary: &mut CostSummary,
    days: &std::collections::HashMap<String, std::collections::HashMap<String, Vec<i32>>>,
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> (f64, bool) {
    let mut total_cost = 0.0;
    let mut has_tokens = false;
    for (day_key, models) in days {
        if !CostUsageDayRange::is_in_range(day_key, &range.since_key, &range.until_key) {
            continue;
        }
        for (model, packed) in models {
            if let Some(resolution) =
                add_codex_packed_tokens_to_summary(summary, model, packed, pricing)
            {
                if let Some(cost) = usd_cost(&resolution) {
                    total_cost += cost;
                }
                has_tokens = true;
            }
        }
    }
    (total_cost, has_tokens)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn scan_codex_file_cost_for_range(
    path: &Path,
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> Option<f64> {
    let parse_result = match JsonlScanner::parse_codex_file(path, range, 0, None, None) {
        Ok(result) => result,
        Err(_) => return None,
    };

    codex_records_cost(&parse_result.records, range, pricing)
}

#[cfg(test)]
pub(crate) fn scan_codex_file_cost(path: &Path, pricing: &dyn PricingResolver) -> Option<f64> {
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
    if let Some(cost) = usd_cost(&resolution) {
        *summary.by_model.entry(model_key.clone()).or_insert(0.0) += cost;
        *summary
            .by_speed
            .entry(speed_bucket.to_string())
            .or_insert(0.0) += cost;
    } else if !CostUsagePricing::is_codex_unattributed_model(&model_key) {
        summary.unknown_models.insert(model_key);
    }
    Some(resolution)
}

#[cfg_attr(not(test), allow(dead_code))]
fn codex_records_cost(
    records: &[CodexUsageRecord],
    range: &CostUsageDayRange,
    pricing: &dyn PricingResolver,
) -> Option<f64> {
    let mut total_cost = 0.0;
    let mut complete = true;

    for record in records.iter().filter(|record| {
        CostUsageDayRange::is_in_range(&record.day_key, &range.since_key, &range.until_key)
    }) {
        if CostUsagePricing::is_codex_unattributed_model(&record.model) {
            continue;
        }
        let tokens = CodexTokenCounts::from_values(record.input, record.cached, record.output);
        if !tokens.is_empty() {
            let resolution = CostUsagePricing::resolve(
                pricing,
                &record.model,
                tokens.input,
                tokens.cached,
                tokens.output,
            );
            match usd_cost(&resolution) {
                Some(cost) => total_cost += cost,
                None => complete = false,
            }
        }
    }

    complete.then_some(total_cost)
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

fn usd_cost(resolution: &CostResolution) -> Option<f64> {
    if resolution.currency != Currency::Usd {
        return None;
    }
    resolution.amount.map(|amount| amount.to_major_units_f64())
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

        let (cost, has_tokens) =
            add_codex_records_to_summary(&mut summary, &records, &range, &catalog);

        assert!(has_tokens);
        assert_eq!(summary.input_tokens, 400_000);
        assert!((cost - 0.8).abs() < f64::EPSILON);
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

        let (cost, has_tokens) =
            add_codex_records_to_summary(&mut summary, &records, &range, &catalog);

        assert!(has_tokens);
        assert_eq!(cost, 0.0);
        assert_eq!(summary.input_tokens, 55_000_000);
        assert!(
            !summary
                .by_model
                .contains_key(CostUsagePricing::CODEX_UNATTRIBUTED_MODEL)
        );
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

        let (cost, has_tokens) =
            add_codex_records_to_summary(&mut summary, &records, &range, &PricingCatalog::empty());

        assert!(has_tokens);
        assert_eq!(cost, 0.0);
        assert!(!summary.by_model.contains_key("gpt-mystery"));
        assert!(summary.unknown_models.contains("gpt-mystery"));
    }

    #[test]
    fn test_codex_speed_bucket() {
        assert_eq!(codex_speed_bucket("gpt-5.5-fast"), "fast");
        assert_eq!(codex_speed_bucket("gpt-5.3-codex-spark"), "fast");
        assert_eq!(codex_speed_bucket("gpt-5-codex"), "standard");
    }
}
