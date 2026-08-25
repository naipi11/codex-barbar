//! Compatibility helpers around the dynamic pricing resolver.
//!
//! This module intentionally owns no numeric model rates. Callers that still
//! use the historical helper name must supply a [`PricingResolver`].

use crate::pricing::{CostResolution, PricingResolver};

/// Compatibility facade for model labeling and the dynamic resolver boundary.
pub struct CostUsagePricing;

impl CostUsagePricing {
    /// Sentinel model key for model-less Codex token events.
    ///
    /// Usage remains visible under this key but is never priced as a real model,
    /// including when a catalog accidentally contains the same string.
    pub const CODEX_UNATTRIBUTED_MODEL: &'static str = "unknown";

    pub fn is_codex_unattributed_model(model: &str) -> bool {
        Self::normalize_codex_model(model) == Self::CODEX_UNATTRIBUTED_MODEL
    }

    /// Syntax-only normalization for grouping model IDs.
    ///
    /// Canonical aliases are resolved exclusively by [`PricingResolver`]; this
    /// helper never strips prefixes, versions, or suffixes.
    pub fn normalize_codex_model(raw: &str) -> String {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || normalized == Self::CODEX_UNATTRIBUTED_MODEL
            || normalized == "unpriced"
        {
            Self::CODEX_UNATTRIBUTED_MODEL.to_string()
        } else {
            normalized
        }
    }

    /// Syntax-only normalization for grouping Claude model IDs.
    pub fn normalize_claude_model(raw: &str) -> String {
        raw.trim().to_ascii_lowercase()
    }

    /// Resolve cost through the supplied dynamic catalog boundary.
    pub fn resolve(
        resolver: &dyn PricingResolver,
        model: &str,
        input_tokens: u64,
        cached_input_tokens: u64,
        output_tokens: u64,
    ) -> CostResolution {
        if Self::is_codex_unattributed_model(model) {
            CostResolution::unpriced()
        } else {
            resolver.resolve(model, input_tokens, cached_input_tokens, output_tokens)
        }
    }

    /// Format a model name for display without making any pricing decision.
    pub fn format_model_name(model: &str) -> String {
        let lower = model.to_ascii_lowercase();

        if lower.contains("gpt-") {
            let version = regex_lite::Regex::new(r"gpt-(\d+(?:\.\d+)?)")
                .ok()
                .and_then(|regex| regex.captures(&lower))
                .and_then(|captures| captures.get(1))
                .map(|matched| matched.as_str().to_string());
            let suffix = if lower.contains("nano") {
                " Nano"
            } else if lower.contains("mini") {
                " Mini"
            } else {
                ""
            };
            return version
                .map(|version| format!("GPT-{version}{suffix}"))
                .unwrap_or_else(|| model.to_string());
        }

        let version = regex_lite::Regex::new(r"(\d+(?:\.\d+)?)")
            .ok()
            .and_then(|regex| regex.find(&lower))
            .map(|matched| matched.as_str().to_string());
        let family = if lower.contains("opus") {
            "Opus"
        } else if lower.contains("sonnet") {
            "Sonnet"
        } else if lower.contains("haiku") {
            "Haiku"
        } else {
            return model.to_string();
        };
        version
            .map(|version| format!("{family} {version}"))
            .unwrap_or_else(|| family.to_string())
    }
}

#[cfg(test)]
#[path = "cost_pricing_tests.rs"]
mod tests;
