# codex-barbar V1 Phase 0 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the imported Win-CodexBar history into a branded, Codex-only, buildable `codex-barbar` desktop baseline without carrying legacy product capabilities into the active release graph.

**Architecture:** Preserve the shared Rust crate and proven Tauri window/tray shell, but introduce one canonical application-path boundary, one shipping-provider registry, a minimal V1 command registry, and minimal tray/settings React routes. Legacy source may remain temporarily for history and later deletion, but it must be unreachable from the compiled desktop entry point, invoke bridge, Vite import graph, capabilities, and packaging configuration.

**Tech Stack:** Tauri 2, React 18, TypeScript 5.6, Rust stable edition 2024, serde, existing tray/window modules, Vitest 3, PowerShell 5.1+.

## Global Constraints

- Supported platform is native Windows 11 23H2 or newer on x64; Windows 10, Windows on ARM, WSL, macOS, and Linux builds are outside V1.
- The shipping product supports Codex only. Claude, Gemini, every other provider, browser-cookie import, generic API-key/token accounts, cost charts, sessions, workspaces, PTY, FloatBar, PopOut, and usage notifications stay outside the release surface.
- Account and quota network access goes only through the official but experimental `codex app-server` stdio JSONL process; `experimentalApi` remains `false`, and private `/wham/*` calls are removed without a release fallback.
- `CurrentCli` is read-only: it may call `initialize`, send `initialized`, call `account/read`, and call `account/rateLimits/read`; it has no login, logout, delete, switch, or configuration-write method.
- Every `Managed` profile uses an isolated `CODEX_HOME`, forces `cli_auth_credentials_store = "file"`, clears authentication override variables in the child only, and keeps idle credentials in strict DPAPI Current User ciphertext.
- DPAPI failure is fatal for the vault operation. There is no Local Machine scope fallback and no plaintext fallback. Vault replacement is temporary-file, flush, then atomic replace.
- The React WebView receives no OAuth token, refresh token, raw `auth.json`, arbitrary filesystem capability, arbitrary shell/process capability, or arbitrary network capability.
- Startup does not check for, download, or apply updates. A user-initiated action may check a public GitHub Release or open the fixed Releases page; no PAT is embedded or requested.
- Default refresh interval is 5 minutes; panel-open refresh threshold is 60 seconds; manual cooldown is 15 seconds; transient backoff is 30 seconds, 2 minutes, 5 minutes, then 15 minutes with ±20% jitter.
- Product defaults are remaining-quota display, system theme, system language, autostart off, no telemetry, and `%LOCALAPPDATA%\codex-barbar` storage for installed and portable builds.
- Toolchain is pnpm 10.18.1, Node 20, Rust stable edition 2024, and target `x86_64-pc-windows-msvc`. Use pnpm only.
- Do not introduce a new third-party crate, package, build tool, hosted service, or telemetry endpoint without explicit user confirmation. Additional features on the already-present `windows` crate must be limited to the Win32 APIs named in the approved design.
- Preserve the complete Win-CodexBar Git history, MIT license, author attribution, `win-upstream`, `mac-reference`, and tag `upstream/win-codexbar-2026-08-03`.
- Every UI/tray/settings change requires a fresh desktop build, termination of the old single instance, and real Windows CUA evidence as described by `AGENTS.md`.
- Do not push, publish a GitHub Release, open a pull request, buy a signing certificate, or contact either upstream repository unless the user explicitly authorizes that external action.

---

## File Responsibility Map

| Path | Responsibility after Phase 0 |
|---|---|
| `UPSTREAMS.md` | Immutable source, baseline, license, reuse, and sync record |
| `rust/src/app_paths.rs` | One `%LOCALAPPDATA%\codex-barbar` path derivation boundary |
| `rust/src/core/provider.rs` | Existing provider identities plus a Codex-only shipping registry |
| `rust/src/core/provider_factory.rs` | Reject every non-Codex runtime instantiation |
| `apps/desktop-tauri/src-tauri/src/commands/app.rs` | Minimal non-secret bootstrap DTO |
| `apps/desktop-tauri/src-tauri/src/commands/window.rs` | Fixed tray/settings window actions and one trusted quit entry |
| `apps/desktop-tauri/src-tauri/src/commands/mod.rs` | Re-export only the active V1 command modules |
| `apps/desktop-tauri/src-tauri/src/main.rs` | Single-instance Tauri entry with minimal command/module graph |
| `apps/desktop-tauri/src-tauri/tauri.conf.json` | Product identity, one hidden tray window, NSIS target, production CSP |
| `apps/desktop-tauri/src-tauri/capabilities/default.json` | Main/settings windows with core event/window permissions only |
| `apps/desktop-tauri/src/types/bridge.ts` | Phase-0 bootstrap and surface DTOs only |
| `apps/desktop-tauri/src/lib/tauri.ts` | Phase-0 invoke allowlist only |
| `apps/desktop-tauri/src/App.tsx` | Route only tray panel and settings surfaces |
| `apps/desktop-tauri/src/surfaces/TrayPanel.tsx` | Honest connection-pending tray placeholder |
| `apps/desktop-tauri/src/surfaces/Settings.tsx` | Identity/build-information settings placeholder |
| `scripts/assert-v1-boundaries.ps1` | Static release-graph guard for forbidden legacy commands/capabilities |

## Test Support Contract

- Rust tests in this phase use inline `#[cfg(test)]` modules. Path tests call `AppPaths::from_local_app_data` with a fixed synthetic base and never read or write the real LocalAppData tree.
- Frontend tests use the existing `apps/desktop-tauri/src/test/setup.ts` for `invokeMock`, DOM cleanup, and Tauri stubs; every test resets calls/listeners in `afterEach`.
- Manifest/source-shape tests read tracked repository files with `include_str!` or an explicit repository-relative path and assert exact values; they do not modify those files.
- CUA proof mode uses only the Phase-0 synthetic not-connected state and never resolves Codex or opens an account/Vault.
- Every helper referenced below is defined in one of these named locations before its first test is compiled and is not production-exported.

### Task 1: Record provenance and baseline invariants

**Files:**
- Create: `UPSTREAMS.md`
- Create: `docs/architecture/V1_BASELINE.md`
- Modify: `README.md`
- Test: PowerShell assertions executed from the repository root

**Interfaces:**
- Consumes: Git remotes `origin`, `win-upstream`, `mac-reference`; tag `upstream/win-codexbar-2026-08-03`.
- Produces: a human-auditable source record used by Phase 4 license/SBOM checks.

- [ ] **Step 1: Run the failing provenance assertions**

```powershell
$required = @('UPSTREAMS.md', 'docs/architecture/V1_BASELINE.md')
$missing = $required | Where-Object { -not (Test-Path -LiteralPath $_) }
if ($missing) { throw "Missing provenance files: $($missing -join ', ')" }
```

Expected: FAIL with `Missing provenance files`.

- [ ] **Step 2: Verify the imported history before documenting it**

```powershell
git merge-base --is-ancestor b167e328147b93f997034a6b50c8b769d2a37f3b HEAD
git rev-parse upstream/win-codexbar-2026-08-03
git remote get-url origin
git remote get-url win-upstream
git remote get-url mac-reference
```

Expected: the first command exits `0`; the tag resolves to `b167e328147b93f997034a6b50c8b769d2a37f3b`; the three URLs match the approved Spec.

- [ ] **Step 3: Add the exact source record and replace README product claims**

Write `UPSTREAMS.md` with this table and the existing MIT attribution:

```markdown
# Upstream sources

| Role | Repository | Frozen baseline |
|---|---|---|
| Windows implementation base | https://github.com/Finesssee/Win-CodexBar | `b167e328147b93f997034a6b50c8b769d2a37f3b` / `upstream/win-codexbar-2026-08-03` |
| Behavior reference | https://github.com/steipete/CodexBar | Reference only; no Swift platform code is shipped |

codex-barbar preserves the imported Win-CodexBar Git history and MIT license. V1 reuses its Tauri, React, Rust, Windows tray, testing, and packaging foundations while replacing the Codex private HTTP integration and removing non-Codex product surfaces from the release graph.
```

Write `docs/architecture/V1_BASELINE.md` with the approved platform, Codex-only scope, upstream sync procedure (`git fetch win-upstream --tags` followed by an explicit reviewed merge), and the rule that no upstream issue or pull request is created without user authorization. Rewrite the README title, repository URLs, install status, and V1 scope so it does not advertise Winget, other providers, the old CLI, or completed release artifacts.

- [ ] **Step 4: Re-run the provenance and copy audit**

```powershell
$text = Get-Content -Raw -Encoding utf8 UPSTREAMS.md
foreach ($needle in @('Finesssee/Win-CodexBar', 'steipete/CodexBar', 'b167e328147b93f997034a6b50c8b769d2a37f3b', 'MIT')) {
    if (-not $text.Contains($needle)) { throw "UPSTREAMS.md missing $needle" }
}
if ((Get-Content -Raw -Encoding utf8 README.md) -match 'winget install|supports .*Claude|CodexBar-[0-9]+\.[0-9]+\.[0-9]+-portable\.exe') {
    throw 'README still advertises a legacy release path'
}
```

Expected: PASS with no output.

- [ ] **Step 5: Commit**

```powershell
git add UPSTREAMS.md README.md docs/architecture/V1_BASELINE.md
git commit -m "Document codex-barbar upstream baseline"
```

### Task 2: Establish canonical V1 data paths

**Files:**
- Create: `rust/src/app_paths.rs`
- Modify: `rust/src/lib.rs`
- Test: `rust/src/app_paths.rs` inline tests

**Interfaces:**
- Consumes: a trusted `%LOCALAPPDATA%` base from `dirs::data_local_dir()` in production.
- Produces: `AppPaths::discover() -> Result<AppPaths, AppPathError>` and `AppPaths::from_local_app_data(base: &Path) -> AppPaths`.

- [ ] **Step 1: Write the failing path-layout test**

```rust
#[test]
fn derives_every_v1_path_from_local_app_data() {
    let paths = AppPaths::from_local_app_data(Path::new(r"C:\Users\A\AppData\Local"));
    assert_eq!(paths.root, PathBuf::from(r"C:\Users\A\AppData\Local\codex-barbar"));
    assert_eq!(paths.database, paths.root.join(r"data\codex-barbar.db"));
    assert_eq!(paths.vault, paths.root.join("vault"));
    assert_eq!(paths.runtime, paths.root.join("runtime"));
    assert_eq!(paths.logs, paths.root.join("logs"));
}
```

- [ ] **Step 2: Run the focused test and verify the missing module failure**

```powershell
cargo test --manifest-path rust/Cargo.toml derives_every_v1_path_from_local_app_data
```

Expected: FAIL because `app_paths`/`AppPaths` does not exist.

- [ ] **Step 3: Implement the focused path boundary**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub vault: PathBuf,
    pub runtime: PathBuf,
    pub logs: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathError> {
        let base = dirs::data_local_dir().ok_or(AppPathError::LocalAppDataUnavailable)?;
        Ok(Self::from_local_app_data(&base))
    }

    pub fn from_local_app_data(base: &Path) -> Self {
        let root = base.join("codex-barbar");
        Self {
            database: root.join("data").join("codex-barbar.db"),
            vault: root.join("vault"),
            runtime: root.join("runtime"),
            logs: root.join("logs"),
            root,
        }
    }
}
```

Export it with `pub mod app_paths;` from `rust/src/lib.rs`. Do not create directories in `from_local_app_data`; Phase 2 creates them with the correct security semantics.

- [ ] **Step 4: Run focused and shared-crate regressions**

```powershell
cargo test --manifest-path rust/Cargo.toml derives_every_v1_path_from_local_app_data
cargo test --manifest-path rust/Cargo.toml
```

Expected: both commands exit `0`.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/app_paths.rs rust/src/lib.rs
git commit -m "Add canonical codex-barbar data paths"
```

### Task 3: Rebrand manifests, executable, window, and assets

**Files:**
- Modify: `rust/Cargo.toml`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `Cargo.lock`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/settings_window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/flyout_window.rs`
- Create: `rust/icons/codex-barbar.ico`
- Create: `rust/icons/codex-barbar.png`
- Modify: `scripts/dev.ps1`
- Test: `apps/desktop-tauri/src-tauri/src/main.rs` inline tests

**Interfaces:**
- Consumes: the existing MIT-licensed tray icon pixels and Tauri shell.
- Produces: initial product version `0.1.0-alpha.1`, package `codex-barbar-desktop`, GUI binary `codex-barbar.exe`, identifier/AppUserModelID `com.naipi11.codexbarbar`, and window/product title `codex-barbar`.

- [ ] **Step 1: Add a failing Tauri configuration identity test**

```rust
#[test]
fn tauri_config_has_v1_identity() {
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["productName"], "codex-barbar");
    assert_eq!(config["identifier"], "com.naipi11.codexbarbar");
    assert_eq!(config["app"]["windows"][0]["title"], "codex-barbar");
    assert_eq!(config["bundle"]["targets"], serde_json::json!(["nsis"]));
    assert_eq!(config["version"], "0.1.0-alpha.1");
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0-alpha.1");
}
```

- [ ] **Step 2: Run the test and verify the old identity failure**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tauri_config_has_v1_identity
```

Expected: FAIL because the current product is `CodexBar Desktop` / `com.codexbar.desktop`.

- [ ] **Step 3: Apply the exact identity values**

Add an explicit desktop binary target:

```toml
[[bin]]
name = "codex-barbar"
path = "src/main.rs"
```

Set the Tauri crate package name to `codex-barbar-desktop`. Set `rust/Cargo.toml`, the Tauri crate, `apps/desktop-tauri/package.json`, and `tauri.conf.json` to the same initial version `0.1.0-alpha.1`, then regenerate `Cargo.lock` using Cargo; do not edit lockfile contents manually.

Set the Tauri values:

```json
{
  "productName": "codex-barbar",
  "identifier": "com.naipi11.codexbarbar",
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": [
      "../../../rust/icons/codex-barbar.ico",
      "../../../rust/icons/codex-barbar.png"
    ]
  }
}
```

Copy the existing icon bytes into the two new tracked paths, then update every active manifest/window/dev-script reference. Set Rust package descriptions and repository URLs to `https://github.com/naipi11/codex-barbar`; do not rename the shared `codexbar` crate in this phase because that would create broad import-only churn.

- [ ] **Step 4: Verify the identity and binary name**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml tauri_config_has_v1_identity
pnpm --dir apps/desktop-tauri run tauri:build:debug
if (-not (Test-Path 'target\debug\codex-barbar.exe') -and -not (Test-Path 'target\x86_64-pc-windows-msvc\debug\codex-barbar.exe')) {
    throw 'codex-barbar.exe was not produced'
}
```

Expected: test passes and exactly one expected target layout contains `codex-barbar.exe`.

- [ ] **Step 5: Commit**

```powershell
git add rust/Cargo.toml Cargo.lock rust/icons apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/src-tauri/tauri.conf.json apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/shell/settings_window.rs apps/desktop-tauri/src-tauri/src/shell/flyout_window.rs scripts/dev.ps1
git commit -m "Rebrand desktop shell as codex-barbar"
```

### Task 4: Make Codex the only shipping provider

**Files:**
- Modify: `rust/src/core/provider.rs`
- Modify: `rust/src/core/provider_factory.rs`
- Modify: `rust/src/core/mod.rs`
- Modify: `rust/src/providers/mod.rs`
- Modify: `rust/src/settings.rs`
- Modify: `rust/src/settings/tests.rs`
- Test: `rust/src/core/provider_factory.rs` inline tests

**Interfaces:**
- Consumes: existing `ProviderId::Codex` and `CodexProvider`.
- Produces: `shipping_provider_ids() -> &'static [ProviderId]` and `instantiate_shipping_provider(ProviderId) -> Result<Box<dyn Provider>, ProviderError>`, which rejects every non-Codex ID.

- [ ] **Step 1: Write failing shipping-registry tests**

```rust
#[test]
fn v1_shipping_registry_contains_only_codex() {
    assert_eq!(shipping_provider_ids(), &[ProviderId::Codex]);
}

#[test]
fn factory_rejects_non_codex_provider() {
    let error = instantiate_shipping_provider(ProviderId::Claude).unwrap_err();
    assert!(matches!(error, ProviderError::UnsupportedProvider(_)));
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

```powershell
cargo test --manifest-path rust/Cargo.toml v1_shipping_registry_contains_only_codex
cargo test --manifest-path rust/Cargo.toml factory_rejects_non_codex_provider
```

Expected: first test does not compile because the registry is absent; the old factory still instantiates Claude.

- [ ] **Step 3: Add the shipping registry and fail-closed factory gate**

```rust
pub const fn shipping_provider_ids() -> &'static [ProviderId] {
    &[ProviderId::Codex]
}

pub fn instantiate_shipping_provider(id: ProviderId) -> Result<Box<dyn Provider>, ProviderError> {
    if id != ProviderId::Codex {
        return Err(ProviderError::UnsupportedProvider(id.cli_name().to_owned()));
    }
    Ok(Box::new(CodexProvider::new()))
}
```

Add `ProviderError::UnsupportedProvider(String)` and re-export the new shipping factory from `core/mod.rs`. Normalize `Settings.enabled_providers` and `provider_order` to `codex` only when the desktop loads settings from the new product path. Keep the existing non-shipping `instantiate_provider` only for historical shared-crate/CLI tests until Phase 4 removes the old CLI and collapses the final compiled factory to Codex; the desktop command graph introduced in Task 5 must never call it.

- [ ] **Step 4: Run focused and Rust regression suites**

```powershell
cargo test --manifest-path rust/Cargo.toml v1_shipping_registry_contains_only_codex
cargo test --manifest-path rust/Cargo.toml factory_rejects_non_codex_provider
cargo test --manifest-path rust/Cargo.toml
```

Expected: all commands exit `0`; tests that previously required non-Codex factory instantiation are rewritten to exercise their provider modules directly or are removed when they test retired product behavior.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/core/provider.rs rust/src/core/provider_factory.rs rust/src/core/mod.rs rust/src/providers/mod.rs rust/src/settings.rs rust/src/settings/tests.rs
git commit -m "Restrict the shipping provider registry to Codex"
```

### Task 5: Replace the desktop entry with the minimal V1 command and module graph

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/commands/app.rs`
- Create: `apps/desktop-tauri/src-tauri/src/commands/window.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/state.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/surface.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/surface_target.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/shell/transition.rs`
- Test: `apps/desktop-tauri/src-tauri/src/commands/app.rs` inline tests
- Test: `apps/desktop-tauri/src-tauri/src/main.rs` inline tests

**Interfaces:**
- Consumes: retained single-instance, tray panel, settings window, proof harness, and shell positioning modules.
- Produces: `BootstrapDto`, `get_bootstrap_state`, and `commands/window.rs` with exactly `open_settings_window`, `close_settings_window`, `dismiss_tray_panel`, `set_flyout_size`, `get_current_surface_state`, and `quit_app`; the Phase-0 `invoke_handler` contains only those names plus `get_bootstrap_state`.

- [ ] **Step 1: Write failing bootstrap and surface tests**

```rust
#[test]
fn bootstrap_is_branded_and_secret_free() {
    let dto = BootstrapDto::phase_zero(env!("CARGO_PKG_VERSION"));
    let json = serde_json::to_value(dto).unwrap();
    assert_eq!(json["productName"], "codex-barbar");
    assert_eq!(json["connectionStatus"], "notConnected");
    let text = json.to_string().to_ascii_lowercase();
    for forbidden in ["access_token", "refresh_token", "auth_json", "cookie", "api_key"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn v1_surface_modes_are_only_hidden_tray_and_settings() {
    assert_eq!(SurfaceMode::ALL, &[SurfaceMode::Hidden, SurfaceMode::TrayPanel, SurfaceMode::Settings]);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml bootstrap_is_branded_and_secret_free
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml v1_surface_modes_are_only_hidden_tray_and_settings
```

Expected: FAIL because `BootstrapDto::phase_zero` and `SurfaceMode::ALL` do not exist and legacy modes are active.

- [ ] **Step 3: Implement the minimal active graph**

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDto {
    pub product_name: &'static str,
    pub version: String,
    pub connection_status: &'static str,
}

impl BootstrapDto {
    fn phase_zero(version: impl Into<String>) -> Self {
        Self {
            product_name: "codex-barbar",
            version: version.into(),
            connection_status: "notConnected",
        }
    }
}

#[tauri::command]
pub fn get_bootstrap_state() -> BootstrapDto {
    BootstrapDto::phase_zero(env!("CARGO_PKG_VERSION"))
}
```

Remove module declarations and setup calls for `auto_refresh`, `coding_activity`, `floatbar`, `powertoys`, `shortcut_bridge`, and legacy provider/event command modules. Keep the proven single-instance plugin and left-click tray shell. Make ordinary desktop launch target `TrayPanel`, not `PopOut`. `quit_app` must only stop the currently active shell in this phase; Phase 2 adds bounded profile sealing and Phase 4 adds final shutdown orchestration.

- [ ] **Step 4: Run the focused tests and shell regression**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml bootstrap_is_branded_and_secret_free
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml v1_surface_modes_are_only_hidden_tray_and_settings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
```

Expected: all commands exit `0`; no compiled module references FloatBar or PopOut.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/state.rs apps/desktop-tauri/src-tauri/src/surface.rs apps/desktop-tauri/src-tauri/src/surface_target.rs apps/desktop-tauri/src-tauri/src/shell/mod.rs apps/desktop-tauri/src-tauri/src/shell/transition.rs
git commit -m "Reduce the desktop backend to the V1 surface"
```

### Task 6: Replace the React graph with honest Phase-0 tray and settings surfaces

**Files:**
- Replace: `apps/desktop-tauri/src/App.tsx`
- Replace: `apps/desktop-tauri/src/App.test.tsx`
- Replace: `apps/desktop-tauri/src/types/bridge.ts`
- Replace: `apps/desktop-tauri/src/types/bridge.test.ts`
- Replace: `apps/desktop-tauri/src/lib/tauri.ts`
- Replace: `apps/desktop-tauri/src/surfaces/TrayPanel.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/Settings.tsx`
- Replace: `apps/desktop-tauri/src/surfaces/Settings.test.ts`
- Modify: `apps/desktop-tauri/src/main.tsx`
- Modify: `apps/desktop-tauri/src/styles.css`

**Interfaces:**
- Consumes: `get_bootstrap_state`, `open_settings_window`, `close_settings_window`, `dismiss_tray_panel`, and `quit_app`.
- Produces: a Vite graph with only tray/settings product surfaces and no startup network/update call.

- [ ] **Step 1: Write failing frontend scope tests**

```tsx
it("renders an honest not-connected state without starting an update check", async () => {
  render(<App />);
  expect(await screen.findByRole("heading", { name: "codex-barbar" })).toBeInTheDocument();
  expect(screen.getByText("Codex connection is not configured yet.")).toBeInTheDocument();
  expect(invokeMock).not.toHaveBeenCalledWith("check_for_updates", expect.anything());
  expect(invokeMock).not.toHaveBeenCalledWith("download_update", expect.anything());
});

it("exports only the phase-zero bridge", () => {
  expect(Object.keys(tauriBridge).sort()).toEqual([
    "closeSettingsWindow",
    "dismissTrayPanel",
    "getBootstrapState",
    "openSettingsWindow",
    "quitApp",
  ]);
});
```

- [ ] **Step 2: Run the tests and verify the legacy graph failure**

```powershell
pnpm --dir apps/desktop-tauri test -- src/App.test.tsx src/types/bridge.test.ts
```

Expected: FAIL because the old App starts updater/provider hooks and the bridge exports legacy commands.

- [ ] **Step 3: Implement the minimal surfaces and bridge**

```ts
export interface BootstrapDto {
  productName: "codex-barbar";
  version: string;
  connectionStatus: "notConnected";
}

export const getBootstrapState = () => invoke<BootstrapDto>("get_bootstrap_state");
export const openSettingsWindow = () => invoke<void>("open_settings_window");
export const closeSettingsWindow = () => invoke<void>("close_settings_window");
export const dismissTrayPanel = () => invoke<void>("dismiss_tray_panel");
export const quitApp = () => invoke<void>("quit_app");
```

Route by Tauri window label: `main` renders `TrayPanel`; `settings` renders `Settings`; any other label renders nothing and logs no data. The placeholder must say the connection is not configured, not show a fake percentage. Remove imports of provider grids, charts, sessions, updates, FloatBar, PopOut, token accounts, cookies, API keys, and generic external actions from the active Vite graph.

- [ ] **Step 4: Run frontend tests and production build**

```powershell
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
```

Expected: both commands exit `0`; Vite reports no unresolved legacy imports.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop-tauri/src/App.tsx apps/desktop-tauri/src/App.test.tsx apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/surfaces/TrayPanel.tsx apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/Settings.test.ts apps/desktop-tauri/src/main.tsx apps/desktop-tauri/src/styles.css
git commit -m "Replace the frontend with the Codex-only V1 shell"
```

### Task 7: Lock down capabilities, CSP, and the forbidden-surface guard

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/capabilities/default.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Create: `scripts/assert-v1-boundaries.ps1`
- Modify: `scripts/local-check.ps1`
- Test: `scripts/assert-v1-boundaries.ps1`

**Interfaces:**
- Consumes: Phase-0 Tauri and frontend entry graphs.
- Produces: a repeatable static gate that rejects forbidden invoke names, window labels, plugins, network origins, and private Codex endpoints in active release files.

- [ ] **Step 1: Create the guard with one deliberate failing assertion**

The script must scan these active files only:

```powershell
$activeFiles = @(
  'apps/desktop-tauri/src-tauri/src/main.rs',
  'apps/desktop-tauri/src-tauri/src/commands/mod.rs',
  'apps/desktop-tauri/src/lib/tauri.ts',
  'apps/desktop-tauri/src/App.tsx',
  'apps/desktop-tauri/src-tauri/capabilities/default.json',
  'apps/desktop-tauri/src-tauri/tauri.conf.json'
)
$forbidden = @(
  'download_update', 'apply_update', 'open_external_url', 'open_path',
  'manual_cookie', 'api_key', 'token_account', 'floatbar', 'PopOut',
  'global-shortcut', 'http://localhost', 'ws://localhost', '/wham/'
)
foreach ($path in $activeFiles) {
  $text = Get-Content -Raw -Encoding utf8 -LiteralPath $path
  foreach ($needle in $forbidden) {
    if ($text.Contains($needle)) { throw "$path contains forbidden release token: $needle" }
  }
}
```

- [ ] **Step 2: Run it before capability cleanup**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
```

Expected: FAIL on at least the current localhost CSP or global-shortcut capability.

- [ ] **Step 3: Remove excess permissions and make the production CSP exact**

Set the production CSP to:

```text
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; frame-ancestors 'none'
```

Limit capability windows to `main` and `settings`; remove `global-shortcut` permissions and the `tauri-plugin-global-shortcut` dependency. Retain only event listen/unlisten and window operations actually invoked by the two surfaces. Add the boundary script to the default `scripts/local-check.ps1` sequence.

- [ ] **Step 4: Run the boundary gate and complete Phase-0 verification**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

Expected: every command exits `0`; the guard and `git diff --check` print no error.

- [ ] **Step 5: Capture fresh Windows shell evidence**

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
$env:CODEXBAR_PROOF_MODE = 'tray'
$DesktopExe = @(
    '.\target\debug\codex-barbar.exe',
    '.\target\x86_64-pc-windows-msvc\debug\codex-barbar.exe'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $DesktopExe) { throw 'Fresh codex-barbar.exe was not found' }
& $DesktopExe
```

Close any older instance first. Use CUA to record that the process owns one tray icon, left click opens the tray placeholder, Settings opens a separate window, Escape dismisses the tray panel, and no FloatBar/PopOut window exists. Save the observation and screenshots under `docs/verification/windows/2026-08-03/`.

- [ ] **Step 6: Commit**

```powershell
git add apps/desktop-tauri/src-tauri/capabilities/default.json apps/desktop-tauri/src-tauri/tauri.conf.json apps/desktop-tauri/src-tauri/Cargo.toml scripts/assert-v1-boundaries.ps1 scripts/local-check.ps1 docs/verification/windows/2026-08-03
git commit -m "Enforce the V1 desktop security boundary"
```

## Phase 0 Exit Gate

- `codex-barbar.exe` builds from the Tauri crate.
- Tauri product name is `codex-barbar`; identifier/AppUserModelID is `com.naipi11.codexbarbar`.
- App data is derived only below `%LOCALAPPDATA%\codex-barbar`.
- The shipping provider registry contains only Codex.
- The active Tauri command graph and frontend bridge contain no cookie, key, token-account, chart, session, workspace, FloatBar, PopOut, arbitrary URL/path, or update-download/apply capability.
- Startup performs no update or provider network request and shows an honest not-connected state.
- Both Rust manifests, all frontend tests/build, the static boundary guard, and fresh Windows CUA shell evidence pass.
