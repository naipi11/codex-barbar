# Taskbar and Tray Preferences Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inherited Menu bar placeholder with a Windows-native Taskbar & Tray settings page that controls taskbar fields, account-name visibility, density, opacity, tray icon mode, tooltip rows, and full-screen hiding without desynchronizing native and React status surfaces.

**Architecture:** Extend the V1 settings bridge with a `TaskbarTrayPreferences` object while preserving the existing top-level taskbar enable/opacity compatibility fields. Feed the same persisted preferences into the React taskbar presentation and Rust native tray presentation; use the existing full-screen guard only when the new preference is enabled.

**Tech Stack:** Rust 2024, Tauri 2 tray APIs, React 18, TypeScript, Vitest, existing taskbar overlay/full-screen guard, CUA Driver on Windows.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- User-facing tab title is Taskbar & Tray / 任务栏与托盘; the bridge ID remains `menuBar`.
- The account label is a visibility switch only; it is always the first six Unicode grapheme clusters of the display name or email local part.
- Preserve the existing user-visible presentation when a pre-feature settings file is loaded.
- The universal 10,080-minute weekly window is the only quota source for the taskbar, tray icon, tray tooltip, and floating-ball band; 5-hour/model-specific windows never influence it.
- `hideStatusSurfacesInFullscreen` defaults to true and applies to taskbar and floating overlays, not the native tray icon.
- Do not add a close button to the taskbar status surface or expose arbitrary native-window controls.
- Do not add dependencies or change Tauri capabilities/permissions.

## File Structure

| File | Responsibility |
| --- | --- |
| `rust/src/storage/settings_repository.rs` | Store and validate Taskbar & Tray preferences. |
| `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` | Serialize/deserialize the preference DTO. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.tsx` | Accessible presentation controls and immediate settings patches. |
| `apps/desktop-tauri/src/surfaces/Settings.tsx` | Route `menuBar` to the concrete tab. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx` | Remove duplicate taskbar controls; retain floating-ball controls. |
| `apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts` | Derive the six-grapheme account identity and universal weekly data. |
| `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts` | Filter taskbar display fields and density from persisted preferences. |
| `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx` | Render only enabled taskbar elements while preserving measurement parity. |
| `apps/desktop-tauri/src/surfaces/TaskbarStatus.css` | Apply compact/standard density without hiding content through CSS clipping. |
| `apps/desktop-tauri/src-tauri/src/tray_bridge.rs` | Apply icon mode and tooltip field choices to native tray state. |
| `apps/desktop-tauri/src-tauri/src/status_surfaces.rs` | Respect the full-screen hide preference during startup and monitor transitions. |

---

### Task 1: Add persisted Taskbar & Tray preferences

**Files:**
- Modify: `rust/src/storage/settings_repository.rs:55-214`
- Modify: `rust/src/storage/mod.rs:14-20`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs:19-154`
- Modify: `apps/desktop-tauri/src/types/bridge.ts:37-64`
- Modify: `apps/desktop-tauri/src/hooks/useSettings.ts:15-27`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts:39-73`
- Test: `rust/src/storage/settings_repository.rs`
- Test: `apps/desktop-tauri/src/types/bridge.test.ts`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskbarDensity { Compact, Standard }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayIconMode { Dynamic, Monochrome }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskbarTrayPreferences {
    pub show_taskbar_icon: bool,
    pub show_taskbar_account: bool,
    pub show_weekly_label: bool,
    pub show_weekly_percent: bool,
    pub show_reset_date: bool,
    pub density: TaskbarDensity,
    pub tray_icon_mode: TrayIconMode,
    pub tooltip_account: bool,
    pub tooltip_weekly: bool,
    pub tooltip_reset_date: bool,
    pub tooltip_updated_at: bool,
    pub hide_status_surfaces_in_fullscreen: bool,
}
```

- `AppSettings` gains `taskbar_tray: TaskbarTrayPreferences` and `SettingsPatch` gains `taskbar_tray: Option<TaskbarTrayPreferencesPatch>`.

- [ ] **Step 1: Write failing migration/default tests**

Test defaults and old settings JSON:

```rust
let prefs = AppSettings::default().taskbar_tray;
assert!(prefs.show_taskbar_icon);
assert!(prefs.show_taskbar_account);
assert!(prefs.show_weekly_label);
assert!(prefs.show_weekly_percent);
assert!(prefs.show_reset_date);
assert_eq!(prefs.density, TaskbarDensity::Compact);
assert_eq!(prefs.tray_icon_mode, TrayIconMode::Dynamic);
assert!(prefs.hide_status_surfaces_in_fullscreen);
```

Also assert that an invalid JSON enum falls back through the existing safe settings-recovery path rather than partially applying a layout.

- [ ] **Step 2: Run storage and DTO tests to verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
pnpm --dir apps/desktop-tauri test -- bridge useSettings
```

Expected: compilation/test failure because `taskbarTray` is absent.

- [ ] **Step 3: Implement nested preference storage and bridge mapping**

Use the same merge-before-validate path established by the foundation plan. The patch must accept only the two density values and two icon modes. Keep `taskbarStatusEnabled` and `taskbarStatusOpacity` at their existing top-level JSON keys to preserve installed settings.

```ts
export interface TaskbarTrayPreferencesDto {
  showTaskbarIcon: boolean;
  showTaskbarAccount: boolean;
  showWeeklyLabel: boolean;
  showWeeklyPercent: boolean;
  showResetDate: boolean;
  density: "compact" | "standard";
  trayIconMode: "dynamic" | "monochrome";
  tooltipAccount: boolean;
  tooltipWeekly: boolean;
  tooltipResetDate: boolean;
  tooltipUpdatedAt: boolean;
  hideStatusSurfacesInFullscreen: boolean;
}
```

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same commands. Confirm a partial nested patch changes one field while preserving all peers.

- [ ] **Step 5: Commit the preference contract**

```powershell
git add rust/src/storage/settings_repository.rs rust/src/storage/mod.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/hooks/useSettings.ts apps/desktop-tauri/src/hooks/useStatusSurface.ts
git commit -m "Add taskbar and tray preferences"
```

### Task 2: Build the Taskbar & Tray settings tab and remove duplicate controls

**Files:**
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx:15-25,143-178`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx:37-195`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts:5-69,99-123`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsTabs.test.ts`

**Interfaces:**
- Consumes: `AppSettingsDto.taskbarTray`, `update(SettingsPatchDto)`, and existing `setSurfaceEnabled("taskbarStatus", enabled)`.
- Produces: `TaskbarTrayTab` rendered for `tab === "menuBar"`; General no longer renders the taskbar status card.

- [ ] **Step 1: Write failing UI tests**

Assert that the tab title changes in both languages and that a field toggle emits a nested patch:

```tsx
await user.click(screen.getByRole("checkbox", { name: /show account name/i }));
expect(update).toHaveBeenCalledWith({
  taskbarTray: { showTaskbarAccount: false },
});
```

Test taskbar enable, every taskbar field checkbox, density select, opacity slider, tray icon mode, tooltip fields, and full-screen switch. Assert General contains floating-ball controls but no taskbar-status heading.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- TaskbarTrayTab GeneralTab Settings settingsTabs
```

Expected: missing component and stale General assertions fail.

- [ ] **Step 3: Implement compact grouped controls**

Use semantic `fieldset`/`legend` groups: Taskbar status, Tray icon and tooltip, and Full-screen behavior. Each input has an explicit label and patches only its nested field. Retain the existing float-ball card and glow controls in General. Do not add a user control for account length.

- [ ] **Step 4: Run tests and verify GREEN**

Run the same focused command and ensure keyboard focus and checkbox labels are covered.

- [ ] **Step 5: Commit the settings tab**

```powershell
git add apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.test.tsx apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.test.tsx apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts apps/desktop-tauri/src/surfaces/settings/settingsTabs.test.ts
git commit -m "Add taskbar and tray settings tab"
```

### Task 3: Apply taskbar presentation preferences without measurement drift

**Files:**
- Modify: `apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts:62-105,240-320`
- Modify: `apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts:7-55`
- Modify: `apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx:19-96`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TaskbarStatus.css:10-121`

**Interfaces:**
- Produces a preference-aware presentation:

```ts
export interface TaskbarStatusPresentation {
  showIcon: boolean;
  showAccount: boolean;
  showWeeklyLabel: boolean;
  showWeeklyPercent: boolean;
  showResetDate: boolean;
  density: "compact" | "standard";
  compactIdentity: string | null;
  weeklyText: string | null;
  resetDateText: string | null;
  // existing trust/aria/alpha fields remain
}
```

- [ ] **Step 1: Write failing view-model and DOM parity tests**

Cover the identity rule using display name, email local part, emoji, and a `Current CLI` fallback. For example:

```ts
expect(compactIdentity("👩🏽‍💻abcdefghi")).toBe("👩🏽‍💻abcde");
expect(compactIdentity("name@example.com")).toBe("name");
```

Then render visible and measurement modes with identical preferences and assert their text content is identical. Add a test that every taskbar field disabled except one valid metric does not render phantom gaps or a clipped percent sign.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- statusSurfaceViewModel taskbarStatusPresentation TaskbarStatus TaskbarStatusMeasure
```

Expected: presentation fields and preference filtering are missing.

- [ ] **Step 3: Implement field filtering and robust six-grapheme shortening**

Use `Intl.Segmenter` when available, with `Array.from()` fallback, to take six grapheme clusters. Strip the `@domain` only when the chosen identity is an email. Derive one weekly label/percent string from `surface.universalMetric`; never iterate the generic metrics array for the taskbar.

Render the same `TaskbarStatusContents` conditionals in visible and measurement modes. Add `data-density` and adapt CSS spacing only; do not hide text with overflow or a hard width.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command. Confirm visible/measurement parity and account-hidden, icon-hidden, percent-hidden, reset-hidden, compact, and standard cases pass.

- [ ] **Step 5: Commit the React presentation slice**

```powershell
git add apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.ts apps/desktop-tauri/src/surfaces/taskbarStatusPresentation.test.ts apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.css
git commit -m "Apply taskbar presentation preferences"
```

### Task 4: Apply native tray and full-screen preferences

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs:20-286`
- Modify: `rust/src/tray/icon.rs`
- Modify: `rust/src/tray/render.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs` tests
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs:89-124,177-220,340-370`
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs` tests

**Interfaces:**
- Changes `presentation_from` to accept `&TaskbarTrayPreferences`.
- Adds a palette input without changing quota classification:

```rust
pub enum TrayIconPalette { Dynamic, Monochrome }

pub fn render_tray_icon_rgba_with_palette(
    state: TrayVisualState,
    palette: TrayIconPalette,
) -> (Vec<u8>, u32, u32);
```

`render_tray_icon_rgba(state)` remains a compatibility wrapper using `TrayIconPalette::Dynamic`.

- [ ] **Step 1: Write failing native presentation tests**

Add tests that prove:

```rust
let prefs = TaskbarTrayPreferences { tooltip_account: false, tooltip_updated_at: false, ..Default::default() };
let presentation = presentation_from(Some(&profile), &[profile], Some(&usage), "en-US", &prefs);
assert!(!presentation.tooltip.contains("Work"));
assert!(!presentation.tooltip.contains("Updated"));
assert!(presentation.tooltip.contains("Weekly 66%"));
```

Add full-screen transition tests where disabled preference leaves surfaces visible and enabled preference calls both hide paths. Test that model-specific 5-hour data cannot alter icon color or tooltip weekly percentage.

- [ ] **Step 2: Run focused shell tests and verify RED**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_bridge::tests status_surfaces::tests -- --nocapture
```

Expected: function-signature and preference assertions fail.

- [ ] **Step 3: Implement native preference application**

Load `settings.taskbar_tray` once with the selected profile usage in `load_presentation`. Build tooltip lines only for enabled fields and preserve the Windows length bound. For monochrome mode, call `render_tray_icon_rgba_with_palette(..., TrayIconPalette::Monochrome)` to render the same transparent knot/keyline silhouette using the neutral slate tint for every quota band; do not add text or a background rectangle.

In `status_surfaces.rs`, gate initial and monitor-time `hide_for_fullscreen()` calls behind `settings.taskbar_tray.hide_status_surfaces_in_fullscreen`. Reload settings at transitions so a live toggle takes effect without restart. The taskbar and float overlays must resume normally when full-screen ends.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same shell command and inspect the tooltip test's full string for privacy-safe truncation.

- [ ] **Step 5: Commit the native presentation changes**

```powershell
git add apps/desktop-tauri/src-tauri/src/tray_bridge.rs apps/desktop-tauri/src-tauri/src/status_surfaces.rs rust/src/tray/icon.rs rust/src/tray/render.rs
git commit -m "Apply tray presentation preferences"
```

### Task 5: Verify Taskbar & Tray on a fresh Windows build

**Files:**
- Verify only unless a task-owned defect is found.

**Interfaces:**
- Consumes Tasks 1–4.
- Produces automated and CUA-native proof.

- [ ] **Step 1: Run automated checks**

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

- [ ] **Step 2: Fresh-build and launch proof mode**

Close only the resolved running codex-barbar process. Build with:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Launch the fresh executable with `CODEXBAR_PROOF_MODE=settings:menuBar`.

- [ ] **Step 3: Validate actual Windows observables using CUA**

Capture and verify:

- English and Chinese tab title;
- account hidden/displayed at exactly six graphemes;
- taskbar width remeasures after every field/density change and never clips `%`;
- tray tooltip honors each enabled row and stays privacy-safe;
- dynamic versus monochrome tray icon remains transparent and legible;
- full-screen switch off keeps overlays visible, switch on hides both overlays while the tray icon remains.

- [ ] **Step 4: Finish the milestone review**

Run `git status --short`; confirm only task-owned files changed and no user account name, email, tokens, or screenshots with private data are committed. Do not push or release.
