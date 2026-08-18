# codex-barbar V1 Phase 4 Windows Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden the complete Windows app, produce verifiable NSIS and portable artifacts, establish official Windows CI/security/license gates, record clean-machine RC evidence, and prepare—but do not externally publish without authorization—the `1.0.0` release.

**Architecture:** One `AppCoordinator` owns startup/quit order and observable timing; one redacting log/diagnostics pipeline owns all disk diagnostics. The final Tauri/Rust/Vite module graphs and ACL/CSP allowlists contain only V1 capabilities. Tauri builds a per-user NSIS installer; PowerShell stages a portable ZIP, hashes, SPDX SBOM, and deterministic manifest. Official GitHub Windows runners repeat the same gates; real Windows CUA/installer matrices remain explicit evidence outside unit tests.

**Tech Stack:** Tauri 2 NSIS bundler, Rust stable edition 2024, React/Vitest, Windows APIs via the existing `windows`/`winreg` crates, PowerShell 5.1+, GitHub Actions `windows-2025`, pnpm 10.18.1/Node 20, Cargo metadata, SPDX 2.3 JSON.

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

| Path | Responsibility after Phase 4 |
|---|---|
| `apps/desktop-tauri/src-tauri/src/app_coordinator.rs` | Ordered startup, one-shot graceful quit, timing evidence |
| `rust/src/rolling_log.rs` | 5 MiB line-redacted segments and 14-day retention |
| `rust/src/diagnostics.rs` | Fixed-location, twice-scanned, redacted diagnostic export |
| `scripts/assert-v1-boundaries.ps1` | Final command/module/CSP/dependency/source release-graph gate |
| `apps/desktop-tauri/src-tauri/tauri.conf.json` | Current-user NSIS and final CSP/product bundle config |
| `apps/desktop-tauri/src-tauri/windows/installer-hooks.nsh` | Explicit preserve-or-purge uninstall choice |
| `rust/src/platform/windows/data_cleanup.rs` | Exact-root, no-reparse user-data purge |
| `scripts/windows-release-build.ps1` | Clean-checkout NSIS + portable staging + hash/SBOM orchestration |
| `scripts/verify-release-artifacts.ps1` | Names, PE subsystem/arch, contents, version, hash, SBOM assertions |
| `.github/workflows/pr-check.yml` | Official Windows PR gate |
| `.github/workflows/release.yml` | Authorized tag/manual RC build and draft artifact upload |
| `scripts/generate-sbom.ps1` | SPDX 2.3 from locked Cargo/pnpm metadata using existing tools |
| `scripts/audit-licenses.ps1` | Fail-closed license policy check |
| `docs/release/v1-rc-report.md` | Redacted automated/CUA/clean-machine acceptance evidence |

## Test Support Contract

- `AppCoordinatorFixture` lives in `app_coordinator.rs` behind `#[cfg(test)]` and injects fake recovery, cache, scheduler, account-service, window, and clock ports so ordering/timeout assertions never launch a real app.
- Redactor/log/diagnostics fixtures live in their owning Rust modules, write only to `tempfile::TempDir`, include fixed synthetic secret strings, and expose only redacted assertion helpers.
- `DataCleanupFixture` creates a uniquely named child below a temporary fake LocalAppData root and injects that root; it never derives a deletion target from the developer's real environment.
- Every release PowerShell script implements `-SelfTest` using a uniquely named directory below `[IO.Path]::GetTempPath()` and deletes only that validated directory in `finally`. Artifact fixtures are synthetic PE/ZIP metadata unless the step explicitly requests a real build.
- Every helper referenced below is defined in one of these named locations before its first test is compiled and is not included in production artifacts.

### Task 1: Centralize startup, single-instance, shutdown, and cached-start timing

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/app_coordinator.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/state.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/window.rs`
- Modify: `rust/src/platform/windows/autostart.rs`
- Test: inline coordinator/lifecycle tests

**Interfaces:**
- Consumes: database/recovery/bootstrap, tray setup, scheduler, profile service, App Server Job ownership.
- Produces: `AppCoordinator::start`, `request_quit`, `StartupMilestone`, and a one-shot shutdown state.

- [ ] **Step 1: Write failing startup-order/one-shot/latency tests**

```rust
#[test]
fn cached_start_orders_recovery_before_tray_and_network() {
    let trace = coordinator_fixture().start_with_cache().unwrap();
    assert_eq!(trace.names(), [
        "logging", "database", "recovery", "cache", "tray",
        "codex-discovery", "background-refresh",
    ]);
}

#[tokio::test]
async fn repeated_quit_requests_run_shutdown_once() {
    let fixture = coordinator_fixture();
    tokio::join!(fixture.coordinator.request_quit(), fixture.coordinator.request_quit());
    assert_eq!(fixture.scheduler.stop_calls(), 1);
    assert_eq!(fixture.profile_service.shutdown_calls(), 1);
    assert_eq!(fixture.exit_calls(), 1);
}

#[test]
fn cached_tray_ready_budget_is_three_seconds() {
    assert_eq!(StartupBudget::CACHED_TRAY_READY, Duration::from_secs(3));
}
```

- [ ] **Step 2: Run tests and verify lifecycle scattering failure**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml cached_start_orders_recovery_before_tray_and_network
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml repeated_quit_requests_run_shutdown_once
```

Expected: FAIL because lifecycle ownership is distributed.

- [ ] **Step 3: Implement the coordinator and bounded quit**

```rust
pub enum QuitState { Running, Stopping, Exited }

impl AppCoordinator {
    pub fn start(&self, app: &tauri::AppHandle) -> Result<BootstrapDto, StartupError>;
    pub async fn request_quit(&self, app: tauri::AppHandle) {
        if !self.begin_stopping() { return; }
        self.scheduler.stop().await;
        let _ = tokio::time::timeout(Duration::from_secs(3), self.profiles.shutdown(Duration::from_secs(3))).await;
        self.jobs.terminate_remaining();
        self.mark_exited();
        app.exit(0);
    }
}
```

Second instances only focus/toggle the existing main flyout and do not initialize repositories. Startup writes non-secret monotonic milestones; cached tray construction is never blocked on Codex discovery/network. After tray creation, resolve/probe Codex once without blocking cached visibility, publish compatibility state, and only then queue the selected Profile's background refresh. Audit autostart to canonical `codex-barbar.exe --background` and ensure Settings load has no registry side effect.

- [ ] **Step 4: Run coordinator/single-instance/autostart suites**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml app_coordinator::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml single_instance -- --nocapture
cargo test --manifest-path rust/Cargo.toml platform::windows::autostart::tests -- --nocapture
```

Expected: ordered start, cache-first network independence, one-shot quit, successful/error/timeout quit, tree termination, second-instance focus, registry quoting, and CurrentCli no-write assertions pass.

- [ ] **Step 5: Measure a real cached launch**

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
$env:CODEXBAR_PROOF_MODE = 'trayPanel:ready'
$DesktopExe = @(
    '.\target\debug\codex-barbar.exe',
    '.\target\x86_64-pc-windows-msvc\debug\codex-barbar.exe'
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $DesktopExe) { throw 'Fresh codex-barbar.exe was not found' }
$process = Start-Process $DesktopExe -PassThru -WindowStyle Hidden
```

Set `$ExecutionDate = Get-Date -Format 'yyyy-MM-dd'`. Use CUA to timestamp process start and visible tray/flyout cached content. Record at least five runs and require each ≤3.0 seconds in `docs/verification/windows/${ExecutionDate}/startup-performance.md`; record hardware/OS version without device identifiers.

- [ ] **Step 6: Commit**

```powershell
git add apps/desktop-tauri/src-tauri/src/app_coordinator.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/state.rs apps/desktop-tauri/src-tauri/src/tray_bridge.rs apps/desktop-tauri/src-tauri/src/commands/window.rs rust/src/platform/windows/autostart.rs docs/verification/windows
git commit -m "Harden the Windows app lifecycle"
```

### Task 2: Add rolling redacted logs and fixed-location diagnostics

**Files:**
- Replace: `rust/src/core/redactor.rs`
- Create: `rust/src/rolling_log.rs`
- Modify: `rust/src/logging.rs`
- Create: `rust/src/diagnostics.rs`
- Modify: `rust/src/lib.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/commands/diagnostics.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src/lib/tauri.ts`
- Modify: `apps/desktop-tauri/src/surfaces/settings/tabs/AdvancedTab.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/Settings.test.ts`
- Test: inline Rust tests plus Settings Vitest

**Interfaces:**
- Consumes: app paths, redacted errors/capabilities/timestamps, line-oriented tracing.
- Produces: `SecretRedactor`, `RollingLogWriter`, `Diagnostics::summary/export`, `get_diagnostics_summary`, and `export_diagnostics`.

- [ ] **Step 1: Write failing recursive-secret/export-cleanup tests**

```rust
#[test]
fn recursively_redacts_keys_jwt_bearer_pat_and_query_tokens() {
    let input = json!({
        "AccessToken": TEST_JWT,
        "nested": { "refresh-token": "refresh-secret", "url": "https://x.test/?token=abc" },
        "header": "Bearer abc.def.ghi",
        "pat": "github_pat_EXAMPLE_NOT_A_SECRET"
    });
    let output = SecretRedactor::default().redact_value(input);
    let text = output.to_string();
    for secret in [TEST_JWT, "refresh-secret", "token=abc", "abc.def.ghi", "github_pat_"] { assert!(!text.contains(secret)); }
}

#[test]
fn failed_final_scan_removes_temporary_export_and_preserves_previous_file() {
    let fixture = diagnostics_fixture_with_injected_secret();
    let previous = fixture.previous_export_bytes();
    assert!(fixture.export().is_err());
    assert_eq!(fixture.previous_export_bytes(), previous);
    assert!(!fixture.temporary_export_exists());
}
```

- [ ] **Step 2: Run tests and verify existing redactor/logging gaps**

```powershell
cargo test --manifest-path rust/Cargo.toml recursively_redacts_keys_jwt_bearer_pat_and_query_tokens
cargo test --manifest-path rust/Cargo.toml failed_final_scan_removes_temporary_export_and_preserves_previous_file
```

Expected: FAIL before the complete redactor/diagnostics path exists.

- [ ] **Step 3: Implement exact retention and export policy**

Log to `%LOCALAPPDATA%\codex-barbar\logs\codex-barbar.log*`, rotate at 5 MiB, delete segments older than 14 days on startup, default level info, and reset session-only diagnostic verbosity after restart. Buffer complete lines before redaction. Recursively normalize case/snake/camel/kebab variants of `token`, `access_token`, `refresh_token`, `authorization`, `cookie`, `api_key`, and `auth_json`; scan JWT, Bearer, GitHub token forms, token query parameters, and high-entropy long strings while retaining ordinary UUIDs.

Diagnostics accepts no frontend path and writes `%LOCALAPPDATA%\codex-barbar\diagnostics\codex-barbar-diagnostics-yyyyMMddTHHmmssZ.json`, where the suffix is the current UTC timestamp formatted with invariant digits. Include product/OS/Codex versions, resolved-path class with home replaced, capabilities, Profile kinds/count only, refresh times, `AppErrorKind`, Vault/recovery/storage status, tested-version summary, and redacted log tail. Scan the model before serialization and the completed temporary file before atomic publish; delete temp on failure.

- [ ] **Step 4: Run redactor/log/diagnostics/bridge tests**

```powershell
cargo test --manifest-path rust/Cargo.toml core::redactor::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml rolling_log::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml diagnostics::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::diagnostics::tests -- --nocapture
pnpm --dir apps/desktop-tauri exec vitest run src/surfaces/Settings.test.ts
```

Expected: recursive keys, JWT/Bearer/PAT/query/high-entropy, UUID false-positive, segmented writes, rotation, 14-day cleanup, fixed path, pre/post scan, temp cleanup, redacted DTO, and UI export-result tests pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/core/redactor.rs rust/src/rolling_log.rs rust/src/logging.rs rust/src/diagnostics.rs rust/src/lib.rs apps/desktop-tauri/src-tauri/src/commands/diagnostics.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/surfaces/settings/tabs/AdvancedTab.tsx apps/desktop-tauri/src/surfaces/Settings.test.ts
git commit -m "Add redacted diagnostics"
```

### Task 3: Minimize the final compiled module, command, dependency, capability, and CSP graph

**Files:**
- Replace: `apps/desktop-tauri/src-tauri/capabilities/default.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `rust/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `rust/src/lib.rs`
- Replace: `rust/src/core/provider_factory.rs`
- Modify: `rust/src/core/mod.rs`
- Replace: `rust/src/providers/mod.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `scripts/assert-v1-boundaries.ps1`
- Delete: `apps/desktop-tauri/src-tauri/src/floatbar/`
- Delete: `apps/desktop-tauri/src-tauri/src/coding_activity.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/powertoys.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/shortcut_bridge.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/agent_sessions.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/browser_import.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/chart.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/codex_workspaces.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/credential_detection.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/credentials.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/provider_detail.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/provider_settings.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/shortcuts.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/tokens.rs`
- Delete: `apps/desktop-tauri/src-tauri/src/commands/usage_spend.rs`
- Delete: `apps/desktop-tauri/src/floatbar/`
- Delete: `apps/desktop-tauri/src/components/AgentSessions.tsx`
- Delete: `apps/desktop-tauri/src/components/charts/`
- Delete: `apps/desktop-tauri/src/components/MenuCard.tsx`
- Delete: `apps/desktop-tauri/src/components/MenuCardDetails.tsx`
- Delete: `apps/desktop-tauri/src/components/ProviderGrid.tsx`
- Delete: `apps/desktop-tauri/src/components/PopOutTitleBar.tsx`
- Delete: `apps/desktop-tauri/src/components/ShortcutCapture.tsx`
- Delete: `apps/desktop-tauri/src/surfaces/PopOutPanel.tsx`
- Delete: `apps/desktop-tauri/src/surfaces/settings/providers/`
- Delete: `apps/desktop-tauri/src/surfaces/settings/tokens/`
- Delete: all provider icon SVGs under `apps/desktop-tauri/src/components/providers/icons/` except `ProviderIcon-codex.svg`

**Interfaces:**
- Consumes: completed V1 source graph.
- Produces: exact final invoke allowlist and a static release boundary gate.

- [ ] **Step 1: Extend the boundary script with failing exact-set checks**

```powershell
$expectedCommands = @(
  'get_bootstrap_state','get_settings_snapshot','update_settings','get_locale_strings',
  'select_profile','refresh_selected_profile','start_managed_login','cancel_managed_login',
  'rename_managed_profile','remove_managed_profile','validate_codex_executable',
  'get_diagnostics_summary','export_diagnostics','check_for_updates','open_release_page',
  'open_codex_usage_page','open_settings_window','close_settings_window','dismiss_tray_panel',
  'set_flyout_size','get_current_surface_state','quit_app'
)
$forbidden = @('download_update','apply_update','open_external_url','open_path','account/logout','floatbar','popout','manual_cookie','api_key','token_account','global-shortcut','telemetry','analytics','sentry')
```

Parse `main.rs` command names and fail unless the sorted set equals `$expectedCommands`; scan active Rust/TS/config files for every forbidden token.

- [ ] **Step 2: Run the guard and capture legacy failures**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
```

Expected: FAIL while legacy modules/files/permissions remain.

- [ ] **Step 3: Remove legacy graph and set exact capabilities/CSP**

Set capability windows to `main` and `settings`. Retain only core event listen/unlisten and window close/minimize/start-dragging permissions that React actually calls; prefer trusted commands for size/position/actions. Production CSP is exactly:

```text
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; frame-ancestors 'none'
```

Remove `tauri-plugin-global-shortcut`. Set shared Rust `autobins = false` and remove the old CLI target from `rust/Cargo.toml`; `rust/src/lib.rs` and `rust/src/providers/mod.rs` compile only V1 modules/Codex, so dormant provider sources are not in the compiled release graph. Replace the historical all-provider factory with only `instantiate_shipping_provider`, importing only `CodexProvider` and returning `UnsupportedProvider` for any retained non-Codex enum value; remove the old infallible `instantiate_provider` re-export. Remove third-party dependencies only after `cargo tree` plus `rg` proves no active use; retain reqwest for manual update and `open` only behind fixed actions.

- [ ] **Step 4: Run static graph, tree, test, and bundle scans**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
cargo tree --manifest-path rust/Cargo.toml
cargo tree --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
rg -n "localhost|127\.0\.0\.1|ws://" apps/desktop-tauri/src-tauri
```

Expected: boundary/test/build commands pass; the final `rg` returns no match and exit code `1`.

- [ ] **Step 5: Commit**

```powershell
git add -A rust/Cargo.toml Cargo.lock rust/src/lib.rs rust/src/core/provider_factory.rs rust/src/core/mod.rs rust/src/providers/mod.rs apps/desktop-tauri/src apps/desktop-tauri/src-tauri scripts/assert-v1-boundaries.ps1
git commit -m "Minimize the desktop runtime surface"
```

### Task 4: Configure current-user NSIS and safe optional user-data purge

**Files:**
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Create: `apps/desktop-tauri/src-tauri/windows/installer-hooks.nsh`
- Create: `rust/src/platform/windows/data_cleanup.rs`
- Modify: `rust/src/platform/windows/mod.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Test: inline purge/config tests

**Interfaces:**
- Consumes: canonical executable/data root and an explicit NSIS uninstall confirmation.
- Produces: NSIS current-user bundle and `DataPurger::purge_exact_local_app_data_root`.

- [ ] **Step 1: Write failing installer-config and safe-target tests**

```rust
#[test]
fn nsis_is_current_user_and_only_bundle_target() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["bundle"]["targets"], json!(["nsis"]));
    assert_eq!(config["bundle"]["windows"]["nsis"]["installMode"], "currentUser");
}

#[test]
fn purge_rejects_parent_relative_and_reparse_targets() {
    for path in [Path::new(r"C:\Users\A\AppData\Local"), Path::new("..") ] {
        assert!(purger().validate_target(path).is_err());
    }
    assert!(purger().validate_target(fake_reparse_codex_barbar()).is_err());
}
```

- [ ] **Step 2: Run tests and verify old bundler/purge failure**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml nsis_is_current_user_and_only_bundle_target
cargo test --manifest-path rust/Cargo.toml purge_rejects_parent_relative_and_reparse_targets
```

Expected: FAIL until NSIS/purge implementation is added.

- [ ] **Step 3: Implement exact bundle and uninstall choice**

Set `tauri:build` to `tauri build --bundles nsis`, `installMode` to `currentUser`, no elevation, and installer hooks path to `windows/installer-hooks.nsh`. Default uninstall preserves data. The hook asks “Delete local codex-barbar accounts and cache?”; only an explicit Yes invokes the installed binary with fixed `--purge-user-data`. That internal mode derives `%LOCALAPPDATA%\codex-barbar` itself, verifies canonical exact equality, ordinary directory/no reparse at every component, stops if the app is running, and deletes no other target. It never accepts a path argument.

- [ ] **Step 4: Run config/purge tests and build a local NSIS bundle**

```powershell
cargo test --manifest-path rust/Cargo.toml platform::windows::data_cleanup::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml nsis_ -- --nocapture
pnpm --dir apps/desktop-tauri run tauri:build
```

Expected: tests pass and `target\release\bundle\nsis\` contains one x64 NSIS executable; no Inno/WiX/MSI artifact is produced.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/tauri.conf.json apps/desktop-tauri/src-tauri/windows/installer-hooks.nsh rust/src/platform/windows/data_cleanup.rs rust/src/platform/windows/mod.rs apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Configure the per-user NSIS installer"
```

### Task 5: Build portable ZIPs and verify release artifacts/smoke behavior

**Files:**
- Replace: `scripts/windows-release-build.ps1`
- Replace: `scripts/windows-smoke-install.ps1`
- Create: `scripts/windows-smoke-portable.ps1`
- Create: `scripts/verify-release-artifacts.ps1`
- Replace: `scripts/release-doctor.ps1`
- Delete: `rust/installer/codexbar.iss`
- Delete: `rust/wix/`
- Delete: `rust/tauri.conf.json`
- Delete: `scripts/macos-windows-cross-build.sh`
- Delete: `scripts/verify-windows-executables.ps1`
- Create: `PORTABLE.md`
- Test: each new script's `-SelfTest` mode

**Interfaces:**
- Consumes: clean checkout, version `0.1.0-alpha.1` during development and a required `-Version` parameter matching `^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta|rc)\.[0-9]+)?$`; the parameter must equal every product manifest version.
- Produces: setup EXE, portable ZIP, `SHA256SUMS.txt`, SPDX file, and artifact manifest.

- [ ] **Step 1: Add failing self-tests for names and portable contents**

```powershell
.\scripts\verify-release-artifacts.ps1 -SelfTest
.\scripts\windows-smoke-portable.ps1 -SelfTest
```

Expected: FAIL until the new scripts exist.

- [ ] **Step 2: Implement fail-closed build/staging behavior**

Require a clean worktree and exact HEAD/ref; never reset or clean the user's checkout. Run frozen pnpm install, tests/build, Tauri NSIS bundle, then copy/rename the setup. Stage portable contents only:

```text
codex-barbar.exe
LICENSE
UPSTREAMS.md
README.md
README.zh-CN.md
PORTABLE.md
```

Compress with `Compress-Archive`; portable data still writes LocalAppData. The examples below use the validated PowerShell parameter `${Version}` and emit:

```text
codex-barbar_${Version}_x64-setup.exe
codex-barbar_${Version}_x64-portable.zip
SHA256SUMS.txt
codex-barbar_${Version}_sbom.spdx.json
artifact-manifest.json
```

Manifest records version, commit, target, filenames, sizes, SHA-256, signed/unsigned status, and build time; no environment dump.

- [ ] **Step 3: Implement installer/portable/artifact verification**

Installer smoke checks HKCU install scope, Start Menu shortcut, GUI subsystem/x64, displayed version, running tray, upgrade preserves data, default uninstall preserves data, explicit purge removes exactly app data. Portable smoke expands to a temp directory, launches the GUI, verifies no file is created beside the executable, then stops the process. Artifact verifier checks exact filenames/set, ZIP contents, PE x64 GUI subsystem, version consistency, SHA256SUMS, SPDX JSON parse, and absence of old CLI/legacy names.

- [ ] **Step 4: Run script self-tests and produce alpha artifacts**

```powershell
.\scripts\verify-release-artifacts.ps1 -SelfTest
.\scripts\windows-smoke-portable.ps1 -SelfTest
.\scripts\windows-release-build.ps1 -Ref HEAD -Version 0.1.0-alpha.1 -OutputDirectory .\artifacts\release
.\scripts\verify-release-artifacts.ps1 -Version 0.1.0-alpha.1 -AssetsDirectory .\artifacts\release
.\scripts\windows-smoke-install.ps1 -InstallerPath .\artifacts\release\codex-barbar_0.1.0-alpha.1_x64-setup.exe -ExpectedVersion 0.1.0-alpha.1
.\scripts\windows-smoke-portable.ps1 -ArchivePath .\artifacts\release\codex-barbar_0.1.0-alpha.1_x64-portable.zip
```

Expected: every command exits `0`; hashes/manifest agree and smoke cleanup leaves user data according to chosen mode.

- [ ] **Step 5: Commit**

```powershell
git add -A scripts rust/installer rust/wix rust/tauri.conf.json PORTABLE.md
git commit -m "Package Windows V1 artifacts"
```

### Task 6: Move CI to official Windows runners and add an authorized draft-release workflow

**Files:**
- Replace: `.github/workflows/pr-check.yml`
- Create: `.github/workflows/release.yml`
- Modify: `scripts/local-check.ps1`
- Modify: `.github/CI.md`
- Test: action YAML/static script assertions

**Interfaces:**
- Consumes: local-check/release scripts and repository short-lived `GITHUB_TOKEN`.
- Produces: repeatable PR gates and an external-action-gated draft release build.

- [ ] **Step 1: Add failing hosted-runner/workflow assertions**

```powershell
$pr = Get-Content -Raw -Encoding utf8 .github/workflows/pr-check.yml
if ($pr -match 'blacksmith|CI_BUDGET_MODE') { throw 'Legacy hosted runner gate remains' }
if ($pr -notmatch 'windows-2025') { throw 'Official Windows runner missing' }
if (-not (Test-Path .github/workflows/release.yml)) { throw 'Release workflow missing' }
```

- [ ] **Step 2: Run assertions before rewrite**

Expected: FAIL on Blacksmith and missing release workflow.

- [ ] **Step 3: Implement exact PR and release jobs**

PR CI on `windows-2025` sets Node 20, pnpm 10.18.1, stable Rust with rustfmt/clippy and x86_64-pc-windows-msvc, frozen install, format, both manifest Clippy/tests, Vitest/build, boundary scan, `pnpm audit --prod --audit-level high`, license audit, and Tauri x64 production build. Remove the Blacksmith runner and budget conditional.

Release workflow triggers only `workflow_dispatch` or a pushed `v*` tag, repeats all gates, runs `windows-release-build.ps1`, verifies artifacts, and uploads them to an Actions artifact. Creation/upload of a draft GitHub Release is guarded by an explicit boolean dispatch input `publish_draft` or a tag event; it uses only repository `GITHUB_TOKEN`, never a PAT, never Winget, and marks unsigned builds in notes.

- [ ] **Step 4: Validate YAML and local mirror**

```powershell
node -e "for (const f of ['.github/workflows/pr-check.yml','.github/workflows/release.yml']) { const s=require('fs').readFileSync(f,'utf8'); if (!s.includes('windows-2025')) process.exit(1) }"
.\scripts\local-check.ps1 -All
```

Expected: Node assertion and all local checks exit `0`.

- [ ] **Step 5: Commit**

```powershell
git add .github/workflows/pr-check.yml .github/workflows/release.yml .github/CI.md scripts/local-check.ps1
git commit -m "Add official Windows release gates"
```

### Task 7: Generate SPDX 2.3 and enforce dependency/license policy without new tooling

**Files:**
- Create: `scripts/generate-sbom.ps1`
- Create: `scripts/audit-licenses.ps1`
- Create: `scripts/license-policy.json`
- Modify: `scripts/windows-release-build.ps1`
- Modify: `scripts/local-check.ps1`
- Test: `-SelfTest` modes and generated JSON assertions

**Interfaces:**
- Consumes: `Cargo.lock`/`cargo metadata --locked`, `pnpm-lock.yaml`, `pnpm list/licenses`.
- Produces: deterministic SPDX 2.3 JSON and fail-closed license report.

- [ ] **Step 1: Add failing self-tests**

```powershell
.\scripts\generate-sbom.ps1 -SelfTest
.\scripts\audit-licenses.ps1 -SelfTest
```

Expected: FAIL until the scripts/policy exist.

- [ ] **Step 2: Implement the exact policy and SPDX shape**

Allow these SPDX IDs/expressions unless a reviewed exception is added with package/version/reason: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`, `MPL-2.0`, `CC0-1.0`, `Unlicense`, `BSL-1.0`. Fail on missing/unknown licenses and GPL/AGPL/SSPL. Build SPDX 2.3 JSON with document namespace based on commit+version, package name/version/license/source/purl, checksums when available, and `DEPENDS_ON` relationships. Sort packages/relationships before serialization for deterministic output.

- [ ] **Step 3: Add vulnerability boundary without false claims**

Run `pnpm audit --prod --audit-level high`. For Rust and transitive ecosystem alerts, the release workflow queries enabled GitHub Dependabot open alerts using its short-lived token and fails on high/critical. If Dependabot/security-events read access is not enabled, fail the public-release job with a clear external-gate message; do not claim a RustSec audit occurred and do not install `cargo-audit` without user approval.

- [ ] **Step 4: Run audit/SBOM generation twice and compare**

```powershell
.\scripts\audit-licenses.ps1
.\scripts\generate-sbom.ps1 -Version 0.1.0-alpha.1 -OutputPath .\artifacts\sbom-a.json
.\scripts\generate-sbom.ps1 -Version 0.1.0-alpha.1 -OutputPath .\artifacts\sbom-b.json
if ((Get-FileHash .\artifacts\sbom-a.json).Hash -ne (Get-FileHash .\artifacts\sbom-b.json).Hash) { throw 'SBOM is not deterministic' }
```

Expected: policy passes for the locked graph and both hashes match.

- [ ] **Step 5: Commit**

```powershell
git add scripts/generate-sbom.ps1 scripts/audit-licenses.ps1 scripts/license-policy.json scripts/windows-release-build.ps1 scripts/local-check.ps1
git commit -m "Audit release dependencies and licenses"
```

### Task 8: Rewrite V1 product, privacy, support, build, and release documentation

**Files:**
- Replace: `README.md`
- Create or replace: `README.zh-CN.md`
- Verify/modify: `UPSTREAMS.md`
- Create: `PRIVACY.md`
- Create: `TROUBLESHOOTING.md`
- Replace: `docs/ARCHITECTURE.md`
- Replace: `docs/BUILDING.md`
- Create: `docs/RELEASING.md`
- Create: `docs/TESTED_CODEX_VERSIONS.md`
- Create: `docs/WINDOWS_ACCEPTANCE.md`
- Replace: `docs/release/ci-cd.md`
- Modify: `CHANGELOG.md`
- Delete: `README.ja-JP.md`
- Delete: `README.ko-KR.md`
- Delete: `README.es-MX.md`
- Delete: `README.zh-TW.md`
- Delete: `docs/WSL.md`
- Delete: `docs/CLI.md`
- Delete: `docs/CONFIGURATION.md`
- Delete: `docs/PROVIDERS.md`
- Delete: `docs/COOKIES.md`

**Interfaces:**
- Consumes: completed architecture, compatibility matrix, package behavior.
- Produces: accurate bilingual user/developer/release documentation.

- [ ] **Step 1: Run the failing stale-product documentation scan**

```powershell
$hits = rg -n -i "winget install|automatic update|Inno Setup|portable EXE|supports .*Claude|supports .*Gemini|Windows 10|WSL support" README*.md docs CHANGELOG.md
if ($LASTEXITCODE -eq 0) { throw "Stale V1 product claims remain:`n$hits" }
```

Expected: FAIL with multiple legacy claims.

- [ ] **Step 2: Write exact V1 documentation statements**

Both READMEs state Windows 11 23H2+ x64, Codex-only, App Server experimental, CurrentCli read-only, Managed isolated/DPAPI, no telemetry, manual-only update, portable data in LocalAppData, NSIS/ZIP names, unsigned SmartScreen behavior, and tested-version limits. Privacy documents data/credential/log/diagnostic handling and threat boundary. Troubleshooting maps every `AppErrorKind` to an action. Architecture/build/release docs use only active paths/commands. `UPSTREAMS.md` preserves both repositories, baseline commit/tag, reuse, and MIT attribution.

- [ ] **Step 3: Add controlled-occurrence and link/file checks**

```powershell
$allowedLegacyFiles = @('UPSTREAMS.md','CHANGELOG.md','docs/architecture/V1_BASELINE.md')
$hits = rg -l -i "Win-CodexBar|Claude|Gemini|Winget|Inno Setup|FloatBar|PopOut" README*.md docs CHANGELOG.md
$unexpected = $hits | Where-Object { $allowedLegacyFiles -notcontains $_ }
if ($unexpected) { throw "Unexpected legacy documentation: $($unexpected -join ', ')" }
foreach ($required in @('README.md','README.zh-CN.md','UPSTREAMS.md','PRIVACY.md','TROUBLESHOOTING.md','docs/RELEASING.md','docs/TESTED_CODEX_VERSIONS.md','docs/WINDOWS_ACCEPTANCE.md')) {
  if (-not (Test-Path -LiteralPath $required)) { throw "Missing $required" }
}
```

Where a non-goal must mention a legacy term, place it in `docs/architecture/V1_BASELINE.md` and keep user docs focused on V1.

- [ ] **Step 4: Run documentation and boundary audits**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
.\scripts\audit-licenses.ps1
git diff --check
```

Expected: all exit `0` and no stale install/provider claim remains.

- [ ] **Step 5: Commit**

```powershell
git add -A README*.md UPSTREAMS.md PRIVACY.md TROUBLESHOOTING.md docs CHANGELOG.md
git commit -m "Document the Windows V1 release"
```

### Task 9: Build and record the `1.0.0-rc.1` real Windows acceptance matrix

**Files:**
- Modify: `rust/Cargo.toml`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `Cargo.lock`
- Modify: `docs/TESTED_CODEX_VERSIONS.md`
- Modify: `docs/WINDOWS_ACCEPTANCE.md`
- Create: `docs/release/v1-rc-report.md`
- Create: selected redacted screenshots under `docs/images/windows-proof/v1/`

**Interfaces:**
- Consumes: complete code, automation, CUA proof modes, disposable test accounts/machines.
- Produces: traceable RC artifacts/evidence and a go/no-go list for final V1.

- [ ] **Step 1: Set the exact RC version and run the full automated gate**

Set all four product manifests to `1.0.0-rc.1`, regenerate lock metadata through normal Cargo/pnpm commands, then run:

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
.\scripts\local-check.ps1 -All
.\scripts\windows-release-build.ps1 -Ref HEAD -Version 1.0.0-rc.1 -OutputDirectory .\artifacts\release
.\scripts\release-doctor.ps1 -Version 1.0.0-rc.1 -AssetsDirectory .\artifacts\release
```

Expected: all commands exit `0` and exact RC artifacts/hashes/SBOM exist.

- [ ] **Step 2: Execute the account/protocol failure matrix**

Record pass/fail with version/date and no identity for: no Codex, incompatible Codex, not signed in, ChatGPT file credentials, ChatGPT Windows credential store, API key, browser/device-code Managed login, two Managed switch/rename/atomic re-login/remove, offline, timeout, rate limit, recovery, App Server crash, and application crash during unseal/start/refresh/seal/replace. Verify CurrentCli auth state before/after.

- [ ] **Step 3: Execute the shell/install/platform matrix**

Record: four taskbar edges, two displays, 100/150/200% DPI, animations off, keyboard/screen reader names, single instance, autostart, ≤3-second cached startup, NSIS fresh install/upgrade/default-retain uninstall/explicit purge, portable ZIP, Windows 11 23H2 x64, and the release-time current supported Windows 11 x64 build. Use fresh builds and CUA; screenshots use synthetic proof data.

- [ ] **Step 4: Run final secret/scope/artifact audits**

```powershell
rg -n -i "ghp_|github_pat_|access[_-]?token|refresh[_-]?token|authorization|cookie|api[_-]?key" . --glob '!target/**' --glob '!node_modules/**' --glob '!Cargo.lock' --glob '!pnpm-lock.yaml'
rg -n "download_update|apply_update|account/logout|open_external_url|open_path|FloatBar|PopOut|/wham/" rust apps scripts
.\scripts\verify-release-artifacts.ps1 -Version 1.0.0-rc.1 -AssetsDirectory .\artifacts\release
```

Review every `rg` match: only redaction tests, security documentation, and explicit negative assertions may remain. The artifact verifier must exit `0`.

- [ ] **Step 5: Write the RC report and commit**

The report contains commit, artifact hashes, unsigned/signed status, automated commands, Codex matrix, Windows matrix, CUA paths, known non-blocking limitations, and a binary go/no-go conclusion. Any security invariant or hard success criterion failure is `NO-GO` and remains unfixed before this task can commit.

```powershell
git add rust/Cargo.toml apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/tauri.conf.json Cargo.lock docs/TESTED_CODEX_VERSIONS.md docs/WINDOWS_ACCEPTANCE.md docs/release/v1-rc-report.md docs/images/windows-proof/v1
git commit -m "Record Windows V1 release candidate acceptance"
```

### Task 10: Prepare the local `1.0.0` release commit and stop at the publication gate

**Files:**
- Modify: `rust/Cargo.toml`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `Cargo.lock`
- Modify: `CHANGELOG.md`
- Create: `docs/release/v1.0.0-release-notes.md`
- Modify: `docs/release/v1-rc-report.md`

**Interfaces:**
- Consumes: `GO` RC report and unchanged code except accepted RC fixes.
- Produces: local release commit/tag and final verified artifacts; external push/release remains authorization-gated.

- [ ] **Step 1: Bump exact final version and write release notes**

Set all product manifests to `1.0.0`. Release notes state Codex-only Windows support, experimental App Server boundary, CurrentCli/Managed security model, tested versions, NSIS/portable names, hashes/SBOM, manual updates, privacy, known limitations, and unsigned SmartScreen warning when no Authenticode certificate was supplied.

- [ ] **Step 2: Rebuild from the final commit candidate and verify**

```powershell
.\scripts\local-check.ps1 -All
.\scripts\windows-release-build.ps1 -Ref HEAD -Version 1.0.0 -OutputDirectory .\artifacts\release-1.0.0
.\scripts\release-doctor.ps1 -Version 1.0.0 -AssetsDirectory .\artifacts\release-1.0.0
git diff --check
```

Expected: all exit `0`; final artifact manifest points at the release commit candidate.

- [ ] **Step 3: Commit and create an annotated local tag**

```powershell
git add rust/Cargo.toml apps/desktop-tauri/src-tauri/Cargo.toml apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/tauri.conf.json Cargo.lock CHANGELOG.md docs/release/v1.0.0-release-notes.md docs/release/v1-rc-report.md
git commit -m "Release codex-barbar 1.0.0"
git tag -a v1.0.0 -m "codex-barbar 1.0.0"
```

- [ ] **Step 4: Verify local tag/artifact traceability**

```powershell
git status --short
git show --no-patch --format='%H %D' v1.0.0
.\scripts\verify-release-artifacts.ps1 -Version 1.0.0 -AssetsDirectory .\artifacts\release-1.0.0
```

Expected: clean worktree, tag points to the release commit, artifacts/hashes/SBOM pass.

- [ ] **Step 5: Stop and request explicit publication authorization**

Do not run either command until the user authorizes external publication:

```powershell
git push origin HEAD
git push origin v1.0.0
gh release create v1.0.0 --draft --title "codex-barbar 1.0.0" --notes-file docs/release/v1.0.0-release-notes.md .\artifacts\release-1.0.0\codex-barbar_1.0.0_x64-setup.exe .\artifacts\release-1.0.0\codex-barbar_1.0.0_x64-portable.zip .\artifacts\release-1.0.0\SHA256SUMS.txt .\artifacts\release-1.0.0\codex-barbar_1.0.0_sbom.spdx.json
```

After authorization, verify the draft asset hashes against local files before marking the release non-draft. Winget, MSIX, Store, and automatic updater work remain outside V1.

## Phase 4 / V1 Exit Gate

- Cached startup is measured ≤3 seconds; single-instance/autostart/quit/Job cleanup are bounded and proven.
- Logs rotate at 5 MiB/14 days, all secret patterns are recursively redacted, and diagnostics pass a final secret scan.
- Final compiled/backend/frontend/capability/CSP graphs exactly match the V1 allowlist; other providers and legacy surfaces are unreachable and unbundled.
- NSIS installs per user without elevation; default uninstall preserves data; explicit purge deletes only `%LOCALAPPDATA%\codex-barbar`; portable writes no data beside the executable.
- Setup, ZIP, SHA256SUMS, SPDX SBOM, source/license/upstream/docs all pass artifact verification.
- Official Windows CI, dependency/license gates, real Codex matrix, real Windows 11/CUA matrix, installer/portable matrix, and RC report pass.
- Final local tag/artifacts are traceable. Push and GitHub Release creation occur only after explicit user authorization.
