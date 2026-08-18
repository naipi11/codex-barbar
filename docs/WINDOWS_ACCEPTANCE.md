# Windows Acceptance

UI, tray, installer, and platform changes require real Windows evidence.
Unit tests do not prove WebView2, DWM, tray, or NSIS behavior.

## Automated gates

Every PR and release runs:

- `cargo fmt --all --check`
- clippy (`-D warnings`) and tests for `rust/` and the Tauri crate
- frontend tests and production build
- V1 boundary scan (`scripts/assert-v1-boundaries.ps1`)
- `pnpm audit --prod --audit-level high` and license audit
- Tauri x64 production build (PR gate and release)

## Manual / CUA matrix

Record evidence for:

- Tray icon visible and left-click opens the flyout; blur dismisses it
- Settings window opens from the tray menu; every tab renders
- Single instance: second launch focuses the existing app
- Autostart off by default; toggling writes the canonical
  `codex-barbar.exe --background` command
- Cached start under 3 seconds after warm-up
- Theme `auto` keeps all surfaces dark when WebView2 is shared
- 100/150/200% DPI on two displays and all four taskbar edges
- NSIS fresh install (HKCU scope), upgrade preserves data, default uninstall
  preserves data, explicit purge removes only `%LOCALAPPDATA%\codex-barbar`
- Portable ZIP expands to a temp dir, writes no file beside the exe, and
  still uses LocalAppData
- Windows 11 23H2+ x64 with the release-time current supported build

## Proof tooling

Use Cua Drivers (background computer-use) on a fresh local build:

```powershell
$env:CODEXBAR_PROOF_MODE = 'settings:menu'   # or 'trayPanel:ready'
.\target\release\codex-barbar.exe
```

Close any running instance before launching (single-instance plugin hands
off to the old process). Attach screenshots or a short proof note to PRs.

## Records

Past evidence: `docs/verification/windows/` (screenshots, startup
performance, proof matrices). New RC evidence for 1.0.0-rc.1 goes to
`docs/release/v1-rc-report.md`.

## 1.0.0-rc.1 host evidence (2026-08-07)

Host: Windows 11 x64, 1920x1080 at 100% scaling (no device identifiers
recorded). Cua Drivers are not installed on this host; equivalent Win32
window enumeration + pixel screenshots were used (documented fallback in
AGENTS.md).

Passed on the release build:

- Fresh real (non-proof) launch of `target/release/codex-barbar.exe` stays
  running as the tray process; single instance mutex is created.
- Proof scenarios `trayPanel:ready` and `settings:about` render the tray
  panel (400x520) and settings window (720x580) with synthetic,
  credential-free data; screenshots under
  `docs/images/windows-proof/v1/`.
- Cached startup: 5 runs measured 66–1203 ms (all ≤ 3 s budget); record in
  `docs/verification/windows/2026-08-07/startup-performance.md`.
- Portable ZIP expands to a temp dir, launches the GUI, writes nothing
  beside the executable, then stops cleanly.
- NSIS fresh silent install into a temp dir: HKCU uninstall key,
  DisplayVersion, Start Menu shortcut, x64 GUI binary, running tray
  process, upgrade preserving data, default uninstall preserving data, and
  uninstall cleanup all passed.
- Artifact verifier, release doctor, boundary scan, license audit, and
  deterministic SBOM generation all passed for the RC artifact set.

Still required on a clean machine before final 1.0.0:

- CUA Driver matrix: four taskbar edges, two displays, 150/200% DPI,
  animations off, keyboard/screen-reader names.
- Account/protocol failure matrix on disposable accounts: no Codex,
  incompatible version, not signed in, API key, managed login
  browser/device-code, two managed profiles, offline/timeout/rate-limit,
  recovery, App Server crash, and crash during vault operations.
