# v1.1 Dynamic Pricing Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax - [ ] for tracking.

**Goal:** Replace embedded numeric model prices with a daily refreshed, source-labelled catalog that estimates local Codex session cost safely in USD or CNY without pretending to be a provider invoice.

**Architecture:** The shared Rust crate owns vendor source adapters, price validation, model aliases, local catalog/FX caches, and cost resolution. The Tauri shell schedules non-blocking refreshes and emits compact health data. Usage & Spend receives typed cost resolutions and renders exact/official/official-equivalent/unpriced states without losing usable token data.

**Tech Stack:** Rust 2024, existing reqwest/serde_json/regex-lite/chrono/SQLite support, Tauri 2, React 18, TypeScript, Vitest, existing notifications controller, CUA proof.

**Spec:** docs/superpowers/specs/2026-08-24-v1-1-identity-pricing-surfaces-design.md

## Global Constraints

- Follow docs/superpowers/plans/2026-08-24-v1-1-rollout-index.md and complete the v2 settings migration before this plan.
- Numeric prices are not embedded in Rust/TypeScript source. Fixtures may contain synthetic numbers solely for parser tests.
- Public source fetches never use user credentials, cookies, account identifiers, or session logs.
- An unambiguous alias is allowed; fuzzy/provider-name guessing is forbidden.
- Unknown/ambiguous models retain token totals and display no cost.
- The user-selected display currency is USD or CNY. A conversion only uses a cached official USD/CNY rate with an explicit date/source.
- Catalog health notifications are opt-in under the existing master notification preference.
- CI must use fixtures/mocks only; no test or build waits on a live vendor page.

---

### Task 1: Define the dynamic catalog, cache, and resolver contracts

**Files:**
- Create: rust/src/pricing/mod.rs
- Create: rust/src/pricing/catalog.rs
- Create: rust/src/pricing/model_alias.rs
- Create: rust/src/pricing/cache.rs
- Modify: rust/src/lib.rs
- Modify: rust/src/core/mod.rs
- Modify: rust/src/core/cost_pricing.rs
- Modify: rust/src/core/cost_pricing_tests.rs
- Test: rust/src/pricing/catalog.rs
- Test: rust/src/pricing/model_alias.rs
- Test: rust/src/pricing/cache.rs

**Interfaces:**
- Produce Currency::{Usd, Cny}.
- Produce PriceProvenance::{ExactObserved, OfficialLive, OfficialCached, SupplementalCatalog, OfficialEquivalent, Unpriced}.
- Produce TokenRates { currency, input_per_million, cached_input_per_million, output_per_million, context_tiers }.
- Produce CatalogEntry { canonical_model, vendor, rates, source_url, fetched_at, parser_revision, provenance }.
- Produce MoneyMicros(i64) and CostResolution { amount: Option<MoneyMicros>, currency, provenance, canonical_model: Option<String>, source_updated_at: Option<DateTime<Utc>> }.
- Produce trait PricingResolver { fn resolve(&self, model: &str, input: u64, cached: u64, output: u64) -> CostResolution; }.

- [ ] **Step 1: Write failing resolver and cache tests.**

~~~rust
#[test]
fn direct_catalog_rate_uses_cache_input_and_output_dimensions() {
    let catalog = catalog_with(entry("gpt-test", usd_rates(2, 1, 8)));
    let result = catalog.resolve("gpt-test", 1_000_000, 250_000, 1_000_000);
    assert_eq!(result.provenance, PriceProvenance::OfficialCached);
    assert_eq!(result.amount.unwrap().micros(), 9_500_000);
}

#[test]
fn unknown_model_is_unpriced_not_zero() {
    let result = PricingCatalog::empty().resolve("mystery-model", 9, 0, 3);
    assert_eq!(result.provenance, PriceProvenance::Unpriced);
    assert_eq!(result.amount, None);
}

#[test]
fn cache_write_replaces_only_a_complete_catalog() {
    let store = CatalogStore::for_test(tempdir().unwrap().path());
    store.save(&catalog_with(entry("a", usd_rates(1, 1, 1)))).unwrap();
    assert_eq!(store.load().unwrap().unwrap().entries.len(), 1);
}
~~~

- [ ] **Step 2: Run the focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing::catalog -- --nocapture
cargo test --manifest-path rust/Cargo.toml pricing::cache -- --nocapture
cargo test --manifest-path rust/Cargo.toml pricing::model_alias -- --nocapture
~~~

Expected: the pricing module and resolver types do not exist.

- [ ] **Step 3: Implement typed, decimal-safe catalog records.**

~~~rust
pub trait PricingResolver {
    fn resolve(&self, model: &str, input: u64, cached: u64, output: u64) -> CostResolution;
}

impl PricingResolver for PricingCatalog {
    fn resolve(&self, model: &str, input: u64, cached: u64, output: u64) -> CostResolution {
        let Some(entry) = self.lookup(model) else { return CostResolution::unpriced(); };
        CostResolution::from_rates(entry, input, cached, output)
    }
}
~~~

Use integer micros or another fixed-point representation; do not calculate monetary
totals with binary float values. Persist catalog JSON atomically under the
application cache root. A corrupt cache is rejected and leaves the prior file
untouched.

- [ ] **Step 4: Replace static tables with the dynamic resolver boundary.**

Retire numeric CODEX_PRICING and CLAUDE_PRICING tables. Keep only a
compatibility facade if needed, but it must delegate to PricingResolver and
return Unpriced when catalog data is absent. Update all current static call
sites found by this command:

~~~powershell
rg -n "CostUsagePricing::|CODEX_PRICING|CLAUDE_PRICING" rust/src
~~~

The target files include rust/src/usage_spend.rs, rust/src/codex_costs.rs,
rust/src/cost_scanner.rs, rust/src/pi_session_cost.rs,
rust/src/codex_workspaces/indexer.rs, and rust/src/core/jsonl_scanner.rs.

- [ ] **Step 5: Run shared-crate checks and commit.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing:: -- --nocapture
cargo test --manifest-path rust/Cargo.toml core::cost_pricing_tests -- --nocapture
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
~~~

Commit:

~~~powershell
git add rust/src/pricing rust/src/core rust/src/usage_spend.rs rust/src/codex_costs.rs rust/src/cost_scanner.rs rust/src/pi_session_cost.rs rust/src/codex_workspaces rust/src/lib.rs
git commit -m "Add dynamic pricing catalog"
~~~

### Task 2: Implement official source adapters and safe aliases

**Files:**
- Create: rust/src/pricing/source.rs
- Create: rust/src/pricing/sources/openai.rs
- Create: rust/src/pricing/sources/deepseek.rs
- Create: rust/src/pricing/sources/xai.rs
- Create: rust/src/pricing/sources/kimi.rs
- Create: rust/src/pricing/sources/qwen.rs
- Create: rust/src/pricing/sources/supplemental.rs
- Create: rust/src/pricing/fixtures/openai.json
- Create: rust/src/pricing/fixtures/deepseek.json
- Create: rust/src/pricing/fixtures/xai.json
- Create: rust/src/pricing/fixtures/kimi.json
- Create: rust/src/pricing/fixtures/qwen.json
- Modify: rust/src/pricing/mod.rs
- Modify: rust/src/core/models_dev_pricing.rs
- Test: rust/src/pricing/source.rs
- Test: rust/src/pricing/model_alias.rs

**Interfaces:**
- Produce trait PricingSourceAdapter { fn id(&self) -> PricingSourceId; async fn fetch(&self, client: &reqwest::Client) -> Result<SourceSnapshot, PricingSourceError>; }.
- Produce PricingSourceId::{OpenAi, DeepSeek, Xai, Kimi, Qwen, ModelsDev}.
- Produce ModelAliasResolver::resolve_alias(model: &str) -> AliasResolution.
- AliasResolution is Exact(canonical_model), Ambiguous, or None.

- [ ] **Step 1: Write failing source-fixture parser tests.**

~~~rust
#[tokio::test]
async fn deepseek_fixture_preserves_native_cny_and_cache_dimensions() {
    let snapshot = DeepSeekAdapter::parse(include_str!("fixtures/deepseek.json")).unwrap();
    let entry = snapshot.entries.iter().find(|entry| entry.canonical_model == "deepseek-v4-flash").unwrap();
    assert_eq!(entry.rates.currency, Currency::Cny);
    assert!(entry.rates.cached_input_per_million < entry.rates.input_per_million);
}

#[test]
fn gateway_alias_maps_only_an_exact_model_suffix() {
    assert_eq!(ModelAliasResolver::default().resolve_alias("4sapi-gpt/gpt-5.6-sol"), AliasResolution::Exact("gpt-5.6-sol".into()));
    assert_eq!(ModelAliasResolver::default().resolve_alias("4sapi-gpt/gpt-5.6"), AliasResolution::None);
}
~~~

- [ ] **Step 2: Run source/alias tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing::sources -- --nocapture
cargo test --manifest-path rust/Cargo.toml pricing::model_alias -- --nocapture
~~~

Expected: adapters, fixtures, and exact alias resolution are absent.

- [ ] **Step 3: Implement schema-guarded official adapters.**

Each adapter declares a fixed official URL, fetches with the existing reqwest
client, and validates source currency, model ID, token units, non-negative
rates, and documented cache/context dimensions before returning SourceSnapshot.
Use structured official data when supplied. A page parser accepts only the
fixture-validated structure needed by that vendor; it rejects a page whose
shape no longer matches. ModelsDev is a supplemental adapter and cannot replace
an OfficialLive or OfficialCached record.

- [ ] **Step 4: Implement alias rules without numeric pricing.**

~~~rust
const GATEWAY_PREFIXES: &[&str] = &["4sapi-gpt/", "4sapi-kimi/", "openai/", "deepseek/", "xai/", "kimi/", "qwen/"];

pub fn resolve_alias(&self, raw: &str) -> AliasResolution {
    let normalized = normalize_model_id(raw);
    self.exact_aliases.get(&normalized).cloned()
        .map(AliasResolution::Exact)
        .unwrap_or(AliasResolution::None)
}
~~~

Store only canonical string mappings and mapping revision metadata. Never map
prefix-only, partial-version, or nearest-name values.

- [ ] **Step 5: Run all pricing source tests and commit.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing:: -- --nocapture
cargo fmt --all -- --check
git diff --check
~~~

Commit:

~~~powershell
git add rust/src/pricing rust/src/core/models_dev_pricing.rs
git commit -m "Fetch official model pricing sources"
~~~

### Task 3: Add USD/CNY conversion and catalog refresh scheduling

**Files:**
- Create: rust/src/pricing/fx.rs
- Create: rust/src/pricing/refresh.rs
- Create: apps/desktop-tauri/src-tauri/src/pricing_refresh.rs
- Modify: rust/src/app_paths.rs
- Modify: rust/src/pricing/cache.rs
- Modify: rust/src/notifications/v1.rs
- Modify: rust/src/storage/settings_repository.rs
- Modify: apps/desktop-tauri/src-tauri/src/notification_controller.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Test: rust/src/pricing/fx.rs
- Test: rust/src/pricing/refresh.rs
- Test: rust/src/notifications/v1.rs

**Interfaces:**
- Produce FxRateSnapshot { base: Currency::Usd, quote: Currency::Cny, rate: FixedAmount, observed_at, source_url }.
- Produce PricingRefreshOutcome::{Updated, Unchanged, UsedCached, Failed}.
- Produce start_pricing_refresh_monitor(app: AppHandle).
- Extend NotificationPreferences with pricing_changed_enabled and pricing_refresh_failure_enabled, both false by default.
- Extend settings with pricing_display_currency.

- [ ] **Step 1: Write failing FX, cadence, and notification-dedupe tests.**

~~~rust
#[test]
fn cny_conversion_requires_a_dated_usd_cny_rate() {
    let result = convert_amount(usd_micros(1_000_000), Currency::Usd, Currency::Cny, None);
    assert_eq!(result, None);
}

#[test]
fn refresh_is_due_after_24_hours_but_not_before() {
    assert!(!refresh_due(at("2026-08-24T00:00:00Z"), at("2026-08-24T23:59:59Z")));
    assert!(refresh_due(at("2026-08-24T00:00:00Z"), at("2026-08-25T00:00:00Z")));
}

#[test]
fn third_identical_catalog_failure_emits_once() {
    let mut state = CatalogNotificationState::default();
    assert!(!state.record_failure());
    assert!(!state.record_failure());
    assert!(state.record_failure());
    assert!(!state.record_failure());
}
~~~

- [ ] **Step 2: Run focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing::fx -- --nocapture
cargo test --manifest-path rust/Cargo.toml pricing::refresh -- --nocapture
cargo test --manifest-path rust/Cargo.toml notifications::v1 -- --nocapture
~~~

Expected: FX records, refresh cadence, and catalog notification state are absent.

- [ ] **Step 3: Implement FX cache and daily coordinator.**

The coordinator runs in the background only when the last successful catalog
snapshot is at least 24 hours old. It coalesces concurrent requests, fetches
official sources independently with bounded timeouts, keeps valid old snapshots
on failure, and atomically saves only a fully validated merged catalog. The FX
adapter stores the official USD/CNY source date. It must not issue any HTTP call
from React.

- [ ] **Step 4: Wire opt-in notification decisions.**

The master notification switch remains authoritative. Price change and third
failure/recovery events require their own enabled fields and persist a small
non-secret dedupe state. Toast copy contains source names/counts only, never
prices from a raw response, account identifiers, URLs, or local log content.

- [ ] **Step 5: Run shared and shell checks, then commit.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml pricing:: -- --nocapture
cargo test --manifest-path rust/Cargo.toml notifications:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml pricing_refresh -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller -- --nocapture
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git diff --check
~~~

Commit:

~~~powershell
git add rust/src/pricing rust/src/app_paths.rs rust/src/notifications rust/src/storage/settings_repository.rs apps/desktop-tauri/src-tauri/src/pricing_refresh.rs apps/desktop-tauri/src-tauri/src/notification_controller.rs apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Refresh pricing catalog daily"
~~~

### Task 4: Bridge dynamic estimates into Usage & Spend

**Files:**
- Modify: rust/src/usage_spend.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Modify: apps/desktop-tauri/src/lib/tauri.ts
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
- Test: rust/src/usage_spend.rs
- Test: apps/desktop-tauri/src/types/bridge.test.ts

**Interfaces:**
- Produce CostEstimateDto { amount: number | null, currency: "USD" | "CNY", provenance: "exactObserved" | "officialDirect" | "officialEquivalent" | "unpriced", canonicalModel: string | null, sourceUpdatedAt: string | null }.
- Extend LocalUsageSpendDto with displayCurrency, pricingStatus, partialEstimate, unpricedModelCount, and per-row CostEstimateDto.
- Change column copy from Cost unavailable / 费用不可用 to Cost / 费用.

- [ ] **Step 1: Write failing report conversion tests.**

~~~rust
#[test]
fn mixed_priced_and_unpriced_models_return_a_partial_total() {
    let report = report_with_models([priced("gpt-test"), unpriced("mystery")]);
    let dto = local_dto(report, range(), Currency::Usd);
    assert!(dto.partial_estimate);
    assert_eq!(dto.unpriced_model_count, 1);
    assert!(dto.estimated_cost.amount.is_some());
}
~~~

~~~tsx
it("labels a mapped gateway row as an official-equivalent estimate", () => {
  render(<UsageSpendTab copy={copy} language="en-US" />);
  expect(screen.getByText("Official-equivalent estimate")).toBeInTheDocument();
  expect(screen.getAllByRole("columnheader", { name: "Cost" }).length).toBeGreaterThan(0);
});
~~~

- [ ] **Step 2: Run focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml usage_spend::tests::mixed_priced_and_unpriced_models_return_a_partial_total -- --nocapture
pnpm --dir apps/desktop-tauri test -- UsageSpendTab bridge
~~~

Expected: current strict None aggregate and Cost unavailable labels fail the assertions.

- [ ] **Step 3: Thread PricingCatalog through local scanning.**

~~~rust
pub fn scan_local_codex_usage(
    range: CodexUsageRange,
    cache_root: &Path,
    pricing: &dyn PricingResolver,
    cancellation: Option<&AtomicBool>,
) -> Result<LocalUsageSpendReport, LocalUsageSpendError>
~~~

Load the cached catalog in the command, pass the resolver to scanning, and
preserve token aggregation when resolve returns Unpriced. Use exact observed
provider cost only when local records contain a validated exact amount.

- [ ] **Step 4: Render source-aware cost states in the UI.**

Add USD/CNY selector, source/update summary, partial-estimate summary, and
per-row provenance copy. Unknown rows render an em dash; no row says cost
unavailable. Preserve the persistent local-estimate billing disclosure and
existing no-log/cancelled/stale states.

- [ ] **Step 5: Run all affected suites and commit.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml usage_spend:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::usage_spend::tests -- --nocapture
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
~~~

Commit:

~~~powershell
git add rust/src/usage_spend.rs apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.tsx apps/desktop-tauri/src/surfaces/settings
git commit -m "Show dynamic local cost estimates"
~~~

### Task 5: Verify pricing behavior without live-test dependence

**Files:**
- Verification evidence only unless a defect is exposed by Tasks 1–4.

**Interfaces:**
- Consume persisted catalog and FX fixtures.
- Produce an audit record containing source health, conversion date, and UI screenshots without raw source payloads.

- [ ] **Step 1: Run cache/fixture tests with networking disabled.**

Run:

~~~powershell
$env:HTTP_PROXY = 'http://127.0.0.1:9'
cargo test --manifest-path rust/Cargo.toml pricing:: -- --nocapture
cargo test --manifest-path rust/Cargo.toml usage_spend:: -- --nocapture
Remove-Item Env:HTTP_PROXY
~~~

Expected: parser/cache/report tests pass without a live HTTP response.

- [ ] **Step 2: Fresh-build the Tauri app and inspect Usage & Spend with CUA.**

Verify USD/CNY selection, source date, exact/direct/equivalent/unpriced labels,
partial totals, empty/stale cache paths, and the local-estimate disclosure.

- [ ] **Step 3: Restore original settings and capture proof.**

Do not alter a user-selected currency permanently. Record screenshots and
non-secret cache health only; do not publish, tag, or install.
