//! Pi-compatible + OMP agent session cost scan (upstream #2269).
//!
//! Walks `~/.pi/agent/sessions/**/*.jsonl` and `~/.omp/agent/sessions/**/*.jsonl`
//! and attributes openai-codex / anthropic assistant rows into cost summaries
//! without double-counting the same entry id across shared files.

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::core::CostUsagePricing;
use crate::cost_scanner::CostSummary;
use crate::pricing::{CostResolution, Currency, PricingResolver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiMappedProvider {
    Codex,
    Claude,
}

/// Session roots to scan: `.pi` and `.omp` under the user home.
pub fn pi_compatible_session_roots(home: Option<PathBuf>) -> Vec<PathBuf> {
    let Some(home) = home else {
        return Vec::new();
    };
    [".pi", ".omp"]
        .into_iter()
        .map(|dir| home.join(dir).join("agent").join("sessions"))
        .collect()
}

pub fn scan_pi_compatible_into(
    summary: &mut CostSummary,
    target: PiMappedProvider,
    pricing: &dyn PricingResolver,
    days: u32,
    cancel: Option<&AtomicBool>,
    seen_entries: &mut HashSet<String>,
) {
    let cutoff = Utc::now() - Duration::days(days as i64);
    let mut sessions = 0u32;
    for root in pi_compatible_session_roots(dirs::home_dir()) {
        if cancelled(cancel) {
            break;
        }
        if !root.is_dir() {
            continue;
        }
        walk_jsonl(&root, cancel, &mut |path| {
            if cancelled(cancel) {
                return;
            }
            let before = seen_entries.len();
            let counted = for_each_pi_entry(path, cutoff, target, pricing, seen_entries, |entry| {
                apply_entry(summary, &entry);
            });
            if counted > 0 || seen_entries.len() > before {
                sessions += 1;
            }
        });
    }
    summary.sessions_count = summary.sessions_count.saturating_add(sessions);
}

struct PiEntry {
    model: String,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_create: u64,
    cost: CostResolution,
}

fn apply_entry(summary: &mut CostSummary, entry: &PiEntry) {
    summary.input_tokens += entry.input;
    summary.output_tokens += entry.output;
    summary.cached_tokens += entry.cache_read + entry.cache_create;
    if let Some(cost) = usd_cost(&entry.cost) {
        summary.total_cost_usd += cost;
        *summary.by_model.entry(entry.model.clone()).or_insert(0.0) += cost;
    } else if !CostUsagePricing::is_codex_unattributed_model(&entry.model) {
        summary.unknown_models.insert(entry.model.clone());
    }
    let tokens = summary
        .by_model_tokens
        .entry(entry.model.clone())
        .or_default();
    tokens.input_tokens += entry.input;
    tokens.output_tokens += entry.output;
    tokens.cached_tokens += entry.cache_read + entry.cache_create;
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed))
}

fn walk_jsonl(root: &Path, cancel: Option<&AtomicBool>, on_file: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if cancelled(cancel) {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_jsonl(&path, cancel, on_file);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
        {
            on_file(&path);
        }
    }
}

fn for_each_pi_entry(
    path: &Path,
    cutoff: DateTime<Utc>,
    target: PiMappedProvider,
    pricing: &dyn PricingResolver,
    seen: &mut HashSet<String>,
    mut on_entry: impl FnMut(PiEntry),
) -> u32 {
    let Ok(file) = File::open(path) else {
        return 0;
    };
    let mut counted = 0u32;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(entry) = parse_pi_assistant_entry(&value, target, pricing) else {
            continue;
        };
        if let Some(ts) = entry_timestamp(&value)
            && ts < cutoff
        {
            continue;
        }
        let entry_id = entry_dedup_key(&value, path, counted);
        if !seen.insert(entry_id) {
            continue;
        }
        on_entry(entry);
        counted += 1;
    }
    counted
}

fn entry_dedup_key(value: &Value, path: &Path, ordinal: u32) -> String {
    if let Some(id) = value
        .get("id")
        .or_else(|| value.get("messageId"))
        .or_else(|| value.pointer("/message/id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return id.to_string();
    }
    format!("{}#{ordinal}", path.display())
}

fn entry_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let raw = value
        .get("timestamp")
        .or_else(|| value.get("createdAt"))
        .or_else(|| value.pointer("/message/timestamp"))
        .and_then(|v| v.as_str())?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| {
            // epoch ms
            value
                .get("timestamp")
                .and_then(|v| v.as_i64())
                .and_then(DateTime::from_timestamp_millis)
        })
}

fn map_provider(raw: &str) -> Option<PiMappedProvider> {
    let n = raw.trim().to_ascii_lowercase();
    if n.contains("openai-codex") || n == "codex" || n == "openai" {
        return Some(PiMappedProvider::Codex);
    }
    if n.contains("anthropic") || n.contains("claude") {
        return Some(PiMappedProvider::Claude);
    }
    None
}

fn parse_pi_assistant_entry(
    value: &Value,
    target: PiMappedProvider,
    pricing: &dyn PricingResolver,
) -> Option<PiEntry> {
    // Accept either flat or nested { message: {...} } pi-compatible rows.
    let message = value.get("message").unwrap_or(value);
    let role = message
        .get("role")
        .or_else(|| value.get("role"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !role.eq_ignore_ascii_case("assistant") {
        // Some pi rows use type=message with role nested differently.
        let typ = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if typ != "assistant" && typ != "message" {
            return None;
        }
        if !role.is_empty() && !role.eq_ignore_ascii_case("assistant") {
            return None;
        }
    }

    let provider_raw = message
        .get("provider")
        .or_else(|| value.get("provider"))
        .or_else(|| message.get("api"))
        .or_else(|| value.get("api"))
        .and_then(|v| v.as_str())?;
    let mapped = map_provider(provider_raw)?;
    if mapped != target {
        return None;
    }

    let model = message
        .get("model")
        .or_else(|| message.get("modelId"))
        .or_else(|| value.get("model"))
        .or_else(|| value.get("modelId"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| CostUsagePricing::CODEX_UNATTRIBUTED_MODEL.to_string());

    let usage = message
        .get("usage")
        .or_else(|| value.get("usage"))
        .cloned()
        .unwrap_or(Value::Null);

    let input = num(
        &usage,
        &["input", "inputTokens", "input_tokens", "promptTokens"],
    );
    let output = num(
        &usage,
        &[
            "output",
            "outputTokens",
            "output_tokens",
            "completionTokens",
        ],
    );
    let cache_read = num(
        &usage,
        &[
            "cacheRead",
            "cache_read",
            "cache_read_input_tokens",
            "cached",
        ],
    );
    let cache_create = num(
        &usage,
        &[
            "cacheWrite",
            "cache_write",
            "cache_creation_input_tokens",
            "cacheCreate",
        ],
    );
    if input == 0 && output == 0 && cache_read == 0 && cache_create == 0 {
        return None;
    }

    let cost = if mapped == PiMappedProvider::Claude && cache_create > 0 {
        // The Task 1 resolver has no cache-creation dimension. Keep these rows
        // visible and unpriced until a source can represent that dimension.
        CostResolution::unpriced()
    } else {
        let resolver_input = if mapped == PiMappedProvider::Claude {
            input.saturating_add(cache_read)
        } else {
            input
        };
        CostUsagePricing::resolve(pricing, &model, resolver_input, cache_read, output)
    };

    Some(PiEntry {
        model,
        input,
        output,
        cache_read,
        cache_create,
        cost,
    })
}

fn usd_cost(resolution: &CostResolution) -> Option<f64> {
    if resolution.currency != Currency::Usd {
        return None;
    }
    resolution.amount.map(|amount| amount.to_major_units_f64())
}

fn num(usage: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        if let Some(v) = usage.get(*key) {
            if let Some(n) = v.as_u64() {
                return n;
            }
            if let Some(n) = v.as_i64() {
                return n.max(0) as u64;
            }
            if let Some(n) = v.as_f64() {
                return n.max(0.0) as u64;
            }
            if let Some(s) = v.as_str()
                && let Ok(n) = s.parse::<u64>()
            {
                return n;
            }
        }
    }
    0
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
    use tempfile::tempdir;

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
    fn maps_openai_codex_and_anthropic_providers() {
        assert_eq!(
            map_provider("openai-codex-responses"),
            Some(PiMappedProvider::Codex)
        );
        assert_eq!(map_provider("anthropic"), Some(PiMappedProvider::Claude));
        assert_eq!(map_provider("google"), None);
    }

    #[test]
    fn parses_assistant_usage_row() {
        let raw = serde_json::json!({
            "id": "msg-1",
            "role": "assistant",
            "provider": "openai-codex",
            "model": "gpt-5",
            "timestamp": "2026-07-20T12:00:00Z",
            "usage": { "input": 100, "output": 20, "cacheRead": 10 }
        });
        let entry =
            parse_pi_assistant_entry(&raw, PiMappedProvider::Codex, &PricingCatalog::empty())
                .unwrap();
        assert_eq!(entry.input, 100);
        assert_eq!(entry.output, 20);
        assert_eq!(entry.cache_read, 10);
        assert_eq!(entry.model, "gpt-5");
        assert_eq!(entry.cost.provenance, PriceProvenance::Unpriced);
        assert_eq!(entry.cost.amount, None);
    }

    #[test]
    fn claude_pi_usage_preserves_uncached_and_cache_read_dimensions() {
        let raw = serde_json::json!({
            "id": "msg-claude",
            "role": "assistant",
            "provider": "anthropic",
            "model": "claude-test",
            "usage": { "input": 100, "output": 5, "cacheRead": 20 }
        });

        let entry = parse_pi_assistant_entry(
            &raw,
            PiMappedProvider::Claude,
            &fixture_catalog("claude-test"),
        )
        .unwrap();

        assert_eq!(entry.cost.amount.unwrap().micros(), 260);
    }

    #[test]
    fn model_less_pi_usage_is_retained_as_unattributed_and_unpriced() {
        let raw = serde_json::json!({
            "id": "msg-model-less",
            "role": "assistant",
            "provider": "openai-codex",
            "usage": { "input": 100, "output": 5 }
        });

        let entry =
            parse_pi_assistant_entry(&raw, PiMappedProvider::Codex, &fixture_catalog("gpt-test"))
                .unwrap();

        assert_eq!(entry.model, CostUsagePricing::CODEX_UNATTRIBUTED_MODEL);
        assert_eq!(entry.cost.provenance, PriceProvenance::Unpriced);
        assert_eq!(entry.cost.amount, None);
    }

    #[test]
    fn dedupes_shared_entry_ids_across_files() {
        let dir = tempdir().unwrap();
        let sessions = dir.path().join("agent").join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let recent = (Utc::now() - Duration::hours(1))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let line = format!(
            r#"{{"id":"shared-1","role":"assistant","provider":"openai-codex","model":"gpt-5","timestamp":"{recent}","usage":{{"input":50,"output":5}}}}"#
        );
        for name in ["a.jsonl", "b.jsonl"] {
            let mut f = File::create(sessions.join(name)).unwrap();
            writeln!(f, "{line}").unwrap();
        }

        // Point home at temp so roots resolve under .omp
        let home = dir.path().to_path_buf();
        // Manually walk the sessions we created via for_each
        let mut seen = HashSet::new();
        let mut summary = CostSummary::default();
        let mut total = 0u32;
        for name in ["a.jsonl", "b.jsonl"] {
            total += for_each_pi_entry(
                &sessions.join(name),
                Utc::now() - Duration::days(30),
                PiMappedProvider::Codex,
                &PricingCatalog::empty(),
                &mut seen,
                |entry| apply_entry(&mut summary, &entry),
            );
        }
        assert_eq!(seen.len(), 1);
        assert_eq!(summary.input_tokens, 50);
        assert_eq!(total, 1); // second file deduped
        let _ = home;
    }

    #[test]
    fn session_roots_include_pi_and_omp() {
        let roots = pi_compatible_session_roots(Some(PathBuf::from("/home/user")));
        assert!(
            roots
                .iter()
                .any(|p| p.ends_with(".pi/agent/sessions") || p.ends_with(".pi\\agent\\sessions"))
        );
        assert!(roots.iter().any(|p| p.ends_with(".omp/agent/sessions") || p.ends_with(".omp\\agent\\sessions")));
    }
}
