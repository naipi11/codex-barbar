# Independent Taskbar Measurement Windows Verification

Date: 2026-08-14

Source commit: `c6a90181a94d61e883835d62fc38c8cd38b73983`

Overall result: **STOP before Task 5 — weekly hard gate passed, but the
post-weekly opacity, close/disable, and float-ball gates did not pass.**

This document is the evidence timeline for the independent hidden
`taskbar-status-measure` WebView architecture. The adjacent
`cua-observations.md` is an immutable STOP record for the superseded
same-WebView/off-screen-replica architecture and is not passing evidence for
this run.

## Source gates

The pinned Task 4 focused checks completed before documentation or UI proof:

- Codex App Server: 72 passed, 0 failed, 1 ignored; 207 filtered out. Its
  integration target ran 0 tests with 17 filtered out.
- Settings repository: 9 passed, 0 failed; 271 filtered out. Its integration
  target ran 0 tests with 17 filtered out.
- Taskbar overlay: 32 passed, 0 failed; 111 filtered out.
- Status surfaces: 18 passed, 0 failed; 125 filtered out.
- Proof harness: 16 passed, 0 failed; 127 filtered out.
- Frontend: 26 files and 184 tests passed.

The complete pinned local gate
`./scripts/local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy` exited 0.
It passed the V1 boundary guard, Rust formatting, shared and Tauri clippy,
shared Rust tests (279 passed and 1 ignored, plus 17 contract tests), Tauri
tests (143 passed), frontend tests (26 files / 184 tests), and the frontend
production build (69 modules transformed). The first one-second observation
attempt was terminated by its caller after only the boundary guard; it was not
counted as a verification result. The command was rerun in full to its explicit
`Local checks passed.` terminal result.

The frontend commands used the repository-pinned pnpm 10.18.1 entry.

## Fresh debug build

Before the build, no running `codex-barbar.exe` process was present. The exact
pinned pnpm 10.18.1 CJS entry ran
`--dir apps/desktop-tauri run tauri:build:debug` and exited 0. The fresh debug
executable facts were:

| Fact | Value |
| --- | --- |
| Repository-relative path | `target/debug/codex-barbar.exe` |
| SHA-256 | `2425485D6183FD7623B3E6C1AD3E6E3DBBFFE487450E8E52AE36A0471FD10743` |
| Size | 27,509,248 bytes |
| Build result | PASS, including debug NSIS bundle generation |

Only that worktree executable was launched, as PID 25992, with
`CODEXBAR_PROOF_MODE=taskbar-status:weekly`.

## Decisive `taskbar-status:weekly` gate

Result: **PASS**

The bundled `@oai/sky` Computer Use service was initialized first. Both
`list_apps` and two `list_windows` observations returned other desktop apps but
did not expose the running non-activating CodexBar auxiliary windows. No window
object or coordinate was guessed. The documented read-only fallback was used:
DPI-aware Win32 enumeration, `IsWindowVisible`, `GetWindowRect`, extended-style
inspection, foreground state, taskbar-child rectangles, UI Automation, and
`PrintWindow(PW_RENDERFULLCONTENT)`.

| Criterion | Observation | Result |
| --- | --- | --- |
| Measurement helper exists | `codex-barbar taskbar measurement` / `Tauri Window` | PASS |
| Measurement visibility | `IsWindowVisible=false` | PASS |
| Measurement foreground/focus | not foreground; UIA keyboard-focusable=false; `WS_EX_NOACTIVATE` present | PASS |
| Measurement viewport | 557x70 physical at 168 DPI = 318x40 logical | PASS |
| Skip-taskbar behavior | hidden helper; zero matching CodexBar/measurement items in taskbar UIA | PASS |
| Visible complete sequence | `ProofU`, `周 98%`, `8/20`, `×` all present and adjacent | PASS |
| `5H` absent | no `5H` field in the capture | PASS |
| Visible logical size | 197x40, within 104..317 and exact height 40 | PASS |
| Visible focus behavior | not foreground; UIA keyboard-focusable=false | PASS |
| Taskbar-safe placement | task-list rect `(95,1356)-(326,1440)`; notification area begins at x=2040; overlay `(1687,1363)-(2032,1433)` lies inside the resulting x=326..2040 slot and ends 8 px before notification | PASS |

The visible overlay extended style was `0x80C0198`, containing no-activate,
tool-window, and layered behavior. The hidden helper extended style was
`0x8040110`: it contained no-activate but not the `WS_EX_TOOLWINDOW` bit. The
observable acceptance criterion is nevertheless satisfied because the helper
remained hidden, non-focusable, non-foreground, and absent from taskbar UIA.
This style difference is retained as a tooling/implementation note rather than
silently described as a tool-window flag.

Fresh evidence:

- `screenshots/taskbar-weekly-independent-measurement.png`
  (SHA-256 `5156E42DCEE4301F1FE94C2EA4A3EEF74D8AABF7AA750F320B6D67CFC5F44F47`).

Visual inspection confirmed the image contains only the proof fixture and no
real account data, secret, or private path.

## Post-weekly UI proof

The weekly PASS authorized the following work. It uncovered independent
post-weekly failures; no source fix, release build, installation, or Task 5 work
was attempted.

### Settings General

The regular Settings window was uniquely targetable through `@oai/sky` before
the helper later became unresponsive. UIA and screenshot-backed actions proved:

- the Taskbar status opacity slider reached 0% and 80%; the document text
  reported each endpoint;
- ArrowDown moved selection from General to Accounts and changed the visible
  pane; ArrowUp returned to General; and
- Escape dismissed Settings, after which `list_windows` returned zero matching
  Settings windows.

The Float-ball opacity slider was visible in the Settings capture but was not
independently operated in this run; its interaction result is **INCONCLUSIVE**.

`screenshots/settings-independent-general.png` records the General page at the
80% slider endpoint (SHA-256
`99DDDE70D2412323DBF1AC66C7D8243F05380208AFD11C23FE69EBB2B7704C81`).
It contains only local proof/settings UI and no account secret or private path.

Result: **PASS for the Taskbar status slider, keyboard navigation, and Escape;
INCONCLUSIVE for the Float-ball slider.**

### Taskbar opacity propagation

The Taskbar status slider value changed and persisted, but the running taskbar
surface did not visually update. Exact screen-composited crops were taken from
the resolved overlay rectangle after the 0% and 80% endpoints:

- `screenshots/taskbar-independent-opacity-0.png`
- `screenshots/taskbar-independent-opacity-80.png`

Both files are 25,550 bytes and have the identical SHA-256
`B9D45D3E28FB4D3BA0D7681A0C852F350A3276E9E7B75A1F3B9C5A9CF9221764`.
Visual inspection likewise found no changed background compositing. An initial
`PrintWindow` comparison was rejected because layered WebViews return
uncomposited content; the decisive comparison used `CopyFromScreen` on the
exact physical overlay rectangle.

Result: **FAIL — the Taskbar status slider accepted 0/80, but the live taskbar
did not reflect the change.** The images contain only the synthetic `Ming Z`
proof fixture and no private data.

### Close behavior

#### Frontend X and red retry state

The standard CUA driver executable was absent. Bundled `@oai/sky` never exposed
the no-activate taskbar window. DPI-aware `SendInput` and direct child
`WM_LBUTTONDOWN`/`WM_LBUTTONUP` were tested against the resolved close glyph;
the safe main-region control showed that this fallback session did not deliver
WebView pointer interaction, so no red retry screenshot was fabricated.

A separate SQLite writer held an uncommitted `BEGIN EXCLUSIVE`; a second
`BEGIN IMMEDIATE` returned the exact `database is locked` result. This proved
the persistence-failure precondition, but the unavailable WebView click path
prevented a valid visual red-button assertion.

Result: **INCONCLUSIVE/BLOCKED for the frontend X and red retry state.** No
red-state screenshot was fabricated or committed.

#### Native close, persistence, and helper destruction

The native lifecycle boundary was tested independently by posting standard
`WM_CLOSE` to the exact visible `taskbar-status` HWND. Tauri owns a
`CloseRequested` handler for that message and routes it through the typed
controller. After four seconds:

- the visible taskbar window still existed;
- `taskbar-status-measure` still existed; and
- the persisted `taskbarStatusEnabled` field remained true.

Result: **FAIL — native close did not converge, close persistence did not
become false, and disable did not destroy the helper.**

### Float-ball weekly, drag/click, and dark-theme isolation

#### Proof activation and visibility

After an explicit per-process environment launch with
`CODEXBAR_PROOF_MODE=float-ball:weekly`, Win32 found the float-ball HWND but
`IsWindowVisible` was false. Its rectangle was `(88,88)-(324,242)` physical at
168 DPI, not a visible 88x88 logical weekly ball. The persisted taskbar surface
remained visible, indicating that proof activation did not converge through the
taskbar-disable/float-enable transition.

Result: **FAIL — float-ball proof activation did not produce a visible weekly
surface.**

#### Collapsed/expanded, drag/click, and dark-theme isolation

Because the decisive collapsed surface was not visible, expanded geometry,
drag-versus-click, and dark-theme isolation could not be truthfully tested.
`PrintWindow` on the hidden float HWND timed out and made the app unresponsive.
That timeout is a tooling limitation, not a product-failure classification. The
invalid background crop and incomplete hidden-window capture were excluded from
the evidence commit.

Result: **INCONCLUSIVE/BLOCKED — no valid visible collapsed/expanded,
drag/click, or dark-theme-isolation evidence.**

### Tool recovery and local-state restoration

On a later Settings restart, `@oai/sky` timed out twice while activating the
fresh window. Per the Computer Use recovery contract, no further CUA input was
issued. Direct Settings UIA toggle fallback also timed out, and the exact
app-under-test process became unresponsive. Only that process was stopped.

The Task 4 interactions had changed only the four status-surface fields. With
the app stopped, those fields were restored to the observed pre-proof values
without reading or replacing the rest of the settings JSON:

```text
taskbarStatusEnabled=false
taskbarStatusOpacity=20
floatBallEnabled=true
floatBallOpacity=20
```

Final running `codex-barbar.exe` process count: 0.

The two invalid generated files `taskbar-independent-close-retry.png` and
`float-ball-independent-weekly-20.png` were resolved to this run's exact
screenshots directory before deletion was attempted. The delete was denied by
tool policy, so both remain untracked and are excluded from every stage/commit.

## Tooling and privacy boundary

The preferred automation was bundled Computer Use through `@oai/sky`. It could
not target the non-activating auxiliary windows, so the documented Win32/UIA,
`PrintWindow(PW_RENDERFULLCONTENT)`, and exact screen-crop fallback recorded the
limitations above. Only the app-under-test was stopped. Every selected image
was inspected and contains proof fixtures/settings only: no real account data,
secret, or private path.
