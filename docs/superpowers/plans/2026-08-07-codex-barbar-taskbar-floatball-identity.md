# codex-barbar Taskbar Status, Float Ball, and Account Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add opt-in Windows taskbar status and animated float-ball surfaces that share cached Codex usage data and show the real OpenAI account identity.

**Architecture:** Keep account identity and usage ownership in the shared `codexbar` Rust crate. Extend the existing redacted Tauri bridge with identity and two boolean settings, then let a shell-owned status-surface manager create and position two detached WebViews (`taskbar-status` and `float-ball`). React surfaces consume the same bootstrap/event contract as the tray panel and never read files or SQLite directly.

**Tech Stack:** Rust 2024, Tokio, rusqlite, serde, existing `secure_file`/DPAPI helper, Tauri 2 WebviewWindowBuilder, raw Win32 FFI already used by `shell/dwm.rs`, React 18, TypeScript, Vitest, Testing Library, Cua/Win32 proof on Windows.

## Global Constraints

- Work only in `C:\Users\stack\Documents\codex-barbar\.worktrees\taskbar-floatball-identity` on branch `codex/taskbar-floatball-identity`.
- Do not modify the user's dirty root worktree or the `.worktrees/v1-implementation` worktree.
- Do not add a new crate or npm dependency; reuse the existing `windows` dependency in `codexbar`, `raw-window-handle`, Tauri APIs, and current `secure_file`.
- New settings default to `false`; missing fields in old SQLite JSON decode as `false`.
- Account identity is Codex/OpenAI-only and must never be copied from another provider.
- Do not log or emit tokens, cookies, vault contents, paths, raw protocol JSON, or clear-text identity outside the intended local UI DTO.
- Preserve the existing event names exactly: `accounts-updated`, `profile-usage-state-changed`, `refresh-state-changed`, and `settings-changed`.
- Status color thresholds are exact: green when `usedPercent < 75`, amber when `75 <= usedPercent < 90`, red when `usedPercent >= 90` or the current refresh has an error, and gray when no usable snapshot exists.
- The primary display window is `primary`, falling back to `secondary`; if both are absent, show the empty/unavailable state.
- The taskbar surface is a visual overlay, not Explorer injection, DeskBand registration, or a shell extension.
- Every production behavior change follows a red-green-refactor cycle: write a focused failing test, run it and observe the expected failure, implement the smallest change, rerun the focused test, then run the relevant crate/frontend suite.
- Before claiming completion, run fresh verification on both Rust manifests, the frontend suite/build, and a fresh Windows debug binary with the old process closed.

---

## Task 1: Add account identity parsing, secure cache, and service ownership

**Files:**

- Create: `rust/src/accounts/identity.rs`
- Modify: `rust/src/accounts/mod.rs`
- Modify: `rust/src/providers/codex/app_server/model.rs`
- Modify: `rust/src/accounts/service.rs`
- Modify: `rust/src/app_paths.rs`
- Modify: `rust/src/accounts/test_support.rs`
- Test: `rust/src/accounts/identity.rs`
- Test: `rust/src/providers/codex/app_server/model.rs`
- Test: `rust/src/accounts/service.rs`

**Interfaces:**

- `AccountIdentity` gains `display_name: Option<String>` while retaining `auth_mode`, `email`, and `plan_type`.
- `AccountIdentityRecord` is a serializable cache record:

  ```rust
  pub struct AccountIdentityRecord {
      pub display_name: Option<String>,
      pub email: Option<String>,
      pub plan_type: Option<String>,
      pub updated_at: DateTime<Utc>,
  }
  ```

- `AccountIdentityCache` exposes:

  ```rust
  pub fn new(path: PathBuf) -> Self;
  pub fn load(&self, profile_id: ProfileId) -> Result<Option<AccountIdentityRecord>, IdentityCacheError>;
  pub fn save(&self, profile_id: ProfileId, record: &AccountIdentityRecord) -> Result<(), IdentityCacheError>;
  pub fn remove(&self, profile_id: ProfileId) -> Result<(), IdentityCacheError>;
  ```

- `AccountProfileService` stores an `Arc<AccountIdentityCache>`, exposes `identity_for(ProfileId)`, and updates the cache after every successful `account/read` for both current CLI and managed profiles.

### Step 1: Write failing identity parser tests

Add tests to `rust/src/providers/codex/app_server/model.rs` for:

```rust
#[test]
fn display_name_precedes_email_and_accepts_camel_case() { /* ... */ }

#[test]
fn snake_case_name_and_full_name_are_supported() { /* ... */ }

#[test]
fn empty_name_falls_back_to_email() { /* ... */ }

#[test]
fn identity_record_ignores_token_and_cookie_fields() { /* ... */ }
```

Use nested `serde_json::json!` account payloads with `displayName`, `display_name`, `name`, `fullName`, `email`, `emailAddress`, `token`, and `cookie`; assert only the typed identity fields are present.

### Step 2: Run the focused parser tests and observe failure

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model
```

Expected: the new tests fail because `AccountIdentity` has no display-name field and no normalization helper.

### Step 3: Implement tolerant parsing

In `AccountIdentity::from_value`:

- Find the account object exactly as the current parser does.
- Normalize candidate keys case-insensitively and accept both camelCase and snake_case.
- Trim values and treat empty strings as absent.
- Use `displayName`, `display_name`, `name`, then `fullName` for `display_name`.
- Use `email`, then `emailAddress` for `email`.
- Keep the existing auth-mode validation and plan parsing.
- Never retain the original `Value` after parsing.

### Step 4: Run the focused parser tests and refactor

Run the same command and require all focused tests to pass. Then run:

```powershell
cargo fmt --all
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model
```

### Step 5: Write failing cache tests

Create `rust/src/accounts/identity.rs` with tests for:

```rust
#[test]
fn cache_round_trips_a_profile_identity() { /* ... */ }

#[test]
fn failed_atomic_replace_keeps_previous_cache_file() { /* ... */ }

#[test]
fn removing_a_profile_removes_only_that_identity() { /* ... */ }
```

Inject a temporary file path into `AccountIdentityCache`; use a test-only replace hook or a read-only parent directory to force replacement failure and assert the previous record remains readable.

### Step 6: Run cache tests and observe failure

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::identity
```

Expected: compilation fails because the cache type does not exist.

### Step 7: Implement the DPAPI-backed cache

- Add `identity` to `rust/src/accounts/mod.rs`.
- Add `AppPaths::identity_cache` (or an equivalent method) returning `root/identity/profiles.json`.
- Store a single JSON object keyed by `ProfileId` strings.
- Read/write the JSON through `codexbar::secure_file::read_string` and `write_string`.
- Create the parent directory before writes.
- Write to a sibling temporary file, flush it, then replace the target with a same-directory rename; on Windows use a remove-and-rename fallback only when the target replacement API reports a sharing violation.
- Return redacted error codes (`IDENTITY_CACHE_READ_FAILED`, `IDENTITY_CACHE_WRITE_FAILED`, `IDENTITY_CACHE_DECODE_FAILED`) without embedding identity values.
- On load failure return `Ok(None)` to keep startup non-blocking; callers may log only the diagnostic code.

### Step 8: Run cache tests and the full Rust account suite

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::identity
cargo test --manifest-path rust/Cargo.toml accounts
```

Expected: all cache and account tests pass with no clear-text identity in error strings.

### Step 9: Write service integration tests

Extend `rust/src/accounts/service.rs` tests to assert:

```rust
#[tokio::test]
async fn current_cli_refresh_caches_account_identity_before_usage_event() { /* ... */ }

#[tokio::test]
async fn managed_refresh_caches_identity_and_removal_clears_it() { /* ... */ }

#[tokio::test]
async fn account_read_failure_keeps_last_successful_identity() { /* ... */ }
```

Update `test_support` constructors to inject a temporary identity-cache path and expose it to assertions.

### Step 10: Implement service cache updates

- Add the cache field and constructor parameter.
- In current-CLI refresh, save the parsed account identity immediately after `account_read` succeeds, before parsing/saving rate limits.
- In managed refresh, save identity after `account_read` succeeds and before credential resealing.
- In managed login completion, save identity after the account read succeeds.
- On profile removal, call `cache.remove(profile_id)` after the SQLite/vault deletion succeeds.
- Add `identity_for` and a snapshot helper that returns a cloned `BTreeMap<ProfileId, AccountIdentityRecord>` for bridge assembly.
- Preserve the last successful cache record when a later account read fails.

### Step 11: Run service tests and commit the bounded backend slice

Run:

```powershell
cargo fmt --all
cargo test --manifest-path rust/Cargo.toml accounts
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

Commit:

```powershell
git add rust/src/accounts rust/src/providers/codex/app_server/model.rs rust/src/app_paths.rs
git commit -m "Add secure Codex account identity cache"
```

---

## Task 2: Extend settings and the redacted bridge DTOs

**Files:**

- Modify: `rust/src/storage/settings_repository.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/hooks/useSettings.ts`
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`
- Test: `rust/src/storage/settings_repository.rs`
- Test: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Test: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`
- Test: `apps/desktop-tauri/src/types/bridge.test.ts`
- Test: `apps/desktop-tauri/src/hooks/useSettings.test.ts`

**Interfaces:**

- `AppSettings` adds `taskbar_status_enabled: bool` and `float_ball_enabled: bool`.
- `SettingsPatch` and `SettingsPatchDto` add optional `Option<bool>` fields.
- `AppSettingsDto` and `AppSettingsDto` TypeScript interface expose camelCase `taskbarStatusEnabled` and `floatBallEnabled`.
- `ProfileSummaryDto` exposes `accountDisplayName: string | null` and `accountEmail: string | null`; the historical `email: string | null` field remains serialized as `null` for V1 wire compatibility and is not used by new surfaces.
- `ProfileSummaryDto::from_profile(profile, identity)` derives the visible label for `currentCli` as `display name -> email -> "未登录"`, while preserving managed custom labels.

### Step 1: Write failing settings migration tests

Add Rust tests:

```rust
#[test]
fn new_surface_settings_default_to_disabled() { /* ... */ }

#[test]
fn old_settings_json_without_surface_fields_loads_as_disabled() { /* ... */ }

#[test]
fn partial_surface_patch_changes_only_requested_flag() { /* ... */ }
```

Use an in-memory SQLite repository and a legacy JSON string that contains only the original V1 fields.

### Step 2: Run focused settings tests and observe failure

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
```

Expected: compilation or deserialization failures because the new fields are absent.

### Step 3: Implement backward-compatible settings

- Add `#[serde(default)]` to `AppSettings`.
- Add both booleans to `Default`, `SettingsPatch`, and `AppSettings::apply`.
- Keep unknown fields ignored by serde and invalid patch types rejected by the existing bridge parser.
- Add exact assertions to `v1_settings_defaults_are_exact`.

### Step 4: Run Rust settings tests

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::settings_repository
```

Expected: all settings tests pass.

### Step 5: Write failing DTO tests

Add bridge tests asserting:

```rust
#[test]
fn current_cli_profile_uses_account_identity_instead_of_current_cli_label() { /* ... */ }

#[test]
fn managed_profile_keeps_custom_label_and_exposes_identity_separately() { /* ... */ }

#[test]
fn profile_dto_never_serializes_credentials_or_paths() { /* ... */ }
```

Add TypeScript contract tests that parse a bootstrap fixture with both identity fields and both settings flags.

### Step 6: Run focused DTO/frontend tests and observe failure

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge
pnpm --dir apps/desktop-tauri test -- src/types/bridge.test.ts
```

Expected: new fields are missing or the current label still contains `Current CLI`.

### Step 7: Implement bridge changes

- Add the fields to Rust DTOs and TypeScript types.
- Build profile DTOs from the service identity map; do not read the secure file from the WebView.
- Change fixtures and `parseProfileUsageState` contract tests to include the new fields.
- Keep `BootstrapDto` top-level shape unchanged apart from the enriched nested settings/profile objects.
- Update proof-harness fixtures so deterministic UI tests use a synthetic display name and never regress to `Current CLI`.

### Step 8: Add the non-focusing tray-panel command and wire settings events to the shell manager boundary

Add this fixed command to `commands/window.rs`, register it in `main.rs`, and expose it from
`lib/tauri.ts`:

```rust
#[tauri::command]
pub fn open_tray_panel(app: tauri::AppHandle) -> Result<(), String> {
    crate::shell::flyout_window::open_or_focus(&app, None)
}
```

Create a small shell-facing function (implemented in Task 3/4) with this signature:

```rust
pub fn apply_status_surface_settings(
    app: &tauri::AppHandle,
    settings: &codexbar::storage::AppSettings,
) -> Result<(), String>;
```

Call it after releasing the repository/state lock in `commands::settings::update_settings`, and once from `setup` after the tray is initialized. A surface creation/position error must be logged and returned only to the surface manager; it must not fail the settings command or startup.

### Step 9: Update React settings defaults and tests

- Add both booleans to `defaultAppSettings` and all bootstrap fixtures.
- Add two labeled checkboxes to `GeneralTab.tsx`.
- Each change handler sends only one property in `update({ ... })`.
- Add tests for checked state, independent updates, and event-driven state refresh.

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useSettings.test.ts src/types/bridge.test.ts
pnpm --dir apps/desktop-tauri run build
```

### Step 10: Commit the bridge/settings slice

```powershell
git add rust/src/storage/settings_repository.rs apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src/types apps/desktop-tauri/src/hooks apps/desktop-tauri/src/test apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx
git commit -m "Expose status surface settings and account identity"
```

---

## Task 3: Implement pure taskbar slot geometry and the Windows overlay manager

**Files:**

- Create: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Create: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs`
- Create: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs`
- Create: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/win32.rs`
- Create: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/dwm.rs`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Test: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/positioning.rs`
- Test: `apps/desktop-tauri/src-tauri/src/taskbar_overlay/win32.rs`
- Test: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Test: `apps/desktop-tauri/src-tauri/src/main.rs`

**Interfaces:**

- Pure geometry types:

  ```rust
  pub enum TaskbarEdge { Bottom, Top, Left, Right }
  pub struct TaskbarSnapshot { pub taskbar: Rect, pub app_area: Option<Rect>, pub notification_area: Option<Rect>, pub edge: TaskbarEdge, pub dpi: u32, pub auto_hide: bool }
  pub fn compute_slot(snapshot: &TaskbarSnapshot, logical_width: u32) -> Rect
  ```

- `TaskbarOverlay` owns the `taskbar-status` WebView and provides:

  ```rust
  pub fn apply_enabled(&mut self, app: &tauri::AppHandle, enabled: bool);
  pub fn reposition(&self, app: &tauri::AppHandle);
  pub fn handle_shell_change(&mut self, app: &tauri::AppHandle);
  ```

- `StatusSurfaceState` is the single managed shell state:

  ```rust
  pub struct StatusSurfaceState {
      pub taskbar: TaskbarOverlay,
      pub float_ball: crate::float_ball::FloatBall,
  }
  ```

  Register it with `Builder::manage(Mutex::new(StatusSurfaceState::default()))`; commands and
  startup use this state instead of separate globals.

### Step 1: Write failing pure geometry tests

Cover bottom/top/left/right, 100/125/150/200% DPI, missing notification area, auto-hide rectangles, narrow slots, and negative monitor coordinates:

```rust
#[test]
fn bottom_taskbar_places_slot_between_app_area_and_notification_area() { /* ... */ }

#[test]
fn vertical_taskbar_uses_cross_axis_centering() { /* ... */ }

#[test]
fn narrow_slot_shrinks_to_minimum_and_clamps_inside_taskbar() { /* ... */ }

#[test]
fn dpi_scales_logical_width_without_overflow() { /* ... */ }
```

### Step 2: Run geometry tests and observe failure

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay::positioning
```

Expected: module/types do not exist.

### Step 3: Implement pure geometry

- Represent all Win32 rectangles as physical pixels at the boundary.
- Convert the requested 260 logical pixels using `dpi / 96.0`.
- Use the notification-area start and app-area end as the preferred slot bounds.
- Clamp to the taskbar rectangle and reserve an 8 physical-pixel margin.
- Shrink to 160 logical pixels when necessary; return the actual physical rectangle so the WebView width matches the slot.
- Keep auto-hide state as metadata; geometry itself remains inside the taskbar rect.

### Step 4: Run geometry tests and refactor

Run the focused tests again, then:

```powershell
cargo fmt --all
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml taskbar_overlay::positioning
```

### Step 5: Write failing Windows adapter tests

Add deterministic tests for:

- case-insensitive `Shell_TrayWnd`, `TrayNotifyWnd`, and `MSTaskSwWClass` discovery helpers;
- mapping a taskbar rectangle to `TaskbarEdge`;
- failure returning `None` instead of panicking.

Keep OS calls behind a small `Win32TaskbarApi` trait so pure test doubles can supply handles and rectangles.

### Step 6: Implement Win32 discovery without a new dependency

- Put raw `extern "system"` declarations in `win32.rs` under `#[cfg(windows)]`, following the existing style in `shell/dwm.rs`.
- Use `FindWindowW`, `FindWindowExW`, `GetWindowRect`, `MonitorFromWindow`, `GetMonitorInfoW`, `GetDpiForWindow`, and `IsWindowVisible`.
- Return only rectangles, edge, DPI, and auto-hide status; never expose HWND values to React.
- Provide a non-Windows implementation returning `None` so Rust tests and cross-compilation stay deterministic.

### Step 7: Write failing overlay-window tests

Add tests that assert:

```rust
#[test]
fn overlay_label_and_frontend_route_are_stable() { /* ... */ }

#[test]
fn overlay_failure_is_non_fatal_to_startup() { /* ... */ }
```

### Step 8: Implement window builder and lifecycle

- Register `mod taskbar_overlay` in `main.rs`.
- Build label `taskbar-status` with URL `index.html?window=taskbar-status`, transparent/no decorations/no shadow, `skip_taskbar(true)`, `always_on_top(true)`, `focused(false)`, and hidden initial visibility.
- Apply `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED` through a helper in `shell/dwm.rs` after the native handle is available.
- `apply_enabled(false)` hides the window; `apply_enabled(true)` creates/reuses it and calls `reposition`.
- Poll/reposition on a short backoff after taskbar discovery failure. Reposition on monitor/DPI changes and after the Explorer taskbar HWND changes.
- Clicking the WebView invokes the existing tray-panel open command; the overlay never calls `set_focus`.

### Step 9: Wire manager into setup and settings

- Register `StatusSurfaceState` as a Tauri managed value in `main.rs` before `run`; do not put WebView handles inside `AppState`.
- Call `apply_enabled` once in setup and after each settings update.
- Keep the manager independent of account refresh; it only owns windows and emits no secrets.
- Add an `on_window_event` branch for the overlay to ignore blur-dismiss and to hide when Explorer/taskbar is unavailable.

### Step 10: Run shell tests and commit

Run:

```powershell
cargo fmt --all
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

Commit:

```powershell
git add apps/desktop-tauri/src-tauri/src/taskbar_overlay apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/shell apps/desktop-tauri/src-tauri/tauri.conf.json
git commit -m "Add Windows taskbar status overlay"
```

---

## Task 4: Implement float-ball geometry, native window lifecycle, and persistence

**Files:**

- Create: `apps/desktop-tauri/src-tauri/src/float_ball/mod.rs`
- Create: `apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs`
- Create: `apps/desktop-tauri/src-tauri/src/float_ball/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/status_surfaces.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/geometry_store.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/dwm.rs`
- Test: `apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs`
- Test: `apps/desktop-tauri/src-tauri/src/float_ball/mod.rs`
- Test: `apps/desktop-tauri/src-tauri/src/geometry_store.rs`

**Interfaces:**

- Pure geometry:

  ```rust
  pub const FLOAT_BALL_LOGICAL_SIZE: u32 = 72;
  pub fn initial_position(monitor: Rect, work_area: Rect, scale: f64) -> Point;
  pub fn clamp_position(position: Point, monitor: Rect, work_area: Rect, size: i32, margin: i32) -> Point;
  pub fn physical_to_logical(position: Point, scale: f64) -> Point;
  pub fn logical_to_physical(position: Point, scale: f64) -> Point;
  ```

- Window label is fixed to `float-ball`; persisted geometry key is also `float-ball`.

### Step 1: Write failing geometry tests

Add tests for first-run bottom-right placement, taskbar avoidance, negative monitor origins, detached-monitor fallback, DPI conversion, and clamp-to-work-area.

### Step 2: Run focused geometry tests and observe failure

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball::geometry
```

Expected: missing module/functions.

### Step 3: Implement geometry and store integration

- Reuse `geometry_store::load_entry/save_entry` with key `float-ball`.
- Store logical coordinates, never physical coordinates.
- If the saved point is outside every current monitor, use the primary monitor first-run position.
- Preserve the existing geometry file version/migration behavior.

### Step 4: Write failing window lifecycle tests

Assert label/route stability, no-op close behavior, and that a disabled setting hides the window without deleting the saved point.

### Step 5: Implement the float-ball window

- Build a transparent, borderless, always-on-top, skip-taskbar window at `index.html?window=float-ball`.
- Pin `.theme(Some(tauri::Theme::Dark))` to avoid WebView2 shared-profile theme flips.
- Apply no-activate/tool-window styles with the existing native helper.
- On `Moved`, read the physical outer position, convert using the window monitor DPI, and persist the logical point.
- On monitor/display changes, clamp or restore to the primary work area.
- Clicking the surface invokes the fixed `open_tray_panel` command; pointer dragging remains in the
  WebView and does not open the panel on pointer-up.

### Step 6: Run float-ball and shell tests

```powershell
cargo fmt --all
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball geometry_store
```

### Step 7: Commit the float-ball slice

```powershell
git add apps/desktop-tauri/src-tauri/src/float_ball apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/geometry_store.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/shell/dwm.rs
git commit -m "Add persistent animated float ball window"
```

---

## Task 5: Add React status surfaces, shared status derivation, and window routing

**Files:**

- Create: `apps/desktop-tauri/src/hooks/useStatusSurface.ts`
- Create: `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- Create: `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`
- Create: `apps/desktop-tauri/src/surfaces/FloatBall.tsx`
- Create: `apps/desktop-tauri/src/surfaces/FloatBall.css`
- Modify: `apps/desktop-tauri/src/App.tsx`
- Modify: `apps/desktop-tauri/src/styles.css`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Test: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx`
- Test: `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- Test: `apps/desktop-tauri/src/surfaces/FloatBall.test.tsx`
- Test: `apps/desktop-tauri/src/App.test.tsx`

**Interfaces:**

- `useStatusSurface()` returns:

  ```ts
  {
    bootstrap: BootstrapDto | null;
    profile: ProfileSummaryDto | null;
    state: ProfileUsageStateDto;
    percent: number | null;
    displayName: string;
    status: "ready" | "warning" | "critical" | "refreshing" | "stale" | "missing";
    isDragging: boolean;
    openPanel(): Promise<void>;
  }
  ```

- `TaskbarStatus` and `FloatBall` receive no provider-specific props; they call the hook and render the same derived status.

### Step 1: Write failing hook/surface tests

Add fixtures for:

- display name over email;
- email fallback;
- long identity truncation with full `title`;
- green/amber/red/gray thresholds;
- stale/refreshing state;
- no profile/no quota;
- click opens the panel;
- float-ball pointer drag does not trigger click;
- reduced-motion media query removes animation classes.

### Step 2: Run focused frontend tests and observe failure

```powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useStatusSurface.test.tsx src/surfaces/TaskbarStatus.test.tsx src/surfaces/FloatBall.test.tsx
```

Expected: modules and routes are missing.

### Step 3: Implement shared status derivation

- Call `getBootstrapState` on mount.
- Reuse the existing event constants and `useProfileUsage` state normalization where possible; do not create a second quota cache.
- Derive the selected profile identity using `accountDisplayName ?? accountEmail ?? "未登录"`.
- Choose `primary ?? secondary`; percent is `remainingPercent` rounded to an integer for display.
- Mark errors/stale states before applying the numeric threshold color.

### Step 4: Implement TaskbarStatus

- Render a single compact button with Codex icon, truncated identity, and `N%`.
- Add `aria-label`, `title`, and `data-status`.
- Keep the surface transparent and width-flexible so the native slot can shrink to 160 logical pixels.
- Use the exact colors from the shared CSS variables: green `#65c466`, amber `#e6a23c`, red `#e85d5d`, gray `#9299a5`.
- On click call `openTrayPanel`, which invokes the fixed `open_tray_panel` command without
  focusing the status WebView.

### Step 5: Implement FloatBall

- Render an SVG circular progress ring and central percent text.
- Add `status` classes and `float-ball--refreshing` only while refreshing.
- Use CSS `animation` for breathing/pulsing/rotation and the existing `prefers-reduced-motion` media query to disable them.
- Implement pointer capture drag with a movement threshold of 4 CSS pixels; only a pointer-up with no drag calls `openPanel`.
- Set the full identity in `title` and a short identity inside the circle when space permits.

### Step 6: Implement app routing and bridge helpers

- Route `taskbar-status` to `TaskbarStatus` and `float-ball` to `FloatBall`.
- Keep `main` and `settings` routes unchanged.
- Add `openTrayPanel = () => invoke<void>(commands.openTrayPanel)` and keep the command name in
  the bridge contract tests.
- Unknown labels continue to render `null`.

### Step 7: Run focused and full frontend tests

```powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useStatusSurface.test.tsx src/surfaces/TaskbarStatus.test.tsx src/surfaces/FloatBall.test.tsx src/App.test.tsx
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
```

### Step 8: Commit the React slice

```powershell
git add apps/desktop-tauri/src/App.tsx apps/desktop-tauri/src/hooks apps/desktop-tauri/src/surfaces/TaskbarStatus* apps/desktop-tauri/src/surfaces/FloatBall* apps/desktop-tauri/src/styles.css apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.ts
git commit -m "Add taskbar and float ball status surfaces"
```

---

## Task 6: Integrate proof harness, settings tabs, and account display regression coverage

**Files:**

- Modify: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/surface_target.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/AccountsTab.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsTabs.ts`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Test: `apps/desktop-tauri/src-tauri/src/proof_harness.rs`
- Test: `apps/desktop-tauri/src-tauri/src/surface_target.rs`
- Test: `apps/desktop-tauri/src/surfaces/settings/tabs/AccountsTab.test.tsx`

**Steps:**

- Add proof targets `taskbar-status`, `float-ball`, and `settings:general`/`settings:providers` while keeping the existing whitelist behavior.
- Replace synthetic visible `Current CLI` labels with `Ming Zhao`/`user@example.com` fixture values and add a test that the serialized bootstrap does not contain `Current CLI` for the selected current profile.
- Keep managed custom labels unchanged and show identity as secondary text.
- Add the two surface toggles to the existing General tab rather than introducing a new settings tab.
- Ensure all tab IDs remain synchronized between `settingsTabs.ts` and `surface_target.rs`.
- Run the proof-harness unit tests and the complete frontend suite.

Commit:

```powershell
git add apps/desktop-tauri/src-tauri/src/proof_harness.rs apps/desktop-tauri/src-tauri/src/surface_target.rs apps/desktop-tauri/src/surfaces/settings apps/desktop-tauri/src/types/bridge.ts
git commit -m "Cover status surfaces in deterministic proof harness"
```

---

## Task 7: Fresh verification and Windows-native acceptance

**Files:**

- Modify: `docs/WINDOWS_PROOF.md` (append the new acceptance checklist and evidence paths)
- Create: `docs/verification/taskbar-floatball-identity-2026-08-07.md`

### Step 1: Run the repository local gate

```powershell
cargo fmt --all -- --check
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
.\scripts\local-check.ps1
```

Record exact counts and any pre-existing ignored test in the verification note.

### Step 2: Build a fresh Windows debug binary

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
Get-Process codex-barbar -ErrorAction SilentlyContinue | Stop-Process
```

Launch the freshly built `codex-barbar.exe`; do not validate against an older installed binary.

### Step 3: Drive proof mode and capture observables

Use:

```powershell
$env:CODEXBAR_PROOF_MODE = 'settings:general'
```

Then use the installed Cua driver (or the documented Win32 fallback) to verify:

- each toggle creates/removes the corresponding `taskbar-status` or `float-ball` window;
- taskbar overlay is between the app area and notification area and does not cover system icons;
- bottom/top/left/right taskbar positions;
- 100/125/150/200% DPI;
- primary/secondary monitor relocation;
- auto-hide and Explorer restart recovery within five seconds;
- clicking does not steal foreground focus;
- float-ball drag, restart persistence, animation, and reduced-motion;
- the selected Codex profile shows the actual account name or email, never `Current CLI`.

### Step 4: Record evidence and final review

Write screenshot paths, window rectangles, process version, and command output to the verification note. Run `git diff --check`, inspect `git status`, and confirm every spec requirement maps to a test or acceptance observation before making a completion claim.

If all checks pass, commit the evidence note:

```powershell
git add docs/WINDOWS_PROOF.md docs/verification/taskbar-floatball-identity-2026-08-07.md
git commit -m "Document taskbar and float ball verification"
```
