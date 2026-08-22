# Settings Expansion Integration and Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the completed About, Notifications, Taskbar & Tray, Menu, and Usage & Spend milestones into one localized, migratable, responsive Windows settings experience with fresh native proof.

**Architecture:** This plan contains no new product subsystem. It audits and closes cross-cutting seams: one settings snapshot schema, localized copy, state-specific UI, CSS density, migration compatibility, and Windows-native observables. It depends on the four feature plans already passing their focused tests.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, SQLite, pnpm, CUA Driver.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- Do not expand scope beyond the approved settings features.
- Keep all Codex account/usage operations read-only except existing unrelated account-management flows.
- Retain no tokens, cookies, raw session logs, reset-credit identifiers, or private account values in fixtures, logs, screenshots, or committed proof.
- Keep `general`, `providers`, `notifications`, `menuBar`, `menu`, `usageSpend`, `advanced`, and `about` as the only valid settings IDs.
- All new user-facing text must have English and Simplified Chinese versions.
- Preserve existing behavior until a user changes a corresponding new preference.
- Do not push, tag, package, or release without user approval.

## File Structure

| File | Responsibility |
| --- | --- |
| `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts` | Canonical bilingual copy audit for all new tabs and states. |
| `apps/desktop-tauri/src/surfaces/Settings.tsx` | Final tab routing/loading/error integration. |
| `apps/desktop-tauri/src/surfaces/settings/*.test.tsx` | Cross-tab localization, state, keyboard, and size regression tests. |
| `apps/desktop-tauri/src/styles.css` and settings-related CSS | Compact cards, minimum-size resilience, focus visibility, reduced-motion behavior. |
| `rust/src/storage/settings_repository.rs` | End-to-end migration/round-trip regression matrix. |
| `apps/desktop-tauri/src-tauri/src/proof_harness.rs` | Safe synthetic fixtures only if coverage gaps prevent CUA proof. |
| `docs/verification/*` | Optional sanitized evidence references only if the existing review process requires a committed document. |

---

### Task 1: Close bilingual and state-specific UI gaps

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx`
- Modify: relevant `apps/desktop-tauri/src/surfaces/settings/tabs/*.test.tsx`
- Modify: `apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts` only if a new taskbar string remains hard-coded to one language

**Interfaces:**
- Consumes all settings copy keys introduced in prior plans.
- Produces a compile-time-complete `SettingsCopy` object where English and Chinese satisfy the same interface.

- [ ] **Step 1: Write a failing bilingual key-parity test**

Export the two copy objects only through a test helper or inspect the returned objects. Assert every newly introduced leaf is present in both locales and render each tab in both languages:

```tsx
for (const language of ["en-US", "zh-CN"] as const) {
  render(<SettingsForTest language={language} initialTab="usageSpend" />);
  expect(screen.getByTestId("usage-spend-tab")).toBeInTheDocument();
}
```

Add assertions that unsupported/stale/empty/error states use locale-specific text rather than the legacy generic protocol-anomaly message.

- [ ] **Step 2: Run focused frontend tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- Settings NotificationsTab TaskbarTrayTab MenuTab UsageSpendTab
```

Expected: at least one missing or hard-coded new-state string fails until the audit is completed.

- [ ] **Step 3: Complete copy and replace hard-coded new feature text**

Add every label, helper, status, action, and error string through `SettingsCopy`. Keep source-generated version numbers, exact percentages, dates, and model identifiers as data rather than translated literals. Do not modify existing account identity privacy semantics.

- [ ] **Step 4: Run tests and verify GREEN**

Run the same command. Confirm all concrete settings tabs render in both languages with no placeholder text.

- [ ] **Step 5: Commit localization completion**

```powershell
git add apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts
git commit -m "Complete settings localization"
```

### Task 2: Verify migration, malformed settings, and presentation invariants

**Files:**
- Modify: `rust/src/storage/settings_repository.rs` tests
- Modify: `apps/desktop-tauri/src/types/bridge.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs` tests

**Interfaces:**
- Consumes final `AppSettings`, nested preferences, menu layouts, notification preferences, and `ProfileUsageSnapshot.reset_credits`.
- Produces a test matrix proving a pre-expansion settings JSON, malformed nested values, and new settings JSON round-trip safely.

- [ ] **Step 1: Add failing full-matrix storage tests**

Seed these exact classes of persisted JSON:

```text
1. Pre-expansion V1 JSON with only the original fields.
2. Valid expanded JSON with every preference changed.
3. Invalid notification threshold ordering.
4. Unknown taskbar enum value.
5. Menu layouts containing duplicates, unknown IDs, and hidden required IDs.
6. Empty/null reset-credit snapshot.
```

Assert expected default, safe recovery, or atomic rejection behavior for each. Add a cross-surface invariant test that taskbar/tray/float use the same universal weekly rounded percentage and never choose an additional five-hour window.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_bridge::tests -- --nocapture
pnpm --dir apps/desktop-tauri test -- bridge taskbarStatusPresentation
```

Expected: missing matrix fixtures/assertions fail until final behavior is verified.

- [ ] **Step 3: Fix only defects exposed by the matrix**

Use nested serde defaults for omitted values, normalize only documented recoverable menu data, and reject user-entered invalid threshold patches atomically. Keep invalid on-disk scalar/enumeration handling on the existing safe recovery route rather than inventing partial silent repairs.

- [ ] **Step 4: Run tests and verify GREEN**

Run the same commands and confirm all six data classes have explicit expected behavior.

- [ ] **Step 5: Commit invariant coverage**

```powershell
git add rust/src/storage/settings_repository.rs apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts apps/desktop-tauri/src-tauri/src/tray_bridge.rs
git commit -m "Verify settings migration invariants"
```

### Task 3: Compact responsive visual integration

**Files:**
- Modify: `apps/desktop-tauri/src/styles.css` and the existing settings/tray/taskbar stylesheet files that own the affected selectors
- Modify: focused settings surface tests where DOM/state assertions are needed

**Interfaces:**
- Consumes final semantic markup and `data-density` attributes from prior plans.
- Produces compact grouped cards that stay readable at the supported settings minimum size and preserve visible focus/keyboard controls.

- [ ] **Step 1: Add failing DOM layout-state tests**

Assert each concrete tab has a non-placeholder heading and its grouped controls are discoverable. Add taskbar content tests for compact/standard density so disabled elements do not leave empty separators or fixed-width gaps.

- [ ] **Step 2: Run focused frontend tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- Settings TaskbarStatus TrayPanel
```

Expected: the newly asserted compact group/data-density contract fails before CSS/markup cleanup.

- [ ] **Step 3: Implement scoped CSS cleanup**

Use existing settings classes; add no global reset. Ensure form groups wrap or scroll inside the current settings window rather than extending beneath the Windows taskbar. Preserve `:focus-visible`, high-contrast-readable colors, and `prefers-reduced-motion` behavior. Do not recreate prior large empty cards simply to fill visual space.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command. Confirm settings tab navigation, taskbar measurement, tray panel, and action stripping all retain their test contracts.

- [ ] **Step 5: Commit scoped visual polish**

```powershell
git add apps/desktop-tauri/src/styles.css apps/desktop-tauri/src/surfaces apps/desktop-tauri/src/hooks
git commit -m "Polish settings layout"
```

### Task 4: Full automated checks and Windows CUA acceptance run

**Files:**
- Verify only; create no binary/evidence artifact in Git without an explicit user request.

**Interfaces:**
- Consumes every prior milestone.
- Produces a final validation report, screenshots stored outside the repository unless the project review process explicitly requires them, and a clean scope audit.

- [ ] **Step 1: Run the local CI slice and all focused commands**

Run:

```powershell
.\scripts\local-check.ps1
cargo fmt --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

- [ ] **Step 2: Fresh-build after closing only the resolved running instance**

Run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Record the new executable path and close no unrelated process. Launch without proof mode for resident-surface checks, then with each supported proof-mode settings tab for deterministic settings captures.

- [ ] **Step 3: CUA test the complete user journey**

With CUA Driver, verify all of these on the fresh binary:

- About version matches the installed package version.
- Notifications begin disabled, save after toggle, dedupe, and test without a visible console window.
- Taskbar & Tray controls apply instantly and account visibility shows at most six graphemes.
- Native tray menu and panel quick-action order/visibility apply immediately and recover defaults.
- Usage & Spend distinguishes fresh/stale/unsupported/zero reset-credit states and never offers a mutation.
- Changing language updates settings, taskbar, tray tooltip, and floating-ball text consistently.
- Full-screen hiding follows the setting and desktop/taskbar shell interactions do not cause resident surfaces to disappear permanently.

- [ ] **Step 4: Run privacy and release-boundary audit**

Check `git status --short`, `git diff --check`, generated screenshots, diagnostics, notification cache, and test fixtures. Confirm no credential, account email, real username, raw log path, token, cookie, or reset-credit identifier is staged. Report that installer/release work remains out of scope until separately authorized.
