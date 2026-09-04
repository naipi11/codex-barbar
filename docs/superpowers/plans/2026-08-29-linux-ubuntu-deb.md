# codex-barbar Ubuntu DEB Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (-) syntax for tracking.

**Goal:** Add a supported Ubuntu 24.04 amd64 build of codex-barbar with Linux-native tray, notifications, XDG autostart, secure credentials, floating-ball behavior, and a Debian release artifact while preserving the Windows product contract.

**Architecture:** Keep the shared Rust domain, SQLite/pricing logic, React surfaces, and Tauri shell. Replace direct Windows type construction with platform-selected adapters, compile Win32 taskbar/fullscreen code only on Windows, and expose a typed capability snapshot so Linux hides unsupported controls. Build Windows and Ubuntu artifacts in separate native jobs and publish them through one release aggregation job.

**Tech Stack:** Rust 2024, Tokio, serde/serde_json, SQLite, Tauri 2.11, React 18, TypeScript 5.6, Vitest 3, pnpm 10.18.1, keyring 4.1 with zbus Secret Service support, notify-rust 4.18 with zbus, Ubuntu 24.04 amd64, WebKitGTK 4.1, GTK 3, Ayatana AppIndicator, XDG Autostart, GitHub Actions, dpkg-deb.

**Spec:** docs/superpowers/specs/2026-08-28-linux-ubuntu-deb-design.md

## Global Constraints

- Target Ubuntu 24.04 LTS amd64; GNOME is primary and KDE Plasma is best effort.
- Linux never creates a Windows-style taskbar overlay or measurement window.
- The Linux floating ball is a normal draggable utility window; no click-through, taskbar anchoring, or global fullscreen claim.
- Managed credentials use Secret Service only; no plaintext fallback for credentials, cookies, API keys, or refresh tokens.
- Chromium automatic cookie decryption is unavailable until an audited Secret-Service/OSCrypt implementation exists.
- Existing Windows NSIS, DPAPI, Windows Toast, DWM, Win32 taskbar, fullscreen, and HKCU Run behavior remains unchanged.
- Add only the approved Rust dependencies keyring and notify-rust; use the standard library for platform glue.
- Preserve typed camelCase bridge DTOs, provider data isolation, and redacted error/logging rules.
- Use pnpm 10.18.1, Node 20 in CI, stable Rust, cargo fmt, and clippy with -D warnings for both crates.
- Do not commit tmp-cua-proof, credentials, cookies, tokens, generated runtime data, or local build output.
- Every task has a RED test, a GREEN implementation, a focused verification command, and an imperative commit.

---

### Task 1: Add the platform capability contract and conditional module fence

**Files:**
- Create: rust/src/platform/linux/mod.rs
- Modify: rust/src/platform/mod.rs
- Create: apps/desktop-tauri/src-tauri/src/platform_capabilities.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/mod.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Modify: apps/desktop-tauri/src/App.tsx
- Test: platform capability Rust tests and apps/desktop-tauri/src/App.test.tsx

**Interfaces:**
- rust::platform::PlatformKind::{Windows,Linux,Other} and rust::platform::kind() -> PlatformKind.
- PlatformCapabilitiesDto fields: platform, systemTray, taskbarStatus, floatingBall, autostart, notifications, managedCredentials.
- BootstrapDto.platform: PlatformCapabilitiesDto.
- commands::get_platform_capabilities() -> PlatformCapabilitiesDto.

- [ ] Step 1: Write RED tests

```rust
#[cfg(target_os = "linux")]
#[test]
fn linux_capabilities_disable_windows_taskbar_status() {
    let capabilities = crate::platform_capabilities::snapshot();
    assert_eq!(capabilities.platform, "linux");
    assert!(!capabilities.taskbar_status);
    assert!(capabilities.floating_ball);
}
```

```tsx
it("keeps Linux capabilities on the bootstrap bridge", async () => {
  webviewWindowMocks.label = "main";
  invokeMock.mockResolvedValue({
    ...bootstrapFixture,
    platform: { platform: "linux", taskbarStatus: false },
  });
  render(<App />);
  const bootstrap = { platform: { platform: "linux", taskbarStatus: false } };
  expect(bootstrap.platform.taskbarStatus).toBe(false);
});
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml platform_capabilities
corepack pnpm@10.18.1 --dir apps/desktop-tauri test -- src/App.test.tsx
```

Expected result: the capability module and BootstrapDto.platform field are missing.

- [ ] Step 3: Implement the minimal contract

Add cfg-gated Linux exports, the capability DTO, bootstrap field, command registration, and frontend types. Return taskbarStatus=false on Linux and taskbarStatus=true on Windows. Keep notification and keyring statuses explicit.

- [ ] Step 4: Run GREEN

```powershell
cargo fmt --all -- --check
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml platform_capabilities
corepack pnpm@10.18.1 --dir apps/desktop-tauri test -- src/App.test.tsx
corepack pnpm@10.18.1 --dir apps/desktop-tauri run build
```

- [ ] Step 5: Commit

```powershell
git add rust/src/platform apps/desktop-tauri/src-tauri/src/platform_capabilities.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/App.tsx apps/desktop-tauri/src/App.test.tsx
git commit -m "Add Linux platform capability contract"
```

### Task 2: Make Codex discovery and process activity native to Linux

**Files:**
- Create: rust/src/platform/linux/process.rs
- Modify: rust/src/providers/codex/app_server/discovery.rs
- Modify: rust/src/providers/codex/app_server/npm.rs
- Modify: rust/src/providers/codex/app_server/process.rs
- Modify: rust/src/providers/codex/app_server/job.rs
- Modify: rust/src/agent_sessions/focus.rs
- Modify: apps/desktop-tauri/src-tauri/src/float_ball_motion.rs
- Test: Linux resolver, ProcReader, App Server, and process-group tests

**Interfaces:**
- linux::process::ProcReader with read_cmdline(pid), read_parent(pid), and read_children(pid).
- linux::process::discover_codex_processes(reader: &impl ProcReader) -> Vec<LinuxProcess>.
- CodexCommandResolver::resolve_override accepts an absolute Linux codex executable and verified npm layout.
- Unix App Server children receive a distinct process group.

- [ ] Step 1: Write RED fixture tests

```rust
#[cfg(target_os = "linux")]
#[test]
fn linux_resolver_accepts_codex_without_a_windows_suffix() {
    let path = fixture_executable("codex");
    let command = CodexCommandResolver::new().resolve_override(&path).unwrap();
    assert_eq!(command.launch_program(), path.as_os_str());
}

#[test]
fn proc_reader_discovers_codex_and_app_server_children() {
    let reader = FixtureProcReader::from([
        (100, "codex --serve", 1),
        (101, "codex app-server", 100),
    ]);
    let processes = discover_codex_processes(&reader);
    assert_eq!(processes.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![100, 101]);
}
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::discovery platform::linux::process
```

Expected result: Linux resolver and ProcReader tests fail because only Windows executable assumptions exist.

- [ ] Step 3: Implement Linux resolution and procfs inspection

Use the saved absolute override or PATH entry named codex, reject relative paths and shell fragments, and retain the Windows verification branch. Read only procfs command-line/parent/children files; permission errors become idle. Remove Linux calls to powershell.exe and tasklist.exe. Apply Unix process-group setup and bounded cancellation.

- [ ] Step 4: Run GREEN

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server -- --test-threads=1
cargo test --manifest-path rust/Cargo.toml platform::linux::process
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

- [ ] Step 5: Commit

```powershell
git add rust/src/platform/linux/process.rs rust/src/providers/codex/app_server rust/src/agent_sessions/focus.rs apps/desktop-tauri/src-tauri/src/float_ball_motion.rs
git commit -m "Add Linux Codex process discovery"
```

### Task 3: Store managed credentials in Secret Service

**Files:**
- Modify: rust/Cargo.toml
- Modify: Cargo.lock
- Modify: rust/src/accounts/vault/crypto.rs
- Modify: rust/src/accounts/vault/store.rs
- Modify: rust/src/accounts/vault/mod.rs
- Modify: rust/src/secure_file.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/accounts.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/bridge.rs
- Test: vault crypto/store/secure-file tests and an Ubuntu D-Bus integration

**Interfaces:**
- Linux dependency: keyring = { version = "4.1", default-features = false, features = ["v1", "zbus-secret-service-keyring-store"] }.
- CredentialProtector::remove_current_user(profile_id) -> Result<(), VaultError>, default no-op for existing test/Windows protectors.
- LinuxSecretServiceProtector::new() -> Self.
- platform_credential_protector() -> Arc<dyn CredentialProtector>.
- Marker format: codex-barbar-secret-service:v1:<profile-uuid>.

- [ ] Step 1: Write RED tests

```rust
#[cfg(target_os = "linux")]
#[test]
fn secret_service_marker_contains_no_credential_bytes() {
    let marker = secret_service_marker(Uuid::nil());
    assert!(marker.starts_with(b"codex-barbar-secret-service:v1:"));
    assert!(!marker.windows(3).any(|part| part == b"sk-"));
}

#[test]
fn removing_a_vault_deletes_the_protector_entry() {
    let (vault, protector) = test_vault_fixture_with_remove_tracking();
    vault.remove(Uuid::nil()).unwrap();
    assert_eq!(protector.removed_profiles(), &[Uuid::nil()]);
}
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::vault
```

Expected result: marker parsing and protector cleanup are missing.

- [ ] Step 3: Implement the Secret Service backend

Use keyring::Entry with fixed service com.naipi11.codexbarbar and the profile UUID as username. Call set_secret, get_secret, and delete_credential. Store the real bundle in Secret Service and only the opaque marker in the local envelope. Map locked/unavailable/ambiguous errors to coarse VaultError values. Construct the platform protector from main.rs. Never rewrite Windows DPAPI data as plaintext.

- [ ] Step 4: Run GREEN and the Ubuntu integration

```powershell
cargo fmt --all -- --check
cargo test --manifest-path rust/Cargo.toml accounts::vault -- --test-threads=1
```

On Ubuntu, run the round trip under a disposable D-Bus session and assert that the local envelope contains the marker but not the test secret. With no Secret Service, assert an unsupported result and no credential file.

- [ ] Step 5: Commit

```powershell
git add rust/Cargo.toml Cargo.lock rust/src/accounts/vault rust/src/secure_file.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/accounts.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs
git commit -m "Store Linux credentials in Secret Service"
```

### Task 4: Implement XDG autostart and Linux locale selection

**Files:**
- Create: rust/src/platform/linux/autostart.rs
- Create: rust/src/platform/linux/system_locale.rs
- Modify: rust/src/platform/mod.rs
- Modify: rust/src/platform/windows/autostart.rs
- Modify: rust/src/platform/windows/system_locale.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs
- Modify: apps/desktop-tauri/src-tauri/src/tray_bridge.rs
- Test: Linux autostart and locale fixture tests

**Interfaces:**
- platform::autostart::set_enabled(enabled: bool) -> Result<(), AutostartError>.
- linux::autostart::path(config_root: &Path) -> PathBuf.
- linux::autostart::desktop_entry(executable: &Path) -> Result<String, AutostartError>.
- platform::system_locale::language() -> LanguagePreference.

- [ ] Step 1: Write RED tests

```rust
#[test]
fn linux_autostart_entry_uses_only_an_absolute_executable() {
    let text = desktop_entry(Path::new("/opt/codex-barbar/codex-barbar")).unwrap();
    assert!(text.contains("Type=Application"));
    assert!(text.contains("Exec=/opt/codex-barbar/codex-barbar --background"));
    assert!(!text.contains("sh -c"));
}

#[test]
fn disabling_autostart_removes_only_the_fixed_desktop_file() {
    let root = tempfile::tempdir().unwrap();
    set_enabled_at(root.path(), false).unwrap();
    assert!(!path(root.path()).exists());
}
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path rust/Cargo.toml platform::linux::autostart
```

- [ ] Step 3: Implement XDG routing

Write the fixed desktop entry atomically under dirs::config_dir()/autostart, validate absolute executable names, remove only the fixed file, and route startup reconciliation, settings updates, and tray locale through platform facades. Preserve HKCU Run behavior.

- [ ] Step 4: Run GREEN

```powershell
cargo test --manifest-path rust/Cargo.toml platform::windows::autostart platform::linux::autostart
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

- [ ] Step 5: Commit

```powershell
git add rust/src/platform apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src-tauri/src/tray_bridge.rs
git commit -m "Add Linux XDG autostart support"
```

### Task 5: Add Linux Freedesktop notifications

**Files:**
- Modify: apps/desktop-tauri/src-tauri/Cargo.toml
- Modify: Cargo.lock
- Modify: apps/desktop-tauri/src-tauri/src/notification_controller.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs
- Modify: apps/desktop-tauri/src-tauri/src/commands/update.rs
- Modify: apps/desktop-tauri/src-tauri/src/pricing_refresh.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx
- Test: notification controller and NotificationsTab tests

**Interfaces:**
- Linux dependency: notify-rust = { version = "4.18", default-features = false, features = ["z"] }.
- DesktopNotificationSink enum with Windows, Linux, and Unsupported variants implementing ToastSink.
- platform_notification_sink() -> DesktopNotificationSink.
- notification_capability() probes the Linux session D-Bus and preserves the Windows probe.

- [ ] Step 1: Write RED test

```rust
#[test]
fn controller_accepts_the_platform_sink_type() {
    let _: NotificationController<DesktopNotificationSink> =
        NotificationController::new(test_engine(), DesktopNotificationSink::Unsupported);
}
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller
```

Expected result: command state is still typed as NotificationController<WindowsToastSink>.

- [ ] Step 3: Implement the Linux sink

Use notify_rust::Notification with fixed app name codex-barbar, summary/title/body, and a bounded timeout. Probe the notification server without displaying a toast. Preserve master switch, thresholds, deduplication, and unsupported-state copy.

- [ ] Step 4: Run GREEN

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml notification_controller commands::settings commands::update
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
corepack pnpm@10.18.1 --dir apps/desktop-tauri test -- src/surfaces/settings/tabs/NotificationsTab.test.tsx
```

- [ ] Step 5: Commit

```powershell
git add apps/desktop-tauri/src-tauri/Cargo.toml Cargo.lock apps/desktop-tauri/src-tauri/src/notification_controller.rs apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/surfaces/settings/tabs/NotificationsTab.tsx
git commit -m "Add Linux desktop notifications"
```

### Task 6: Gate Windows-only status surfaces and add Linux floating-ball runtime

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/win32.rs
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/window.rs
- Modify: apps/desktop-tauri/src-tauri/src/shell/dwm.rs
- Modify: apps/desktop-tauri/src-tauri/src/shell/fullscreen_guard.rs
- Modify: apps/desktop-tauri/src-tauri/src/shell/foreground_events.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs
- Test: status-surface, taskbar-overlay, float-ball, and fullscreen tests

**Interfaces:**
- StatusSurfaceController::supports(StatusSurfaceKind) -> bool returns false for TaskbarStatus on Linux.
- Linux TaskbarOverlay::apply_enabled returns TASKBAR_STATUS_UNSUPPORTED_PLATFORM without creating a window or changing the stored flag.
- Linux float_ball::window::get_or_create uses generic Tauri APIs and no DWM/user32 calls.
- Linux fullscreen and foreground monitors are safe idle implementations.

- [ ] Step 1: Write RED test

```rust
#[cfg(target_os = "linux")]
#[test]
fn linux_does_not_create_taskbar_or_measurement_windows() {
    assert!(!StatusSurfaceController::supports(StatusSurfaceKind::TaskbarStatus));
    assert!(!taskbar_overlay::window::creates_windows_on_this_platform());
}
```

- [ ] Step 2: Run RED

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces taskbar_overlay float_ball
```

- [ ] Step 3: Implement the platform split

Put Win32 discovery, DWM shaping, foreground hooks, and renderer fullscreen inspection behind cfg(windows). Add a Linux no-op taskbar runtime and a generic Tauri floating-ball builder with skip_taskbar and best-effort always_on_top. Preserve taskbar settings without runtime writes during Linux reconciliation.

- [ ] Step 4: Run GREEN

```powershell
cargo fmt --all -- --check
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces taskbar_overlay float_ball shell::fullscreen_guard
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

On Ubuntu, also run the complete Tauri test manifest to compile all Linux branches.

- [ ] Step 5: Commit

```powershell
git add apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/status_surfaces apps/desktop-tauri/src-tauri/src/taskbar_overlay apps/desktop-tauri/src-tauri/src/float_ball apps/desktop-tauri/src-tauri/src/shell apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Gate Windows status surfaces on Linux"
```

### Task 7: Make React settings and surfaces platform-aware

**Files:**
- Modify: apps/desktop-tauri/src/types/bridge.ts
- Modify: apps/desktop-tauri/src/test/profileUsageFixtures.ts
- Modify: apps/desktop-tauri/src/surfaces/Settings.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsTabs.ts
- Modify: apps/desktop-tauri/src/surfaces/settings/settingsCopy.ts
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/TaskbarTrayTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.tsx
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx
- Modify: apps/desktop-tauri/src/styles.css
- Test: Settings.test.tsx, TaskbarTrayTab.test.tsx, App.test.tsx, FloatBall.test.tsx

**Interfaces:**
- BootstrapDto.platform: PlatformCapabilitiesDto is required in every fixture.
- Settings passes platform to TaskbarTrayTab.
- TaskbarTrayTab renders floating-ball controls on Linux and omits taskbar-status controls.
- Linux copy keys include taskbarUnavailable, trayUnavailable, keyringUnavailable, and waylandFloatFallback in English and Simplified Chinese.

- [ ] Step 1: Write RED test

```tsx
it("hides Windows taskbar controls on a Linux bootstrap", async () => {
  renderSettings({
    platform: { platform: "linux", taskbarStatus: false, floatingBall: true },
  });
  expect(screen.getByRole("group", { name: "Floating ball" })).toBeInTheDocument();
  expect(screen.queryByRole("group", { name: "Taskbar status" })).not.toBeInTheDocument();
});
```

- [ ] Step 2: Run RED

```powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri test -- src/surfaces/Settings.test.tsx src/surfaces/settings/tabs/TaskbarTrayTab.test.tsx
```

- [ ] Step 3: Implement platform-aware UI

Thread bootstrap capabilities into settings, hide only unsupported taskbar controls on Linux, keep the floating-ball group, add typed unavailable copy, and leave Windows DOM and persistence unchanged.

- [ ] Step 4: Run GREEN

```powershell
corepack pnpm@10.18.1 --dir apps/desktop-tauri test
corepack pnpm@10.18.1 --dir apps/desktop-tauri run build
```

- [ ] Step 5: Commit

```powershell
git add apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/test/profileUsageFixtures.ts apps/desktop-tauri/src/surfaces apps/desktop-tauri/src/styles.css
git commit -m "Hide unsupported Linux taskbar controls"
```

### Task 8: Add Ubuntu Tauri configuration and Debian artifact verification

**Files:**
- Create: apps/desktop-tauri/src-tauri/tauri.linux.conf.json
- Modify: apps/desktop-tauri/package.json
- Modify: apps/desktop-tauri/src-tauri/Cargo.toml
- Create: scripts/linux-release-build.sh
- Create: scripts/verify-linux-release-artifacts.sh
- Create: scripts/linux-release-build.test.sh
- Test: shell syntax, Debian metadata, manifest, hash, and dpkg-deb tests

**Interfaces:**
- pnpm run tauri:build:windows keeps the existing NSIS command.
- pnpm run tauri:build:linux runs tauri build --config src-tauri/tauri.linux.conf.json --bundles deb.
- linux-release-build.sh --version 1.1.0 --output artifacts/linux-release stages the amd64 deb, SHA256SUMS.txt, SPDX SBOM, and target manifest.
- verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release validates metadata and hashes.

- [ ] Step 1: Write RED checks

```bash
test -f apps/desktop-tauri/src-tauri/tauri.linux.conf.json
test "$(jq -r '.bundle.targets[0]' apps/desktop-tauri/src-tauri/tauri.linux.conf.json)" = "deb"
grep -q "codex-barbar_.*_amd64.deb" scripts/verify-linux-release-artifacts.sh
```

- [ ] Step 2: Run RED

```bash
bash scripts/linux-release-build.test.sh
```

Expected result: the Linux config and scripts are absent.

- [ ] Step 3: Implement packaging

Add package id com.naipi11.codexbarbar, Utility category, MIT metadata, generated desktop entry, icons, and WebKitGTK/GTK/AppIndicator/Secret Service runtime dependencies. Stage the Debian package, SBOM, manifest, and hashes. Verify package name, version, architecture amd64, expected desktop/executable/icon paths, no traversal/absolute entries, and hash matches.

- [ ] Step 4: Run the Ubuntu package build

```bash
sudo apt-get update
sudo apt-get install -y curl libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev patchelf libfuse2 file build-essential jq dpkg-dev
export TAURI_LINUX_AYATANA_APPINDICATOR=1
corepack pnpm@10.18.1 --dir apps/desktop-tauri install --frozen-lockfile
corepack pnpm@10.18.1 --dir apps/desktop-tauri run tauri:build:linux
bash scripts/linux-release-build.sh --version 1.1.0 --output artifacts/linux-release
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release
```

- [ ] Step 5: Commit

```powershell
git add apps/desktop-tauri/src-tauri/tauri.linux.conf.json apps/desktop-tauri/package.json apps/desktop-tauri/src-tauri/Cargo.toml scripts/linux-release-build.sh scripts/verify-linux-release-artifacts.sh scripts/linux-release-build.test.sh
git commit -m "Add Ubuntu Debian packaging"
```

### Task 9: Extend PR and release workflows for Windows and Ubuntu

**Files:**
- Modify: .github/workflows/pr-check.yml
- Modify: .github/workflows/release.yml
- Create: scripts/aggregate-release-assets.mjs
- Create: scripts/aggregate-release-assets.test.mjs
- Modify: scripts/release-doctor.ps1
- Modify: docs/release/ci-cd.md
- Test: workflow policy and aggregation tests

**Interfaces:**
- PR jobs: existing Windows gate plus linux-check on ubuntu-24.04.
- Release jobs: windows-build, linux-build, and publish with needs on both builds.
- aggregate-release-assets.mjs --version 1.1.0 --windows artifacts/windows-release --linux artifacts/linux-release --output artifacts/aggregate-release creates one aggregate SHA256SUMS.txt and validates both target manifests.
- Only publish may call gh release create.

- [ ] Step 1: Write RED aggregation test

```js
test("aggregates Windows and Linux assets and rejects target mismatch", async () => {
  await expect(aggregateReleaseAssets({
    version: "1.1.0",
    windows: windowsFixture,
    linux: linuxFixture,
  })).resolves.toBeUndefined();
  expect(readText("SHA256SUMS.txt")).toContain("_x64-setup.exe");
  expect(readText("SHA256SUMS.txt")).toContain("_amd64.deb");
});
```

- [ ] Step 2: Run RED

```powershell
node scripts/aggregate-release-assets.test.mjs
./scripts/assert-release-workflow.ps1
```

Expected result: the aggregation module is missing and the current policy has no Linux build/publish split.

- [ ] Step 3: Implement the dual-platform workflow

Keep the Windows gate and staging behavior. Add Ubuntu dependency/setup/build/test/Debian verification and upload a Linux artifact group. Download both groups in publish, run the Dependabot high/critical gate once, aggregate hashes/SBOM manifests, and create exactly one draft Release. Preserve v* tag resolution and workflow-dispatch version checks.

- [ ] Step 4: Run GREEN

```powershell
node scripts/aggregate-release-assets.test.mjs
./scripts/assert-release-workflow.ps1
./scripts/assert-v1-boundaries.ps1
```

- [ ] Step 5: Commit

```powershell
git add .github/workflows/pr-check.yml .github/workflows/release.yml scripts/aggregate-release-assets.mjs scripts/aggregate-release-assets.test.mjs scripts/release-doctor.ps1 docs/release/ci-cd.md
git commit -m "Build Ubuntu artifacts in CI"
```

### Task 10: Document and manually accept the Ubuntu release

**Files:**
- Create: docs/LINUX_ACCEPTANCE.md
- Modify: README.md
- Modify: README.zh-CN.md
- Modify: docs/BUILDING.md
- Modify: docs/RELEASING.md
- Modify: docs/ARCHITECTURE.md
- Modify: CHANGELOG.md
- Modify: VERSIONING.md
- Create: docs/verification/linux/ubuntu-24.04-acceptance.md
- Test: documentation consistency and Ubuntu desktop acceptance

**Interfaces:**
- README installation links distinguish _x64-setup.exe from _amd64.deb.
- LINUX_ACCEPTANCE.md records Ubuntu version, session type, package hash, tray/panel/settings/float-ball behavior, notification result, XDG autostart result, Secret Service result, and unsupported taskbar behavior.
- Release publication requires Windows and Ubuntu CI success plus the acceptance record.

- [ ] Step 1: Write RED documentation check

```powershell
rg -n "Ubuntu|\.deb|amd64|Wayland|Secret Service|AppIndicator" README.md README.zh-CN.md docs/BUILDING.md docs/RELEASING.md docs/ARCHITECTURE.md
```

Expected result: current docs describe only Windows NSIS/portable artifacts.

- [ ] Step 2: Implement documentation

Add English and Simplified Chinese quick starts, Linux dependencies, AppIndicator caveat, XDG autostart behavior, keyring/no-plaintext policy, Wayland float-ball fallback, and exact Debian asset name. Keep Windows instructions unchanged.

- [ ] Step 3: Perform Ubuntu desktop acceptance

```bash
sudo apt install ./codex-barbar_1.1.0_amd64.deb
codex-barbar
```

Record tray menu, panel, settings, Current CLI refresh, float-ball movement/rotation, notification capability, autostart create/remove, Secret Service round-trip, no taskbar helper windows, and clean exit under GNOME Wayland and X11 when available.

- [ ] Step 4: Run final gates

Windows:

```powershell
./scripts/local-check.ps1 -All -ReleaseDoctor -Version 1.1.0
```

Ubuntu:

```bash
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
corepack pnpm@10.18.1 --dir apps/desktop-tauri test
corepack pnpm@10.18.1 --dir apps/desktop-tauri run build
bash scripts/verify-linux-release-artifacts.sh --version 1.1.0 --assets artifacts/linux-release
```

Expected result: both platform suites and artifact verification pass with no unresolved acceptance item.

- [ ] Step 5: Commit

```powershell
git add README.md README.zh-CN.md docs/BUILDING.md docs/RELEASING.md docs/ARCHITECTURE.md docs/LINUX_ACCEPTANCE.md docs/verification/linux CHANGELOG.md VERSIONING.md
git commit -m "Document Ubuntu release and acceptance"
```

After Task 10, bump every product manifest to the selected release version, tag the exact commit, run the dual-platform release workflow, inspect the draft assets, and publish only after Windows and Ubuntu jobs are green.
