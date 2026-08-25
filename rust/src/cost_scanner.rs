//! Local cost-usage scanner for Codex and Claude
//!
//! Scans local JSONL log files to aggregate token usage and calculate costs.
//!
//! Codex production path loads/saves [`crate::core::CostUsageCache`] under
//! `{cache}/CodexBar/cost-usage/`, skips unchanged files by mtime+size, resumes
//! partial files from `parsed_bytes`, honors [`crate::core::CostScanOptions`]
//! debounce (default 60s; `app_driven` forces a fresh inspection), and checks
//! cancel flags between files.

use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::codex_costs::scan_codex_file_cost;
use crate::codex_costs::{
    add_codex_days_map_to_summary, add_codex_records_to_summary, codex_period_start,
    codex_scan_dates, merge_codex_records_into_days,
};
use crate::codex_sessions::{codex_sessions_dir_candidates, default_wsl_roots};
use crate::core::{
    CostScanOptions, CostUsageCache, CostUsageDayRange, CostUsageFileUsage, CostUsagePricing,
    JsonlScanner, ProviderId,
};
use crate::pricing::{CostResolution, Currency, MoneyMicros, PricingCatalog, PricingResolver};
use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CostCompleteness {
    Complete,
    Partial,
    Unpriced,
}

/// Fixed-point USD aggregate that distinguishes true zero from incomplete cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostAggregate {
    known_micros: MoneyMicros,
    has_priced_usage: bool,
    has_unpriced_usage: bool,
}

impl Default for CostAggregate {
    fn default() -> Self {
        Self {
            known_micros: MoneyMicros::from_micros(0),
            has_priced_usage: false,
            has_unpriced_usage: false,
        }
    }
}

impl CostAggregate {
    pub fn completeness(self) -> CostCompleteness {
        match (self.has_priced_usage, self.has_unpriced_usage) {
            (true, true) => CostCompleteness::Partial,
            (false, true) => CostCompleteness::Unpriced,
            _ => CostCompleteness::Complete,
        }
    }

    pub fn total_micros(self) -> Option<MoneyMicros> {
        (!self.has_unpriced_usage).then_some(self.known_micros)
    }

    pub fn known_micros(self) -> Option<MoneyMicros> {
        self.has_priced_usage.then_some(self.known_micros)
    }

    pub fn total_usd(self) -> Option<f64> {
        self.total_micros().map(MoneyMicros::to_major_units_f64)
    }

    pub fn known_usd(self) -> Option<f64> {
        self.known_micros().map(MoneyMicros::to_major_units_f64)
    }

    pub fn record_resolution(&mut self, resolution: &CostResolution) {
        let Some(amount) = resolution.amount.filter(|amount| amount.micros() >= 0) else {
            self.mark_unpriced_usage();
            return;
        };
        if resolution.currency != Currency::Usd {
            self.mark_unpriced_usage();
            return;
        }
        self.record_usd_amount(amount);
    }

    fn record_usd_amount(&mut self, amount: MoneyMicros) {
        if amount.micros() < 0 {
            self.mark_unpriced_usage();
            return;
        }
        let Some(total) = self.known_micros.micros().checked_add(amount.micros()) else {
            self.mark_unpriced_usage();
            return;
        };
        self.known_micros = MoneyMicros::from_micros(total);
        self.has_priced_usage = true;
    }

    fn mark_unpriced_usage(&mut self) {
        self.has_unpriced_usage = true;
    }

    pub fn format_usd(self) -> String {
        match self.completeness() {
            CostCompleteness::Complete => {
                format!("${:.2}", self.total_usd().unwrap_or_default())
            }
            CostCompleteness::Partial => {
                format!(
                    "Partial (known ${:.2})",
                    self.known_usd().unwrap_or_default()
                )
            }
            CostCompleteness::Unpriced => "Unpriced".to_string(),
        }
    }
}

impl std::fmt::Display for CostAggregate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.format_usd())
    }
}

impl Serialize for CostAggregate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CostAggregate", 4)?;
        state.serialize_field("total_usd", &self.total_usd())?;
        state.serialize_field("known_usd", &self.known_usd())?;
        state.serialize_field("currency", "USD")?;
        state.serialize_field("completeness", &self.completeness())?;
        state.end()
    }
}

/// Cost summary from scanning local logs
#[derive(Debug, Clone, Default)]
pub struct CostSummary {
    /// Complete, partial, or unavailable USD total for the period.
    pub total_cost: CostAggregate,
    /// Total input tokens
    pub input_tokens: u64,
    /// Total output tokens
    pub output_tokens: u64,
    /// Total cached input tokens
    pub cached_tokens: u64,
    /// Number of sessions/conversations scanned
    pub sessions_count: u32,
    /// Cost breakdown by model
    pub by_model: HashMap<String, CostAggregate>,
    /// Token breakdown by model
    pub by_model_tokens: HashMap<String, ModelTokenCounts>,
    /// Codex cost split by speed/tier when local logs expose it.
    pub by_speed: HashMap<String, CostAggregate>,
    /// Codex token split by speed/tier when local logs expose it.
    pub by_speed_tokens: HashMap<String, ModelTokenCounts>,
    /// Model IDs that remain unpriced because no safe catalog resolution is available.
    pub unknown_models: HashSet<String>,
    /// Period start date
    pub period_start: Option<NaiveDate>,
    /// Period end date
    pub period_end: Option<NaiveDate>,
}

/// Per-model token counts
#[derive(Debug, Clone, Default)]
pub struct ModelTokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
}

impl ModelTokenCounts {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

impl CostSummary {
    pub fn format_total(&self) -> String {
        self.total_cost.format_usd()
    }
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn system_time_to_unix_ms(modified: Option<SystemTime>) -> i64 {
    modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn rebuild_cache_days(cache: &mut CostUsageCache) {
    cache.days.clear();
    for usage in cache.files.values() {
        for (day, models) in &usage.days {
            let day_entry = cache.days.entry(day.clone()).or_default();
            for (model, packed) in models {
                let dest = day_entry
                    .entry(model.clone())
                    .or_insert_with(|| vec![0, 0, 0]);
                if dest.len() < 3 {
                    dest.resize(3, 0);
                }
                for (i, value) in packed.iter().take(3).enumerate() {
                    dest[i] = dest[i].saturating_add(*value);
                }
            }
        }
    }
}

fn resolve_claude_with_cache_ttl(
    pricing: &dyn PricingResolver,
    model: &str,
    input: u64,
    cache_create: u64,
    cache_create_1h: u64,
    cache_read: u64,
    output: u64,
) -> CostResolution {
    if cache_create > 0 || cache_create_1h > 0 {
        // Cache creation has distinct provider rates that Task 1's catalog
        // contract cannot represent. Preserve tokens without inventing a rate.
        return CostResolution::unpriced();
    }
    // Claude logs expose uncached input and cache-read tokens separately, while
    // the shared resolver's input dimension includes cached input.
    let Some(total_input) = input.checked_add(cache_read) else {
        return CostResolution::unpriced();
    };
    CostUsagePricing::resolve(pricing, model, total_input, cache_read, output)
}

/// JSONL event structures for Codex
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    event_msg: Option<CodexEventMsg>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexEventMsg {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

/// JSONL event structures for Claude transcripts. Unknown fields are
/// ignored, so lines that are not assistant usage events still parse.
#[derive(Debug, Deserialize)]
struct ClaudeEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId", alias = "request_id")]
    request_id: Option<String>,
    message: Option<ClaudeMessage>,
}

impl ClaudeEvent {
    fn parsed_timestamp(&self) -> Option<DateTime<Utc>> {
        let timestamp = self.timestamp.as_deref()?;
        DateTime::parse_from_rfc3339(timestamp)
            .ok()
            .map(|ts| ts.with_timezone(&Utc))
    }
}

#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation: Option<ClaudeCacheCreation>,
}

impl ClaudeUsage {
    /// One-hour cache-write tokens, clamped to the total cache-write count.
    fn one_hour_cache_creation_tokens(&self, total: u64) -> u64 {
        self.cache_creation
            .as_ref()
            .and_then(|cache_creation| cache_creation.ephemeral_1h_input_tokens)
            .unwrap_or(0)
            .min(total)
    }
}

/// TTL breakdown of cache writes reported by the API.
#[derive(Debug, Deserialize)]
struct ClaudeCacheCreation {
    ephemeral_1h_input_tokens: Option<u64>,
}

#[derive(Debug)]
struct ClaudeUsageRecord {
    model: String,
    timestamp: Option<DateTime<Utc>>,
    dedup_key: Option<String>,
    input: u64,
    output: u64,
    cache_create: u64,
    cache_read: u64,
    cost: CostResolution,
}

/// Per-pass counters for cache/resume behavior (tests + diagnostics).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CostScanStats {
    pub files_seen: u32,
    pub files_parsed: u32,
    pub files_skipped: u32,
    pub files_resumed: u32,
    pub used_cache_debounce: bool,
}

/// Inclusive local calendar range for a read-only usage/spend scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexUsageRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// One local calendar day of Codex token totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyCodexUsage {
    pub date: NaiveDate,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

/// One local calendar day and model of Codex token totals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyModelCodexUsage {
    pub date: NaiveDate,
    pub model: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
}

impl DailyCodexUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.cached_input_tokens + self.output_tokens
    }
}

/// Outcome of a read-only Codex usage range scan.
#[derive(Debug, Clone)]
pub struct CodexUsageScanReport {
    pub summary: CostSummary,
    pub daily: Vec<DailyCodexUsage>,
    pub daily_models: Vec<DailyModelCodexUsage>,
    pub sessions_count: u32,
    pub malformed_records_skipped: u64,
    pub used_cache_debounce: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexRangeScanError {
    InvalidRange,
    Cancelled,
}

/// Cost usage scanner
pub struct CostScanner {
    days: u32,
    options: CostScanOptions,
    cache_root: Option<PathBuf>,
    /// When set, bypass normal sessions-dir discovery (tests / inject roots).
    sessions_dirs_override: Option<Vec<PathBuf>>,
    pricing: Arc<dyn PricingResolver>,
}

impl CostScanner {
    /// Create a new scanner for the last N days (default 60s cache debounce).
    pub fn new(days: u32) -> Self {
        Self {
            days,
            options: CostScanOptions::default(),
            cache_root: None,
            sessions_dirs_override: None,
            pricing: Arc::new(PricingCatalog::empty()),
        }
    }

    /// Override scan options (e.g. [`CostScanOptions::app_driven`] for force refresh).
    pub fn with_options(mut self, options: CostScanOptions) -> Self {
        self.options = options;
        self
    }

    /// Override on-disk cache root (`{root}/cost-usage/…`).
    pub fn with_cache_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.cache_root = Some(root.into());
        self
    }

    /// Override Codex sessions roots (primarily for tests).
    pub fn with_sessions_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.sessions_dirs_override = Some(dirs);
        self
    }

    /// Supply a possibly-empty dynamic pricing resolver for this scan.
    pub fn with_pricing(mut self, pricing: Arc<dyn PricingResolver>) -> Self {
        self.pricing = pricing;
        self
    }

    /// Scan Codex local logs
    pub fn scan_codex(&self) -> CostSummary {
        self.scan_codex_with_cancel(None)
    }

    /// Scan Codex local logs, stopping early when the caller cancels the scan.
    pub fn scan_codex_with_cancel(&self, cancel: Option<&AtomicBool>) -> CostSummary {
        self.scan_codex_detailed(cancel).0
    }

    /// Scan Codex and return cache/resume stats alongside the summary.
    pub fn scan_codex_detailed(&self, cancel: Option<&AtomicBool>) -> (CostSummary, CostScanStats) {
        let mut summary = CostSummary::default();
        let mut stats = CostScanStats::default();
        let today = Local::now().date_naive();
        let start_date = codex_period_start(today, self.days);
        let range = CostUsageDayRange::new(start_date, today);
        let now_ms = unix_now_ms();

        summary.period_start = Some(start_date);
        summary.period_end = Some(today);

        let cache_root = self.cache_root.as_deref();
        let mut cache = JsonlScanner::load_cache(ProviderId::Codex, cache_root);

        // Debounce: rebuild from disk cache without re-walking session files.
        if JsonlScanner::should_skip_cached_scan(&cache, self.options, now_ms)
            && JsonlScanner::cache_covers_range(&cache, &range)
            && (!cache.days.is_empty() || !cache.files.is_empty())
        {
            stats.used_cache_debounce = true;
            add_codex_days_map_to_summary(&mut summary, &cache.days, &range, self.pricing.as_ref());
            summary.sessions_count = cache
                .files
                .values()
                .filter(|usage| {
                    usage.days.keys().any(|day| {
                        CostUsageDayRange::is_in_range(day, &range.since_key, &range.until_key)
                    })
                })
                .count() as u32;

            // Pi-compatible sessions are outside the Codex JSONL cache.
            let mut seen_pi = HashSet::new();
            crate::pi_session_cost::scan_pi_compatible_into(
                &mut summary,
                crate::pi_session_cost::PiMappedProvider::Codex,
                self.pricing.as_ref(),
                self.days,
                cancel,
                &mut seen_pi,
            );
            return (summary, stats);
        }

        for sessions_dir in self.get_codex_sessions_dirs() {
            if is_cancelled(cancel) {
                break;
            }
            if sessions_dir.exists() {
                self.scan_codex_sessions_dir(
                    &sessions_dir,
                    &range,
                    &mut summary,
                    &mut cache,
                    cancel,
                    &mut stats,
                );
            }
        }

        if !is_cancelled(cancel) {
            rebuild_cache_days(&mut cache);
            cache.last_scan_unix_ms = now_ms;
            cache.scan_since_key = Some(range.since_key.clone());
            cache.scan_until_key = Some(range.until_key.clone());
            JsonlScanner::save_cache(ProviderId::Codex, &cache, cache_root);
        }

        // OMP / pi-compatible agent sessions (upstream #2269). Dedup by entry id.
        let mut seen_pi = HashSet::new();
        crate::pi_session_cost::scan_pi_compatible_into(
            &mut summary,
            crate::pi_session_cost::PiMappedProvider::Codex,
            self.pricing.as_ref(),
            self.days,
            cancel,
            &mut seen_pi,
        );

        (summary, stats)
    }

    /// Scan Claude local logs
    pub fn scan_claude(&self) -> CostSummary {
        self.scan_claude_with_cancel(None)
    }

    /// Scan Claude local logs, stopping early when the caller cancels the scan.
    pub fn scan_claude_with_cancel(&self, cancel: Option<&AtomicBool>) -> CostSummary {
        let projects_dir = self.get_claude_projects_dir();
        let mut summary = CostSummary::default();
        let today = Utc::now().date_naive();
        let start_date = today - Duration::days(self.days as i64);
        let cutoff = Utc::now() - Duration::days(self.days as i64);

        summary.period_start = Some(start_date);
        summary.period_end = Some(today);

        // Walk through projects directory, de-duplicating usage records
        // that appear across multiple files.
        if projects_dir.exists() {
            let mut seen = HashSet::new();
            let mut handle_file = |path: &Path| {
                let counted = for_each_claude_usage_record(
                    path,
                    &cutoff,
                    self.pricing.as_ref(),
                    &mut seen,
                    cancel,
                    |record| {
                        add_claude_record_to_summary(&mut summary, record);
                    },
                );
                if counted > 0 {
                    summary.sessions_count += 1;
                }
            };
            self.walk_claude_files(&projects_dir, &cutoff, cancel, &mut handle_file);
        }

        // OMP / pi-compatible anthropic rows, deduped across shared files.
        let mut seen_pi = HashSet::new();
        crate::pi_session_cost::scan_pi_compatible_into(
            &mut summary,
            crate::pi_session_cost::PiMappedProvider::Claude,
            self.pricing.as_ref(),
            self.days,
            cancel,
            &mut seen_pi,
        );

        summary
    }

    fn get_codex_sessions_dirs(&self) -> Vec<PathBuf> {
        if let Some(dirs) = &self.sessions_dirs_override {
            return dirs.clone();
        }
        let settings = Settings::load();
        let codex_home = std::env::var("CODEX_HOME").ok();
        codex_sessions_dir_candidates(
            dirs::home_dir(),
            codex_home,
            &settings.codex_custom_sessions_dirs,
            &default_wsl_roots(),
        )
    }

    fn scan_codex_sessions_dir(
        &self,
        sessions_dir: &Path,
        range: &CostUsageDayRange,
        summary: &mut CostSummary,
        cache: &mut CostUsageCache,
        cancel: Option<&AtomicBool>,
        stats: &mut CostScanStats,
    ) {
        // Iterate through the date-based directory structure with one day of
        // padding on each side. Codex JSONL timestamps are UTC, while the tray
        // presents local calendar days; the parser filters back to `range`.
        for date in codex_scan_dates(range) {
            if is_cancelled(cancel) {
                break;
            }
            let year = date.format("%Y").to_string();
            let month = date.format("%m").to_string();
            let day = date.format("%d").to_string();

            let day_dir = sessions_dir.join(&year).join(&month).join(&day);
            if !day_dir.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(&day_dir) {
                for entry in entries.flatten() {
                    if is_cancelled(cancel) {
                        break;
                    }
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        self.parse_codex_file(&path, range, summary, cache, cancel, stats);
                    }
                }
            }
        }
    }

    fn get_claude_projects_dir(&self) -> PathBuf {
        if let Ok(claude_config) = std::env::var("CLAUDE_CONFIG_DIR") {
            let trimmed = claude_config.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join("projects");
            }
        }

        // Try ~/.claude/projects first
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let claude_dir = home.join(".claude").join("projects");
        if claude_dir.exists() {
            return claude_dir;
        }

        // Fallback to ~/.config/claude/projects
        home.join(".config").join("claude").join("projects")
    }

    fn parse_codex_file(
        &self,
        path: &Path,
        range: &CostUsageDayRange,
        summary: &mut CostSummary,
        cache: &mut CostUsageCache,
        cancel: Option<&AtomicBool>,
        stats: &mut CostScanStats,
    ) {
        if is_cancelled(cancel) {
            return;
        }
        stats.files_seen += 1;

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return,
        };
        let size = metadata.len() as i64;
        let mtime_ms = system_time_to_unix_ms(metadata.modified().ok());
        let path_key = path.to_string_lossy().to_string();
        let cached = cache.files.get(&path_key).cloned();

        // Unchanged complete file: reuse packed days, skip re-parse.
        if let Some(entry) = &cached
            && entry.mtime_unix_ms == mtime_ms
            && entry.size == size
            && entry.parsed_bytes.unwrap_or(0) >= size
            && size > 0
        {
            let has_tokens =
                add_codex_days_map_to_summary(summary, &entry.days, range, self.pricing.as_ref());
            if has_tokens {
                summary.sessions_count += 1;
            }
            stats.files_skipped += 1;
            return;
        }

        // Growing file: resume from last parsed offset when safe.
        if let Some(entry) = &cached {
            let start_offset = entry.parsed_bytes.unwrap_or(0);
            if size > entry.size
                && start_offset > 0
                && start_offset <= size
                && entry.last_totals.is_some()
            {
                let parse_result = match JsonlScanner::parse_codex_file(
                    path,
                    range,
                    start_offset,
                    entry.last_model.clone(),
                    entry.last_totals.clone(),
                ) {
                    Ok(result) => result,
                    Err(_) => return,
                };

                let mut days = entry.days.clone();
                merge_codex_records_into_days(&mut days, &parse_result.records);

                let has_tokens =
                    add_codex_days_map_to_summary(summary, &days, range, self.pricing.as_ref());
                if has_tokens {
                    summary.sessions_count += 1;
                }

                cache.files.insert(
                    path_key,
                    CostUsageFileUsage {
                        mtime_unix_ms: mtime_ms,
                        size,
                        days,
                        parsed_bytes: Some(parse_result.parsed_bytes),
                        last_model: parse_result.last_model.or_else(|| entry.last_model.clone()),
                        last_totals: parse_result
                            .last_totals
                            .or_else(|| entry.last_totals.clone()),
                    },
                );
                stats.files_resumed += 1;
                return;
            }
        }

        // Full parse from offset 0.
        let parse_result = match JsonlScanner::parse_codex_file(path, range, 0, None, None) {
            Ok(result) => result,
            Err(_) => return,
        };

        let mut days = HashMap::new();
        merge_codex_records_into_days(&mut days, &parse_result.records);

        let has_tokens = add_codex_records_to_summary(
            summary,
            &parse_result.records,
            range,
            self.pricing.as_ref(),
        );

        if has_tokens {
            summary.sessions_count += 1;
        }

        cache.files.insert(
            path_key,
            CostUsageFileUsage {
                mtime_unix_ms: mtime_ms,
                size,
                days,
                parsed_bytes: Some(parse_result.parsed_bytes),
                last_model: parse_result.last_model,
                last_totals: parse_result.last_totals,
            },
        );
        stats.files_parsed += 1;
    }

    /// Scan local Codex logs for an explicit inclusive local date range.
    ///
    /// Returns a sanitized report; raw paths, raw JSONL lines, and fallback
    /// scanner estimates never cross this boundary. Cancellation is a
    /// distinct error outcome.
    pub fn scan_codex_range_detailed(
        &self,
        range: &CodexUsageRange,
        cancel: Option<&AtomicBool>,
    ) -> Result<CodexUsageScanReport, CodexRangeScanError> {
        if range.start > range.end {
            return Err(CodexRangeScanError::InvalidRange);
        }
        let day_range = CostUsageDayRange::new(range.start, range.end);
        let now_ms = unix_now_ms();
        let cache_root = self.cache_root.as_deref();
        let mut cache = JsonlScanner::load_cache(ProviderId::Codex, cache_root);
        let mut summary = CostSummary::default();
        let mut stats = CostScanStats::default();
        let mut malformed_records_skipped = 0u64;

        if JsonlScanner::should_skip_cached_scan(&cache, self.options, now_ms)
            && JsonlScanner::cache_covers_range(&cache, &day_range)
            && (!cache.days.is_empty() || !cache.files.is_empty())
        {
            stats.used_cache_debounce = true;
        } else {
            for sessions_dir in self.get_codex_sessions_dirs() {
                if is_cancelled(cancel) {
                    return Err(CodexRangeScanError::Cancelled);
                }
                if sessions_dir.exists()
                    && self.scan_codex_sessions_dir_range(
                        &sessions_dir,
                        &day_range,
                        &mut cache,
                        cancel,
                        &mut stats,
                        &mut malformed_records_skipped,
                    )
                {
                    return Err(CodexRangeScanError::Cancelled);
                }
            }
            if !is_cancelled(cancel) {
                rebuild_cache_days(&mut cache);
                cache.last_scan_unix_ms = now_ms;
                cache.scan_since_key = Some(day_range.since_key.clone());
                cache.scan_until_key = Some(day_range.until_key.clone());
                JsonlScanner::save_cache(ProviderId::Codex, &cache, cache_root);
            }
        }

        let _ = add_codex_days_map_to_summary(
            &mut summary,
            &cache.days,
            &day_range,
            self.pricing.as_ref(),
        );

        let sessions_count = cache
            .files
            .values()
            .filter(|usage| {
                usage.days.keys().any(|day| {
                    CostUsageDayRange::is_in_range(day, &day_range.since_key, &day_range.until_key)
                })
            })
            .count() as u32;
        let (daily, daily_models) = build_daily_codex_usage(&cache, &day_range);
        summary.period_start = Some(range.start);
        summary.period_end = Some(range.end);
        Ok(CodexUsageScanReport {
            summary,
            daily,
            daily_models,
            sessions_count,
            malformed_records_skipped,
            used_cache_debounce: stats.used_cache_debounce,
        })
    }

    fn scan_codex_sessions_dir_range(
        &self,
        sessions_dir: &Path,
        range: &CostUsageDayRange,
        cache: &mut CostUsageCache,
        cancel: Option<&AtomicBool>,
        stats: &mut CostScanStats,
        malformed: &mut u64,
    ) -> bool {
        for date in codex_scan_dates(range) {
            if is_cancelled(cancel) {
                return true;
            }
            let year = date.format("%Y").to_string();
            let month = date.format("%m").to_string();
            let day = date.format("%d").to_string();
            let day_dir = sessions_dir.join(&year).join(&month).join(&day);
            if !day_dir.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&day_dir) {
                for entry in entries.flatten() {
                    if is_cancelled(cancel) {
                        return true;
                    }
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        self.parse_codex_file_range(&path, range, cache, cancel, stats, malformed);
                    }
                }
            }
        }
        false
    }

    fn parse_codex_file_range(
        &self,
        path: &Path,
        range: &CostUsageDayRange,
        cache: &mut CostUsageCache,
        cancel: Option<&AtomicBool>,
        stats: &mut CostScanStats,
        malformed: &mut u64,
    ) {
        if is_cancelled(cancel) {
            return;
        }
        stats.files_seen += 1;
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return,
        };
        let size = metadata.len() as i64;
        let mtime_ms = system_time_to_unix_ms(metadata.modified().ok());
        let path_key = path.to_string_lossy().to_string();
        let cached = cache.files.get(&path_key).cloned();

        if let Some(entry) = &cached
            && entry.mtime_unix_ms == mtime_ms
            && entry.size == size
            && entry.parsed_bytes.unwrap_or(0) >= size
            && size > 0
        {
            stats.files_skipped += 1;
            return;
        }

        if let Some(entry) = &cached {
            let start_offset = entry.parsed_bytes.unwrap_or(0);
            if size > entry.size
                && start_offset > 0
                && start_offset <= size
                && entry.last_totals.is_some()
            {
                let parse_result = match JsonlScanner::parse_codex_file_detailed(
                    path,
                    range,
                    start_offset,
                    entry.last_model.clone(),
                    entry.last_totals.clone(),
                ) {
                    Ok(result) => result,
                    Err(_) => return,
                };
                *malformed = malformed.saturating_add(parse_result.malformed_lines);
                let mut days = entry.days.clone();
                merge_codex_records_into_days(&mut days, &parse_result.records);
                cache.files.insert(
                    path_key,
                    CostUsageFileUsage {
                        mtime_unix_ms: mtime_ms,
                        size,
                        days,
                        parsed_bytes: Some(parse_result.parsed_bytes),
                        last_model: parse_result.last_model.or_else(|| entry.last_model.clone()),
                        last_totals: parse_result
                            .last_totals
                            .or_else(|| entry.last_totals.clone()),
                    },
                );
                stats.files_resumed += 1;
                return;
            }
        }

        let parse_result = match JsonlScanner::parse_codex_file_detailed(path, range, 0, None, None)
        {
            Ok(result) => result,
            Err(_) => return,
        };
        *malformed = malformed.saturating_add(parse_result.malformed_lines);
        let mut days = HashMap::new();
        merge_codex_records_into_days(&mut days, &parse_result.records);
        cache.files.insert(
            path_key,
            CostUsageFileUsage {
                mtime_unix_ms: mtime_ms,
                size,
                days,
                parsed_bytes: Some(parse_result.parsed_bytes),
                last_model: parse_result.last_model,
                last_totals: parse_result.last_totals,
            },
        );
        stats.files_parsed += 1;
    }

    fn walk_claude_files<F>(
        &self,
        dir: &Path,
        cutoff: &DateTime<Utc>,
        cancel: Option<&AtomicBool>,
        on_file: &mut F,
    ) where
        F: FnMut(&Path),
    {
        if is_cancelled(cancel) {
            return;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            if is_cancelled(cancel) {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                self.walk_claude_files(&path, cutoff, cancel, on_file);
            } else if path.extension().is_some_and(|e| e == "jsonl") {
                // Check file modification time
                if let Ok(metadata) = fs::metadata(&path)
                    && let Ok(modified) = metadata.modified()
                {
                    let modified_dt: DateTime<Utc> = modified.into();
                    if modified_dt >= *cutoff {
                        on_file(&path);
                    }
                }
            }
        }
    }
}

fn build_daily_codex_usage(
    cache: &CostUsageCache,
    range: &CostUsageDayRange,
) -> (Vec<DailyCodexUsage>, Vec<DailyModelCodexUsage>) {
    let mut rows = Vec::new();
    let mut model_rows = Vec::new();
    for (day_key, models) in &cache.days {
        if !CostUsageDayRange::is_in_range(day_key, &range.since_key, &range.until_key) {
            continue;
        }
        let Some(date) = CostUsageDayRange::parse_day_key(day_key) else {
            continue;
        };
        let mut input_tokens = 0u64;
        let mut cached_input_tokens = 0u64;
        let mut output_tokens = 0u64;
        for (model, packed) in models {
            let model_input = packed.first().copied().unwrap_or(0).max(0) as u64;
            let model_cached = packed.get(1).copied().unwrap_or(0).max(0) as u64;
            let model_output = packed.get(2).copied().unwrap_or(0).max(0) as u64;
            input_tokens = input_tokens.saturating_add(model_input);
            cached_input_tokens = cached_input_tokens.saturating_add(model_cached);
            output_tokens = output_tokens.saturating_add(model_output);
            model_rows.push(DailyModelCodexUsage {
                date,
                model: model.clone(),
                input_tokens: model_input,
                cached_input_tokens: model_cached,
                output_tokens: model_output,
            });
        }
        rows.push(DailyCodexUsage {
            date,
            input_tokens,
            cached_input_tokens,
            output_tokens,
        });
    }
    rows.sort_by_key(|row| row.date);
    model_rows.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.model.cmp(&b.model)));
    (rows, model_rows)
}
/// Stream the de-duplicated, in-window usage records from one transcript
/// file into `on_record`. Both the summary scan and the daily-history scan
/// consume this single reader, so Claude log semantics live in one place.
/// Returns the number of records consumed, so callers can tell whether the
/// file contributed anything.
fn for_each_claude_usage_record<F>(
    path: &Path,
    cutoff: &DateTime<Utc>,
    pricing: &dyn PricingResolver,
    seen: &mut HashSet<String>,
    cancel: Option<&AtomicBool>,
    mut on_record: F,
) -> usize
where
    F: FnMut(&ClaudeUsageRecord),
{
    let Ok(file) = File::open(path) else {
        return 0;
    };

    let mut counted = 0;
    // Use read_until so a final incomplete line (no trailing newline) is still
    // processed when it is valid UTF-8 JSON, and so a single bad line does not
    // stop the walk the way `lines().map_while(Result::ok)` would.
    for_each_jsonl_text_line(BufReader::new(file), |line| {
        if is_cancelled(cancel) {
            return false;
        }
        if let Ok(event) = serde_json::from_str::<ClaudeEvent>(line)
            && let Some(record) = claude_usage_record_from_event(&event, pricing)
            && should_count_claude_record(&record, cutoff, seen)
        {
            counted += 1;
            on_record(&record);
        }
        true
    });
    counted
}

/// Walk JSONL text lines from `reader`, including a final incomplete line at EOF.
/// Continues past invalid UTF-8 segments. `on_line` returns `false` to stop early.
fn for_each_jsonl_text_line<R, F>(mut reader: R, mut on_line: F)
where
    R: BufRead,
    F: FnMut(&str) -> bool,
{
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        while matches!(buf.last(), Some(b'\n' | b'\r')) {
            buf.pop();
        }
        let Ok(line) = std::str::from_utf8(&buf) else {
            continue;
        };
        if !on_line(line) {
            break;
        }
    }
}

fn claude_usage_record_from_event(
    event: &ClaudeEvent,
    pricing: &dyn PricingResolver,
) -> Option<ClaudeUsageRecord> {
    if event.event_type.as_deref() != Some("assistant") {
        return None;
    }

    let message = event.message.as_ref()?;
    let usage = message.usage.as_ref()?;
    let model = message
        .model
        .as_deref()
        .unwrap_or(CostUsagePricing::CODEX_UNATTRIBUTED_MODEL);

    let input = usage.input_tokens.unwrap_or(0);
    let output = usage.output_tokens.unwrap_or(0);
    let cache_create = usage.cache_creation_input_tokens.unwrap_or(0);
    let cache_read = usage.cache_read_input_tokens.unwrap_or(0);

    if input == 0 && output == 0 && cache_create == 0 && cache_read == 0 {
        return None;
    }

    let cache_create_1h = usage.one_hour_cache_creation_tokens(cache_create);
    let cost = resolve_claude_with_cache_ttl(
        pricing,
        model,
        input,
        cache_create,
        cache_create_1h,
        cache_read,
        output,
    );

    Some(ClaudeUsageRecord {
        model: model.to_string(),
        timestamp: event.parsed_timestamp(),
        dedup_key: claude_usage_dedup_key(message.id.as_deref(), event.request_id.as_deref()),
        input,
        output,
        cache_create,
        cache_read,
        cost,
    })
}

fn claude_usage_dedup_key(message_id: Option<&str>, request_id: Option<&str>) -> Option<String> {
    match (message_id, request_id) {
        (Some(message_id), Some(request_id)) => Some(format!("{message_id}:{request_id}")),
        (Some(message_id), None) => Some(format!("message:{message_id}")),
        (None, Some(request_id)) => Some(format!("request:{request_id}")),
        (None, None) => None,
    }
}

fn should_count_claude_record(
    record: &ClaudeUsageRecord,
    cutoff: &DateTime<Utc>,
    seen: &mut HashSet<String>,
) -> bool {
    if let Some(timestamp) = record.timestamp
        && timestamp < *cutoff
    {
        return false;
    }

    if let Some(key) = &record.dedup_key
        && !seen.insert(key.clone())
    {
        return false;
    }

    true
}

fn add_claude_record_to_summary(summary: &mut CostSummary, record: &ClaudeUsageRecord) {
    summary.input_tokens += record.input;
    summary.output_tokens += record.output;
    summary.cached_tokens += record.cache_create + record.cache_read;
    summary.total_cost.record_resolution(&record.cost);
    summary
        .by_model
        .entry(record.model.clone())
        .or_default()
        .record_resolution(&record.cost);
    if record.cost.amount.is_none() || record.cost.currency != Currency::Usd {
        summary.unknown_models.insert(record.model.clone());
    }

    let model_tokens = summary
        .by_model_tokens
        .entry(record.model.clone())
        .or_default();
    model_tokens.input_tokens += record.input;
    model_tokens.output_tokens += record.output;
    model_tokens.cached_tokens += record.cache_create + record.cache_read;
}

/// Add one usage record to the per-day cost buckets, keyed by the record's
/// own timestamp in the local timezone. Records outside the initialized
/// date range (or without a timestamp) are ignored.
fn add_claude_record_to_daily_costs(
    daily_costs: &mut HashMap<String, CostAggregate>,
    record: &ClaudeUsageRecord,
) {
    let Some(timestamp) = record.timestamp else {
        return;
    };
    let date_str = timestamp
        .with_timezone(&Local)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    if let Some(cost) = daily_costs.get_mut(&date_str) {
        cost.record_resolution(&record.cost);
    }
}

/// Check if any cost usage sources are available
#[allow(dead_code)]
pub fn has_cost_usage_sources() -> bool {
    let scanner = CostScanner::new(1);
    scanner
        .get_codex_sessions_dirs()
        .iter()
        .any(|dir| dir.exists())
        || scanner.get_claude_projects_dir().exists()
        || crate::pi_session_cost::pi_compatible_session_roots(dirs::home_dir())
            .iter()
            .any(|dir| dir.exists())
}

/// Get daily cost history for the last N days
/// Returns complete/partial/unpriced daily USD aggregates sorted by date.
pub fn get_daily_cost_history(provider: &str, days: u32) -> Vec<(String, CostAggregate)> {
    let scanner = CostScanner::new(days);
    let today = Local::now().date_naive();
    let mut daily_costs: HashMap<String, CostAggregate> = HashMap::new();

    // Initialize all days with 0
    for days_ago in 0..days {
        let date = today - Duration::days(days_ago as i64);
        let date_str = date.format("%Y-%m-%d").to_string();
        daily_costs.insert(date_str, CostAggregate::default());
    }

    match provider {
        "codex" => {
            // Warm/refresh the disk cache (honors debounce), then price from packed days.
            let _ = scanner.scan_codex();
            let cache = JsonlScanner::load_cache(ProviderId::Codex, scanner.cache_root.as_deref());
            for (day_key, models) in &cache.days {
                let Some(slot) = daily_costs.get_mut(day_key) else {
                    continue;
                };
                let Some(day) = CostUsageDayRange::parse_day_key(day_key) else {
                    continue;
                };
                let day_range = CostUsageDayRange::new(day, day);
                let mut one_day = HashMap::new();
                one_day.insert(day_key.clone(), models.clone());
                let mut scratch = CostSummary::default();
                add_codex_days_map_to_summary(
                    &mut scratch,
                    &one_day,
                    &day_range,
                    scanner.pricing.as_ref(),
                );
                *slot = scratch.total_cost;
            }
        }
        "claude" => {
            // Real per-day breakdown: walk the project logs once,
            // de-duplicating records across files.
            let projects_dir = scanner.get_claude_projects_dir();
            if projects_dir.exists() {
                let cutoff = Utc::now() - Duration::days(days as i64);
                let mut seen = HashSet::new();
                let mut handle_file = |path: &Path| {
                    for_each_claude_usage_record(
                        path,
                        &cutoff,
                        scanner.pricing.as_ref(),
                        &mut seen,
                        None,
                        |record| {
                            add_claude_record_to_daily_costs(&mut daily_costs, record);
                        },
                    );
                };
                scanner.walk_claude_files(&projects_dir, &cutoff, None, &mut handle_file);
            }
        }
        _ => {}
    }

    // Convert to sorted vector
    let mut result: Vec<(String, CostAggregate)> = daily_costs.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::{
        CatalogEntry, Currency, ModelAliasResolver, MoneyMicros, PriceProvenance, PricingCatalog,
        TokenRates,
    };
    use chrono::DateTime;
    use std::io::Write;

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
    fn unknown_claude_model_is_unpriced_without_a_fallback() {
        let result = resolve_claude_with_cache_ttl(
            &PricingCatalog::empty(),
            "claude-mystery",
            100_000,
            0,
            0,
            0,
            100_000,
        );

        assert_eq!(result.provenance, PriceProvenance::Unpriced);
        assert_eq!(result.amount, None);
    }

    #[test]
    fn records_unknown_claude_model_without_adding_a_fallback_cost() {
        let event: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","timestamp":"2026-01-15T10:00:00Z","requestId":"req_unknown","message":{"id":"msg_unknown","model":"claude-retired-unknown","usage":{"input_tokens":100000,"output_tokens":100000}}}"#,
        )
        .unwrap();
        let record =
            claude_usage_record_from_event(&event, &PricingCatalog::empty()).expect("usage record");
        let mut summary = CostSummary::default();

        add_claude_record_to_summary(&mut summary, &record);

        assert_eq!(
            summary.total_cost.completeness(),
            CostCompleteness::Unpriced
        );
        assert_eq!(summary.total_cost.total_usd(), None);
        assert_eq!(summary.total_cost.known_usd(), None);
        assert_eq!(summary.format_total(), "Unpriced");
        let wire = serde_json::to_value(summary.total_cost).unwrap();
        assert_eq!(wire["total_usd"], serde_json::Value::Null);
        assert_eq!(wire["known_usd"], serde_json::Value::Null);
        assert_eq!(wire["completeness"], "unpriced");
        assert_eq!(
            summary.by_model["claude-retired-unknown"].completeness(),
            CostCompleteness::Unpriced
        );
        assert!(summary.unknown_models.contains("claude-retired-unknown"));
    }

    #[test]
    fn mixed_priced_and_unpriced_usage_is_explicitly_partial() {
        let catalog = fixture_catalog("claude-test");
        let priced: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","message":{"id":"priced","model":"claude-test","usage":{"input_tokens":100,"output_tokens":5}}}"#,
        )
        .unwrap();
        let unpriced: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","message":{"id":"unpriced","model":"claude-mystery","usage":{"input_tokens":10,"output_tokens":1}}}"#,
        )
        .unwrap();
        let mut summary = CostSummary::default();

        add_claude_record_to_summary(
            &mut summary,
            &claude_usage_record_from_event(&priced, &catalog).unwrap(),
        );
        add_claude_record_to_summary(
            &mut summary,
            &claude_usage_record_from_event(&unpriced, &catalog).unwrap(),
        );

        assert_eq!(summary.total_cost.completeness(), CostCompleteness::Partial);
        assert_eq!(summary.total_cost.total_usd(), None);
        assert_eq!(summary.total_cost.known_micros().unwrap().micros(), 240);
        assert_eq!(summary.format_total(), "Partial (known $0.00)");
        let wire = serde_json::to_value(summary.total_cost).unwrap();
        assert_eq!(wire["total_usd"], serde_json::Value::Null);
        assert_eq!(wire["known_usd"], 0.000_240);
        assert_eq!(wire["completeness"], "partial");
    }

    #[test]
    fn no_billable_usage_preserves_a_complete_true_zero() {
        let summary = CostSummary::default();

        assert_eq!(
            summary.total_cost.completeness(),
            CostCompleteness::Complete
        );
        assert_eq!(summary.total_cost.total_micros().unwrap().micros(), 0);
        assert_eq!(summary.format_total(), "$0.00");
        let wire = serde_json::to_value(summary.total_cost).unwrap();
        assert_eq!(wire["total_usd"], 0.0);
        assert_eq!(wire["completeness"], "complete");
    }

    #[test]
    fn supported_claude_dimensions_use_the_dynamic_resolver() {
        let catalog = fixture_catalog("claude-test");

        let result = resolve_claude_with_cache_ttl(&catalog, "claude-test", 100, 0, 0, 20, 5);

        assert_eq!(result.amount.unwrap().micros(), 260);
        assert_eq!(result.provenance, PriceProvenance::OfficialCached);
    }

    #[test]
    fn cache_creation_dimension_is_unpriced_until_the_catalog_can_represent_it() {
        let catalog = fixture_catalog("claude-test");

        let result = resolve_claude_with_cache_ttl(&catalog, "claude-test", 100, 30, 20, 20, 5);

        assert_eq!(result.provenance, PriceProvenance::Unpriced);
        assert_eq!(result.amount, None);
    }

    #[test]
    fn claude_dimension_translation_overflow_is_unpriced() {
        let mut catalog = fixture_catalog("claude-test");
        catalog.entries[0].rates.input_per_million = MoneyMicros::from_micros(0);
        catalog.entries[0].rates.cached_input_per_million = MoneyMicros::from_micros(0);
        catalog.entries[0].rates.output_per_million = MoneyMicros::from_micros(0);

        let result = resolve_claude_with_cache_ttl(&catalog, "claude-test", u64::MAX, 0, 0, 1, 0);

        assert_eq!(result.amount, None);
        assert_eq!(result.provenance, PriceProvenance::Unpriced);
    }

    #[test]
    fn parses_current_codex_payload_token_count_events() {
        let path = std::env::temp_dir().join(format!(
            "codexbar-current-codex-token-count-{}.jsonl",
            std::process::id()
        ));
        // Use a recent timestamp so the event stays inside the scanner's
        // 30-day window no matter when the test runs. A hardcoded date
        // silently ages out of the window and makes this test fail with 0
        // sessions once it is more than 30 days in the past.
        let recent = (Utc::now() - Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let mut file = File::create(&path).unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"model":"gpt-5","total_token_usage":{{"input_tokens":125,"cached_input_tokens":30,"output_tokens":15}}}}}}}}"#,
            ts = recent
        )
        .unwrap();
        let scanner = CostScanner::new(30);
        let mut summary = CostSummary::default();
        let today = Local::now().date_naive();
        let range = CostUsageDayRange::new(codex_period_start(today, 30), today);
        let mut cache = CostUsageCache::default();
        let mut stats = CostScanStats::default();
        scanner.parse_codex_file(&path, &range, &mut summary, &mut cache, None, &mut stats);

        assert_eq!(summary.sessions_count, 1);
        assert_eq!(summary.input_tokens, 125);
        assert_eq!(summary.cached_tokens, 30);
        assert_eq!(summary.output_tokens, 15);
        assert_eq!(
            summary
                .by_model_tokens
                .get("gpt-5")
                .map(ModelTokenCounts::total),
            Some(140)
        );
        assert_eq!(
            scan_codex_file_cost(&path, &PricingCatalog::empty())
                .unwrap()
                .completeness(),
            CostCompleteness::Unpriced
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn derives_claude_dedup_key_from_message_and_request_ids() {
        assert_eq!(
            claude_usage_dedup_key(Some("msg_1"), Some("req_1")).as_deref(),
            Some("msg_1:req_1")
        );
        assert_eq!(
            claude_usage_dedup_key(Some("msg_1"), None).as_deref(),
            Some("message:msg_1")
        );
        assert_eq!(
            claude_usage_dedup_key(None, Some("req_1")).as_deref(),
            Some("request:req_1")
        );
        assert_eq!(claude_usage_dedup_key(None, None), None);
    }

    #[test]
    fn counts_claude_usage_once_across_duplicate_records() {
        // The same API response can be replayed into several transcript files
        // (session resume, sidechains); it must only be counted once.
        let event: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","timestamp":"2026-01-15T10:00:00Z","requestId":"req_1","message":{"id":"msg_1","model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":10,"cache_read_input_tokens":20}}}"#,
        )
        .unwrap();

        let record =
            claude_usage_record_from_event(&event, &PricingCatalog::empty()).expect("usage record");
        assert_eq!(record.model, "claude-sonnet-4-6");
        assert_eq!(record.input, 100);
        assert_eq!(record.output, 50);
        assert_eq!(record.cache_create, 10);
        assert_eq!(record.cache_read, 20);
        assert_eq!(record.cost.provenance, PriceProvenance::Unpriced);
        assert_eq!(record.cost.amount, None);

        let cutoff = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut seen = HashSet::new();
        assert!(should_count_claude_record(&record, &cutoff, &mut seen));
        assert!(!should_count_claude_record(&record, &cutoff, &mut seen));
    }

    #[test]
    fn rejects_claude_records_before_cutoff() {
        let event: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","timestamp":"2025-12-01T10:00:00Z","requestId":"req_old","message":{"id":"msg_old","model":"claude-sonnet-4-6","usage":{"input_tokens":1,"output_tokens":1}}}"#,
        )
        .unwrap();
        let record =
            claude_usage_record_from_event(&event, &PricingCatalog::empty()).expect("usage record");
        let cutoff = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut seen = HashSet::new();
        assert!(!should_count_claude_record(&record, &cutoff, &mut seen));
    }

    #[test]
    fn ignores_claude_events_without_countable_usage() {
        // Non-assistant events carry no billable usage.
        let event: ClaudeEvent =
            serde_json::from_str(r#"{"type":"user","message":{"usage":{"input_tokens":5}}}"#)
                .unwrap();
        assert!(claude_usage_record_from_event(&event, &PricingCatalog::empty()).is_none());

        // Zero-token usage blocks (e.g. synthetic messages) are not sessions.
        let event: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","message":{"id":"msg_zero","model":"claude-sonnet-4-6","usage":{"input_tokens":0,"output_tokens":0}}}"#,
        )
        .unwrap();
        assert!(claude_usage_record_from_event(&event, &PricingCatalog::empty()).is_none());
    }

    #[test]
    fn claude_usage_without_model_is_unattributed_and_unpriced() {
        let event: ClaudeEvent = serde_json::from_str(
            r#"{"type":"assistant","message":{"id":"msg_model_less","usage":{"input_tokens":5,"output_tokens":1}}}"#,
        )
        .unwrap();

        let record = claude_usage_record_from_event(&event, &fixture_catalog("claude-3-5-sonnet"))
            .expect("usage record");

        assert_eq!(record.model, CostUsagePricing::CODEX_UNATTRIBUTED_MODEL);
        assert_eq!(record.cost.provenance, PriceProvenance::Unpriced);
        assert_eq!(record.cost.amount, None);
    }

    fn claude_transcript_line(
        timestamp: &str,
        request_key: &str,
        request_id: &str,
        message_id: &str,
    ) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{timestamp}","{request_key}":"{request_id}","message":{{"id":"{message_id}","model":"claude-sonnet-4-6","usage":{{"input_tokens":1000,"output_tokens":500}}}}}}"#
        )
    }

    #[test]
    fn daily_history_dedups_across_files_and_buckets_by_local_day() {
        // End-to-end regression for the daily buckets: two transcript files,
        // two different days, plus a replay of the day-one record in the
        // second file (snake_case request_id, as another writer would emit).
        let dir = std::env::temp_dir();
        let file_a = dir.join(format!(
            "codexbar-claude-daily-a-{}.jsonl",
            std::process::id()
        ));
        let file_b = dir.join(format!(
            "codexbar-claude-daily-b-{}.jsonl",
            std::process::id()
        ));

        // >24h apart guarantees two distinct local calendar days.
        let day_one = Utc::now() - Duration::hours(30);
        let day_two = Utc::now() - Duration::hours(2);
        let ts_one = day_one.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let ts_two = day_two.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        std::fs::write(
            &file_a,
            format!(
                "{}\n{}\n",
                claude_transcript_line(&ts_one, "requestId", "req_1", "msg_1"),
                claude_transcript_line(&ts_two, "requestId", "req_2", "msg_2"),
            ),
        )
        .unwrap();
        std::fs::write(
            &file_b,
            format!(
                "{}\n",
                claude_transcript_line(&ts_one, "request_id", "req_1", "msg_1"),
            ),
        )
        .unwrap();

        let day_key = |ts: &DateTime<Utc>| {
            ts.with_timezone(&Local)
                .date_naive()
                .format("%Y-%m-%d")
                .to_string()
        };
        let mut daily_costs = HashMap::new();
        daily_costs.insert(day_key(&day_one), CostAggregate::default());
        daily_costs.insert(day_key(&day_two), CostAggregate::default());

        let cutoff = Utc::now() - Duration::days(30);
        let mut seen = HashSet::new();
        let catalog = fixture_catalog("claude-sonnet-4-6");
        for path in [&file_a, &file_b] {
            for_each_claude_usage_record(path, &cutoff, &catalog, &mut seen, None, |record| {
                add_claude_record_to_daily_costs(&mut daily_costs, record);
            });
        }

        let day_one_cost = daily_costs[&day_key(&day_one)];
        let day_two_cost = daily_costs[&day_key(&day_two)];
        assert_eq!(day_one_cost.completeness(), CostCompleteness::Complete);
        assert!(day_one_cost.total_usd().unwrap() > 0.0);
        // Identical usage on both days: equal buckets proves the file-b
        // replay was de-duplicated (a leak would double day one).
        assert!(
            (day_one_cost.total_usd().unwrap() - day_two_cost.total_usd().unwrap()).abs()
                < f64::EPSILON,
            "each day should hold exactly one record's cost, got {day_one_cost} vs {day_two_cost}"
        );

        let _ = std::fs::remove_file(&file_a);
        let _ = std::fs::remove_file(&file_b);
    }

    #[test]
    fn daily_unpriced_usage_is_not_serialized_as_zero() {
        let timestamp = Utc::now() - Duration::hours(1);
        let day = timestamp
            .with_timezone(&Local)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let event: ClaudeEvent = serde_json::from_str(&format!(
            r#"{{"type":"assistant","timestamp":"{}","message":{{"id":"daily-unpriced","model":"claude-mystery","usage":{{"input_tokens":10,"output_tokens":1}}}}}}"#,
            timestamp.to_rfc3339()
        ))
        .unwrap();
        let record = claude_usage_record_from_event(&event, &PricingCatalog::empty()).unwrap();
        let mut daily_costs = HashMap::from([(day.clone(), CostAggregate::default())]);

        add_claude_record_to_daily_costs(&mut daily_costs, &record);

        assert_eq!(daily_costs[&day].completeness(), CostCompleteness::Unpriced);
        assert_eq!(daily_costs[&day].total_usd(), None);
    }

    #[test]
    fn public_daily_history_exposes_typed_cost_aggregates() {
        let _function: fn(&str, u32) -> Vec<(String, CostAggregate)> = get_daily_cost_history;
    }

    #[test]
    fn claude_scan_counts_final_incomplete_jsonl_line() {
        let path =
            std::env::temp_dir().join(format!("codexbar-claude-tail-{}.jsonl", std::process::id()));
        let ts = (Utc::now() - Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        // No trailing newline — the last (only) record must still be counted.
        let body = claude_transcript_line(&ts, "requestId", "req_tail", "msg_tail");
        std::fs::write(&path, body.as_bytes()).unwrap();

        let cutoff = Utc::now() - Duration::days(1);
        let mut seen = HashSet::new();
        let counted = for_each_claude_usage_record(
            &path,
            &cutoff,
            &PricingCatalog::empty(),
            &mut seen,
            None,
            |_| {},
        );
        assert_eq!(counted, 1, "incomplete final JSONL line must be processed");
        let _ = std::fs::remove_file(&path);
    }

    fn write_codex_session_fixture(sessions_root: &Path, name: &str, input_tokens: u64) -> PathBuf {
        let today = Local::now().date_naive();
        let day_dir = sessions_root
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());
        std::fs::create_dir_all(&day_dir).unwrap();
        let path = day_dir.join(name);
        let ts = (Utc::now() - Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let body = format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"model":"gpt-test","total_token_usage":{{"input_tokens":{input_tokens},"cached_input_tokens":0,"output_tokens":5}}}}}}}}
"#
        );
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn cost_scan_second_pass_skips_unchanged_files_via_cache() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        let cache_root = root.path().join("cache");
        write_codex_session_fixture(&sessions, "a.jsonl", 100);
        write_codex_session_fixture(&sessions, "b.jsonl", 200);

        let scanner = CostScanner::new(7)
            .with_pricing(std::sync::Arc::new(fixture_catalog("gpt-test")))
            .with_options(CostScanOptions::app_driven())
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![sessions.clone()]);

        let (summary1, stats1) = scanner.scan_codex_detailed(None);
        assert_eq!(stats1.files_parsed, 2, "first pass parses both files");
        assert_eq!(stats1.files_skipped, 0);
        assert!(summary1.total_cost.total_usd().unwrap() > 0.0);
        assert_eq!(summary1.sessions_count, 2);

        // Second pass with default debounce still inspects files but skips re-parse.
        // Use app_driven so we exercise per-file mtime skip rather than whole-scan debounce.
        let (summary2, stats2) = scanner.scan_codex_detailed(None);
        assert_eq!(stats2.files_seen, 2);
        assert_eq!(stats2.files_skipped, 2, "cache hit skips re-parse");
        assert_eq!(stats2.files_parsed, 0);
        assert_eq!(summary2.input_tokens, summary1.input_tokens);
        assert_eq!(summary2.total_cost, summary1.total_cost);

        // Force path already used above; confirm debounce short-circuit with default options.
        let debounced = CostScanner::new(7)
            .with_options(CostScanOptions::default())
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![sessions.clone()]);
        let (summary3, stats3) = debounced.scan_codex_detailed(None);
        assert!(
            stats3.used_cache_debounce,
            "default options debounce within 60s"
        );
        assert_eq!(stats3.files_seen, 0);
        assert_eq!(summary3.input_tokens, summary1.input_tokens);

        // app_driven after debounce still re-reads (skip via mtime, not full re-parse).
        let forced = CostScanner::new(7)
            .with_options(CostScanOptions::app_driven())
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![sessions]);
        let (_, stats4) = forced.scan_codex_detailed(None);
        assert!(!stats4.used_cache_debounce);
        assert_eq!(stats4.files_skipped, 2);
        assert_eq!(stats4.files_parsed, 0);
    }

    #[test]
    fn cost_scan_cancel_stops_between_files() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        let cache_root = root.path().join("cache");
        write_codex_session_fixture(&sessions, "a.jsonl", 100);
        write_codex_session_fixture(&sessions, "b.jsonl", 200);
        write_codex_session_fixture(&sessions, "c.jsonl", 300);

        let cancel = AtomicBool::new(true);
        let scanner = CostScanner::new(7)
            .with_options(CostScanOptions::app_driven())
            .with_cache_root(cache_root)
            .with_sessions_dirs(vec![sessions]);
        let (summary, stats) = scanner.scan_codex_detailed(Some(&cancel));
        assert_eq!(stats.files_seen, 0, "cancel before first file stops walk");
        assert_eq!(summary.sessions_count, 0);
    }

    #[test]
    fn cost_scan_resumes_appended_bytes() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        let cache_root = root.path().join("cache");
        let path = write_codex_session_fixture(&sessions, "grow.jsonl", 50);

        let scanner = CostScanner::new(7)
            .with_options(CostScanOptions::app_driven())
            .with_cache_root(&cache_root)
            .with_sessions_dirs(vec![sessions.clone()]);
        let (s1, st1) = scanner.scan_codex_detailed(None);
        assert_eq!(st1.files_parsed, 1);
        assert_eq!(s1.input_tokens, 50);

        // Append another cumulative token_count event (100 total => +50 delta).
        let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let extra = format!(
            r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"model":"gpt-5","total_token_usage":{{"input_tokens":100,"cached_input_tokens":0,"output_tokens":10}}}}}}}}
"#
        );
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(extra.as_bytes()).unwrap();
        drop(f);

        // Bump mtime/size visibly on some FS by rewriting metadata via reopen.
        let (s2, st2) = scanner.scan_codex_detailed(None);
        assert_eq!(st2.files_resumed, 1, "grown file resumes from offset");
        assert_eq!(st2.files_parsed, 0);
        assert_eq!(s2.input_tokens, 100);
    }
}
