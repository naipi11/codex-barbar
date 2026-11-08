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
$env:CODEXBAR_PROOF_MODE = 'settings:advanced'   # or 'trayPanel:ready'
$env:CODEXBAR_PROOF_DATA_DIR = Join-Path ([IO.Path]::GetTempPath()) ('codexbar-proof-' + [guid]::NewGuid().ToString('N'))
.\target\release\codex-barbar.exe
```

Close any running instance before launching (single-instance plugin hands
off to the old process). Attach screenshots or a short proof note to PRs.

## Records

Past evidence: `docs/verification/windows/` (screenshots, startup
performance, proof matrices). New RC evidence for 1.0.0-rc.1 goes to
`docs/release/v1-rc-report.md`.

## v1.1.5 CI repair and release qualification (2026-11-08)

- The source push of 34f8c827 succeeded; hosted PR check failed on Rust/Clippy 1.99 because async-trait 0.1.89 emitted redundant must-use attributes. The same failure was reproduced locally with 1.99.0, then fixed by upgrading async-trait to 0.1.92 without lint suppression.
- With patched dependencies, Rust 1.99.0 strict Clippy passed for both manifests; shared tests passed (547 unit, 21 process contracts, 3 resources, plus 1 intentionally ignored manual resolver probe), and Tauri tests passed (267). Frontend Vitest 4.1.12 passed 299 tests and Vite 6.4.5 built successfully.
- Full pnpm audit, including development dependencies, reported no known vulnerabilities. License, V1-boundary, release-workflow policy, and format checks passed. rustls 0.23.46/webpki 0.103.16 address the known Rust TLS alerts.
- Release CI now audits production and development dependencies. Its mandatory dedicated-token Dependabot gate remains enabled; unreadable alerts or open high/critical findings still fail the release. Draft creation explicitly targets the validated GITHUB_SHA.
- Published assets must come from the clean release workflow and include its five-file manifest/SBOM contract, not the earlier two-entry local ZIP. This local qualification does not itself assert that a GitHub release is published or that the hosted checks passed.

## v1.1.5 Codex compatibility: local evidence (2026-11-08)

- Platform: native Windows 11 x64, OS 10.0.26200.0, Rust 1.97.1, Node 24.21.0, project pnpm 10.18.1. CI Node 20 parity remains a hosted check.
- Final shared backend: **547 unit tests passed, 1 explicit real-machine resolver test ignored**; **21 process-contract tests** and **3 resource tests passed**.
- Final Tauri crate: **267 tests passed**. Frontend: **39 files / 299 tests passed**, production Web build passed.
- `cargo fmt --all --check` and Clippy for both manifests with `--all-targets -- -D warnings` passed. V1 boundary and release-workflow guards, production dependency audit and license audit passed.
- Real installed Codex 0.167.0: native executable discovery, initialize, account/read and account/rateLimits/read succeeded for the signed-in Current CLI. Empty isolated CODEX_HOME correctly reported signedOut after a successful handshake. See [redacted smoke evidence](verification/windows/codex-0.167.0-smoke.json).
- Proof data can be isolated using an absolute CODEXBAR_PROOF_DATA_DIR; proof does not change the Windows autostart registration. Logs still use the default log directory. Do not invoke managed login or manual account refresh while collecting synthetic UI evidence.
- Optimized Tauri Release / NSIS build completed. Packaged portable ZIP passed CRC checking and its executable SHA-256 matched the release build; Windows file version is 1.1.5. The local installer is unsigned (Authenticode NotSigned).
- CUA Driver 0.22.0 exercised the freshly built Release app with isolated proof data: Advanced path input → Validate and Save displayed detected Codex 0.167.0; a read-only check of the isolated SQLite database confirmed the override was saved. Navigation to About displayed version 1.1.5. [Native screenshot](images/windows-proof/codex-barbar-1.1.5-about.png).
- General showed theme=System with a dark settings surface while the float-ball window existed. Toggling Start at login changed its UIA checked state from true to false, while both canonical and legacy autostart registry values remained unchanged. CUA actions initially reported unverifiable; success is based on subsequent snapshots/pixels and database/registry assertions, not only command exit codes.
- The proof process and task-started CUA daemon were stopped; the original installed tray process was restored. The agent did not replace the installed binary. The user subsequently reported that installing the delivered 1.1.5 build succeeded; this is user-reported installation evidence, not a full upgrade/uninstall matrix.
- Not a complete installer-upgrade/uninstall, multi-monitor/DPI, enterprise proxy, keyring, or real managed-login acceptance run. GitHub release publishing and installed-binary replacement are outside this local update.

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
