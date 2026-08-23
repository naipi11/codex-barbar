//! Read-only local Codex usage/spend aggregation with strict pricing.
//!
//! Token totals come from the shared JSONL scanner/cache. Costs are derived
//! only from the canonical known-model pricing table; unknown or deliberately
//! unattributed models never receive a guessed price.

use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::core::CostUsagePricing;

type TokenTotals = (u64, u64, u64);
type ModelTotalsMap = BTreeMap<String, TokenTotals>;
type DailyTotalsMap = BTreeMap<NaiveDate, TokenTotals>;
use crate::cost_scanner::{
    CodexRangeScanError, CodexUsageRange, CodexUsageScanReport, CostScanner, DailyModelCodexUsage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalUsageSpendError {
    Cancelled,
    InvalidRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalUsageSpendReport {
    pub range: CodexUsageRange,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub sessions_count: u32,
    pub estimated_cost_usd: Option<f64>,
    pub unknown_models: Vec<String>,
    pub daily: Vec<DailyUsageSpend>,
    pub models: Vec<ModelUsageSpend>,
    pub malformed_records_skipped: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyUsageSpend {
    pub date: NaiveDate,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelUsageSpend {
    pub model: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
}

/// Scan local Codex logs for an inclusive local range and convert the result
/// into a strict, read-only usage/spend report.
pub fn scan_local_codex_usage(
    range: CodexUsageRange,
    cache_root: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<LocalUsageSpendReport, LocalUsageSpendError> {
    let scanner = CostScanner::new(30).with_cache_root(cache_root);
    let report =
        scanner
            .scan_codex_range_detailed(&range, cancel)
            .map_err(|error| match error {
                CodexRangeScanError::Cancelled => LocalUsageSpendError::Cancelled,
                CodexRangeScanError::InvalidRange => LocalUsageSpendError::InvalidRange,
            })?;
    Ok(convert_report(report, range))
}

fn convert_report(report: CodexUsageScanReport, range: CodexUsageRange) -> LocalUsageSpendReport {
    let daily_models = report.daily_models;
    let (model_totals, daily_totals) = aggregate_model_and_daily(&daily_models);

    let mut unknown_models = BTreeMap::<String, u64>::new();
    let mut total_cost = 0.0f64;
    let mut cost_complete = true;
    let mut models = Vec::new();
    for (model, totals) in model_totals {
        let (cost, known) = strict_model_cost(&model, totals);
        if !known {
            unknown_models
                .entry(model.clone())
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
            cost_complete = false;
        }
        total_cost += cost;
        models.push(ModelUsageSpend {
            model,
            input_tokens: totals.0,
            cached_input_tokens: totals.1,
            output_tokens: totals.2,
            total_tokens: totals.0 + totals.1 + totals.2,
            estimated_cost_usd: if known { Some(cost) } else { None },
        });
    }
    models.sort_by(|a, b| {
        b.total_tokens
            .cmp(&a.total_tokens)
            .then_with(|| a.model.cmp(&b.model))
    });

    let mut daily = Vec::new();
    for (date, totals) in daily_totals {
        let (cost, known) = strict_day_cost(&daily_models, date);
        if !known {
            cost_complete = false;
        }
        daily.push(DailyUsageSpend {
            date,
            total_tokens: totals.0 + totals.1 + totals.2,
            estimated_cost_usd: if known { Some(cost) } else { None },
        });
    }
    daily.sort_by_key(|row| row.date);

    let mut unknown_model_names = unknown_models.into_keys().collect::<Vec<_>>();
    unknown_model_names.sort();

    LocalUsageSpendReport {
        range,
        input_tokens: report.summary.input_tokens,
        cached_input_tokens: report.summary.cached_tokens,
        output_tokens: report.summary.output_tokens,
        total_tokens: report.summary.input_tokens
            + report.summary.cached_tokens
            + report.summary.output_tokens,
        sessions_count: report.sessions_count,
        estimated_cost_usd: if cost_complete {
            Some(total_cost)
        } else {
            None
        },
        unknown_models: unknown_model_names,
        daily,
        models,
        malformed_records_skipped: report.malformed_records_skipped,
    }
}

fn aggregate_model_and_daily(
    daily_models: &[DailyModelCodexUsage],
) -> (ModelTotalsMap, DailyTotalsMap) {
    let mut model_totals = ModelTotalsMap::new();
    let mut daily_totals = DailyTotalsMap::new();
    for row in daily_models {
        let model = model_totals.entry(row.model.clone()).or_default();
        model.0 = model.0.saturating_add(row.input_tokens);
        model.1 = model.1.saturating_add(row.cached_input_tokens);
        model.2 = model.2.saturating_add(row.output_tokens);
        let day = daily_totals.entry(row.date).or_default();
        day.0 = day.0.saturating_add(row.input_tokens);
        day.1 = day.1.saturating_add(row.cached_input_tokens);
        day.2 = day.2.saturating_add(row.output_tokens);
    }
    (model_totals, daily_totals)
}

fn strict_model_cost(model: &str, totals: TokenTotals) -> (f64, bool) {
    if CostUsagePricing::is_codex_unattributed_model(model) {
        return (0.0, true);
    }
    match CostUsagePricing::codex_cost_usd(model, totals.0, totals.1, totals.2) {
        Some(cost) => (cost, true),
        None => (0.0, false),
    }
}

fn strict_day_cost(daily_models: &[DailyModelCodexUsage], date: NaiveDate) -> (f64, bool) {
    let mut cost = 0.0;
    let mut known = true;
    for row in daily_models.iter().filter(|row| row.date == date) {
        if CostUsagePricing::is_codex_unattributed_model(&row.model) {
            continue;
        }
        match CostUsagePricing::codex_cost_usd(
            &row.model,
            row.input_tokens,
            row.cached_input_tokens,
            row.output_tokens,
        ) {
            Some(row_cost) => cost += row_cost,
            None => known = false,
        }
    }
    (cost, known)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn token_line(ts: &str, model: &str, input: u64, cached: u64, output: u64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "model": model,
                    "total_token_usage": {
                        "input_tokens": input,
                        "cached_input_tokens": cached,
                        "output_tokens": output
                    }
                }
            }
        })
        .to_string()
    }

    fn write_fixture(root: &Path, date: &str, name: &str, body: &str) -> PathBuf {
        let dir = root
            .join("sessions")
            .join(&date[..4])
            .join(&date[5..7])
            .join(&date[8..10]);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        path
    }

    fn fixture_root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn range(start: &str, end: &str) -> CodexUsageRange {
        CodexUsageRange {
            start: NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap(),
            end: NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap(),
        }
    }

    #[test]
    fn strict_range_scan_filters_dates_and_lists_unknown_models() {
        let root = fixture_root();
        let cache_root = root.path().join("cache");
        write_fixture(
            root.path(),
            "2026-08-19",
            "outside.jsonl",
            &(token_line("2026-08-19T12:00:00Z", "gpt-5", 999, 0, 999)
                + "
"),
        );
        write_fixture(
            root.path(),
            "2026-08-20",
            "known.jsonl",
            &(token_line("2026-08-20T12:00:00Z", "gpt-5", 100, 20, 10)
                + "
"),
        );
        write_fixture(
            root.path(),
            "2026-08-21",
            "unknown.jsonl",
            &(token_line("2026-08-21T12:00:00Z", "gpt-mystery", 25, 5, 5)
                + "
"),
        );

        let scanner = CostScanner::new(30)
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![root.path().join("sessions")]);
        let report = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-21"), None)
            .unwrap();

        assert_eq!(report.daily.len(), 2);
        assert_eq!(report.summary.input_tokens, 125);
        assert_eq!(report.summary.cached_tokens, 25);
        assert_eq!(report.summary.output_tokens, 15);
        assert_eq!(report.sessions_count, 2);
        assert_eq!(report.daily_models.len(), 2);

        let converted = convert_report(report, range("2026-08-20", "2026-08-21"));
        assert_eq!(converted.input_tokens, 125);
        assert_eq!(converted.output_tokens, 15);
        assert_eq!(converted.daily.len(), 2);
        assert_eq!(converted.unknown_models, vec!["gpt-mystery".to_string()]);
        assert_eq!(converted.estimated_cost_usd, None);
    }

    #[test]
    fn known_models_only_produce_a_strict_cost_total() {
        let root = fixture_root();
        let cache_root = root.path().join("cache");
        write_fixture(
            root.path(),
            "2026-08-20",
            "known.jsonl",
            &(token_line("2026-08-20T12:00:00Z", "gpt-5", 100, 20, 10)
                + "
"),
        );

        let scanner = CostScanner::new(30)
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![root.path().join("sessions")]);
        let report = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-20"), None)
            .unwrap();
        let converted = convert_report(report, range("2026-08-20", "2026-08-20"));

        assert!(converted.estimated_cost_usd.is_some());
        let expected = CostUsagePricing::codex_cost_usd("gpt-5", 100, 20, 10).unwrap();
        assert!((converted.estimated_cost_usd.unwrap() - expected).abs() < 1e-9);
        assert!(converted.unknown_models.is_empty());
        assert_eq!(converted.models.len(), 1);
        assert_eq!(converted.models[0].model, "gpt-5");
    }

    #[test]
    fn malformed_lines_are_counted_without_content() {
        let root = fixture_root();
        let cache_root = root.path().join("cache");
        let body = format!(
            "{}
this is not json
{}
",
            token_line("2026-08-20T12:00:00Z", "gpt-5", 10, 0, 1),
            "{ broken",
        );
        write_fixture(root.path(), "2026-08-20", "messy.jsonl", &body);

        let scanner = CostScanner::new(30)
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![root.path().join("sessions")]);
        let report = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-20"), None)
            .unwrap();
        assert_eq!(report.malformed_records_skipped, 2);
        assert_eq!(report.summary.input_tokens, 10);
    }

    #[test]
    fn cancelled_scan_is_a_distinct_error() {
        let root = fixture_root();
        let cache_root = root.path().join("cache");
        write_fixture(
            root.path(),
            "2026-08-20",
            "a.jsonl",
            &(token_line("2026-08-20T12:00:00Z", "gpt-5", 1, 0, 0)
                + "
"),
        );
        let cancel = AtomicBool::new(true);
        let scanner = CostScanner::new(30)
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![root.path().join("sessions")]);
        let error = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-20"), Some(&cancel))
            .unwrap_err();
        assert_eq!(error, CodexRangeScanError::Cancelled);
    }

    #[test]
    fn invalid_range_is_rejected_before_scanning() {
        let root = fixture_root();
        let scanner = CostScanner::new(30).with_cache_root(root.path().join("cache"));
        let error = scanner
            .scan_codex_range_detailed(&range("2026-08-21", "2026-08-20"), None)
            .unwrap_err();
        assert_eq!(error, CodexRangeScanError::InvalidRange);
    }

    #[test]
    fn second_scan_within_debounce_reuses_cache() {
        let root = fixture_root();
        let cache_root = root.path().join("cache");
        write_fixture(
            root.path(),
            "2026-08-20",
            "a.jsonl",
            &(token_line("2026-08-20T12:00:00Z", "gpt-5", 100, 0, 0)
                + "
"),
        );
        let scanner = CostScanner::new(30)
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![root.path().join("sessions")]);

        let first = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-20"), None)
            .unwrap();
        assert!(!first.used_cache_debounce);
        assert_eq!(first.summary.input_tokens, 100);

        let second = scanner
            .scan_codex_range_detailed(&range("2026-08-20", "2026-08-20"), None)
            .unwrap();
        assert!(second.used_cache_debounce);
        assert_eq!(second.summary.input_tokens, 100);
        assert_eq!(second.malformed_records_skipped, 0);
    }

    #[test]
    fn scan_local_codex_usage_wires_the_error_outcomes() {
        let root = fixture_root();
        let cancel = AtomicBool::new(true);
        let error = scan_local_codex_usage(
            range("2026-08-20", "2026-08-20"),
            root.path().join("cache").as_path(),
            Some(&cancel),
        )
        .unwrap_err();
        assert_eq!(error, LocalUsageSpendError::Cancelled);
    }
}
