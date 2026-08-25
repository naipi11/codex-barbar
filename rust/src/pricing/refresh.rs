use super::cache::CatalogStore;
use super::catalog::{CatalogEntry, PriceProvenance, PricingCatalog};
use super::fx::{FxRateSnapshot, FxStore, parse_official_usd_cny};
use super::model_alias::ModelAliasResolver;
use super::source::{PricingSourceAdapter, PricingSourceError, SourceSnapshot};
use super::sources::{
    DeepSeekAdapter, KimiAdapter, ModelsDevAdapter, OpenAiAdapter, QwenAdapter, XaiAdapter,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

const REFRESH_INTERVAL: Duration = Duration::hours(24);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingRefreshOutcome {
    Updated,
    Unchanged,
    UsedCached,
    Failed,
}

pub fn refresh_due(last_success: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(last_success) >= REFRESH_INTERVAL
}

pub fn latest_catalog_timestamp(catalog: &PricingCatalog) -> Option<DateTime<Utc>> {
    catalog.entries.iter().map(|entry| entry.fetched_at).max()
}

pub fn merge_source_snapshots(
    snapshots: &[SourceSnapshot],
) -> Result<PricingCatalog, super::catalog::CatalogValidationError> {
    let mut by_model: BTreeMap<String, CatalogEntry> = BTreeMap::new();
    for snapshot in snapshots {
        for entry in &snapshot.entries {
            let key = entry.canonical_model.clone();
            match by_model.get(&key) {
                None => {
                    by_model.insert(key, entry.clone());
                }
                Some(existing) if should_replace(existing, entry) => {
                    by_model.insert(key, entry.clone());
                }
                Some(_) => {}
            }
        }
    }
    PricingCatalog::new(
        by_model.into_values().collect(),
        ModelAliasResolver::empty(),
    )
}

fn should_replace(existing: &CatalogEntry, incoming: &CatalogEntry) -> bool {
    provenance_rank(incoming.provenance) > provenance_rank(existing.provenance)
        || (incoming.provenance == existing.provenance && incoming.fetched_at > existing.fetched_at)
}

fn provenance_rank(provenance: PriceProvenance) -> u8 {
    match provenance {
        PriceProvenance::OfficialLive => 4,
        PriceProvenance::OfficialCached => 3,
        PriceProvenance::OfficialEquivalent => 2,
        PriceProvenance::SupplementalCatalog => 1,
        PriceProvenance::ExactObserved | PriceProvenance::Unpriced => 0,
    }
}

pub fn catalog_changed(previous: Option<&PricingCatalog>, next: &PricingCatalog) -> bool {
    match previous {
        None => true,
        Some(previous) => previous.entries != next.entries || previous.aliases != next.aliases,
    }
}

#[derive(Debug, Default)]
pub struct PricingRefreshCoordinator {
    in_flight: Mutex<bool>,
}

impl PricingRefreshCoordinator {
    pub fn try_begin(&self) -> bool {
        let mut guard = self
            .in_flight
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *guard {
            false
        } else {
            *guard = true;
            true
        }
    }
    pub fn finish(&self) {
        if let Ok(mut guard) = self.in_flight.lock() {
            *guard = false;
        }
    }
}

pub async fn refresh_catalog(
    client: &reqwest::Client,
    cache_root: &Path,
    now: DateTime<Utc>,
    coordinator: &PricingRefreshCoordinator,
) -> PricingRefreshOutcome {
    if !coordinator.try_begin() {
        return PricingRefreshOutcome::Unchanged;
    }
    let outcome = refresh_catalog_inner(client, cache_root, now).await;
    coordinator.finish();
    outcome
}

async fn refresh_catalog_inner(
    client: &reqwest::Client,
    cache_root: &Path,
    now: DateTime<Utc>,
) -> PricingRefreshOutcome {
    let store = CatalogStore::for_cache_root(cache_root);
    let previous = store.load().ok().flatten();
    if let Some(catalog) = &previous
        && let Some(last_success) = latest_catalog_timestamp(catalog)
        && !refresh_due(last_success, now)
    {
        return PricingRefreshOutcome::Unchanged;
    }
    let mut snapshots = Vec::new();
    if let Ok(snapshot) = OpenAiAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    if let Ok(snapshot) = DeepSeekAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    if let Ok(snapshot) = XaiAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    if let Ok(snapshot) = KimiAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    if let Ok(snapshot) = QwenAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    if let Ok(snapshot) = ModelsDevAdapter.fetch(client).await {
        snapshots.push(snapshot);
    }
    let Ok(merged) = merge_source_snapshots(&snapshots) else {
        return if previous.is_some() {
            PricingRefreshOutcome::UsedCached
        } else {
            PricingRefreshOutcome::Failed
        };
    };
    if merged.validate_complete().is_err() {
        return if previous.is_some() {
            PricingRefreshOutcome::UsedCached
        } else {
            PricingRefreshOutcome::Failed
        };
    }
    let changed = catalog_changed(previous.as_ref(), &merged);
    if store.save(&merged).is_err() {
        return if previous.is_some() {
            PricingRefreshOutcome::UsedCached
        } else {
            PricingRefreshOutcome::Failed
        };
    }
    if changed {
        PricingRefreshOutcome::Updated
    } else {
        PricingRefreshOutcome::Unchanged
    }
}

pub async fn refresh_fx(
    client: &reqwest::Client,
    cache_root: &Path,
) -> Result<FxRateSnapshot, PricingSourceError> {
    let body = client
        .get(super::fx::FX_SOURCE_URL)
        .send()
        .await
        .map_err(|error| PricingSourceError::Fetch(error.to_string()))?
        .text()
        .await
        .map_err(|error| PricingSourceError::Fetch(error.to_string()))?;
    let snapshot = parse_official_usd_cny(&body)?;
    let _ = FxStore::for_cache_root(cache_root).save(&snapshot);
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::catalog::{CatalogEntry, Currency, MoneyMicros, TokenRates};
    use crate::pricing::source::{PricingSourceId, SourceSnapshot};

    fn at(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn entry(model: &str, provenance: PriceProvenance) -> CatalogEntry {
        CatalogEntry {
            canonical_model: model.to_string(),
            vendor: "openai".to_string(),
            rates: TokenRates {
                currency: Currency::Usd,
                input_per_million: MoneyMicros::from_micros(1_000_000),
                cached_input_per_million: MoneyMicros::from_micros(100_000),
                output_per_million: MoneyMicros::from_micros(2_000_000),
                context_tiers: Vec::new(),
            },
            source_url: "https://platform.openai.com/pricing".to_string(),
            fetched_at: at("2026-08-24T00:00:00Z"),
            parser_revision: "test".to_string(),
            provenance,
        }
    }

    #[test]
    fn refresh_is_due_after_24_hours_but_not_before() {
        assert!(!refresh_due(
            at("2026-08-24T00:00:00Z"),
            at("2026-08-24T23:59:59Z")
        ));
        assert!(refresh_due(
            at("2026-08-24T00:00:00Z"),
            at("2026-08-25T00:00:00Z")
        ));
    }

    #[test]
    fn official_live_outranks_supplemental_for_the_same_model() {
        let official = SourceSnapshot {
            source_id: PricingSourceId::OpenAi,
            source_url: "https://platform.openai.com/pricing".to_string(),
            fetched_at: at("2026-08-24T00:00:00Z"),
            parser_revision: "openai".to_string(),
            entries: vec![entry("gpt-5.6-sol", PriceProvenance::OfficialLive)],
        };
        let mut supplemental_entry = entry("gpt-5.6-sol", PriceProvenance::SupplementalCatalog);
        supplemental_entry.source_url = "https://models.dev/api.json".to_string();
        let supplemental = SourceSnapshot {
            source_id: PricingSourceId::ModelsDev,
            source_url: "https://models.dev/api.json".to_string(),
            fetched_at: at("2026-08-24T00:00:00Z"),
            parser_revision: "models-dev".to_string(),
            entries: vec![supplemental_entry],
        };
        let catalog = merge_source_snapshots(&[supplemental, official]).unwrap();
        assert_eq!(
            catalog.lookup("gpt-5.6-sol").unwrap().provenance,
            PriceProvenance::OfficialLive
        );
    }
}
