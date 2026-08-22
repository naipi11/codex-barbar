# codex-barbar

> **English** | [中文](README.zh-CN.md)

<p align="center">
  <a href="https://github.com/naipi11/codex-barbar/releases/latest"><img src="https://img.shields.io/github/v/release/naipi11/codex-barbar?display_name=tag&sort=semver&style=flat-square&label=release&color=2563eb" alt="Latest release"></a>
  <a href="https://github.com/naipi11/codex-barbar/actions/workflows/pr-check.yml"><img src="https://img.shields.io/github/actions/workflow/status/naipi11/codex-barbar/pr-check.yml?branch=main&style=flat-square&label=CI&color=16a34a" alt="CI status"></a>
  <a href="https://github.com/naipi11/codex-barbar/releases"><img src="https://img.shields.io/github/downloads/naipi11/codex-barbar/total?style=flat-square&label=downloads&color=7c3aed" alt="Total downloads"></a>
  <a href="https://github.com/naipi11/codex-barbar/stargazers"><img src="https://img.shields.io/github/stars/naipi11/codex-barbar?style=flat-square&label=stars&color=f59e0b" alt="GitHub stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/naipi11/codex-barbar?style=flat-square&color=64748b" alt="MIT license"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows-11%20x64-0078D4?style=flat-square&logo=windows&logoColor=white" alt="Windows 11 x64">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust 2024">
  <img src="https://img.shields.io/badge/React-18-61DAFB?style=flat-square&logo=react&logoColor=111827" alt="React 18">
</p>

**A Windows tray app that shows your Codex usage and limits at a glance.**

The floating ball stays icon-sized and clockwise. Color shows remaining quota
(green / gold / red). Speed shows activity: idle, thinking ×2, Fast ×3.

codex-barbar is a Windows 11 x64 tray application for tracking your
[Codex](https://developers.openai.com/codex/) usage and quota limits. It
talks to the official Codex App Server protocol, keeps credentials local,
and shows live quota in your tray, taskbar, and a floating ball.

![codex-barbar overview](docs/images/showcase/hero-en.png)

_Rendered from the current React/CSS components. Every account name, date,
and quota value in the showcase is synthetic demo data._

## Visual Tour

### Quota color states

![Green, yellow, and red floating-ball quota states](docs/images/showcase/float-ball-colors-en.png)

- Green: 67–100% remaining
- Yellow: 34–66% remaining
- Red: 0–33% remaining

### Activity motion

![Idle, Thinking, and Fast floating-ball rotation speeds](docs/images/showcase/float-ball-motion-en.gif)

Color and speed are independent. Color shows remaining quota; clockwise
rotation shows activity: **Idle 1×**, **Thinking 2×**, **Fast 3×**.

### Resident taskbar status

![Compact taskbar status showing account, weekly quota, and reset date](docs/images/showcase/taskbar-status-en.png)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=naipi11/codex-barbar&type=Date)](https://star-history.com/#naipi11/codex-barbar&Date)

## Features

- Tray panel with account, quota, reset time, and manual refresh
- Taskbar status overlay (optional, opacity adjustable)
- Floating status ball (optional) with green / yellow / red quota colors
- Clockwise activity animation: Idle 1×, Thinking 2×, Fast 3×
- Auto refresh on a configurable interval (default 5 minutes)
- Shows the real OpenAI account name (not "current cli")
- Weekly quota with reset countdown
- Per-user install, no admin required
- No telemetry; credentials stay local (DPAPI protected)

## Quick Start

1. Go to [Releases](https://github.com/naipi11/codex-barbar/releases/latest).
2. Download `codex-barbar_<version>_x64-setup.exe` (recommended) or `codex-barbar_<version>_x64-portable.zip`.
3. Run the installer (per-user, no admin needed) or extract and launch the portable build.
4. The app starts in the system tray. Click the tray icon to open the usage panel.
5. If it shows **Not signed in**, run `codex login` in a terminal, then click **Refresh** in the panel.
6. Open **Settings → General** to configure the taskbar status and floating ball. New installs enable **Start at login** and the floating ball; the taskbar status is optional and off by default.

## Requirements

- Windows 11 x64 (23H2 or newer)
- A working [Codex](https://developers.openai.com/codex/) installation signed in with `codex login`

## First Run

Your data is stored under `%LOCALAPPDATA%\codex-barbar`. To remove all
accounts and cache, close the app and delete that directory, or use the
uninstaller's confirmation prompt.

## Settings

- **General** — start at login, taskbar status, floating ball, opacity, refresh interval, display mode, theme, language
- **Accounts** — manage signed-in Codex accounts
- **Advanced** — Codex executable path validation and diagnostics export
- **About** — version and update check

## Data & Privacy

- Reads usage through the official but experimental `codex app-server` stdio JSONL protocol. `experimentalApi` stays disabled; private `/wham/*` calls are removed.
- Managed profiles use an isolated `CODEX_HOME`, force file credential storage, and protect credentials with Windows DPAPI (Current User only).
- The current CLI profile is read-only. codex-barbar never logs in, logs out, switches, or deletes accounts on your behalf.
- No telemetry. Startup never checks for, downloads, or applies updates; you trigger update checks manually from the UI.

## Development

```text
# Rust backend / CLI
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings

# Tauri shell crate
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings

# Frontend (use pnpm)
pnpm --dir apps/desktop-tauri install
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run tauri:build
```

See [docs/BUILDING.md](docs/BUILDING.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Docs

- [PRIVACY.md](PRIVACY.md) — what is stored, where, and the threat boundary
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) — error states and recovery steps
- [docs/TESTED_CODEX_VERSIONS.md](docs/TESTED_CODEX_VERSIONS.md) — tested Codex version limits
- [CHANGELOG.md](CHANGELOG.md) — release history

## Upstream

codex-barbar is a Windows port of [CodexBar](https://github.com/steipete/CodexBar).
The audited source record and frozen baseline are documented in
[UPSTREAMS.md](UPSTREAMS.md).

## License

MIT — see [LICENSE](LICENSE).
