# codex-barbar Windows proof

This document describes how to produce a fresh `codex-barbar` Windows desktop
binary and how to record real-Windows evidence for tray, flyout, settings,
theme, DPI, and keyboard behavior before a UI/tray/settings release.

## Build

From the repository root (the Tauri crate is the workspace default member):

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

The fresh binary lands at `target/debug/codex-barbar.exe`. Always rebuild after
UI-affecting changes and stop any already-running `codex-barbar` instance
before launching the new binary (the single-instance plugin hands off to the
old process otherwise):

```powershell
Get-Process -Name codex-barbar -ErrorAction SilentlyContinue | Stop-Process -Force
```

## Proof mode

`CODEXBAR_PROOF_MODE` accepts only the fixed, credential-free scenarios:

```text
trayPanel:ready     trayPanel:stale     trayPanel:error
trayPanel:api       trayPanel:profiles
settings:general    settings:providers  settings:advanced    settings:about
taskbar-status:{ready|warning|critical|refreshing|stale|missing}
float-ball:{ready|warning|critical|refreshing|stale|missing}
taskbar-status:weekly    float-ball:weekly
taskbar-status      float-ball            (aliases for the ready state)
```

Proof mode suppresses blur-dismiss and serves synthetic account/usage data to
the proof UI; it does not resolve Codex for those payloads or display real
identity data. Normal local repository and recovery startup still runs, so use
a clean Windows test profile when a strict no-touch environment is required.

`taskbar-status:weekly` and `float-ball:weekly` are deterministic compact
status proofs: one 10080-minute weekly window with 98% remaining, the fixed
2099-08-20 future reset, trusted fresh data, and a safe proof identity whose compact form
is exactly six visible characters. They intentionally contain no 300-minute
window, so no `5H` cell may be rendered. Both proof settings use independent
taskbar-status and float-ball background opacities of 20.

```powershell
$env:CODEXBAR_PROOF_MODE = 'trayPanel:ready'
Start-Process -FilePath '.\target\debug\codex-barbar.exe' -WindowStyle Hidden
```

## CUA driver verification

The preferred automation is [trycua/cua](https://github.com/trycua/cua) via
`cua-driver`:

```powershell
$cua = Join-Path $env:LOCALAPPDATA 'Programs\Cua\cua-driver\bin\cua-driver.exe'
& $cua call list_windows '{}'
```

Use the returned window IDs for `get_window_state`, clicks,
Tab/Enter/Space/Escape, and screenshots. Capture every scenario in the list
above plus the interaction matrix below.

## Required matrix

- Flyout is a 400x520 logical-pixel panel clamped inside the current monitor
  work area (8 px inset) at 100/150/200% DPI; taskbar on all four edges.
- When enabled in General settings, `taskbar-status` is a non-activating
  transparent overlay with 40 logical-pixel height and content-sized width
  clamped to 104–318 logical pixels, placed between the task-list area and the
  notification area. The taskbar starts from a functional 318px failure
  fallback. The visible taskbar WebView is never measured. While taskbar status
  is enabled, a hidden independent 318x40 WebView renders the shared content
  geometry and is the sole width source. Failure preserves the functional 318px
  fallback. A successful weekly proof normally shrinks below 318px. It must
  remain inside the taskbar
  slot after a taskbar move, DPI change, auto-hide transition, or Explorer
  restart. Verify the independently persisted taskbar-status and float-ball
  background opacity states at 0, 20, and 80; text, quota band, focus, and
  close affordances remain visible at opacity 0.
- When enabled in General settings, `float-ball` is an always-on-top animated
  status surface: 88 × 88 logical pixels collapsed and 260 × 148 logical pixels
  expanded. It must remain inside the monitor work area, preserve its logical
  position across DPI changes, and open the tray panel on a click without
  taking focus. Hovering for less than 180 ms keeps it collapsed; at 180 ms it
  expands.
- Left tray click toggles the flyout; right click opens the native menu
  without opening the flyout.
- Keyboard-only traversal: Tab moves focus, Enter/Space activates, Escape
  dismisses the flyout.
- Long Chinese/English account labels do not escape the panel; inner content
  scrolls instead.
- Theme `system` follows the OS signal without one WebView flipping another;
  explicit light/dark palettes apply `data-theme`.
- Settings opens all tabs; About offers only manual update checking.

## Permanent close verification

The surface close X is verified outside proof mode: enable the surface from
Settings, click its X, read only the persisted boolean flag, restart without
`CODEXBAR_PROOF_MODE`, and confirm the surface does not return. Proof mode
intentionally reactivates its requested surface on every launch, so it cannot
be used for close-persistence assertions.

## Status-surface close proof states

```text
Unlocked frontend close:
  CDP clicks the real taskbar close button with no SQLite writer.
  visible + measurement targets disappear and persisted enabled becomes false.

Unlocked native close:
  WM_CLOSE is sent to the exact visible root HWND with no SQLite writer.
  the same typed controller converges to false and destroys both HWNDs.

Locked persistence retry:
  an exclusive diagnostic writer is acquired and verified before the click.
  first click rolls back to true and exposes the fixed-size red retry state.
  release is observed before the second click; retry converges to false.

Locked taskbar persistence retry:
  acquire and independently verify an exclusive writer;
  click the real close control;
  rebuilt visible/helper windows and persisted true prove rollback;
  rebuilt visible root must expose data-error=true, retry title, and live text;
  release must be observed before retry destroys both and persists false.

Locked float-ball persistence retry:
  acquire and independently verify an exclusive writer;
  click the real float close control;
  surviving or rebuilt native float window and persisted true prove rollback;
  close feedback must be visible after recreation;
  release must be observed before retry destroys the window and persists false.
```

The locked rollback preserving `true` is expected: it proves the controller
restored the prior persisted/runtime state after the diagnostic writer blocked
persistence. It is not a normal close failure.

## Real quota comparison

Proof aliases are synthetic visual fixtures only. Outside `CODEXBAR_PROOF_MODE`,
manually refresh the selected Codex account and compare the actual period,
remaining percentage, and reset date with the current Codex UI at the same
observation time. Do not treat the weekly proof's fixed 98% fixture as live
quota evidence.

## Native fallback

When the CUA driver is unavailable, record equivalent evidence with Win32
window enumeration (`GetWindowRect`/`IsWindowVisible`) plus `PrintWindow`
screenshots, and say so in the observation file. Geometry assertions and proof
parsing still have Rust unit coverage in `window_positioner.rs` and
`proof_harness.rs`.

For the two auxiliary surfaces, also record the extended styles
`WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, and `WS_EX_LAYERED`, the foreground
window state, the monitor DPI, and the work-area-safe rectangle. The current
implementation record is
[taskbar-floatball-identity-2026-08-07.md](verification/taskbar-floatball-identity-2026-08-07.md).

## Evidence location

Screenshots captured before the off-screen measurement probe are historical
failure evidence only. They may document clipping regressions, but they cannot
serve as passing evidence for the compact weekly taskbar surface.

`docs/verification/windows/2026-08-14/cua-observations.md` is the immutable
record of that prior failed same-WebView/off-screen-replica architecture. Do not
edit it or reuse it as passing evidence for the independent measurement window.
Record the independent-window proof in
`independent-measurement-observations.md` instead.

Record non-secret observations under:

```text
docs/verification/windows/${ExecutionDate}/cua-observations.md
docs/verification/windows/${ExecutionDate}/independent-measurement-observations.md
docs/verification/windows/${ExecutionDate}/screenshots/*.png
```

Use `$ExecutionDate = Get-Date -Format 'yyyy-MM-dd'`. Screenshots must not
contain emails, tokens, raw paths, or private account data.
