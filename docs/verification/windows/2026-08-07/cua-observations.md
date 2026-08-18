# Windows proof observations — 2026-08-07

**Build:** `codex-barbar` debug desktop built from the current
`codex/taskbar-floatball-identity` worktree with the proof-scenario harness,
taskbar overlay, float ball, and work-area clamping.

**Build command:** `corepack pnpm@10.18.1 run tauri:build:debug`

**Artifacts:** `target/debug/codex-barbar.exe` and
`target/debug/bundle/nsis/codex-barbar_1.0.0_x64-setup.exe`.

**Environment:** Windows 11, 1920x1080 (Windows scaling 100%), primary monitor
work area 1707x912 (taskbar bottom).

**Tooling note:** `%LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe`
is not installed on this machine, so the CUA driver CLI was unavailable.
Equivalent observations were recorded with Win32 window enumeration
(`GetWindowRect`/`IsWindowVisible`) and `PrintWindow` screenshots of the fresh
debug binary. Every scenario was launched with `CODEXBAR_PROOF_MODE=<scenario>`
after stopping the previous instance. The proof UI received synthetic data and
did not display real identity or secret material; normal local repository and
recovery startup still occurred.

## Scenario matrix

| Scenario | Window visible | Size (px) | Work-area safe | Screenshot |
|---|---|---|---|---|
| `trayPanel:ready` | yes | 400x520 | yes (inside 1707x912) | `screenshots/trayPanel-ready.png` |
| `trayPanel:stale` | yes | 400x520 | yes | `screenshots/trayPanel-stale.png` |
| `trayPanel:error` | yes | 400x520 | yes | `screenshots/trayPanel-error.png` |
| `trayPanel:api` | yes | 400x520 | yes | `screenshots/trayPanel-api.png` |
| `trayPanel:profiles` | yes | 400x520 | yes | `screenshots/trayPanel-profiles.png` |
| `settings:general` | yes | 720x580 | yes | `screenshots/settings-general.png` |
| `settings:providers` | yes | 720x580 | yes | `screenshots/settings-providers.png` |
| `settings:advanced` | yes | 720x580 | yes | `screenshots/settings-advanced.png` |
| `settings:about` | yes | 720x580 | yes | `screenshots/settings-about.png` |
| `taskbar-status` | yes | 390x72 physical at 150% DPI | yes | `screenshots/taskbar-status-debug-final.png` |
| `float-ball` | yes | 108x108 physical at 150% DPI | yes | `screenshots/float-ball-debug-final.png` |

## Checks

- Flyout uses the fixed 400x520 logical target and is clamped inside the
  primary monitor work area with an 8 px inset; the observed 400x520 window
  never overflows 1707x912.
- Settings remains a detached 720x580 window; all four tab scenarios open the
  settings window directly.
- `ProofScenario::ALL_NAMES` is the exact fixed, credential-free list and each
  name round-trips through `CODEXBAR_PROOF_MODE` (Rust tests).
- `place_flyout` shrinks rather than escapes a small work area and honors
  monitor scale 1.0–2.0 (Rust tests).
- Blur-dismiss is suppressed in proof mode; windows remained visible for
  capture in every scenario.
- The taskbar overlay was observed between the task-list and notification
  areas, with an 8 px clearance before the notification area. It was
  non-activating and did not become the foreground window.
- The float ball stayed inside the work area at 150% DPI and rendered the
  synthetic `Ming Zhao` identity with a usage percentage.

## Not covered by this machine

- Four taskbar edges, multiple monitors, and 150%/200% DPI were verified in
  pure geometry tests only except for the recorded 150% taskbar/float-ball
  snapshot; no second monitor or changed-DPI session was available on this
  host.
- Native tray left/right click and keyboard-only traversal require an
  interactive desktop session with the CUA driver; they are covered by unit
  tests in this commit and should be re-run on the automation host before
  release.
- Explorer restart, float-ball drag persistence, and click-to-open behavior
  were not covered by the fallback capture.
