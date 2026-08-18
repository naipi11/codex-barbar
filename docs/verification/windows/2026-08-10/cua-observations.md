# Glass Orbit Windows Verification

Date: 2026-08-13
Product surface: codex-barbar desktop (fresh debug build, no credentials used)

## Recorded facts

| Fact | Value |
| --- | --- |
| Binary path | target/debug/codex-barbar.exe |
| Binary length | 27,334,144 bytes |
| Binary last write | 2026-08-13 20:01:34 (local) |
| Commit SHA | 4e271d6a9a65e02ad58c77824c5409309b1d1491 |
| Windows monitor scale | 100% (effective DPI 96x96 reported by GetDpiForMonitor) |
| CUA driver | unavailable (not installed); Win32 fallback used |

CUA driver status: unavailable. The standard install path
`%LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe` does not exist and no
`cua-driver` process is running. Equivalent evidence was recorded with Win32
`EnumWindows`, `IsWindowVisible`, `GetWindowRect`, `GetDpiForMonitor`, and
`PrintWindow` (PW_RENDERFULLCONTENT) plus synthesized mouse input, as
`docs/WINDOWS_PROOF.md` requires.

## Deterministic proof scenarios

Every credential-free scenario launched a fresh instance with
`CODEXBAR_PROOF_MODE` set, produced the expected window, and captured a
`PrintWindow` PNG under `screenshots/`.

| Scenario | Window | Observed rect | Inside taskbar | Screenshot |
| --- | --- | --- | --- | --- |
| taskbar-status:ready | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-ready.png |
| taskbar-status:warning | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-warning.png |
| taskbar-status:critical | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-critical.png |
| taskbar-status:refreshing | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-refreshing.png |
| taskbar-status:stale | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-stale.png |
| taskbar-status:missing | codex-barbar taskbar status | 875,775 318x48 | true | taskbar-missing.png |
| float-ball:warning | codex-barbar float ball | 1257,593 88x88 | n/a | float-warning.png |
| float-ball:critical | codex-barbar float ball | 1257,593 88x88 | n/a | float-critical.png |
| float-ball:refreshing | codex-barbar float ball | 1257,593 88x88 | n/a | float-refreshing.png |
| float-ball:stale | codex-barbar float ball | 1257,593 88x88 | n/a | float-stale.png |
| float-ball:missing | codex-barbar float ball | 1257,593 88x88 | n/a | float-missing.png |
| float-ball:ready (collapsed) | codex-barbar float ball | 1257,593 88x88 | n/a | float-ready-collapsed.png |
| float-ball:ready (hover 700ms) | codex-barbar float ball | 1108,495 260x148 | n/a | float-ready-expanded.png |

The taskbar overlay target width is 318 logical pixels. It stays between the
task-list area and the notification area and is fully contained inside the
taskbar rectangle `(0,775)-(1463,823)` in every state.

The float ball is 88 x 88 collapsed and 260 x 148 expanded. Hovering for less
than 180 ms keeps it collapsed; after the 180 ms threshold the native window
resizes to 260 x 148 and repositions so the expanded card stays inside the
work area.

## Required matrix results

| Check | Result | Evidence |
| --- | --- | --- |
| Taskbar ready layout | PASS | taskbar-ready.png; 318x48 inside taskbar rect; shows identity, 5-hour/weekly quotas, reset, urgent value, and separate X |
| Float hover expansion | PASS | float-ready-collapsed.png (88x88) to float-ready-expanded.png (260x148 at 1108,495), hover 700 ms |
| Taskbar X persists false | PASS | window gone after X click; persisted flag taskbarStatusEnabled=false |
| Float X persists false | PASS | window gone after X click; persisted flag floatBallEnabled=false |
| Float position restore | PASS | collapsed position 1257,593 restored after disable/re-enable/restart |
| Theme isolation | PASS | float surfaces render the dark navy palette while app theme is system; screenshots show the dark palette |

### Interaction observations

- Body click (mouse down/up without movement) opened the hidden main panel;
  `IsWindowVisible` for the `codex-barbar` main window changed false -> true.
- A drag gesture (pointer moved beyond the 4 px threshold) did not open the
  panel; the main window stayed hidden.
- Escape dismissed the panel (main window returned to hidden).
- Float window extended styles 0x80C0198 include WS_EX_NOACTIVATE (0x08000000),
  WS_EX_LAYERED (0x00080000), and WS_EX_TOOLWINDOW (0x00000080).

## Permanent close and restart convergence (outside proof mode)

1. Both surfaces were enabled, the app launched without `CODEXBAR_PROOF_MODE`,
   and both windows appeared (float 1257,593 88x88; taskbar 875,775 318x48).
2. Clicking the float X destroyed the window and persisted
   `floatBallEnabled=false` while `taskbarStatusEnabled` stayed true.
3. Clicking the taskbar X destroyed the window and persisted
   `taskbarStatusEnabled=false`.
4. A restart without proof mode recreated neither surface (0 status windows).
5. Re-enabling both settings and restarting recreated both windows, and the
   float ball restored its collapsed position (1257,593).

Close-X persistence must be checked outside proof mode because proof mode
intentionally reactivates its requested surface on every launch.

## Notes

- Only boolean flags, window labels, rectangles, DPI, and pass/fail states are
  recorded. No emails, tokens, database rows, or secret paths are committed.
- Observed hover timing was occasionally delayed on long-lived instances
  (WebView2 timer throttling for no-activate tool windows); a fresh instance
  measured deterministic 88x88 -> 260x148 expansion within 700 ms and stable
  collapse when the pointer left.