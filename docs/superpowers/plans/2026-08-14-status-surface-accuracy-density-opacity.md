# Status Surface Accuracy, Density, and Opacity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make codex-barbar read the current official Codex quota despite a safe-to-reject OpenCodex wrapper, render only real quota windows in compact transparent status surfaces, and deliver a dense Chinese macOS-style settings experience.

**Architecture:** Extend the fail-closed Codex resolver with a verified official native npm-package fallback, then keep Rust as the source of truth for normalized quota DTOs and persisted opacity settings. A shared React status-surface view model derives dynamic quota rows, trust state, color band, compact identity, and urgent metric; the taskbar and float-ball components only render that model. The taskbar React surface reports its measured content width through a narrowly scoped Tauri command whose Rust side clamps and repositions the native overlay.

**Tech Stack:** Rust 2024, Tokio, serde/serde_json, rusqlite, Tauri 2.10.1, React 18, TypeScript 5.6, Vitest 3, Testing Library, CSS, Windows Win32/CUA, pnpm 10.18.1, NSIS.

## Global Constraints

- Windows 11 23H2+ x64; validate tray, App Server discovery, taskbar, WebView2, DPAPI, and installer behavior on native Windows.
- Do not add dependencies, DLL injection, Explorer injection, private quota APIs, web scraping, telemetry, or automatic update behavior.
- Never execute an unknown `.cmd`, `.ps1`, `.bat`, or shell wrapper; keep OpenCodex installed and unchanged.
- Official quota data remains `account/rateLimits/read`; Rust owns normalization, remaining-percent calculation, persistence, freshness, and error state.
- Render only quota windows actually returned by Rust. Do not invent a 300-minute/5H row when only the 10080-minute weekly window exists.
- Taskbar compact identity is the first six Unicode code points; complete local identity remains in the title/accessible label and main panel.
- `taskbarStatusOpacity` and `floatBallOpacity` are independent integer percentages in `0..=80`, both defaulting to 20.
- Remaining quota color bands are exact: 67–100 green, 34–66 yellow, 0–33 red; refreshing, stale, error, and missing are gray.
- Taskbar status target height is 40 logical px; collapsed/expanded float-ball sizes remain 88×88 and 260×148 logical px.
- Auxiliary WebViews stay `.transparent(true)`, `.decorations(false)`, `.closable(true)`, non-activating where currently required, and the float-ball builder stays `.theme(Some(tauri::Theme::Dark))`.
- Settings target size is 680×500 logical px with a left category sidebar and keyboard-accessible content pane.
- Package manager stays pnpm 10.18.1. Do not add npm/yarn lockfiles.
- Use RED → minimal GREEN → refactor for every production behavior. End each task with a scoped commit and an independent review gate.
- Preserve the five unrelated untracked root files; never stage or delete them as part of this plan.

## File and Interface Map

### Rust shared crate

- `rust/src/providers/codex/app_server/npm.rs`: verify official npm layouts and resolve the platform package's native executable without executing a shim.
- `rust/src/providers/codex/app_server/discovery.rs`: search each PATH npm prefix through the verified official native fallback after a rejected `.cmd` candidate.
- `rust/src/storage/settings_repository.rs`: persist and validate independent surface opacity fields.

### Tauri shell

- `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`: keep Rust settings DTOs aligned with TypeScript.
- `apps/desktop-tauri/src-tauri/src/commands/settings.rs`: accept opacity patches and emit the authoritative snapshot.
- `apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs`: expose a typed taskbar content-width command.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`: replace the 318px fixed contract with min/max/default width constants and 40px height.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`: store current logical content width, clamp it, resize the window, and reposition with that width.
- `apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs`: retain work-area-safe placement for dynamic widths.
- `apps/desktop-tauri/src-tauri/src/shell/settings_window.rs`: change default Settings size to 680×500.
- `apps/desktop-tauri/src-tauri/src/proof_harness.rs`: add a weekly-only deterministic status fixture and opacity defaults.
- `apps/desktop-tauri/src-tauri/src/main.rs`: register the new typed Tauri command.

### React frontend

- `apps/desktop-tauri/src/types/bridge.ts`: add both opacity fields.
- `apps/desktop-tauri/src/lib/tauri.ts`: add `setTaskbarStatusWidth(width: number): Promise<void>`.
- `apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts`: expose dynamic `metrics`, `urgentMetric`, `compactIdentity`, `trustState`, and `quotaBand`.
- `apps/desktop-tauri/src/hooks/useStatusSurface.ts`: subscribe to settings changes so opacity and display mode update without recreating the window.
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`: measure compact content with `ResizeObserver`, round once, and invoke only when the width changes.
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx` / `.css`: implement selected B layout, six-character identity, dynamic rows, color bands, and background-only opacity.
- `apps/desktop-tauri/src/surfaces/FloatBall.tsx` / `.css`: render dynamic real quota windows and independent background-only opacity.
- `apps/desktop-tauri/src/surfaces/Settings.tsx`, `apps/desktop-tauri/src/styles.css`, `apps/desktop-tauri/src/surfaces/settings/settingsTabs.ts`: implement compact localized sidebar shell.
- `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx`: add independent opacity cards/sliders.
- Existing co-located tests plus new `useTaskbarStatusWidth.test.tsx`: lock contracts before implementation.

---

### Task 1: Resolve the Official Native Codex Package Behind a Rejected Wrapper

**Files:**
- Modify: `rust/src/providers/codex/app_server/npm.rs`
- Modify: `rust/src/providers/codex/app_server/discovery.rs`
- Test: `rust/src/providers/codex/app_server/npm.rs`
- Test: `rust/src/providers/codex/app_server/discovery.rs`

**Interfaces:**
- Produces: `pub(crate) fn resolve_official_native_from_prefix(prefix: &Path) -> Result<ResolvedCodexCommand, AppError>`.
- Produces: a `ResolvedCodexCommand` whose program is the canonical official platform-package `bin\codex.exe`, `args_prefix()` is empty, and installation is `CodexInstallation::VerifiedNpmLayout`.
- Consumes later: existing `probe_version`, `AppServerSpawnSpec`, and settings compatibility DTOs without widening their command surface.

- [ ] **Step 1: Add a deterministic official native npm fixture**

In the existing `npm.rs` test module, extend `NpmFixture` with a builder that writes exactly:

```text
npm/
  codex.cmd                         # arbitrary non-official wrapper
  node_modules/@openai/codex/package.json
  node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/package.json
  node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/
    vendor/x86_64-pc-windows-msvc/codex-package.json
    vendor/x86_64-pc-windows-msvc/bin/codex.exe
```

Use package JSON values matching the installed official shape:

```json
{"name":"@openai/codex","version":"0.146.0","optionalDependencies":{"@openai/codex-win32-x64":"npm:@openai/codex@0.146.0-win32-x64"}}
```

```json
{"name":"@openai/codex","version":"0.146.0-win32-x64","os":["win32"],"cpu":["x64"]}
```

```json
{"layoutVersion":1,"version":"0.146.0","target":"x86_64-pc-windows-msvc","variant":"codex","entrypoint":"bin/codex.exe","resourcesDir":"codex-resources","pathDir":"codex-path"}
```

- [ ] **Step 2: Write failing native package verification tests**

Add tests with these exact assertions:

```rust
#[test]
fn rejected_wrapper_can_resolve_verified_official_native_package() {
    let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\necho wrapper\r\n");
    let resolved = resolve_official_native_from_prefix(&fixture.npm_prefix()).unwrap();
    assert_eq!(resolved.program(), fixture.native_exe().canonicalize().unwrap());
    assert!(resolved.args_prefix().is_empty());
    assert_eq!(resolved.installation(), CodexInstallation::VerifiedNpmLayout);
}

#[test]
fn native_package_rejects_root_and_layout_version_mismatch() {
    let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
    fixture.write_root_version("0.145.0");
    assert!(resolve_official_native_from_prefix(&fixture.npm_prefix()).is_err());
    fixture.write_root_version("0.146.0");
    fixture.write_layout_version("0.145.0");
    assert!(resolve_official_native_from_prefix(&fixture.npm_prefix()).is_err());
}

#[test]
fn native_package_rejects_target_variant_and_entrypoint_mismatch() {
    for mutation in [
        NativeFixtureMutation::Target("aarch64-pc-windows-msvc"),
        NativeFixtureMutation::Variant("other"),
        NativeFixtureMutation::Entrypoint("../bin/codex.exe"),
    ] {
        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
        fixture.mutate_layout(mutation);
        assert!(resolve_official_native_from_prefix(&fixture.npm_prefix()).is_err());
    }
}

#[cfg(windows)]
#[test]
fn native_package_rejects_reparse_escape() {
    use std::os::windows::fs::symlink_file;
    let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
    let outside = fixture.root().join("outside-codex.exe");
    fs::write(&outside, b"MZ").unwrap();
    fs::remove_file(fixture.native_exe()).unwrap();
    symlink_file(&outside, fixture.native_exe()).unwrap();
    assert!(resolve_official_native_from_prefix(&fixture.npm_prefix()).is_err());
}
```

Also add a discovery-level test where PATH contains only a non-official `codex.cmd` beside the valid fixture and assert `CodexCommandResolver::resolve` returns the package-native exe rather than `CODEX_NOT_FOUND`.

- [ ] **Step 3: Run RED tests**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml rejected_wrapper_can_resolve_verified_official_native_package -- --exact
cargo test --manifest-path rust/Cargo.toml native_package_rejects -- --nocapture
```

Expected: compile failure because `resolve_official_native_from_prefix` and fixture helpers do not exist.

- [ ] **Step 4: Implement fail-closed platform package verification**

Implement small serde structs local to `npm.rs`:

```rust
#[derive(Deserialize)]
struct RootPackage { name: String, version: String, #[serde(default)] optional_dependencies: BTreeMap<String, String> }

#[derive(Deserialize)]
struct PlatformPackage { name: String, version: String, os: Vec<String>, cpu: Vec<String> }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeLayout { layout_version: u32, version: String, target: String, variant: String, entrypoint: String }
```

The resolver must:

1. Canonicalize the supplied npm prefix and every accepted file.
2. Reject non-files, reparse points, JSON files over 64 KiB, and any canonical path outside the prefix.
3. Require root name `@openai/codex`.
4. Select `@openai/codex-win32-x64` only when `cfg!(all(windows, target_arch = "x86_64"))`.
5. Require platform name `@openai/codex`, `os == ["win32"]`, and `cpu == ["x64"]`.
6. Require platform version `<root-version>-win32-x64`; require the root optional dependency to reference the same version when present.
7. Require layout version 1, target `x86_64-pc-windows-msvc`, variant `codex`, layout version equal to the root version, and entrypoint exactly `bin/codex.exe` after slash normalization.
8. Resolve the entrypoint relative to the directory containing `codex-package.json`, verify it as an ordinary non-reparse `.exe`, and return `ResolvedCodexCommand::from_parts(exe, vec![], VerifiedNpmLayout)`.
9. Return `CODEX_WRAPPER_UNSUPPORTED` for an invalid package; never execute wrapper text.

- [ ] **Step 5: Add the fallback to PATH discovery**

In `resolve_in_path`, when a `.cmd` candidate fails `resolve_npm_shim`, call `resolve_official_native_from_prefix(segment)`. Only continue to later PATH entries when both verifiers reject the prefix.

- [ ] **Step 6: Run GREEN and regression tests**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::npm
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::discovery
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server
```

Expected: all App Server resolver tests pass; arbitrary wrapper tests still prove no wrapper execution.

- [ ] **Step 7: Commit Task 1**

```powershell
git add rust/src/providers/codex/app_server/npm.rs rust/src/providers/codex/app_server/discovery.rs
git commit -m "Resolve verified native Codex packages"
```

---

### Task 2: Persist Independent Surface Opacity Settings

**Files:**
- Modify: `rust/src/storage/settings_repository.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/hooks/useSettings.ts`
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`
- Modify: every frontend test fixture constructing `AppSettingsDto`
- Test: `rust/src/storage/settings_repository.rs`
- Test: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Test: `apps/desktop-tauri/src/types/bridge.test.ts`
- Test: `apps/desktop-tauri/src/hooks/useSettings.test.tsx`

**Interfaces:**
- Produces Rust fields: `taskbar_status_opacity: u8`, `float_ball_opacity: u8` on `AppSettings`; optional `u8` fields on `SettingsPatch`.
- Produces wire fields: `taskbarStatusOpacity: number`, `floatBallOpacity: number`.
- Consumes later: status surfaces read the emitted authoritative settings snapshot; GeneralTab updates one opacity at a time through existing `updateSettings`.

- [ ] **Step 1: Write failing repository migration and validation tests**

Add:

```rust
#[test]
fn old_settings_json_defaults_both_opacities_to_twenty() {
    let (repository, database) = settings_fixture();
    database.with_connection(|connection| {
        connection.execute(
            "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
            params![SETTINGS_KEY, r#"{"startAtLogin":true,"refreshIntervalSeconds":300,"displayMode":"remaining","theme":"system","language":"zh-CN"}"#],
        ).map_err(storage_error)?;
        Ok(())
    }).unwrap();
    let settings = repository.load().unwrap();
    assert_eq!(settings.taskbar_status_opacity, 20);
    assert_eq!(settings.float_ball_opacity, 20);
}

#[test]
fn opacity_bounds_are_inclusive_and_invalid_patch_is_atomic() {
    let (repository, _) = settings_fixture();
    let saved = repository.update(SettingsPatch {
        taskbar_status_opacity: Some(0),
        float_ball_opacity: Some(80),
        ..SettingsPatch::default()
    }).unwrap();
    assert_eq!((saved.taskbar_status_opacity, saved.float_ball_opacity), (0, 80));
    let error = repository.update(SettingsPatch {
        taskbar_status_opacity: Some(81),
        float_ball_opacity: Some(10),
        ..SettingsPatch::default()
    }).unwrap_err();
    assert_eq!(error.code(), "SETTINGS_SURFACE_OPACITY_INVALID");
    let reloaded = repository.load().unwrap();
    assert_eq!((reloaded.taskbar_status_opacity, reloaded.float_ball_opacity), (0, 80));
}

#[test]
fn partial_opacity_patch_preserves_enable_flags_and_peer_opacity() {
    let (repository, _) = settings_fixture();
    repository.update(SettingsPatch {
        taskbar_status_enabled: Some(true),
        float_ball_enabled: Some(true),
        float_ball_opacity: Some(60),
        ..SettingsPatch::default()
    }).unwrap();
    let saved = repository.update(SettingsPatch {
        taskbar_status_opacity: Some(35),
        ..SettingsPatch::default()
    }).unwrap();
    assert!(saved.taskbar_status_enabled && saved.float_ball_enabled);
    assert_eq!(saved.taskbar_status_opacity, 35);
    assert_eq!(saved.float_ball_opacity, 60);
}
```

Add a test-only `settings_fixture() -> (SettingsRepository, Arc<AppDatabase>)` beside the existing repository tests so the setup above is literal and reusable.

- [ ] **Step 2: Write failing bridge contract tests**

Extend `patch_maps_all_frozen_fields`, `dto_round_trips_default_settings`, `types/bridge.test.ts`, and `useSettings.test.tsx` to require both opacity fields and prove a settings event adopts them.

- [ ] **Step 3: Run RED tests**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml opacity -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::settings -- --nocapture
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/types/bridge.test.ts src/hooks/useSettings.test.tsx
```

Expected: compile/type failures for missing fields.

- [ ] **Step 4: Implement settings fields and validation**

Add `const DEFAULT_SURFACE_OPACITY: u8 = 20;` and `const MAX_SURFACE_OPACITY: u8 = 80;`. Deserialize old JSON via `#[serde(default = "default_surface_opacity")]`. In `AppSettings::apply`, validate each requested value before mutating any field so a mixed invalid patch is atomic; return `SETTINGS_SURFACE_OPACITY_INVALID` for values above 80.

- [ ] **Step 5: Map the fields end-to-end**

Add both fields to `AppSettingsDto`, `SettingsPatchDto`, `from_settings`, `into_patch`, TypeScript `AppSettingsDto`, default settings, bootstrap fallbacks, and all exact fixtures. Do not route opacity through `StatusSurfaceController`; ordinary settings persistence plus `SETTINGS_CHANGED` is sufficient because no window lifecycle change occurs.

- [ ] **Step 6: Run GREEN tests and frontend typecheck**

Run the three RED commands again, then:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
```

Expected: focused tests and build pass.

- [ ] **Step 7: Commit Task 2**

```powershell
git add rust/src/storage/settings_repository.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/hooks/useSettings.ts apps/desktop-tauri/src/test/profileUsageFixtures.ts apps/desktop-tauri/src
git commit -m "Persist status surface opacity"
```

Before committing, inspect `git diff --cached --name-only` and unstage any file outside Task 2.

---

### Task 3: Derive Dynamic Quota Metrics, Trust State, and Color Bands

**Files:**
- Modify: `apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts`
- Modify: `apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx`
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`

**Interfaces:**
- Produces `metrics: StatusQuotaMetric[]` in stable `primary`, `secondary`, `additionalWindows` order after duplicate removal.
- Produces `compactIdentity: string` and `trustState: "trusted" | "refreshing" | "cached" | "missing"` on `StatusSurfaceViewModel`.
- Produces `band: "high" | "medium" | "low" | "unknown"` on every `StatusQuotaMetric`, derived once by `quotaBandFor(remainingPercent, trustState)`.
- Preserves `primaryMetric` and `secondaryMetric` only if legacy tray code still consumes them; new status surfaces must consume `metrics`.
- Consumes later: TaskbarStatus and FloatBall render no fabricated rows and use the same band/trust derivation.

- [ ] **Step 1: Write failing weekly-only and arbitrary-window tests**

Add fixtures/tests:

```ts
it("returns only the weekly metric when that is the only backend window", () => {
  const state = weeklyOnlyUsage({ remainingPercent: 98, resetsAt: "2026-08-20T00:00:00Z" });
  const model = modelFor("remaining", state);
  expect(model.metrics.map(metric => metric.kind)).toEqual(["weekly"]);
  expect(model.metrics.some(metric => metric.label === "5H")).toBe(false);
});

it("keeps unknown real windows and removes duplicate limit ids", () => {
  const state = weeklyOnlyUsage({ remainingPercent: 98 });
  state.additionalWindows = [
    { ...state.primary!, remainingPercent: 12, usedPercent: 88 },
    { ...state.primary!, limitId: "spark", label: "Spark", windowDurationMinutes: 1440 },
  ];
  const model = modelFor("remaining", state);
  expect(model.metrics.map(metric => metric.limitId)).toEqual(["weekly", "spark"]);
});

it("selects the lowest remaining real window as urgent with stable tie order", () => {
  const state = readyTwoWindowFixture().usageByProfile.personal!;
  state.primary!.remainingPercent = 34;
  state.secondary!.remainingPercent = 34;
  expect(modelFor("remaining", state).urgentMetric?.limitId).toBe(state.primary!.limitId);
  state.secondary!.remainingPercent = 33;
  expect(modelFor("remaining", state).urgentMetric?.limitId).toBe(state.secondary!.limitId);
});
```

Widen `StatusQuotaMetric.kind` to a string-safe model such as `"fiveHour" | "weekly" | "other"` and include `limitId` plus `shortLabel`.

- [ ] **Step 2: Write failing identity, band, and trust tests**

Use exact boundary table:

```ts
it.each([
  [100, "high"], [67, "high"], [66, "medium"],
  [34, "medium"], [33, "low"], [0, "low"],
] as const)("maps %s remaining to %s", (remaining, expected) => {
  expect(quotaBandFor(remaining, "trusted")).toBe(expected);
});
```

Assert stale, refreshing, currentError, and missing all return `unknown`. Assert `compactIdentity("naipi122899@gmail.com") === "naipi1"`; use `Array.from(cleanName).slice(0, 6).join("")` so surrogate pairs are not split.

- [ ] **Step 3: Run RED tests**

Run:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/lib/statusSurfaceViewModel.test.ts src/hooks/useStatusSurface.test.tsx
```

Expected: failures because the model still hard-codes 300/10080 metrics and used-percent 75/90 thresholds.

- [ ] **Step 4: Implement dynamic model derivation**

Create focused helpers and the exact shared types:

```ts
export type TrustState = "trusted" | "refreshing" | "cached" | "missing";
export type QuotaBand = "high" | "medium" | "low" | "unknown";
export function quotaBandFor(remainingPercent: number, trust: TrustState): QuotaBand
export function compactIdentity(value: string): string
function metricWindows(state: ProfileUsageStateDto): UsageWindowDto[]
function shortWindowLabel(window: UsageWindowDto): string
```

Deduplicate by non-empty `limitId`; for empty ids use a composite of duration, label, and reset. Preserve the first occurrence. Known duration labels are `5H` for 300 and `周` for 10080; unknown rows use the backend label truncated for the status surface or `额度`. `urgentMetric` compares `remainingPercent` independent of display mode.

Trust precedence is `missing/no metrics` → missing, refreshing → refreshing, current error or stale → cached, otherwise trusted. Only trusted metrics get high/medium/low.

- [ ] **Step 5: Make the hook react to settings changes**

Subscribe to `events.settingsChanged` in `useStatusSurface` and merge the authoritative `AppSettingsDto` into bootstrap state so display mode and both opacities update live. Clean up the listener on unmount. Add a hook test that emits a settings snapshot and observes the new opacity/display mode without reloading bootstrap.

- [ ] **Step 6: Run GREEN and regression tests**

Run the RED command again plus:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test
```

Expected: all frontend tests pass with dynamic metrics.

- [ ] **Step 7: Commit Task 3**

```powershell
git add apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts apps/desktop-tauri/src/hooks/useStatusSurface.ts apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx apps/desktop-tauri/src/test/profileUsageFixtures.ts
git commit -m "Derive dynamic quota status"
```

---

### Task 4: Add Safe Dynamic Taskbar Window Sizing

**Files:**
- Create: `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`
- Create: `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Modify: `apps/desktop-tauri/src/types/bridge.test.ts`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs`
- Test: same files' Rust test modules

**Interfaces:**
- Produces frontend `setTaskbarStatusWidth(width: number): Promise<void>` invoking `set_taskbar_status_width` with `{ width }`.
- Produces Tauri command `pub fn set_taskbar_status_width(app: tauri::AppHandle, width: f64) -> Result<(), String>`.
- Produces constants `TASKBAR_MIN_LOGICAL_WIDTH: u32 = 104`, `TASKBAR_MAX_LOGICAL_WIDTH: u32 = 318`, `TASKBAR_DEFAULT_LOGICAL_WIDTH: u32 = 168`, `TASKBAR_LOGICAL_HEIGHT: u32 = 40`.
- Produces manager method `TaskbarOverlay::set_content_width(&mut self, app: &tauri::AppHandle, width: u32) -> Result<(), String>`.
- Produces shared entry point `status_surfaces::set_taskbar_status_width(app: &tauri::AppHandle, width: f64) -> Result<(), String>` that acquires `Mutex<StatusSurfaceState>` and delegates to the manager.

- [ ] **Step 1: Write failing Rust clamp and reposition tests**

Extract a pure helper:

```rust
pub fn clamp_logical_width(width: f64) -> u32
```

Test NaN/negative/103 → 104; 168.4 → the documented rounded value; 318 and 900 → 318. Update positioning tests to call `compute_slot` with 104, 168, and 318 and assert every rectangle remains inside horizontal and vertical taskbars.

- [ ] **Step 2: Write failing bridge and ResizeObserver tests**

In `types/bridge.test.ts`, expect:

```ts
await setTaskbarStatusWidth(168);
expect(invokeMock).toHaveBeenCalledWith("set_taskbar_status_width", { width: 168 });
```

In the hook test, provide a deterministic ResizeObserver stub; report 166.2 then 166.2 then 184.8 and assert only rounded changed widths are invoked. On unmount, assert observer disconnect.

- [ ] **Step 3: Run RED tests**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay -- --nocapture
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/types/bridge.test.ts src/hooks/useTaskbarStatusWidth.test.tsx
```

Expected: missing helper/command/hook failures.

- [ ] **Step 4: Implement Rust dynamic width state**

Replace the fixed width field with `logical_width: u32` in `TaskbarOverlay`, default 168. Use it in builder creation and every `compute_slot` call. `set_content_width` clamps, returns early if unchanged, calls `window.set_size(LogicalSize::new(width, 40))` when a window exists, stores the new value only after resize succeeds, then repositions. Keep the previous width on failure and return stable code `TASKBAR_STATUS_RESIZE_FAILED`.

- [ ] **Step 5: Register the typed command and bridge**

The Tauri command calls only `crate::status_surfaces::set_taskbar_status_width(&app, width)`. Add it to `commands/mod.rs`, `generate_handler!`, and the boundary guard if it enumerates allowed commands. Keep capability scope unchanged because the frontend invokes a Rust command rather than raw window resize permissions.

- [ ] **Step 6: Implement the measurement hook**

The hook accepts `ref: RefObject<HTMLElement>`, observes only its content wrapper, reads `borderBoxSize` when available or `getBoundingClientRect().width`, rounds to one integer, ignores non-finite/zero values, and invokes only on a changed rounded width. Errors are non-fatal and retried on the next observation.

- [ ] **Step 7: Run GREEN tests and clippy**

Run the RED commands again, then:

```powershell
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

- [ ] **Step 8: Commit Task 4**

```powershell
git add apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/taskbar_overlay
git commit -m "Size taskbar status to content"
```

---

### Task 5: Render the Selected Compact Taskbar B Layout

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/App.test.tsx`

**Interfaces:**
- Consumes: Task 3 `metrics`, `compactIdentity`, `trustState`, per-metric `band`; Task 4 width hook.
- Produces: one content-width grid in exact order `avatar | six-char identity | dynamic quota cells | nearest reset date | close` with no flexible spacer.
- Produces CSS custom property `--surface-bg-alpha` derived from `taskbarStatusOpacity / 100` and data attributes for trust/band.

- [ ] **Step 1: Replace old tests with failing selected-layout contracts**

Write tests that assert:

1. Weekly-only bootstrap renders `naipi1`, `周 98%`, and `8/20`, and does not contain `5H`.
2. Full identity remains in `title` and main button aria-label.
3. Root content uses a test id and contains no element carrying `data-flex-spacer`; close is the last grid item and sibling of the main button.
4. `taskbarStatusOpacity: 0`, 20, 80 maps to `--surface-bg-alpha: 0`, `.2`, `.8` without applying `opacity` to the root.
5. Boundary colors are represented through `data-band=high|medium|low`; cached/error gives `unknown` and includes “缓存” plus updated text in the accessible label.
6. Close failure preserves quota content and retry button.

- [ ] **Step 2: Run RED component tests**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/surfaces/TaskbarStatus.test.tsx src/App.test.tsx
```

Expected: old fixed two-window DOM and 318px layout assertions fail.

- [ ] **Step 3: Implement B layout markup**

Use one wrapper ref spanning all visible content, apply the Task 4 hook, and render each `surface.metrics` item as a max-content cell. Format weekly reset as locale date `M/D` in compact UI; keep countdown in title. Do not duplicate an urgent percentage on the right. The close button immediately follows the last reset cell.

- [ ] **Step 4: Implement compact transparent CSS**

Set root to `display:inline-grid`, `grid-auto-flow:column`, `grid-auto-columns:max-content`, `width:max-content`, `height:40px`, `gap:7px`, and padding `0 7px 0 8px`. Remove all `minmax(0,1fr)`, `1fr`, container rules that hide identity, and the old 318px visual stretch. Background alpha is `var(--surface-bg-alpha)`; border/shadow strength use bounded `calc()` values. At alpha 0 keep text shadow/outline, band colors, and focus ring.

Use tokens:

```css
--quota-high: #56d98a;
--quota-medium: #f0b84f;
--quota-low: #f36b72;
--quota-unknown: #9299aa;
```

- [ ] **Step 5: Run GREEN, full frontend tests, and build**

Run the RED command, then:

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run build
```

- [ ] **Step 6: Commit Task 5**

```powershell
git add apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.css apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx apps/desktop-tauri/src/App.test.tsx
git commit -m "Compact the taskbar quota capsule"
```

---

### Task 6: Render Dynamic Transparent Float-Ball Quotas

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.css`
- Modify: `apps/desktop-tauri/src/surfaces/FloatBall.test.tsx`

**Interfaces:**
- Consumes: Task 3 dynamic metrics, urgent metric, trust state, per-metric `band`; Task 2 `floatBallOpacity`.
- Preserves: 88×88 collapsed, 260×148 expanded, 180ms hover, drag threshold, click-to-panel, Escape/collapse path, close behavior, Theme::Dark.
- Produces: zero fixed 5-hour/weekly placeholder rows.

- [ ] **Step 1: Write failing weekly-only, dynamic, opacity, and band tests**

Assert collapsed weekly-only content includes `98` and `周 剩余`, excludes `5H`; expanded weekly-only contains exactly one quota section. Add a two-real-window fixture and assert two sections. Test alpha 0/20/80 and high/medium/low/unknown data attributes. Preserve all existing drag/click/close tests.

- [ ] **Step 2: Run RED tests**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/surfaces/FloatBall.test.tsx
```

Expected: fixed `QuotaTrack` labels and hard-coded two-column rows fail.

- [ ] **Step 3: Implement dynamic rendering**

Map `surface.metrics` into quota rows using metric `shortLabel`, `displayedPercent`, `resetText`, and band. Collapsed footer uses the urgent metric's real label. Missing/cached states use neutral text and gray ring. Expanded account identity remains complete; do not truncate it there.

- [ ] **Step 4: Implement independent background-only opacity and band CSS**

Use `--surface-bg-alpha` for both collapsed and expanded backgrounds. Do not set element-level opacity. Apply band color to ring and each row track, with unknown gray. Let dynamic rows wrap/stack inside the fixed 260×148 card; if more rows exceed the safe count, make the quota area vertically scrollable without changing native dimensions.

- [ ] **Step 5: Run GREEN and frontend regression**

Run the focused test then all frontend tests and build as in Task 5.

- [ ] **Step 6: Commit Task 6**

```powershell
git add apps/desktop-tauri/src/surfaces/FloatBall.tsx apps/desktop-tauri/src/surfaces/FloatBall.css apps/desktop-tauri/src/surfaces/FloatBall.test.tsx
git commit -m "Render real quotas in the float ball"
```

---

### Task 7: Build the Compact Localized macOS-Style Settings Window

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/shell/settings_window.rs`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsTabs.ts`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsTabs.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.test.tsx`
- Modify: `apps/desktop-tauri/src/styles.css`
- Modify: localized settings tab components only where visible English remains

**Interfaces:**
- Consumes: Task 2 settings fields and `update({ taskbarStatusOpacity })` / `update({ floatBallOpacity })`.
- Produces: sidebar buttons for the unchanged frozen tab ids and two independent `input type="range"` controls with min 0, max 80, step 1.
- Preserves: `settings-change-tab`, all eight frozen ids, Current CLI account restrictions, Advanced path validation, About manual update check, keyboard activation.

- [ ] **Step 1: Write failing shell and localization tests**

Add Rust assertions that Settings defaults to 680×500. In React tests set language `zh-CN` and assert navigation labels `通用`, `账户`, `通知`, `菜单栏`, `菜单`, `用量与费用`, `高级`, `关于`; assert sidebar navigation changes pane. For `en-US`, retain English labels.

- [ ] **Step 2: Write failing independent opacity control tests**

Render GeneralTab with taskbar 20 and float-ball 60. Assert two sliders by localized accessible names and displayed `20%` / `60%`. Change taskbar to 35 and assert only `update({ taskbarStatusOpacity: 35 })`; change float-ball to 5 and assert only its patch. Assert switches still use typed `setSurfaceEnabled` and sliders stay enabled while their surface switch is off.

- [ ] **Step 3: Run RED settings tests**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml settings_window -- --nocapture
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/surfaces/Settings.test.tsx src/surfaces/settings/settingsTabs.test.ts src/surfaces/settings/tabs/GeneralTab.test.tsx
```

Expected: old 720×580 and horizontal English-only controls fail.

- [ ] **Step 4: Implement one localized settings-copy boundary**

Add a small local `settingsCopy(language)` object in the settings folder rather than scattering language conditionals. It must cover shell title, close, all tabs, General fields, the two status cards, slider labels, placeholder text, and existing visible button labels touched by the layout. Use `zh-CN` for Chinese; `system` resolves via existing locale behavior or falls back consistently with current product rules.

- [ ] **Step 5: Implement sidebar and compact visual system**

Use CSS grid `148px minmax(0,1fr)` for the settings shell, 12–14px type, 30px controls, 10–12px field gaps, and 12–14px card padding. The left sidebar is a `<nav>` with button semantics and visible focus. The right pane scrolls independently. Placeholder tabs render a short localized explanation, not an empty panel. Header and close control remain visible.

- [ ] **Step 6: Implement General status cards and sliders**

Each card contains title, description, switch, range control, and numeric output. Range handlers parse an integer and call the existing update method. Add `aria-valuetext="20%"` and tick labels 0, 20, 40, 60, 80.

- [ ] **Step 7: Run GREEN, full frontend tests/build, and Tauri test**

Run the RED commands, all frontend tests/build, and full Tauri tests.

- [ ] **Step 8: Commit Task 7**

```powershell
git add apps/desktop-tauri/src-tauri/src/shell/settings_window.rs apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/Settings.test.tsx apps/desktop-tauri/src/surfaces/settings apps/desktop-tauri/src/styles.css
git commit -m "Redesign compact status settings"
```

---

### Task 8: Update Deterministic Proof Scenarios and Documentation Contracts

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs` tests
- Modify: `docs/WINDOWS_PROOF.md`
- Modify: `scripts/assert-v1-boundaries.ps1`

**Interfaces:**
- Produces proof aliases `taskbar-status:weekly` and `float-ball:weekly` with one 10080-minute window, 98% remaining, fixed future reset, trusted freshness, opacity 20.
- Preserves existing ready/warning/critical/refreshing/stale/missing aliases for regression screenshots.
- Documents dynamic taskbar width 104–318 and target default content width instead of a fixed 318 target.

- [ ] **Step 1: Write failing proof parser/payload tests**

Assert both new aliases parse and their bootstrap contains exactly one weekly window, no 300-minute window, opacity fields 20, and a six-character proof identity visible to compact UI.

- [ ] **Step 2: Run RED proof tests**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
```

Expected: aliases and opacity fields missing.

- [ ] **Step 3: Implement weekly proof state without changing legacy states**

Add a dedicated `StatusProofState::Weekly` or explicit proof scenario mapping. Do not redefine `ready`; older evidence paths must remain deterministic.

- [ ] **Step 4: Update Windows proof and boundary guard**

Document the new aliases, 40px height, 104–318 dynamic width, independent opacity states, weekly-only absence of 5H, and real-data comparison outside proof mode. Add `set_taskbar_status_width` to the guard's frozen allowed command list.

- [ ] **Step 5: Run GREEN and guard**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness -- --nocapture
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
```

- [ ] **Step 6: Commit Task 8**

```powershell
git add apps/desktop-tauri/src-tauri/src/proof_harness.rs docs/WINDOWS_PROOF.md scripts/assert-v1-boundaries.ps1
git commit -m "Add weekly status proof coverage"
```

---

### Task 9: Full Verification, Real Quota Comparison, Windows Proof, and Installer

**Files:**
- Create: `docs/verification/windows/2026-08-14/cua-observations.md`
- Create: `docs/verification/windows/2026-08-14/screenshots/taskbar-weekly-20.png`
- Create: `docs/verification/windows/2026-08-14/screenshots/taskbar-weekly-0.png`
- Create: `docs/verification/windows/2026-08-14/screenshots/float-ball-weekly-20.png`
- Create: `docs/verification/windows/2026-08-14/screenshots/float-ball-expanded-weekly.png`
- Create: `docs/verification/windows/2026-08-14/screenshots/settings-general-opacity.png`
- Modify only if evidence reveals a defect: files owned by Tasks 1–8, with a new RED regression test before each fix.

**Interfaces:**
- Consumes: all prior task commits.
- Produces: fresh test/build evidence, non-secret Windows UI evidence, a verified NSIS setup, and an installed current-user application.

- [ ] **Step 1: Inspect scope before verification**

Run:

```powershell
git status --short
git log --oneline -12
git diff --check HEAD~8..HEAD
```

Confirm only planned commits plus the five pre-existing unrelated untracked files are present. Do not delete or stage those files.

- [ ] **Step 2: Run focused source-of-truth and settings tests**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri test -- src/lib/statusSurfaceViewModel.test.ts src/surfaces/TaskbarStatus.test.tsx src/surfaces/FloatBall.test.tsx src/surfaces/Settings.test.tsx
```

Expected: all focused suites pass.

- [ ] **Step 3: Run the complete local CI slice**

```powershell
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
```

If corepack refuses the package-manager version on this machine, invoke the pinned cached pnpm CJS for frontend test/build and run both Cargo test/clippy manifests plus `cargo fmt --all -- --check` separately; record the exact substitute commands.

- [ ] **Step 4: Build a fresh debug NSIS-capable desktop binary**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build:debug
```

Stop the old single-instance process before launch:

```powershell
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
```

This process stop is authorized only for the app under test and is required to avoid handoff to the stale installed binary.

- [ ] **Step 5: Prove official native Codex resolution with OpenCodex retained**

Launch the fresh debug binary normally, trigger manual refresh, then read redacted diagnostics/local database state. Assert:

- installation category is verified official npm/native;
- detected version matches the installed official package;
- last refresh is a new success, not `CODEX_NOT_FOUND`;
- terminal `codex --version` still works through the user's OpenCodex wrapper;
- the wrapper file content and timestamps were not modified by codex-barbar.

Never record token, cookie, raw protocol, or full private paths in evidence.

- [ ] **Step 6: Compare the real weekly quota**

Open the user's Codex usage UI manually or through the already authenticated local app, and compare semantic fields to codex-barbar after refresh: period, remaining percent, and reset date. Record values only with user-approved screenshot redaction. The acceptance example is “weekly 98%, resets 8/20”; current live values may have advanced, so equality at the same observation time is the required assertion, not hard-coding 98 forever.

- [ ] **Step 7: Run deterministic Windows UI proof**

Prefer CUA at `%LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe`. If unavailable after checking that exact path and process, use the documented Win32/UIA fallback and state the limitation.

Capture and assert:

1. `taskbar-status:weekly` at opacity 20: one weekly cell, six-character identity, no 5H, close adjacent, 40px logical height, dynamic width inside 104–318, taskbar-safe rect.
2. Taskbar opacity 0 and 80: background changes, text/band/focus/close remain visible.
3. `float-ball:weekly` collapsed/expanded: one real weekly row, correct green/yellow/red/gray proof states, 88×88 and 260×148.
4. Settings General: 680×500, Chinese sidebar, two independent sliders, keyboard Tab/Space/arrow behavior.
5. Close persistence outside proof mode, Explorer restart/DPI reposition, float-ball drag/click behavior, and Theme::Dark isolation.

- [ ] **Step 8: Build production NSIS installer**

```powershell
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\stack\AppData\Local\node\corepack\v1\pnpm\10.18.1\bin\pnpm.cjs' --dir apps/desktop-tauri run tauri:build
```

Expected artifact:

```text
target\release\bundle\nsis\codex-barbar_1.0.0_x64-setup.exe
```

Run `scripts/verify-release-artifacts.ps1` or the release staging/doctor pipeline required by the current repository. Record installer SHA-256.

- [ ] **Step 9: Perform current-user coverage installation**

Run the fresh NSIS installer interactively or with the repository smoke script. Confirm:

- installed path remains `%LOCALAPPDATA%\Programs\codex-barbar`;
- install needs no elevation;
- existing settings migrate to opacity 20 without losing enable flags/account cache;
- app launches the new binary and refreshes current quota;
- uninstaller remains registered.

Do not purge user data. The uninstaller's explicit data-delete option stays unselected.

- [ ] **Step 10: Write and verify the evidence record**

`cua-observations.md` must list commit SHA, build commands, CUA/fallback status, DPI/taskbar orientation, window rectangles, opacity values, live quota equality result, installer path/hash, and explicit PASS/FAIL per acceptance criterion. Screenshots must not contain full email addresses, tokens, or private account content; use proof identities for public artifacts.

- [ ] **Step 11: Run final verification after installation**

Repeat the focused status tests and read the installed file version/process path. Verify the installed executable timestamp/hash corresponds to the new release build and no debug process remains.

- [ ] **Step 12: Commit verification evidence**

```powershell
git add docs/verification/windows/2026-08-14
git commit -m "Verify accurate compact status surfaces"
```

Do not include installer binaries in git unless the repository release policy explicitly tracks them.

## Final Review Gate

Before declaring the plan complete, the implementer must perform:

1. A spec-compliance review against every acceptance criterion in `docs/superpowers/specs/2026-08-14-status-surface-accuracy-density-opacity-design.md`.
2. A code-quality review covering resolver security, settings atomicity, dynamic quota correctness, accessible color semantics, auxiliary window lifecycle, and unrelated-file scope.
3. Fresh `git status --short`, `git diff --check`, full local CI evidence, Windows proof evidence, NSIS hash, installed process path/version, and real quota equality evidence.
