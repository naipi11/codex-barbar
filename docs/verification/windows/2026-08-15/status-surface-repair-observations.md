# Status Surface Repair Windows Verification

Date: 2026-08-15

Overall result: **STOP before Task 5.** The source, fresh-build, weekly
taskbar, live-opacity, unlocked frontend-close, and unlocked native-close
gates passed. The required locked-persistence red retry state failed to appear,
so the isolated float-ball proof was not authorized.

## Starting boundary

- HEAD and verification base were
  `475d411c70085c8414c088fc7ea9bdaa9465fb18`.
- The worktree contained exactly seven protected untracked 2026-08-14 PNGs.
  They were neither modified nor staged.
- Initial process counts were debug 0, release 0, installed 1. Only the exact
  installed app-under-test instance was stopped before the fresh build.
- The four persisted surface fields started as taskbar disabled, taskbar
  opacity 20, float-ball enabled, and float-ball opacity 20.
- An exclusive transaction probe succeeded and rolled back, establishing that
  no external SQLite writer was present.

## Source gates

Every required source command produced complete output and final exit 0:

| Gate | Fresh result |
| --- | --- |
| Codex app-server focused tests | 72 passed, 0 failed, 1 ignored |
| Settings repository focused tests | 9 passed, 0 failed |
| Tauri proof harness focused tests | 20 passed, 0 failed |
| Tauri taskbar overlay focused tests | 32 passed, 0 failed |
| Tauri status surfaces focused tests | 19 passed, 0 failed |
| Frontend focused/full command | 26 files, 188 tests passed |
| Full shared Rust in local-check | 279 passed, 0 failed, 1 ignored |
| App-server contract in local-check | 17 passed, 0 failed |
| Full Tauri in local-check | 148 passed, 0 failed |
| Full frontend in local-check | 26 files, 188 tests passed |
| Format / shared clippy / Tauri clippy | exit 0 |
| Boundary guard | OK |
| Production frontend build | 69 modules transformed, exit 0 |

The complete gate ended with `Local checks passed.` and final exit 0. Frontend
commands used the repository-pinned pnpm 10.18.1 entry with `CI=true`.

## Fresh debug build

The exact pinned pnpm 10.18.1 command built the Tauri debug executable and NSIS
debug bundle with final exit 0.

| Fact | Value |
| --- | --- |
| Repository-relative executable | `target/debug/codex-barbar.exe` |
| SHA-256 | `D5AD711A886C452355F37DE0582030ED10CDA6CC818D2987955DB99F2674B94D` |
| Size | 27,512,320 bytes |
| Last write UTC | `2026-08-15T13:25:26.5959194Z` |

## Decisive `taskbar-status:weekly` gate

Result: **PASS**.

The no-activate window was not targetable through the preferred Computer Use
path. The documented DPI-aware Win32 and screen-composited fallback was used
without guessing a window handle.

| Criterion | Fresh observation | Result |
| --- | --- | --- |
| Measurement helper | one HWND, hidden | PASS |
| Helper geometry | 557x70 physical at 168 DPI, rounded 318x40 logical | PASS |
| Float peer isolation | zero visible float-ball HWNDs | PASS |
| Visible sequence | `ProofU`, `周 98%`, `8/20`, and `×` all complete | PASS |
| `5H` exclusion | absent from the composited capture | PASS |
| Visible geometry | `(1687,1363)-(2032,1433)`, 345x70 physical, rounded 197x40 logical | PASS |
| Fresh task-list endpoint | right edge x=326 | PASS |
| Fresh notification endpoint | left edge x=2040 | PASS |
| Safe placement | visible rect stayed inside x=326..2040 and ended 8 px before notification | PASS |
| Native style | `0x80C0198`, containing no-activate, tool-window, and layered flags | PASS |

Fresh screen-composited evidence:

- `screenshots/taskbar-repair-weekly.png`
- SHA-256
  `A2BF1A21CE8DD6B355119CC12A0BDF8D75BB826B13C81070BBEFA774C03F51C7`
- 19,564 bytes

Visual inspection confirmed only the fixed proof fixture was visible.

## Live taskbar opacity in `settings:general`

Result: **PASS**.

The fresh settings process exposed CDP only on loopback. The port was kept out
of evidence. Float-ball was disabled and taskbar status enabled through the
real typed surface command. Opacity changes used the real `update_settings`
command.

Before screen capture, the potentially user-derived taskbar text was replaced
in the rendered DOM with the fixed privacy fixture
`ProofU | 周 98% | 8/20 | ×`. This projection did not alter the native root,
runtime alpha, command path, or geometry. Its text remained stable across both
opacity changes.

| Observation | 0% | 80% |
| --- | --- | --- |
| Persisted command result | 0 | 80 |
| Rendered-root inline alpha | `0` | `0.8` |
| Rendered-root computed variable | `0` | `0.8` |
| Computed background | `rgba(24, 26, 34, 0)` | `rgba(24, 26, 34, 0.8)` |
| Visible physical rect | `(1624,1363)-(2032,1433)` | unchanged |
| Measurement physical rect | `(132,132)-(689,202)` | unchanged |
| WebView target / time origin / DOM root / HWND | baseline | all unchanged |

The measurement WebView's Tauri IPC route was instrumented in memory. A
read-only settings invocation proved the observer path was active; zero
`set_taskbar_status_width` calls occurred during the 0/80 changes.

Screen-composited evidence:

| File | SHA-256 | Bytes |
| --- | --- | ---: |
| `screenshots/taskbar-repair-opacity-0.png` | `4145EA17E7A9EA61D457B28FEA307D775CCC40C75C21315C5B9ED9BCD8948B7C` | 21,503 |
| `screenshots/taskbar-repair-opacity-80.png` | `93893F295ED48430A88B844E34CA87E7392957EED548BF97F25674D862D7C64D` | 13,548 |
| `screenshots/settings-repair-general.png` | `60E1CE1A24D5C22587A5B0C9CB9D6AF4A9052F1B1A7B05539AD34A4CE0FB11FA` | 74,841 |

The two taskbar captures had different bytes and hashes. Of 28,560 pixels,
27,890 changed and 27,459 had an aggregate RGB delta of at least 30. Mean
absolute per-channel delta was 139.102; maximum aggregate delta was 467.

Visual inspection confirmed the expected transparent/dark composition change
and found no identity, secret, or private path in any of the three images.

## Close scenarios

### Unlocked frontend close

Result: **PASS**.

An exclusive probe succeeded and rolled back immediately before the action.
CDP clicked the actual `.taskbar-status__close` button. Visible and measurement
CDP targets and HWNDs both reached zero, and the single persisted enabled flag
was false.

### Unlocked native close

Result: **PASS**.

The surface was re-enabled through the typed command. A fresh exclusive probe
again established that no writer existed. `WM_CLOSE` was posted to the exact
visible taskbar root. Visible and measurement CDP targets and HWNDs both
reached zero, and persisted enabled was false.

### Locked persistence retry

Result: **FAIL**.

The surface was re-enabled. A separate diagnostic connection acquired an
exclusive writer, and a second independent immediate transaction was blocked,
proving the required precondition. CDP then clicked the actual close button.

After the rollback settled:

- the visible taskbar HWND existed and was visible;
- the helper HWND existed and was hidden;
- persisted taskbar enabled remained true; and
- the process remained responsive.

Those observations prove the runtime/persistence rollback occurred. However,
for the full 15-second observation window the required retry UI never appeared:

- `data-error` was not `true`;
- the button title remained the normal close title; and
- the live-region retry text was absent.

No red-state screenshot was created because saving a normal close button as
retry evidence would be invalid. The diagnostic writer was explicitly released
and its release was confirmed before cleanup.

## Float-ball gate

Result: **NOT RUN — blocked by the earlier required close failure.**

The plan requires immediate STOP after a failed required item. No collapsed,
expanded, click, drag, close, opacity, or theme claim is made, and no float-ball
image was created.

## Restoration and privacy

The four surface fields were restored through typed commands and independently
read back as taskbar disabled, taskbar opacity 20, float-ball enabled, and
float-ball opacity 20. The exact debug process was stopped. Final counts were:

- app-under-test processes: 0;
- proof DevTools processes: 0;
- external writers: 0; and
- protected untracked 2026-08-14 PNGs: 7, unchanged.

Every retained image was visually inspected. Only the weekly proof fixture,
the fixed opacity privacy fixture, or the General settings page is present. No
real identity, account data, secret, raw database/CDP error, or private path is
included.

## Release decision

**STOP. Task 5, release, installation, and push remain prohibited.** The locked
persistence path must expose the specified red retry state and then pass the
release-and-retry close flow before the float-ball gate or Task 5 may begin.
