# Architecture (V1)

codex-barbar is a Tauri 2 desktop tray app for Windows 11 x64. The active
product surface is `apps/desktop-tauri`; the shared domain/backend and CLI
live in the `rust/` crate `codexbar`.

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
  plugin, command registration, tray setup. Binary: `codex-barbar.exe`.
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
   `%LOCALAPPDATA%\codex-barbar\settings.json` via `Settings::load`/`save`
   and `secure_file` (DPAPI-capable). Frontend `updateSettings` patches →
   save → `codexbar:settings-updated` events.

3. **Tray**
   `tray_bridge` + `tray_menu`; icon pixels come from the shared
   `codexbar::tray::{render_bar_icon_rgba, render_percent_icon_rgba}`.

4. **Accounts**
   `codexbar::accounts` owns the SQLite store, DPAPI vault, isolated
   `CODEX_HOME` runtimes, recovery, and the App Server job. The React
   WebView receives only redacted DTOs.

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
  install.
- Validate tray/DPAPI/installer behavior on native Windows; WSL is
  insufficient.

## Release graph boundaries

V1 ships one provider (Codex via the official App Server), one desktop
binary, an NSIS setup, and a portable ZIP. Legacy providers, cookie import,
auxiliary always-on-top surfaces, telemetry, and auto-update are removed
from the compiled graph; `scripts/assert-v1-boundaries.ps1` enforces the
frozen allowlist.

## Related docs

- [BUILDING.md](./BUILDING.md) — build / test / release commands
- [RELEASING.md](./RELEASING.md) — release workflow and artifacts
- [WINDOWS_PROOF.md](./WINDOWS_PROOF.md) — Windows proof checklist
- Root [AGENTS.md](../AGENTS.md) — agent/contributor guidelines
