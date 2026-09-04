# Architecture (V1)

codex-barbar is a Tauri 2 desktop tray app for Windows 11 x64 and the Ubuntu
24.04 amd64 Debian target. The active product surface is
`apps/desktop-tauri`; the shared domain/backend and CLI live in the `rust/`
crate `codexbar`. Ubuntu package and desktop acceptance are separate gates;
see [LINUX_ACCEPTANCE.md](./LINUX_ACCEPTANCE.md).

## Modules

| Area | Path | Role |
|------|------|------|
| Shared backend + CLI | `rust/` (`codexbar`) | App paths, settings, accounts/vault, providers (Codex App Server), tray icon pixels, diagnostics, purge |
| Desktop shell | `apps/desktop-tauri/src-tauri/` (`codex-barbar-desktop-tauri`) | Tauri 2 host: startup coordinator, tray, windows, IPC commands, proof harness |
| Frontend | `apps/desktop-tauri/src/` | React 18 + Vite surfaces: hidden main webview routing to TrayPanel / Settings |

Cargo workspace (root `Cargo.toml`): members `rust` and
`apps/desktop-tauri/src-tauri`; the **default-member** is the Tauri crate.
The shell depends on `codexbar = { path = "../../../rust" }`.

## Entry points

- **Desktop**: `apps/desktop-tauri/src-tauri/src/main.rs` — fixed internal
  `--purge-user-data` early-exit, logging, coordinator, single-instance
  plugin, command registration, tray setup. Binary: `codex-barbar.exe` on
  Windows and `codex-barbar` in the Debian package.
- **CLI**: `rust/src/cli/` — subcommands only. Binary: `codexbar.exe`.
- **Frontend bootstrap**: `apps/desktop-tauri/src/App.tsx` routes by window
  label / surface mode (`TrayPanel`, `Settings`; `main` starts hidden).

## Data flow

1. **Profile refresh**
   `codexbar::core::instantiate_provider`
   (`rust/src/core/provider_factory.rs`) → `Provider::fetch_usage` → shell
   `commands/providers.rs` (semaphore + timeout) →
   `AppState.provider_cache` → events → React `useProviders`.

2. **Settings**
   `%LOCALAPPDATA%\codex-barbar\settings.json` on Windows and the platform
   config directory on Linux via `Settings::load`/`save` and `secure_file`.
   Frontend `updateSettings` patches → save → `codexbar:settings-updated`
   events.

3. **Tray**
   `tray_bridge` + `tray_menu`; icon pixels come from the shared
   `codexbar::tray::{render_bar_icon_rgba, render_percent_icon_rgba}`.

4. **Accounts**
   `codexbar::accounts` owns the SQLite store, platform credential vault,
   isolated `CODEX_HOME` runtimes, recovery, and the App Server job. Windows
   uses DPAPI Current User. Linux stores credentials in Secret Service and
   writes only a `codex-barbar-secret-service:v1:<profile-uuid>` marker to the
   vault; a locked or unavailable service must fail without a plaintext
   fallback. The React WebView receives only redacted DTOs.

5. **Diagnostics**
   `rust/src/rolling_log.rs` (5 MiB rotated, redacted, 14-day retention) and
   `rust/src/diagnostics.rs` (fixed-path redacted export with pre/post
   secret scans).

## Surfaces (desktop)

- **Tray panel** — left-click tray opens the flyout; blur dismisses it
  (suppressed in proof mode).
- **Settings** — detached window; tabs: `general`, `providers`,
  `notifications`, `menuBar`, `menu`, `usageSpend`, `advanced`, `about`.

## Concurrency & platform

- Rust edition 2024; async via Tokio.
- `AppCoordinator` owns ordered startup and one-shot graceful quit.
- Windows-specific: DPAPI Current User vault, DWM dark caption, tray
  promotion, start-at-login (`HKCU\...\Run`), WebView2, NSIS current-user
  install, and the taskbar status surface.
- Linux-specific: XDG current-user autostart creates/removes
  `~/.config/autostart/com.naipi11.codexbarbar.desktop`; Debian packaging
  depends on WebKitGTK, GTK, Ayatana AppIndicator, and Secret Service.
  Notifications report their platform capability and must be accepted on the
  target desktop. Linux intentionally has no taskbar status/measurement
  window. The floating ball falls back to a normal draggable window on
  Wayland, where compositor policy can limit positioning or always-on-top.
- Validate tray/DPAPI/installer behavior on native Windows. Validate Debian,
  GNOME/KDE, Wayland/X11, D-Bus, and Secret Service behavior on native Ubuntu;
  WSL and Windows-only tests are insufficient.

## Release graph boundaries

V1 ships one provider (Codex via the official App Server), one desktop
binary per platform, Windows NSIS/portable assets, and a Debian amd64 asset.
The current target Debian name is `codex-barbar_1.1.0_amd64.deb`. Its
publication requires green Windows and Ubuntu CI plus a completed Ubuntu
acceptance record. Legacy providers, cookie import, telemetry, and auto-update
are removed from the compiled graph; `scripts/assert-v1-boundaries.ps1`
enforces the frozen allowlist.

## Related docs

- [BUILDING.md](./BUILDING.md) — build / test / release commands
- [RELEASING.md](./RELEASING.md) — release workflow and artifacts
- [WINDOWS_PROOF.md](./WINDOWS_PROOF.md) — Windows proof checklist
- [LINUX_ACCEPTANCE.md](./LINUX_ACCEPTANCE.md) — Ubuntu proof checklist and
  pending evidence fields
- Root [AGENTS.md](../AGENTS.md) — agent/contributor guidelines
