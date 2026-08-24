# v1.1 Identity, Surface, and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax - [ ] for tracking.

**Goal:** Replace email-first status identity with safe username/avatar presentation, migrate visual controls to stable 0–100 semantics, make the native tray fixed, and turn Menu into compact panel personalization.

**Architecture:** Rust owns migration, account presentation identity, avatar storage, fixed tray behavior, and typed bridge DTOs. React uses one avatar component and one committed-range component across panel/taskbar/settings surfaces. The hidden measurement route keeps rendering the same TaskbarStatusContents component as the visible taskbar route.

**Tech Stack:** Rust 2024, Tauri 2 custom protocol, React 18, TypeScript, Vitest, existing reqwest, Canvas API, SQLite-backed settings, Windows CUA proof.

**Spec:** docs/superpowers/specs/2026-08-24-v1-1-identity-pricing-surfaces-design.md

## Global Constraints

- Follow docs/superpowers/plans/2026-08-24-v1-1-rollout-index.md.
- No browser Cookie reads, browser-page scraping, raw file paths, remote avatar URLs, or avatar source URLs in frontend DTOs/logs/diagnostics.
- Avatar download accepts only HTTPS, an approved official host suffix, no redirect, image MIME type, bounded bytes, bounded dimensions, and no private-network target.
- Full email is rendered only by surfaces/tray/ProfileSelector.tsx.
- Refresh is always visible in panel quick actions.
- Percent inputs are valid only in 0..=100; legacy stored 0..80 values scale with round(value * 100 / 80) exactly once.
- No new crate, package, plugin, or Tauri permission.

---

### Task 1: Introduce v2 presentation settings and deterministic migration

**Files:**
- Create: rust/src/storage/settings_migration.rs
- Modify: rust/src/storage/settings_repository.rs
- Modify: rust/src/storage/menu_layout.rs
- Modify: rust/src/storage/mod.rs
- Test: rust/src/storage/settings_repository.rs
- Test: rust/src/storage/menu_layout.rs

**Interfaces:**
- Produce pub const SETTINGS_SCHEMA_VERSION: u8 = 2.
- Produce pub struct SurfaceAppearancePreferences { taskbar_transparency_percent: u8, float_ball_transparency_percent: u8, float_ball_glow_percent: u8 }.
- Produce pub struct PanelPreferences { density: PanelDensity, show_reset_time: bool, show_freshness: bool, show_account_status: bool, actions: MenuLayout }.
- Produce pub fn migrate_settings_json(value: serde_json::Value) -> Result<(serde_json::Value, bool), StorageError>.
- Replace public patch fields with taskbar_transparency_percent, float_ball_transparency_percent, and float_ball_glow_percent.

- [ ] **Step 1: Write failing migration and layout tests.**

~~~rust
#[test]
fn schema_v1_visual_values_scale_once_to_v2() {
    let input = serde_json::json!({
        "taskbarStatusOpacity": 80,
        "floatBallOpacity": 20,
        "floatBallGlow": 0,
    });
    let (migrated, changed) = migrate_settings_json(input).unwrap();
    assert!(changed);
    assert_eq!(migrated["taskbarTransparencyPercent"], 100);
    assert_eq!(migrated["floatBallTransparencyPercent"], 25);
    assert_eq!(migrated["floatBallGlowPercent"], 0);
    assert_eq!(migrated["schemaVersion"], SETTINGS_SCHEMA_VERSION);
}

#[test]
fn panel_layout_restores_refresh_when_hidden_or_missing() {
    let mut layout = MenuLayout { order: vec!["quit".into()], hidden: vec!["refresh".into()] };
    normalize_panel_actions(&mut layout);
    assert_eq!(layout.normalized_order(&PANEL_ACTIONS, &["refresh"])[0], "refresh");
}
~~~

- [ ] **Step 2: Run the focused tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml schema_v1_visual_values_scale_once_to_v2 -- --nocapture
cargo test --manifest-path rust/Cargo.toml panel_layout_restores_refresh_when_hidden_or_missing -- --nocapture
~~~

Expected: compile failure because the v2 migration and panel registry do not exist.

- [ ] **Step 3: Implement migration before deserializing AppSettings.**

~~~rust
pub fn migrate_settings_json(mut value: Value) -> Result<(Value, bool), StorageError> {
    let object = value.as_object_mut().ok_or_else(settings_decode_error)?;
    let version = object.get("schemaVersion").and_then(Value::as_u64).unwrap_or(1);
    if version >= u64::from(SETTINGS_SCHEMA_VERSION) {
        return Ok((value, false));
    }
    migrate_percent(object, "taskbarStatusOpacity", "taskbarTransparencyPercent");
    migrate_percent(object, "floatBallOpacity", "floatBallTransparencyPercent");
    migrate_percent(object, "floatBallGlow", "floatBallGlowPercent");
    migrate_panel_layout(object);
    object.insert("schemaVersion".into(), Value::from(SETTINGS_SCHEMA_VERSION));
    Ok((value, true))
}
~~~

SettingsRepository.load, update, and preview_update must decode through this function. Current serialized settings use only v2 fields; legacy tray fields deserialize harmlessly but no longer drive behavior.

- [ ] **Step 4: Define normalized v2 preferences and validate every patch.**

~~~rust
fn validate_percent(value: u8, code: &'static str) -> Result<(), StorageError> {
    (value <= 100).then_some(()).ok_or_else(|| StorageError::new(
        AppErrorKind::StorageFailure, code, "percentage must be between 0 and 100"
    ))
}
~~~

Move panel action normalization to menu_layout.rs; make refresh required and first, while retaining legacy native_tray data only for compatibility.

- [ ] **Step 5: Run focused Rust tests and inspect serialized migration output.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml storage:: -- --nocapture
cargo fmt --all -- --check
~~~

Expected: all storage tests pass; an old JSON fixture loads as schema version 2 and a second load does not rescale its values.

- [ ] **Step 6: Commit the isolated migration.**

~~~powershell
git add rust/src/storage/settings_migration.rs rust/src/storage/settings_repository.rs rust/src/storage/menu_layout.rs rust/src/storage/mod.rs
git commit -m "Migrate presentation settings to v2"
~~~

### Task 2: Align bridge DTOs, tabs, and every range control

**Files:**
- Create: apps/desktop-tauri/src/surfaces/settings/CommittedRangeField.tsx
- Create: apps/desktop-tauri/src/surfaces/settings/CommittedRangeField.test.tsx
- Modify: apps/desktop-tauri/src/hooks/useCommittedRange.ts
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Modify: apps/desktop-tauri/src/lib/tauri.ts
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
- Modify: apps/desktop-tauri/src/lib/surfaceTransparency.ts
- Test: apps/desktop-tauri/src/lib/surfaceTransparency.test.ts

**Interfaces:**
- Produce CommittedRangeField({ id, label, value, min, max, tickValues, valueText, onCommit }).
- onCommit(next: number): Promise<number> persists one acknowledged value per pointer/keyboard interaction boundary.
- Produce DTO fields taskbarTransparencyPercent, floatBallTransparencyPercent, and floatBallGlowPercent.
- Produce surfaceAlphaFromTransparency(value: number): number with 0 -> 1, 100 -> 0, and a monotonic decrease.

- [ ] **Step 1: Write failing conversion and interaction tests.**

~~~tsx
it("maps the full transparency range monotonically", () => {
  expect(surfaceAlphaFromTransparency(0)).toBe(1);
  expect(surfaceAlphaFromTransparency(50)).toBe(0.5);
  expect(surfaceAlphaFromTransparency(100)).toBe(0);
});

it("commits only the final drag value", async () => {
  const onCommit = vi.fn(async (value: number) => value);
  render(<CommittedRangeField id="glow" label="Glow" value={0} min={0} max={100} tickValues={[0, 25, 50, 75, 100]} valueText={(v) => String(v)} onCommit={onCommit} />);
  const input = screen.getByLabelText("Glow");
  fireEvent.input(input, { target: { value: "20" } });
  fireEvent.input(input, { target: { value: "70" } });
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.pointerUp(input);
  await waitFor(() => expect(onCommit).toHaveBeenCalledWith(70));
});
~~~

- [ ] **Step 2: Run focused frontend tests and verify RED.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test -- CommittedRangeField surfaceTransparency TaskbarTrayTab
~~~

Expected: missing component/new DTO fields or old 80-percent expectations fail.

- [ ] **Step 3: Implement the reusable control and bridge conversion.**

~~~tsx
export function CommittedRangeField(props: CommittedRangeFieldProps) {
  const range = useCommittedRange({ value: props.value, min: props.min, max: props.max, onCommit: props.onCommit });
  return <div className="settings-committed-range">{/* label, output, input, ticks */}</div>;
}
~~~

Use the control for taskbar transparency, float transparency, float glow, and custom-skin radius. Remove every direct range persistence path. Update surfaceTransparency.ts and taskbar presentation tests to v2 semantics.

- [ ] **Step 4: Rewrite the two settings tabs around product ownership.**

TaskbarTrayTab becomes the menuBar implementation with title Taskbar & Float Ball / 任务栏与悬浮球. It owns taskbar controls, float visibility/transparency/glow controls, and the full-screen preference; remove every tray icon/tooltip control. GeneralTab keeps only startup, refresh, theme, and language controls.

- [ ] **Step 5: Run focused tests, full frontend tests, and production build.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::settings::tests -- --nocapture
~~~

Expected: all UI controls render a 0..100 scale; stale settings events do not snap an active thumb; rejected patches restore only the failed control.

- [ ] **Step 6: Commit the shared control and DTO migration.**

~~~powershell
git add apps/desktop-tauri/src/hooks/useCommittedRange.ts apps/desktop-tauri/src/surfaces/settings apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/lib/surfaceTransparency.ts apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs
git commit -m "Unify surface range controls"
~~~

### Task 3: Build protected presentation identity and avatar storage

**Files:**
- Create: rust/src/accounts/avatar.rs
- Create: rust/src/accounts/presentation.rs
- Modify: rust/src/accounts/mod.rs
- Modify: rust/src/accounts/identity.rs
- Modify: rust/src/accounts/service.rs
- Modify: rust/src/app_paths.rs
- Modify: rust/src/providers/codex/app_server/model.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/accounts.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src/lib/tauri.ts
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Test: rust/src/accounts/avatar.rs
- Test: rust/src/accounts/presentation.rs
- Test: rust/src/providers/codex/app_server/model.rs

**Interfaces:**
- Produce PresentationIdentity { display_name: String, avatar_kind: AvatarKind, avatar_revision: Option<String> }.
- Produce AvatarKind::{Default, Official, Manual} and AvatarStore scoped to the profile UUID under application data.
- Produce typed commands save_profile_avatar(profile_id, png_data_url) and clear_profile_avatar(profile_id).
- Extend ProfileSummaryDto with presentationName, avatarKind, and avatarAssetUri; never add avatarUrl or a file path.

- [ ] **Step 1: Write failing parser, identity precedence, and avatar-boundary tests.**

~~~rust
#[test]
fn handle_precedes_display_name_and_email_local_part() {
    let identity = presentation_identity(Some("stack"), Some("Stack User"), Some("stack@example.com"), AccountStatus::SignedIn);
    assert_eq!(identity.display_name, "stack");
}

#[test]
fn avatar_url_rejects_http_redirect_and_private_targets() {
    for value in ["http://cdn.openai.com/a.png", "https://127.0.0.1/a.png", "file:///C:/avatar.png"] {
        assert!(validate_official_avatar_url(value).is_err());
    }
}

#[test]
fn manual_avatar_is_scoped_to_its_profile() {
    let store = AvatarStore::for_test(tempdir().unwrap().path());
    store.write_manual(profile_a(), valid_png_bytes()).unwrap();
    assert!(store.asset_for(profile_a()).is_some());
    assert!(store.asset_for(profile_b()).is_none());
}
~~~

- [ ] **Step 2: Run focused Rust tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml accounts::avatar -- --nocapture
cargo test --manifest-path rust/Cargo.toml accounts::presentation -- --nocapture
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model -- --nocapture
~~~

Expected: missing presentation/asset APIs and optional app-server fields.

- [ ] **Step 3: Parse only safe local app-server metadata and persist local assets.**

Add optional username and avatar_candidate extraction to AccountIdentity::from_value; ignore token/cookie fields exactly as today. AccountProfileService::cache_identity resolves the visible name, attempts an approved official avatar download without cookies/redirects, and preserves a manual override. AvatarStore accepts only bounded PNG bytes from the manual command and atomically writes {profile-id}.png.

- [ ] **Step 4: Register a constrained local asset protocol and bridge commands.**

~~~rust
pub fn avatar_asset_uri(profile_id: ProfileId, revision: &str) -> String {
    format!("account-avatar://profile/{profile_id}?rev={revision}")
}
~~~

Register an asynchronous Tauri protocol handler in main.rs; validate the UUID and serve only the corresponding AvatarStore file with image/png. Reject all other path components and query shapes. Add the two typed avatar commands to the invoke handler and TypeScript bridge.

- [ ] **Step 5: Run focused Rust command/protocol tests and format.**

Run:

~~~powershell
cargo test --manifest-path rust/Cargo.toml accounts:: -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::accounts::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge::tests -- --nocapture
cargo fmt --all -- --check
~~~

Expected: profile DTOs contain generated local URIs only; invalid image payload and invalid protocol paths fail without leaving partial files.

- [ ] **Step 6: Commit the Rust identity boundary.**

~~~powershell
git add rust/src/accounts rust/src/app_paths.rs rust/src/providers/codex/app_server/model.rs apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.ts
git commit -m "Add safe presentation identity avatars"
~~~

### Task 4: Render synchronized identity and panel preferences

**Files:**
- Create: apps/desktop-tauri/src/components/AccountAvatar.tsx
- Create: apps/desktop-tauri/src/components/AccountAvatar.test.tsx
- Modify: apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts
- Modify: apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts
- Modify: apps/desktop-tauri/src/surfaces/tray/TrayHeader.tsx
- Modify: apps/desktop-tauri/src/surfaces/tray/TrayHeader.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatusContents.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatusMeasure.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/AccountsTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/AccountsTab.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.test.tsx
- Modify: apps/desktop-tauri/src/surfaces/TrayPanel.tsx
- Modify: apps/desktop-tauri/src/styles.css
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts

**Interfaces:**
- Produce AccountAvatar({ identity, size, decorative }) with Default, Official, and Manual rendering paths.
- Extend StatusSurfaceViewModel and TaskbarStatusPresentation with avatarAssetUri and avatarKind.
- Produce PanelPreferencesDto and apply_panel_preferences through the existing settings patch path.

- [ ] **Step 1: Write failing React tests for privacy and synchronization.**

~~~tsx
it("uses the presentation name and local avatar in the taskbar", () => {
  render(<TaskbarStatusContents mode="visible" presentation={presentation({ displayName: "stack", avatarAssetUri: "account-avatar://profile/a?rev=1" })} />);
  expect(screen.getByTitle("stack")).toBeInTheDocument();
  expect(screen.queryByText("stack@example.com")).not.toBeInTheDocument();
});

it("keeps full email only in the profile selector", () => {
  render(<ProfileSelector profiles={[profile({ accountEmail: "stack@example.com" })]} />);
  expect(screen.getByDisplayValue(/stack@example\.com/)).toBeInTheDocument();
});
~~~

- [ ] **Step 2: Run focused tests and verify RED.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test -- AccountAvatar TrayHeader ProfileSelector TaskbarStatus AccountsTab MenuTab
~~~

Expected: email-first identity and static ChatGPT mark assertions fail.

- [ ] **Step 3: Implement one identity renderer for panel and taskbar.**

Use AccountAvatar in TrayHeader and TaskbarStatusContents. Preserve TaskbarStatusMeasure's use of TaskbarStatusContents; do not create a measurement-only avatar markup variant. ProfileSelector retains the email; AccountsTab, tooltips, and headers use presentation names.

- [ ] **Step 4: Implement panel density/detail/action preferences.**

Render PanelPreferences in TrayPanel: compact/standard density, optional reset/freshness/account-status lines, and the normalized action order. Keep Refresh visible. Rewrite MenuTab copy and UI as Panel / 面板, removing native-tray layout controls.

- [ ] **Step 5: Run all affected frontend tests and the production build.**

Run:

~~~powershell
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
~~~

Expected: no component outside ProfileSelector renders full email; visible and measurement taskbar routes have identical structural snapshots.

- [ ] **Step 6: Commit the shared identity renderer and panel UI.**

~~~powershell
git add apps/desktop-tauri/src/components apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts apps/desktop-tauri/src/surfaces apps/desktop-tauri/src/styles.css
git commit -m "Show account identity across status surfaces"
~~~

### Task 5: Fix the native tray to a single product contract

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/tray_bridge.rs
- Modify: apps/desktop-tauri/src-tauri/src/tray_menu.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/fixed_actions.rs
- Test: apps/desktop-tauri/src-tauri/src/tray_bridge.rs
- Test: apps/desktop-tauri/src-tauri/src/tray_menu.rs
- Test: apps/desktop-tauri/src-tauri/src/commands/settings.rs

**Interfaces:**
- Produce fixed_tray_tooltip(view: &TrayPresentation) -> String.
- Produce FIXED_TRAY_ITEMS: [&str; 6] = ["open_panel", "refresh", "accounts", "open_usage", "settings", "quit"].
- rebuild_tray consumes shared weekly presentation and ignores legacy tray preference values.

- [ ] **Step 1: Write failing native tray tests.**

~~~rust
#[test]
fn fixed_tray_tooltip_contains_username_weekly_reset_and_update() {
    let tooltip = fixed_tray_tooltip(&tray_view("stack", 67, "Aug 31", "just now"));
    assert!(tooltip.contains("stack"));
    assert!(tooltip.contains("67%"));
}

#[test]
fn legacy_monochrome_preference_cannot_change_fixed_dynamic_icon() {
    assert_eq!(tray_icon_band(&legacy_settings(TrayIconMode::Monochrome)), QuotaBand::High);
}
~~~

- [ ] **Step 2: Run focused shell tests and verify RED.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_bridge::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_menu::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::settings::tests -- --nocapture
~~~

Expected: current preference-driven menu/icon code fails the fixed contract.

- [ ] **Step 3: Replace configurable native paths with fixed functions.**

Remove calls that derive tray icon mode, tooltip rows, or order from TaskbarTrayPreferences and MenuPreferences.native_tray. Build the fixed menu only from FIXED_TRAY_ITEMS; derive icon band/tooltip from the shared universal-weekly view model. Preserve existing invoke handlers for those fixed actions; do not add an arbitrary command path.

- [ ] **Step 4: Run focused/full Rust checks and commit.**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
~~~

Commit:

~~~powershell
git add apps/desktop-tauri/src-tauri/src/tray_bridge.rs apps/desktop-tauri/src-tauri/src/tray_menu.rs apps/desktop-tauri/src-tauri/src/commands
git commit -m "Fix native tray presentation"
~~~

### Task 6: Prove identity/settings surfaces on fresh Windows

**Files:**
- Verification evidence only unless this task exposes a defect owned by Tasks 1–5.

**Interfaces:**
- Consume v2 settings, avatar URI, panel preferences, and fixed tray.
- Produce screenshot paths and a concise Windows proof record attached to the implementation ledger.

- [ ] **Step 1: Build and launch a fresh debug binary.**

~~~powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
~~~

Close only the exact previous codex-barbar binary before launching the new one.

- [ ] **Step 2: Use CUA to prove identity privacy.**

Capture signed-out fallback, signed-in avatar/name, manual override, and manual-override removal. Confirm taskbar and panel change together and full email appears only in the panel selector.

- [ ] **Step 3: Use CUA to prove visual controls and fixed tray.**

Drag taskbar transparency, float transparency, and glow from 0 to 100 without snapback; restart and verify persistence. Inspect native tray icon, tooltip, and fixed action order after changing legacy settings values.

- [ ] **Step 4: Restore the user's original visual settings and record proof.**

Record binary path, version, screenshots, settings values before/after, and the result of each acceptance observation. Do not publish or install anything.
