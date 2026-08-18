# Off-screen Taskbar Measurement Windows Verification

Date: 2026-08-14

Result: **STOP — decisive weekly compact-layout gate failed**

## Build and source gates

| Fact | Value |
| --- | --- |
| Source commit | `6efa1d3d030c9b64a01d672d0d5d3a643e7c46b6` |
| Fresh debug executable | `target/debug/codex-barbar.exe` |
| Debug SHA-256 | `6D4A1F4843EAC951B6A231BB9CFF6DBC390395073C9A96AF9F3F8CD51F1A0DD5` |
| Debug size | 27,447,808 bytes |
| Debug build result | PASS |
| Full local check | PASS |

The original Task 9 focused checks passed:

- Codex App Server: 72 passed, 0 failed, 1 ignored.
- Settings repository: 9 passed, 0 failed.
- Taskbar overlay: 21 passed, 0 failed.
- Proof harness: 15 passed, 0 failed.
- Frontend: 24 files and 168 tests passed.

`./scripts/local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy` exited 0.
Its full runs reported shared Rust 279 unit tests passed with 1 ignored plus 17
contract tests, Tauri 128 tests passed, frontend 24 files / 168 tests passed,
and a successful frontend production build. The pinned build command was the
repository-declared pnpm 10.18.1 CJS entry followed by
`--dir apps/desktop-tauri run tauri:build:debug`; it exited 0 and rebuilt the
debug executable and debug NSIS bundle from the source commit above.

## Tool status

The preferred CUA Driver was unavailable: the standard
`%LOCALAPPDATA%/Programs/Cua/cua-driver/bin/cua-driver.exe` did not exist and no
`cua-driver` process was running. The bundled Windows computer-use service could
not target the non-activating taskbar tool window. The documented fallback was
therefore used: Win32 window enumeration, `IsWindowVisible`, DPI-aware
`GetWindowRect`, extended-style inspection, taskbar child rectangles, and
`PrintWindow(PW_RENDERFULLCONTENT)`.

Only the exact app-under-test process was started and stopped. No Explorer,
Codex, OpenCodex, or unrelated application process was stopped.

## Decisive `taskbar-status:weekly` proof

The fresh debug executable was launched with
`CODEXBAR_PROOF_MODE=taskbar-status:weekly` and allowed to settle before capture.

| Observation | Value | Result |
| --- | --- | --- |
| Monitor DPI | 168 (175%) | recorded |
| Taskbar orientation | bottom | recorded |
| Overlay physical rect | `(1743,1363)-(2088,1433)`, 345x70 | recorded |
| Overlay logical rect | `(996,779)-(1193,819)`, 197x40 | width/height PASS |
| Taskbar physical rect | `(0,1356)-(2560,1440)` | recorded |
| Notification area physical rect | `(2096,1356)-(2560,1440)` | recorded |
| Task-list physical rect | `(95,1356)-(326,1440)` | recorded |
| Taskbar-safe placement | overlay inside taskbar, ending 8 physical px before notification area | PASS |
| Extended styles | `0x80C0198`; no-activate/tool-window/layered flags present | PASS |
| Foreground window | no | PASS |

Field acceptance:

| Criterion | Observation | Result |
| --- | --- | --- |
| Identity `ProofU` | fully visible | PASS |
| Quota `周 98%` | begins after identity but is clipped at the right edge; the full label is not cleanly visible | **FAIL** |
| Reset `8/20` | absent from the captured surface | **FAIL** |
| Close `×` visible and adjacent | absent from the captured surface | **FAIL** |
| No `5H` | no `5H` cell is visible | PASS |
| Logical height exactly 40 | 40 | PASS |
| Logical width 104..318 and below 318 | 197 | PASS |
| Rectangle inside taskbar-safe region | yes | PASS |

Fresh failing evidence:

- `screenshots/taskbar-weekly-probe-20.png` — fresh build, FAIL.
- `screenshots/taskbar-weekly-before-probe.png` — copied from the prior
  `taskbar-weekly-final-check.png`, historical FAIL only.

The fresh 197x40 capture still does not contain the complete required sequence
`ProofU | 周 98% | 8/20 | ×`. This is the decisive STOP condition from the
approved design and Task 3 brief.

## Blocked work and limitations

The following work was intentionally **not run** after the weekly failure:

- taskbar opacity 0/80 proof;
- float-ball and Settings proof;
- close persistence, drag/click, theme, Explorer, and DPI-transition proof;
- official resolver, normal persistence, and live quota comparison;
- production NSIS build and release-artifact verification;
- current-user installation and installed-path classification.

Accordingly there is no production installer SHA-256 and no installed-process
path classification for this run. No release build or install was attempted,
and no user settings, account cache, wrapper, token, cookie, or private account
data was read or changed. A new architecture review is required before another
geometry implementation attempt.
