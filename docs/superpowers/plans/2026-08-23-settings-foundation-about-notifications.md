# Settings Foundation, About, and Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the installed codex-barbar version and replace the Notifications placeholder with opt-in, deduplicated, read-only Windows notifications backed by persisted V1 settings.

**Architecture:** Keep all new preference data in the active SQLite-backed `AppSettings` record and cross the Tauri bridge through typed nested DTOs. Put notification transition/dedupe logic in the Rust library as deterministic, persisted state; the desktop shell owns only the Windows toast dispatcher, settings command, and account-service event wiring.

**Tech Stack:** Rust 2024, Tauri 2, React 18, TypeScript, Vitest, SQLite/rusqlite, existing Windows PowerShell toast transport, CUA Driver for live Windows verification.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- Keep `SettingsTabId` values unchanged; only the visible `menuBar` label changes in its own plan.
- New notifications default to disabled for both fresh and upgraded installations.
- Use remaining quota, not used quota, for the 66 warning / 33 danger defaults.
- A notification may open codex-barbar but must never redeem a reset credit, buy usage, or mutate a Codex account.
- Do not introduce dependencies, new permissions, tokens, cookies, raw app-server payloads, or visible PowerShell windows.
- Preserve all unrelated dirty and untracked user files.
- Do not push, tag, build an installer, or release without a separate user instruction.

## File Structure

| File | Responsibility |
| --- | --- |
| `apps/desktop-tauri/src/surfaces/Settings.tsx` | Route the concrete About and Notifications tabs and pass the real bootstrap version. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.tsx` | Render the installed version and update-check state. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx` | Render accessible notification controls and the safe test-notification action. |
| `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts` | English and Simplified Chinese settings/tab/toast copy. |
| `apps/desktop-tauri/src/types/bridge.ts` | Frontend `NotificationPreferencesDto`, nested patch type, and invoke DTO declarations. |
| `apps/desktop-tauri/src/lib/tauri.ts` | Typed `sendTestNotification()` invoke wrapper. |
| `rust/src/storage/settings_repository.rs` | Persist, migrate, merge, and validate notification preferences. |
| `rust/src/notifications/v1.rs` | Pure observation-to-event logic and non-secret persisted dedupe state. |
| `rust/src/notifications.rs` | Export the V1 notification module without changing legacy provider behavior. |
| `rust/src/app_paths.rs` | Derive the exact non-secret notification-state file path beneath LocalAppData. |
| `apps/desktop-tauri/src-tauri/src/notification_controller.rs` | Convert V1 events into platform toasts using an injectable dispatcher. |
| `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` | Rust notification settings DTO and patch conversion. |
| `apps/desktop-tauri/src-tauri/src/commands/settings.rs` | Validate/save preferences and expose `send_test_notification`. |
| `apps/desktop-tauri/src-tauri/src/commands/update.rs` | Route a manually or scheduled detected release through the notification controller. |
| `apps/desktop-tauri/src-tauri/src/main.rs` | Manage the controller and feed it account refresh/state events. |

---

### Task 1: Render the installed version in About

**Files:**
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx:42-178`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.tsx:1-15`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts:58-69,99-123`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.test.tsx`

**Interfaces:**
- Consumes: `BootstrapDto.version: string`.
- Produces: `AboutTab({ version, copy }: { version: string; copy: SettingsCopy })` and `copy.about.version(version: string): string`.

- [ ] **Step 1: Write failing frontend tests**

Add a focused About test with a non-default version and a Settings routing test:

```tsx
render(<AboutTab version="1.0.24" copy={settingsCopy("en-US")} />);
expect(screen.getByText("Version 1.0.24")).toBeInTheDocument();
expect(screen.queryByText(/1\.0\.0/)).not.toBeInTheDocument();
```

The Settings test must mock `getBootstrapState()` with `version: "9.8.7"`, open the About tab, and assert that `Version 9.8.7` is visible.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- AboutTab Settings
```

Expected: the test fails because `AboutTab` does not accept or render `version`.

- [ ] **Step 3: Implement the minimal version data flow**

Replace the hard-coded product version in localized description copy with a dedicated row. Pass `bootstrap?.version ?? "—"` into `AboutTab`; preserve the existing update buttons and offline behavior.

```tsx
<p className="settings-about__version" data-testid="about-installed-version">
  {copy.about.version(version)}
</p>
```

Use `Version {version}` and `当前版本 {version}`. Do not parse, compare, or overwrite the backend version in React.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run the same command and confirm both English and Simplified Chinese assertions pass.

- [ ] **Step 5: Commit the scoped change**

```powershell
git add apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/Settings.test.tsx apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/AboutTab.test.tsx apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
git commit -m "Show installed version in About"
```

### Task 2: Add typed notification preferences and migration defaults

**Files:**
- Modify: `rust/src/storage/settings_repository.rs:55-214`
- Modify: `rust/src/storage/mod.rs:14-20`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs:19-154`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs:48-115,175-300`
- Modify: `apps/desktop-tauri/src/types/bridge.ts:37-64`
- Modify: `apps/desktop-tauri/src/types/bridge.test.ts`
- Modify: `apps/desktop-tauri/src/hooks/useSettings.ts:15-27`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts:39-73`

**Interfaces:**
- Produces the Rust settings type:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub play_sound: bool,
    pub warning_enabled: bool,
    pub danger_enabled: bool,
    pub weekly_reset_enabled: bool,
    pub reset_credit_increase_enabled: bool,
    pub refresh_failure_enabled: bool,
    pub update_available_enabled: bool,
    pub warning_remaining_percent: u8,
    pub danger_remaining_percent: u8,
}
```

- Produces the matching TypeScript contracts:

```ts
export interface NotificationPreferencesDto {
  enabled: boolean;
  playSound: boolean;
  warningEnabled: boolean;
  dangerEnabled: boolean;
  weeklyResetEnabled: boolean;
  resetCreditIncreaseEnabled: boolean;
  refreshFailureEnabled: boolean;
  updateAvailableEnabled: boolean;
  warningRemainingPercent: number;
  dangerRemainingPercent: number;
}

export interface SettingsPatchDto extends Partial<Omit<AppSettingsDto, "notifications">> {
  notifications?: Partial<NotificationPreferencesDto>;
}
```

- [ ] **Step 1: Write failing Rust migration and atomic-validation tests**

Add tests proving all of the following:

```rust
let settings = AppSettings::default();
assert!(!settings.notifications.enabled);
assert_eq!(settings.notifications.warning_remaining_percent, 66);
assert_eq!(settings.notifications.danger_remaining_percent, 33);

let before = repository.load()?;
let result = repository.update(SettingsPatch {
    notifications: Some(NotificationPreferencesPatch {
        warning_remaining_percent: Some(20),
        danger_remaining_percent: Some(33),
        ..Default::default()
    }),
    ..Default::default()
});
assert_eq!(result.unwrap_err().code(), "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID");
assert_eq!(repository.load()?, before);
```

Also seed pre-notification JSON and prove it loads with disabled notifications and every event preference initialized.

- [ ] **Step 2: Run focused storage tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
```

Expected: compile failure because notification preference types and fields do not exist.

- [ ] **Step 3: Implement merge-before-validate persistence**

Add `NotificationPreferences::default()` with `enabled: false`, `play_sound: true`, all event switches `true`, and `66/33` thresholds. Add a partial patch type. In `SettingsRepository::update`, validate individual patch values, clone/load settings, apply the patch, then validate the fully merged settings before encoding and committing.

```rust
let mut next = load_from_connection(&transaction)?;
patch.validate()?;
next.apply(patch);
next.validate()?;
```

This is required so a patch changing only one threshold is checked against the persisted companion threshold atomically.

- [ ] **Step 4: Extend the bridge symmetrically**

Map the nested Rust DTO in `AppSettingsDto::from_settings`; deserialize only known nested patch fields in `SettingsPatchDto::into_patch`; reject non-finite/out-of-range JavaScript numeric input before it reaches storage. Update `defaultAppSettings`, `EMPTY_BOOTSTRAP`, bridge fixtures, and bridge validators so every frontend fallback has the same nested defaults.

- [ ] **Step 5: Run Rust and frontend contract tests and verify GREEN**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::settings::tests -- --nocapture
pnpm --dir apps/desktop-tauri test -- bridge useSettings
```

Expected: defaults, old JSON migration, partial patch merge, invalid-threshold rollback, and typed bridge fixtures all pass.

- [ ] **Step 6: Commit the preference foundation**

```powershell
git add rust/src/storage/settings_repository.rs rust/src/storage/mod.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/hooks/useSettings.ts apps/desktop-tauri/src/hooks/useSettings.test.tsx apps/desktop-tauri/src/hooks/useStatusSurface.ts
git commit -m "Add notification preferences"
```

### Task 3: Implement deterministic notification decisions and durable dedupe

**Files:**
- Modify: `rust/src/notifications.rs:1-15`
- Create: `rust/src/notifications/v1.rs`
- Modify: `rust/src/lib.rs`
- Modify: `rust/src/app_paths.rs:9-35`
- Test: `rust/src/notifications/v1.rs`
- Test: `rust/src/app_paths.rs`

**Interfaces:**
- Consumes: `NotificationPreferences`, `ProfileUsageState`, selected `ProfileId`, optional available reset-credit count, and a refresh success/failure outcome.
- Produces:

```rust
pub enum V1NotificationEvent {
    Warning { remaining_percent: u8 },
    Danger { remaining_percent: u8 },
    WeeklyReset,
    ResetCreditsIncreased { available_count: u64 },
    RefreshFailed,
    RefreshRecovered,
    UpdateAvailable { version: String },
}

pub struct V1NotificationEngine { /* persisted non-secret observation state */ }

impl V1NotificationEngine {
    pub fn observe_usage(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        state: &ProfileUsageState,
        reset_credits: Option<u64>,
    ) -> Vec<V1NotificationEvent>;
    pub fn observe_refresh(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        success: bool,
    ) -> Vec<V1NotificationEvent>;
}
```

- [ ] **Step 1: Write failing engine tests for transitions, first observation, and restart**

Use a `ProfileUsageState` fixture whose `secondary` window is exactly 10,080 minutes. Assert:

```rust
assert!(engine.observe_usage(&enabled, id, &weekly(80, reset_a), Some(1)).is_empty());
assert_eq!(engine.observe_usage(&enabled, id, &weekly(40, reset_a), Some(1)),
           vec![V1NotificationEvent::Warning { remaining_percent: 60 }]);
assert!(engine.observe_usage(&enabled, id, &weekly(40, reset_a), Some(1)).is_empty());
assert_eq!(engine.observe_usage(&enabled, id, &weekly(20, reset_a), Some(2)),
           vec![V1NotificationEvent::Danger { remaining_percent: 80 },
                V1NotificationEvent::ResetCreditsIncreased { available_count: 2 }]);
```

Add exact tests for: disabled master switch, `None -> Some(n)` baseline with no credit toast, a new weekly reset cycle re-arming bands, three failed refreshes followed by one recovery, and serialize/load preserving dedupe state.

- [ ] **Step 2: Run the new unit tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml notifications::v1::tests -- --nocapture
```

Expected: module-not-found failure.

- [ ] **Step 3: Implement the engine without platform side effects**

Make `rust/src/notifications.rs` export `pub mod v1;`. In the V1 module, select only a 10,080-minute window from `primary` then `secondary`; never inspect additional/model-specific 5-hour windows. Persist only profile ID, weekly reset timestamp, armed band, known reset-credit count, and consecutive refresh failures as JSON at `AppPaths::notification_state`.

The observer must update baseline state even when notifications are disabled, but return no events until `preferences.enabled` is true. Persist after every successful observation; a failed state-file write logs a sanitized warning and leaves notification delivery running in memory.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command plus:

```powershell
cargo test --manifest-path rust/Cargo.toml app_paths::tests -- --nocapture
```

Expected: every threshold, reset, credit, failure/recovery, and restart case passes without using a Windows toast.

- [ ] **Step 5: Commit the pure engine**

```powershell
git add rust/src/notifications.rs rust/src/notifications/v1.rs rust/src/lib.rs rust/src/app_paths.rs
git commit -m "Add deduplicated notification engine"
```

### Task 4: Wire the controller, test action, and Notifications tab

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/notification_controller.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs:3-18,109-235`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs:80-175`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts:17-60,134-165`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx:15-25,143-178`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`

**Interfaces:**
- Produces a controller boundary that can be tested without Windows:

```rust
trait ToastSink: Send {
    fn send(&mut self, title: &str, body: &str, play_sound: bool) -> Result<(), String>;
}

pub struct NotificationController<S: ToastSink> { /* engine + sink */ }

pub fn send_test_notification(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    controller: tauri::State<'_, Mutex<NotificationController<WindowsToastSink>>>,
) -> Result<(), String>;
```

- [ ] **Step 1: Write failing controller and component tests**

Create a fake `ToastSink` test that records events. Verify the controller reads current persisted preferences, dispatches no toast while the master switch is false, dispatches exactly one toast for a qualified event, sends a test toast without consuming any reset credit, and sends exactly one `UpdateAvailable` toast for a newly observed release version.

Render the tab and verify:

```tsx
expect(screen.getByRole("checkbox", { name: /enable notifications/i })).not.toBeChecked();
await user.click(screen.getByRole("checkbox", { name: /enable notifications/i }));
expect(update).toHaveBeenCalledWith({ notifications: { enabled: true } });
```

Assert warning/danger controls expose numeric values, event switches are disabled while the master switch is off, and both languages have complete labels.

- [ ] **Step 2: Run focused shell/frontend tests and verify RED**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller::tests -- --nocapture
pnpm --dir apps/desktop-tauri test -- NotificationsTab Settings
```

Expected: missing controller/module and missing Notifications tab failures.

- [ ] **Step 3: Implement a hidden-window Windows toast sink and update observation**

Move or call the existing hidden `CREATE_NO_WINDOW` toast transport; do not use a visible shell, `Start-Process`, or a taskbar-visible PowerShell window. XML-escape all title/body text, log only sanitized dispatch errors, and return a user-safe error for the test action.

Register the controller in `main.rs`, then call it from `UsageStateChanged` and terminal refresh outcomes in the existing account-service subscriber. The controller loads the active settings snapshot before deciding each event. Do not emit a toast while in proof mode.

Extend `check_for_updates` so a returned `ManualUpdateResult::Available` calls `controller.observe_update_available(latest_version)` after the manual result is constructed. Add a controller-owned once-per-24-hours async check loop that runs only when both `notifications.enabled` and `notifications.update_available_enabled` are true; it uses `ManualUpdateChecker` and records the version for dedupe. Network failures remain silent and are not notification failures.

- [ ] **Step 4: Implement the user-facing tab and bridge command**

Render a compact master card, event-toggle card, remaining-percent threshold controls, sound switch, and test action. Patch only the nested field being changed:

```tsx
void update({ notifications: { dangerRemainingPercent: Number(event.target.value) } });
```

Surface rejected threshold patches in an inline `role="alert"` message and keep the prior saved values. Add the `send_test_notification` invoke name and map command failure to the localized inline diagnostic.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run the same focused tests and confirm fake sink, disabled-master, safe test toast, bridge patch, locale, and validation cases pass.

- [ ] **Step 6: Commit the vertical slice**

```powershell
git add apps/desktop-tauri/src-tauri/src/notification_controller.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src-tauri/src/commands/update.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.test.tsx apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
git commit -m "Add opt-in usage notifications"
```

### Task 5: Verify the first milestone on Windows

**Files:**
- Verify only; add screenshots under the task-owned proof location only if the project review template requires checked-in proof.

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: automated evidence and fresh native Windows proof for About and Notifications.

- [ ] **Step 1: Run automated verification**

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

- [ ] **Step 2: Build and launch a fresh Windows binary**

Resolve and close only the exact running `codex-barbar.exe` path, then run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Launch that freshly built binary with `CODEXBAR_PROOF_MODE=settings:notifications`; do not validate against an already-running single-instance process.

- [ ] **Step 3: Prove native observables with CUA**

Using CUA Driver, capture and assert:

- About displays the actual package version;
- Notifications master switch starts disabled;
- enabling it persists across a restart;
- warning/danger controls reject an invalid pair without changing saved values;
- test notification arrives without a visible PowerShell window;
- disabling the master switch stops further testable automatic event delivery.

- [ ] **Step 4: Final scope review and commit verification evidence only when authorized**

Run `git status --short` and inspect the scoped diff. Confirm pre-existing untracked files are untouched and notification state/test data contains no secrets. Do not push or release.
