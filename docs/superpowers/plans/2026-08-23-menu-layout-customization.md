# Menu Layout Customization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Menu placeholder with safe visibility and ordering controls for the native tray right-click menu and tray-panel quick actions, with immediate application and deterministic recovery from malformed layouts.

**Architecture:** Store only stable built-in item IDs and visibility/order preferences. A shared Rust normalizer owns registry filtering, duplicate removal, mandatory native item restoration, and default fallback; the frontend never sends commands, scripts, URLs, executable paths, or arbitrary action definitions.

**Tech Stack:** Rust 2024, SQLite-backed V1 settings, Tauri 2 native menu APIs, React 18, TypeScript, Vitest, CUA Driver.

**Spec:** `docs/superpowers/specs/2026-08-23-settings-feature-expansion-design.md`

## Global Constraints

- Configure only the existing native tray menu and existing tray-panel quick actions.
- Do not add arbitrary user commands, shell scripts, command-line arguments, URLs, executable paths, or provider-account actions.
- Native Settings and Quit items are always visible and cannot be reordered behind an empty/unreachable menu state.
- Unknown IDs are ignored; duplicates collapse; empty separators are generated, never persisted.
- Persist only a layout after the candidate native menu applies successfully; restore the prior native menu if persistence then fails.
- Keep the native tray profile submenu behavior and no-logout/no-delete security boundary unchanged.
- Preserve unrelated dirty/untracked files and do not push or release.

## File Structure

| File | Responsibility |
| --- | --- |
| `rust/src/storage/menu_layout.rs` | Stable ID registries, layout structs, defaults, normalization, and validation-free recovery. |
| `rust/src/storage/settings_repository.rs` | Persist `MenuPreferences` inside V1 settings and provide a validated preview before native application. |
| `rust/src/storage/mod.rs` | Export safe layout types/normalizers to the shell. |
| `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` | Menu layout DTOs and patch conversion. |
| `apps/desktop-tauri/src-tauri/src/commands/settings.rs` | Dedicated transactional `apply_menu_preferences` command. |
| `apps/desktop-tauri/src-tauri/src/tray_menu.rs` | Build native items in normalized order and derive separators. |
| `apps/desktop-tauri/src-tauri/src/tray_bridge.rs` | Rebuild from candidate settings and roll back on persistence failure. |
| `apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.tsx` | Visibility list, drag/drop reorder, keyboard order buttons, and restore-default action. |
| `apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx` | Render only configured panel quick actions in normalized order. |
| `apps/desktop-tauri/src/surfaces/TrayPanel.tsx` | Pass `bootstrap.settings.menu` into `TrayActions`. |

---

### Task 1: Define stable menu registries and normalization

**Files:**
- Create: `rust/src/storage/menu_layout.rs`
- Modify: `rust/src/storage/mod.rs:3-20`
- Modify: `rust/src/storage/settings_repository.rs:55-214`
- Test: `rust/src/storage/menu_layout.rs`
- Test: `rust/src/storage/settings_repository.rs`

**Interfaces:**
- Produces:

```rust
pub const NATIVE_TRAY_ITEMS: [&str; 7] = [
    "open_panel", "refresh", "accounts", "open_usage", "settings", "about", "quit",
];
pub const TRAY_PANEL_ACTIONS: [&str; 5] = [
    "refresh", "open_usage", "settings", "dismiss", "quit",
];
pub const REQUIRED_NATIVE_TRAY_ITEMS: [&str; 2] = ["settings", "quit"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MenuLayout { pub order: Vec<String>, pub hidden: Vec<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MenuPreferences { pub native_tray: MenuLayout, pub tray_panel: MenuLayout }

pub fn normalize_layout(
    layout: &MenuLayout,
    registry: &[&str],
    required_visible: &[&str],
) -> Vec<String>;
```

- [ ] **Step 1: Write failing normalization tests**

Add exact cases:

```rust
assert_eq!(
    normalize_layout(
        &MenuLayout { order: vec!["quit".into(), "unknown".into(), "refresh".into(), "refresh".into()], hidden: vec!["settings".into(), "refresh".into()] },
        &NATIVE_TRAY_ITEMS,
        &REQUIRED_NATIVE_TRAY_ITEMS,
    ),
    vec!["quit", "settings", "open_panel", "accounts", "open_usage", "about"],
);
```

Also test empty layout restores the default registry order, hidden unknown IDs are harmless, mandatory native IDs are visible, and a tray-panel layout has no mandatory Quit/Settings requirement unless explicitly specified by the registry.

- [ ] **Step 2: Run focused Rust tests and verify RED**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::menu_layout::tests -- --nocapture
```

Expected: module-not-found failure.

- [ ] **Step 3: Implement defaults and pure normalization**

Use `Vec<String>` deliberately so older/newer binaries can read unknown IDs without serde failure. Normalization must:

1. retain first occurrence of known ordered IDs that are not hidden;
2. append known registry IDs omitted from `order` unless hidden;
3. append required IDs even if hidden;
4. deduplicate while retaining first valid placement;
5. fall back to the default visible order if result is empty.

Do not store separators or create a generic action enum that can represent external commands.

- [ ] **Step 4: Add settings persistence tests**

Add `menu: MenuPreferences` to `AppSettings` with both defaults, then test old JSON migrates to default layouts and partial menu patch preserves the peer surface layout.

- [ ] **Step 5: Run tests and verify GREEN**

Run the focused normalizer and settings tests:

```powershell
cargo test --manifest-path rust/Cargo.toml storage::menu_layout::tests storage::settings_repository::tests -- --nocapture
```

- [ ] **Step 6: Commit the storage slice**

```powershell
git add rust/src/storage/menu_layout.rs rust/src/storage/mod.rs rust/src/storage/settings_repository.rs
git commit -m "Add normalized menu layout preferences"
```

### Task 2: Add a candidate-apply native menu transaction

**Files:**
- Modify: `rust/src/storage/settings_repository.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs:34-154`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/settings.rs:13-115`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_menu.rs:1-163`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs:191-286`
- Test: `apps/desktop-tauri/src-tauri/src/tray_menu.rs`
- Test: `apps/desktop-tauri/src-tauri/src/commands/settings.rs`

**Interfaces:**
- Adds a non-writing settings preview:

```rust
impl SettingsRepository {
    pub fn preview_update(&self, patch: SettingsPatch) -> Result<AppSettings, StorageError>;
}
```

- Adds the focused command:

```rust
#[tauri::command]
pub fn apply_menu_preferences(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    preferences: MenuPreferencesPatchDto,
) -> Result<AppSettingsDto, String>;
```

- Changes `build_native_menu` to accept the normalized native order:

```rust
pub fn build_native_menu<R: Runtime>(
    app: &AppHandle<R>,
    profiles: &[TrayProfileMenuItem],
    language: &str,
    order: &[String],
) -> tauri::Result<Menu<R>>;
```

- [ ] **Step 1: Write failing candidate/rollback tests**

Add tests proving preview does not write to SQLite, native menu order follows a normalized candidate, and malformed candidate entries cannot remove Settings/Quit.

Add a shell-level fake rebuild path test:

```rust
let old = AppSettings::default();
let candidate = repository.preview_update(menu_patch)?;
assert_ne!(candidate.menu.native_tray.order, old.menu.native_tray.order);
assert_eq!(repository.load()?, old); // preview has no persistence side effect
```

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tray_menu::tests commands::settings::tests -- --nocapture
```

Expected: missing preview/command/signature assertions fail.

- [ ] **Step 3: Implement preflight, native apply, save, and rollback**

`apply_menu_preferences` must follow this order:

```text
deserialize patch -> repository.preview_update -> tray_bridge::apply_candidate_menu
-> repository.update -> emit settings-changed
```

If `apply_candidate_menu` fails, return `MENU_APPLY_FAILED` without writing. If persistence fails after a successful native apply, call `tray_bridge::apply_candidate_menu` with the prior settings to restore the old menu and return `SETTINGS_SAVE_FAILED`. The settings event is emitted only after persistence succeeds.

`tray_menu::build_native_menu` iterates normalized IDs and builds only corresponding built-in `MenuItem`/`Submenu` values. Generate separator placement from neighboring visible group IDs; do not save separator IDs.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command. Confirm Settings/Quit remain visible, unknown items are absent, order changes deterministically, and an unapplied candidate did not reach storage.

- [ ] **Step 5: Commit the transactional native path**

```powershell
git add rust/src/storage/settings_repository.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src-tauri/src/tray_menu.rs apps/desktop-tauri/src-tauri/src/tray_bridge.rs
git commit -m "Apply tray menu layouts safely"
```

### Task 3: Add the Menu settings UI and panel-action rendering

**Files:**
- Modify: `apps/desktop-tauri/src/types/bridge.ts:37-64`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts:17-80`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.tsx`
- Create: `apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.tsx:15-25,143-178`
- Modify: `apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts`
- Modify: `apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx:1-47`
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayActions.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.tsx:98-106`

**Interfaces:**
- Produces frontend data types:

```ts
export interface MenuLayoutDto { order: string[]; hidden: string[]; }
export interface MenuPreferencesDto { nativeTray: MenuLayoutDto; trayPanel: MenuLayoutDto; }
export interface MenuLayoutPatchDto { order?: string[]; hidden?: string[]; }
export interface MenuPreferencesPatchDto { nativeTray?: MenuLayoutPatchDto; trayPanel?: MenuLayoutPatchDto; }
export const applyMenuPreferences = (preferences: MenuPreferencesPatchDto) =>
  invoke<AppSettingsDto>(commands.applyMenuPreferences, { preferences });
```

- `TrayActions` accepts `order: readonly string[]` and renders only known action IDs.

- [ ] **Step 1: Write failing MenuTab and TrayActions tests**

Test visibility, drag/drop, keyboard reorder, restore defaults, and safe required-item messaging. For keyboard controls:

```tsx
await user.click(screen.getByRole("button", { name: /move refresh down/i }));
expect(applyMenuPreferences).toHaveBeenCalledWith({
  nativeTray: { order: ["open_panel", "accounts", "refresh", "open_usage", "settings", "about", "quit"] },
});
```

For panel actions, provide an order that hides `dismiss` and assert no dismiss button is rendered, while Refresh/Usage/Settings/Quit handlers remain bound only to their existing callbacks.

- [ ] **Step 2: Run focused frontend tests and verify RED**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- MenuTab TrayActions Settings
```

Expected: missing tab/component/prop failures.

- [ ] **Step 3: Implement accessible layout editors**

Render two independent lists with checkbox visibility, HTML drag-and-drop reordering, and `Move up` / `Move down` buttons. Every row uses a stable label from localized built-in metadata; no free-text field exists.

When a required native item is selected, render it as checked and disabled with helper copy. Restore defaults calls the dedicated command with the exact default `MenuLayoutDto`; never reset unrelated settings.

- [ ] **Step 4: Render tray-panel actions from normalized layout**

Map only `refresh`, `open_usage`, `settings`, `dismiss`, and `quit` to existing callbacks. Ignore unknown IDs defensively even though Rust normalizes storage. Preserve keyboard auto-focus behavior by assigning it to the first visible actionable button.

- [ ] **Step 5: Run tests and verify GREEN**

Run the same focused test command. Confirm drag and keyboard paths emit equivalent normalized order and no custom command field is present.

- [ ] **Step 6: Commit the UI slice**

```powershell
git add apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/MenuTab.test.tsx apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx apps/desktop-tauri/src/surfaces/tray/TrayActions.test.tsx apps/desktop-tauri/src/surfaces/TrayPanel.tsx
git commit -m "Customize tray and panel menus"
```

### Task 4: Verify menu layouts on Windows

**Files:**
- Verify only unless a task-owned defect is found.

**Interfaces:**
- Consumes Tasks 1–3.
- Produces automated and native Windows proof for both configured menu surfaces.

- [ ] **Step 1: Run automated validation**

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

- [ ] **Step 2: Fresh-build the desktop application**

Resolve and close only the exact running codex-barbar binary, then run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Launch the new binary with `CODEXBAR_PROOF_MODE=settings:menu`.

- [ ] **Step 3: Use CUA to verify menu behavior**

Capture and assert:

- changing order in Settings updates the native right-click menu immediately;
- a hidden eligible item disappears from the native menu and panel action strip;
- Settings and Quit remain reachable in the native menu;
- restore defaults restores both lists without restarting;
- keyboard move buttons work without drag-and-drop;
- app restart retains layouts;
- malformed saved IDs recover without startup failure.

- [ ] **Step 4: Scope and safety audit**

Verify no UI accepts an arbitrary executable, script, path, URL, or account-mutation action. Check `git status --short` and do not push/release.
