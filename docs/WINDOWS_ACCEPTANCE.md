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
- Autostart on by default; toggling reconciles the canonical
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

## v1.1.4 notification and taskbar fix: pre-release evidence

- The v1.1.3 notification engine failed the new regression for fresh snapshots
  with corrected reset times: it emitted `WeeklyReset` instead of a quota
  warning. The corrected engine passes, including restart, missing timestamps,
  backward corrections, separate profiles, and subsequent real weekly cycles.
- Native Windows tests for both Rust crates and Clippy with `-D warnings`
  passed. Frontend: 39 files / 299 tests passed. Production Tauri/NSIS build
  completed with pnpm 10.18.1.
- Pre-fix WebView2 at 114x26 CSS pixels / DPR 2 had intersecting quota/date
  bounds and clipped text. The fresh Release build passed layout checks at
  DPR 1, 1.5, and 2, plus 114x26 and 104x40 constrained viewports; all text
  remained within the viewport without intersecting bounds.
- CUA screenshots confirmed legible taskbar text on real 150% and 200% displays.
  Foreground CUA dragging moved both primary and secondary overlays; a drag
  across DPI regions restored the assigned taskbar's full 342x80 physical bounds.
  A subsequent click opened the tray panel. Background CUA drags were ineffective;
  window positions and captures, not the driver's `unverifiable` result, were used
  to verify foreground actions.
- Screenshot: `docs/images/windows-proof/taskbar-cross-dpi-fixed.png` uses the
  synthetic weekly proof identity and quota. Native 100% display testing was not
  performed; DPR 1 coverage above used WebView2 emulation.
- Deployed the local Release binary over the installed executable after backing
  up the previous binary. SHA-256 matched the build; normal startup persisted the
  new weekly-cycle observation state. Account data and preferences were not changed;
  window geometry was restored after the proof run. This local build retains the
  1.1.3 version number and is not a new published release.

## v1.0.34 panel regression

Date: 2026-09-10

Passed:

- Release CI run `34441665799` completed all Windows build, test, audit, and
  artifact-verification steps successfully.
- The tray panel keeps logical sizing at native Windows DPI and measures its
  rendered content height so lower actions remain visible.
- A fresh installation of v1.0.34 was manually opened and the panel was
  confirmed to display completely.

Focused automated evidence:

- `corepack pnpm@10.18.1 test --run src/surfaces/TrayPanel.test.tsx`
- `cargo test --quiet --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml flyout_window`

The full multi-monitor, taskbar-edge, installer-upgrade, and keyboard/CUA
matrix remains a separate acceptance requirement.

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
