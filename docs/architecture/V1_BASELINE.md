# V1 baseline

## Platform

V1 targets native Windows 11 23H2 or newer on x64 and uses the imported Win-CodexBar Tauri, React, Rust, Windows tray, testing, and packaging foundations. Windows 10, Windows on ARM, WSL, macOS, and Linux builds are outside V1.

## Product scope

V1 is Codex-only. Claude, Gemini, every other provider, browser-cookie import, generic API-key/token accounts, cost charts, sessions, workspaces, PTY, FloatBar, PopOut, and usage notifications are outside the release surface. The former Codex private HTTP integration is replaced as part of the V1 work.

Startup does not check for, download, or apply updates. A user-initiated action may check a public GitHub Release or open the fixed Releases page; no PAT is embedded or requested.

Product defaults are remaining-quota display, system theme, system language, autostart off, and no telemetry. Installed and portable builds store data under `%LOCALAPPDATA%\codex-barbar`.

## Upstream record and synchronization

The Windows implementation baseline is the imported Win-CodexBar history frozen at `b167e328147b93f997034a6b50c8b769d2a37f3b`, recorded by tag `upstream/win-codexbar-2026-08-03`. The upstream remote is a reference source, not an automatic update channel.

To consider a future Windows-base update, first run:

```powershell
git fetch win-upstream --tags
```

Then inspect the incoming history and make an explicit, reviewed merge. Do not merge upstream changes automatically.

The macOS CodexBar repository is a behavior reference only. No upstream issue or pull request may be created without user authorization.
