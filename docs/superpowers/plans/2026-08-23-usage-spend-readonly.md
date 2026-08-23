# Read-Only Usage and Spend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Usage & Spend placeholder with a read-only view of the universal weekly Codex allowance, banked reset-credit count, local token usage, strict local cost estimates, daily trend, and per-model totals.

**Architecture:** Extend the existing Codex app-server rate-limit parser with a redacted reset-credit summary that contains only an available count. Add a Codex-local log aggregation service that reuses cached JSONL scanning for tokens but recomputes costs from canonical known-model pricing only; the Tauri command combines that local report with the selected profile's official weekly state without falsely attributing device logs to an account.

**Tech Stack:** Rust 2024, Tauri 2, serde JSON, SQLite snapshot persistence, existing `CostScanner`/`JsonlScanner`/`CostUsagePricing`, React 18, TypeScript, Vitest, CUA Driver.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- The page is strictly read-only: no reset redemption, credit purchase, account-plan action, updater action, or Codex mutation.
- Display only the universal 10,080-minute weekly allowance; do not show model-specific 5-hour allowances as universal quota.
- Reset-credit identifiers, titles, grant timestamps, and redemption endpoints never cross the domain/bridge boundary.
- `0` reset credits, unsupported reset credits, and stale usage are distinct states.
- Local log results are always labeled This device combined / 此设备合计 unless durable attribution is proven; initial implementation deliberately uses the combined label.
- Cost values are explicitly local estimates; unknown or unattributed models receive no fallback/guessed cost.
- Scan only local data, do not upload logs, and keep raw log contents out of errors, diagnostics, DTOs, and tests.
- Do not add dependencies, arbitrary file pickers, cloud sync, export features, or release actions.

## File Structure

| File | Responsibility |
| --- | --- |
| `rust/src/core/profile_usage.rs` | Persist redacted reset-credit summary with a profile quota snapshot. |
| `rust/src/providers/codex/app_server/model.rs` | Parse only `rateLimitResetCredits.availableCount` from app-server payloads. |
| `rust/src/providers/codex/app_server/fixtures/*.json` | Deterministic known/zero/null/malformed reset-credit response samples. |
| `rust/src/cost_scanner.rs` | Add explicit date-range scanning/report output and sanitized malformed-record counts. |
| `rust/src/usage_spend.rs` | Convert scanner report into V1 strict-pricing local usage DTO-ready model. |
| `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` | Define redacted Usage & Spend DTOs and map selected profile state. |
| `apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs` | Run the read-only aggregation command off the UI thread. |
| `apps/desktop-tauri/src-tauri/src/main.rs` | Register the read-only command and wire reset-credit data into notification observation. |
| `apps/desktop-tauri/src/types/bridge.ts` | Frontend Usage & Spend contract validation. |
| `apps/desktop-tauri/src/lib/tauri.ts` | Typed `getUsageSpend(range)` invoke wrapper. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.tsx` | Range selector, official allowance card, local estimate cards, chart, model table, and state-specific UI. |

---

### Task 1: Parse and persist a redacted reset-credit summary

**Files:**
- Modify: `rust/src/core/profile_usage.rs:103-174`
- Modify: `rust/src/providers/codex/app_server/model.rs:77-174,390-636`
- Modify: `rust/src/providers/codex/app_server/session.rs:130-144,169-183`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-known.json`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-zero.json`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-malformed.json`
- Modify: `rust/src/storage/usage_repository.rs` tests

**Interfaces:**
- Produces the redacted domain shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCreditsSummary {
    pub available_count: u64,
}

pub struct ParsedRateLimits {
    pub selected_limit_id: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub additional_windows: Vec<UsageWindow>,
    pub reset_credits: Option<ResetCreditsSummary>,
    pub protocol_anomaly: bool,
}

pub struct ProfileUsageSnapshot {
    // existing fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_credits: Option<ResetCreditsSummary>,
}
```

- [ ] **Step 1: Write failing parser and snapshot compatibility tests**

Add tests for:

```rust
let parsed = ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-known.json"))?;
assert_eq!(parsed.reset_credits.unwrap().available_count, 2);

let zero = ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-zero.json"))?;
assert_eq!(zero.reset_credits.unwrap().available_count, 0);

let malformed = ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-malformed.json"))?;
assert!(malformed.reset_credits.is_none());
assert!(malformed.protocol_anomaly);
```

Serialize a pre-feature `ProfileUsageSnapshot` JSON without `resetCredits` and verify it deserializes to `None`. Serialize a known summary and assert no JSON key contains `id`, `credit`, `title`, `grantedAt`, or `redeem` beyond the safe `resetCredits.availableCount` path.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model::tests storage::usage_repository::tests -- --nocapture
```

Expected: fixtures/fields are missing.

- [ ] **Step 3: Implement tolerant, count-only parsing**

In `ParsedRateLimits::from_value`, inspect only the root `rateLimitResetCredits` object. A missing or explicit `null` field yields `None`; a non-negative integral `availableCount` yields `Some(ResetCreditsSummary { available_count })`; strings, negatives, fractions, or unrelated shapes set `protocol_anomaly` and expose no summary.

Never deserialize `credits`, `id`, `title`, `grantedAt`, `resetType`, or `status`. Do not add any `account/rateLimitResetCredit/consume` request method to `session.rs`.

- [ ] **Step 4: Map the summary into saved profile snapshots**

Have `parse_profile_usage` copy `rates.reset_credits` into `ProfileUsageSnapshot`. Existing SQLite save/load then receives the optional serde field automatically; do not create a separate secret-bearing table.

- [ ] **Step 5: Run tests and verify GREEN**

Run the same focused command and confirm known, zero, null/missing, malformed, and pre-feature snapshot cases all pass.

- [ ] **Step 6: Commit the app-server data slice**

```powershell
git add rust/src/core/profile_usage.rs rust/src/providers/codex/app_server/model.rs rust/src/providers/codex/app_server/session.rs rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-known.json rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-zero.json rust/src/providers/codex/app_server/fixtures/rate-limits-reset-credits-malformed.json rust/src/storage/usage_repository.rs
git commit -m "Read Codex reset credit count"
```

### Task 2: Produce strict local token/cost reports for explicit ranges

**Files:**
- Modify: `rust/src/cost_scanner.rs:33-80,271-399`
- Create: `rust/src/usage_spend.rs`
- Modify: `rust/src/lib.rs`
- Test: `rust/src/cost_scanner.rs`
- Test: `rust/src/usage_spend.rs`

**Interfaces:**
- Produces a range-aware scanner API:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexUsageRange { pub start: NaiveDate, pub end: NaiveDate }

#[derive(Debug, Clone)]
pub struct CodexUsageScanReport {
    pub summary: CostSummary,
    pub daily: Vec<DailyCodexUsage>,
    pub malformed_records_skipped: u64,
    pub used_cache_debounce: bool,
}

pub struct LocalUsageSpendReport {
    pub range: CodexUsageRange,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub sessions_count: u32,
    pub estimated_cost_usd: Option<f64>,
    pub unknown_models: Vec<String>,
    pub daily: Vec<DailyUsageSpend>,
    pub models: Vec<ModelUsageSpend>,
    pub malformed_records_skipped: u64,
}

pub fn scan_local_codex_usage(
    range: CodexUsageRange,
    cache_root: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<LocalUsageSpendReport, LocalUsageSpendError>;
```

- [ ] **Step 1: Write failing strict-pricing and range tests**

Create a temp JSONL fixture containing known and unknown models across four dates. Assert:

```rust
let report = scan_local_codex_usage(range("2026-08-20", "2026-08-21"), cache.path(), None)?;
assert_eq!(report.input_tokens, 125);
assert_eq!(report.output_tokens, 15);
assert_eq!(report.daily.len(), 2);
assert_eq!(report.unknown_models, vec!["gpt-mystery"]);
assert_eq!(report.estimated_cost_usd, None); // unknown model prevents a falsely complete total
```

Add a second known-model-only fixture that returns `Some(cost)`, a malformed-line fixture that increments only the sanitized skipped count, a cancelled scan fixture, and a cache-debounce fixture.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml cost_scanner::tests usage_spend::tests -- --nocapture
```

Expected: range/report APIs do not exist.

- [ ] **Step 3: Add explicit range scanning without changing legacy behavior**

Keep `CostScanner::new(days).scan_codex()` behavior unchanged for existing callers. Add a separate `scan_codex_range_detailed(range, cancel)` path that reuses the existing cache/file-resume code but never includes dates outside the requested inclusive local range.

Expose daily model-token totals from the scan report and count malformed JSONL records without retaining their content. Preserve cancellation by checking the existing `AtomicBool` before each directory/file parse and return a distinct cancelled outcome.

- [ ] **Step 4: Convert scanner output to V1 strict estimates**

In `usage_spend.rs`, derive costs from `CostUsagePricing::codex_cost_usd(model, input, cached, output)` only. Do not use `CostSummary.total_cost_usd` because its legacy scanner fallback may estimate unknown models. If any priced-token model is unknown/unattributed, list it and set aggregate `estimated_cost_usd` to `None`; still show all token totals and per-model token rows.

Sort daily rows ascending by date and model rows by total tokens descending, then model ID ascending for a stable UI/test contract.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run the same command. Confirm exact range filtering, known-only cost, unknown no-cost, cached reuse, cancellation, and sanitized malformed count behavior.

- [ ] **Step 6: Commit the local aggregation layer**

```powershell
git add rust/src/cost_scanner.rs rust/src/usage_spend.rs rust/src/lib.rs
git commit -m "Add strict local usage estimates"
```

### Task 3: Expose a read-only Usage & Spend bridge command

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs:117-146`
- Modify: `apps/desktop-tauri/src-tauri/src/notification_controller.rs` (from notifications plan)
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/types/bridge.test.ts`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Test: `apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs`

**Interfaces:**
- Produces:

```ts
export type UsageSpendRange = "today" | "last7Days" | "last30Days" | "currentWeekly";
export type ResetCreditsState = "available" | "unsupported" | "stale";

export interface UsageSpendDto {
  official: {
    remainingPercent: number | null;
    resetsAt: string | null;
    fetchedAt: string | null;
    freshness: "fresh" | "stale" | "missing";
    resetCredits: { state: ResetCreditsState; availableCount: number | null; observedAt: string | null };
  };
  local: {
    attribution: "deviceCombined";
    range: UsageSpendRange;
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    totalTokens: number;
    sessionsCount: number;
    estimatedCostUsd: number | null;
    unknownModels: string[];
    daily: Array<{ date: string; totalTokens: number; estimatedCostUsd: number | null }>;
    models: Array<{ model: string; inputTokens: number; cachedInputTokens: number; outputTokens: number; totalTokens: number; estimatedCostUsd: number | null }>;
    state: "ready" | "empty" | "unavailable" | "cancelled";
    malformedRecordsSkipped: number;
  };
}
```

- Adds `get_usage_spend(range: UsageSpendRangeDto) -> Result<UsageSpendDto, String>`; no command accepts credit IDs or mutation parameters.

- [ ] **Step 1: Write failing DTO and command mapping tests**

Build a selected profile fixture with a five-hour window, a universal weekly window, and a reset-credit count. Assert only the weekly window crosses the bridge:

```rust
assert_eq!(dto.official.remaining_percent, Some(99));
assert_eq!(dto.official.reset_credits.available_count, Some(2));
assert_ne!(dto.official.remaining_percent, Some(five_hour.remaining_percent));
```

Add explicit tests for `availableCount: 0`, unsupported `None`, stale state, invalid range string, and no selected profile/local-log source.

- [ ] **Step 2: Run focused shell and frontend contract tests to verify RED**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::usage_spend::tests -- --nocapture
pnpm --dir apps/desktop-tauri test -- bridge
```

Expected: missing command/DTO failures.

- [ ] **Step 3: Implement selection, date-range resolution, and background scan**

Resolve Today, 7, and 30 day local dates deterministically. For Current Weekly, use the selected universal weekly reset time minus seven days only if the timestamp is valid and not implausible; otherwise return local `state: "unavailable"` with no fabricated range.

Read selected profile usage from repositories. Derive reset-credit `available` only when a fresh snapshot contains the summary; derive `stale` only when a cached snapshot contains the summary plus stale/error freshness; otherwise derive `unsupported`.

Run local scanning through `tauri::async_runtime::spawn_blocking`, using an app-owned cache root. The UI command returns sanitized data only; raw paths, raw JSONL lines, account tokens, and reset-credit detail arrays remain in Rust.

- [ ] **Step 4: Feed actual reset-credit counts into notifications**

Replace the temporary `None` passed by the notification controller with `snapshot.reset_credits.as_ref().map(|summary| summary.available_count)`. Preserve first-observation baseline behavior from the notification plan and add a controller test that `1 -> 2` produces only the increase event when notifications are enabled.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run the same command plus the notification-controller test. Confirm bridge DTO validation rejects malformed payloads and no command string contains `consume`, `redeem`, or a reset-credit identifier argument.

- [ ] **Step 6: Commit the bridge slice**

```powershell
git add apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/notification_controller.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/lib/tauri.ts
git commit -m "Expose read-only usage spend data"
```

### Task 4: Build the Usage & Spend settings tab

**Files:**
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx:15-25,143-178`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`
- Modify: `apps/desktop-tauri/src/styles.css` or the existing settings stylesheet that owns `.settings-panel` cards

**Interfaces:**
- Consumes `getUsageSpend(range)` and `UsageSpendDto`.
- Produces a read-only page with `data-testid="usage-spend-tab"`, range controls, official/local cards, daily trend, model table, and explicit state labels.

- [ ] **Step 1: Write failing component tests for every data state**

Mock each DTO state and assert:

```tsx
expect(screen.getByText(/local estimate, not an openai bill/i)).toBeInTheDocument();
expect(screen.getByText(/this device combined/i)).toBeInTheDocument();
expect(screen.getByText(/2 reset credits available/i)).toBeInTheDocument();
expect(screen.queryByRole("button", { name: /use reset/i })).not.toBeInTheDocument();
```

Cover English/Chinese, fresh/stale/unsupported/zero credits, no logs, cancelled scan, unknown models/no aggregate cost, malformed-record count, range change, and table sorting.

- [ ] **Step 2: Run focused UI tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- UsageSpendTab Settings
```

Expected: missing component/routing failures.

- [ ] **Step 3: Implement a compact, non-mutating dashboard**

Render in this order:

1. range selector and refresh-local-data button;
2. official universal weekly allowance card;
3. reset-credit row with Available/Unsupported/Stale state;
4. fixed local-estimate disclaimer and device-combined attribution;
5. token summary cards;
6. accessible daily SVG/table trend with text alternatives;
7. per-model table and unknown-model disclosure.

Use `aria-live="polite"` for scan status. The refresh button may re-run local aggregation only; it does not refresh Codex official account state or call a mutation.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command. Confirm no 5-hour label appears in the official universal section and every user-facing state has localized copy.

- [ ] **Step 5: Commit the UI slice**

```powershell
git add apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/UsageSpendTab.test.tsx apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts apps/desktop-tauri/src/styles.css
git commit -m "Add read-only usage spend dashboard"
```

### Task 5: Verify Usage & Spend end to end

**Files:**
- Verify only unless a task-owned defect is found.

**Interfaces:**
- Consumes Tasks 1–4 and notification plan controller integration.
- Produces automated and real Windows proof of a read-only dashboard.

- [ ] **Step 1: Run all automated checks**

Run:

```powershell
cargo fmt --all -- --check
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 2: Build and launch a fresh desktop binary**

Resolve and close only the exact running codex-barbar executable. Run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Launch with `CODEXBAR_PROOF_MODE=settings:usageSpend` and deterministic proof fixtures for known/zero/unsupported/stale reset-credit states.

- [ ] **Step 3: Capture Windows UI proof with CUA**

Verify:

- weekly official allowance is shown without a 5-hour universal metric;
- reset count distinguishes 0, unsupported, and stale;
- local estimates carry the non-billing and device-combined labels;
- range changes update the dashboard without freezing the settings window;
- unknown model tokens show but no guessed aggregate cost;
- no action to spend/reset/purchase/modify an account exists;
- restart leaves account usage data intact and local cache is reused safely.

- [ ] **Step 4: Final privacy and scope audit**

Inspect DTO logging and screenshots for account email, tokens, raw JSONL, raw paths, credit IDs, and pricing secrets. Remove/redact nonessential proof artifacts before any user-facing handoff. Do not push/release.
