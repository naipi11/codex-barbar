# Status Surface Close Feedback Repair Design

Date: 2026-08-15

Status: Approved design, awaiting implementation plan

## Problem

The taskbar and float-ball close buttons currently keep failure feedback in
component-local React state. The native controller uses a runtime-first
transaction:

1. disable and destroy the status-surface WebView;
2. persist the disabled setting;
3. if persistence fails, restore the previous runtime state.

When persistence is locked, step 1 destroys the WebView that invoked the
command. Step 3 creates a replacement WebView, but the rejected promise and
its React `catch` belong to the destroyed page. The replacement page starts
with `closeFailed = false`, so the runtime and persisted state roll back
correctly while the required retry UI disappears.

Fresh Windows evidence in
`docs/verification/windows/2026-08-15/status-surface-repair-observations.md`
proved this exact split: the taskbar visible and measurement windows returned,
and the persisted flag remained `true`, but `data-error`, the retry title, and
the live-region text did not appear.

The same failure mode exists for the float ball because it also stores close
failure only in its WebView-local React state.

## Goals

- Preserve close failure feedback across destruction and recreation of either
  status-surface WebView.
- Use one shared mechanism for taskbar status and float ball.
- Make frontend close and native `WM_CLOSE` converge through the same
  controller semantics.
- Keep feedback transient, stable, privacy-safe, and recoverable through both
  bootstrap and live events.
- Preserve the existing runtime-first settings transaction, window lifecycle,
  geometry, proof isolation, and provider data boundaries.
- Re-run every blocked Windows gate before release or installation.

## Non-goals

- Do not persist close feedback in `settings.json` or SQLite.
- Do not reorder the transaction to persistence-first.
- Do not introduce a two-phase hide/finalize window lifecycle.
- Do not add a new Tauri command, capability, permission, dependency, database
  column, or package-manager artifact.
- Do not expose raw Rust, SQLite, Tauri, WebView, path, or account data to the
  frontend or logs.
- Do not redesign the taskbar or float-ball visual language.

## Considered Approaches

### 1. Runtime latch plus bootstrap and event synchronization — selected

Store close feedback in the existing native status-surface runtime state.
Latch the error before rollback recreates a WebView, expose it in bootstrap,
and emit a live event for a WebView that survives the failure.

This keeps the current transaction and lifecycle invariants while covering
both destroyed and surviving pages.

### 2. Persistence-first disable — rejected

Persist `false` before destroying the windows. A locked repository would leave
the original WebView alive to show its local error, but a later native close
failure would require a compensating settings write. This reverses the
approved runtime-first invariant and creates a new persisted-false/live-window
failure mode.

### 3. Two-phase hide and finalize — rejected

Hide but retain the WebView until persistence succeeds, then destroy it; show
it again on failure. This would require a larger lifecycle state machine for
the taskbar visible/helper pair and float ball. It is unnecessary once a
native feedback latch can bridge recreation.

## Architecture

### Native feedback state

`StatusSurfaceState` owns a transient feedback value for each surface:

```text
StatusSurfaceFeedbackState
  taskbarStatus: none | closeFailed
  floatBall:     none | closeFailed
```

The state is process-local and defaults to `none`. It is not part of
`AppSettings`, `SettingsPatch`, `SettingsRepository`, or any database schema.
Restarting the application clears it.

The existing `SurfaceRuntime` transaction port gains a narrow feedback
mutation operation. The Tauri adapter implements it by mutating the feedback
inside the same `StatusSurfaceState` mutex that owns the taskbar and float-ball
managers. Test fakes record the feedback operations and their ordering.

### Bootstrap DTO

`BootstrapDto` gains a frozen, read-only nested DTO:

```text
statusSurfaceFeedback:
  taskbarStatusCloseFailed: boolean
  floatBallCloseFailed: boolean
```

The names serialize with the existing camel-case bridge convention. Synthetic
proof bootstrap always returns both values as `false` unless a proof test
explicitly supplies a transient runtime state.

`get_bootstrap_state` reads `AppState` and `StatusSurfaceState` without holding
both mutexes at once. The command first builds the existing account/settings
bootstrap, releases that guard, then reads a feedback snapshot and attaches it.
This preserves the established lock order and avoids coupling account storage
to window runtime state.

### Live feedback event

Add the frozen event name:

```text
status-surface-feedback-changed
```

Its payload is:

```text
surface: taskbarStatus | floatBall
closeFailed: boolean
```

The event is emitted after a close transition succeeds or fails. Event
delivery remains nonfatal and logs only a stable diagnostic code. Bootstrap is
the durable in-process source of truth for a recreated page; the event handles
a page that remains alive. Correctness does not depend on event timing.

No event contains a raw backend error.

### Frontend ownership

`useStatusSurface` becomes the only frontend owner of close feedback:

- initialize from `BootstrapDto.statusSurfaceFeedback`;
- buffer an early feedback event with the same latest-value pattern already
  used for settings events;
- apply later live events by surface;
- optimistically clear the target surface before a retry;
- set the target to failed if the typed command rejects in a surviving page;
- rethrow the stable command failure so existing call semantics remain intact.

`TaskbarStatus` removes its component-local `closeFailed` state and renders the
taskbar value from `useStatusSurface`. `FloatBall` removes only its local close
error; expansion errors remain separate. The components may swallow the
already-reflected close rejection in their click handlers to avoid an unhandled
promise, but they do not own the feedback state.

Existing user-facing copy remains:

- taskbar: `关闭失败，点击重试`;
- float ball: `关闭失败，请重试`.

## Transaction and Feedback Semantics

The controller applies the following rules for each surface independently.

### Attempt start

- Clear stale close feedback before an enable or disable attempt.
- Enabling a surface never creates a close failure state.

### Runtime disable failure

- Set `closeFailed` before returning the stable runtime error.
- A surviving WebView receives the live event and its local command rejection.
- If later reconciliation recreates the WebView, bootstrap restores the latch.

### Persistence failure after runtime disable

- Set `closeFailed` before calling runtime rollback.
- Roll back to the previous enabled state.
- A replacement WebView therefore reads `closeFailed = true` during its first
  bootstrap even if it loads before the original command returns.
- Return the existing stable settings-save error.

### Rollback failure

- Keep `closeFailed` set.
- Preserve the existing force-enabled fallback and stable
  `STATUS_SURFACE_ROLLBACK_FAILED` behavior.
- Do not expose the underlying rollback text.

### Success

- Keep feedback cleared.
- Persist the requested enabled value.
- Emit the nonfatal `closeFailed = false` event.
- A successful disable destroys the surface normally.

### Retry

- Clear feedback at retry start.
- If the retry fails, latch it again using the rules above.
- If the retry succeeds, both native runtime and persisted state converge to
  disabled and the windows disappear.

### Native close

Native `CloseRequested` continues to call the same typed controller through
`schedule_set_enabled`. It uses the same latch, rollback, event, and bootstrap
behavior as a frontend close button. No separate native error path is added.

## Invariants Preserved

- Runtime transition occurs before settings persistence.
- Taskbar measurement closes before the visible taskbar window.
- The taskbar logical range remains 104..318, fallback 318, and height 40.
- The measurement helper remains hidden, non-focusable, and 318x40.
- Width mutation remains authorized only for the exact measurement label.
- The 2-second reconciliation interval and identity-safe deferred cleanup stay
  unchanged.
- Float-ball Dark theme, saved collapsed position, drag threshold, expansion
  dimensions, and monitor behavior stay unchanged.
- Status proof remains runtime-only and mutually exclusive.
- Provider identity and usage data remain siloed and redacted.

## Error Handling and Privacy

- Bridge feedback is a boolean state, not an error string.
- Logs use stable codes only.
- Failure to emit the feedback event is nonfatal and cannot change transaction
  success or failure.
- Bootstrap is the recovery path when an event is lost or arrives before a new
  listener is registered.
- Settings or feedback bootstrap failure keeps the existing unavailable UI; it
  never fabricates success.
- Screenshots use synthetic proof identities and never include account data,
  tokens, cookies, database text, DevTools ports, or private paths.

## Automated Test Strategy

### Rust controller tests

Use RED -> minimal GREEN -> refactor. Tests must prove observable ordering and
state rather than only checking helper constants.

- attempt start clears only the target surface feedback;
- successful disable leaves feedback clear and persists `false` once;
- runtime disable failure sets feedback before returning;
- persistence failure sets feedback before the `false -> true` rollback;
- rollback failure leaves feedback set and preserves force-enabled behavior;
- an enable attempt clears stale close feedback but does not create a close
  error when it fails;
- taskbar and float-ball feedback remain isolated;
- native and frontend callers use the same controller entry point.

### Bridge and event tests

- bootstrap serializes both feedback fields with exact camel-case names;
- production bootstrap reads the native feedback snapshot;
- proof bootstrap defaults both fields to `false`;
- the event name and payload wire values are frozen;
- event failure remains nonfatal and never logs raw error text;
- no repository write is introduced by feedback reads or events.

### React hook tests

- bootstrap initializes both surface feedback values;
- an early feedback event wins over a later stale bootstrap;
- live events update only their target surface;
- a retry clears its target without clearing the peer;
- command rejection sets failure for a surviving page and rethrows;
- remounting after rollback recovers failure from bootstrap.

### Surface tests

- taskbar renders the red close state, stable title, fixed geometry, and hidden
  live region from shared feedback;
- taskbar retry clears then restores failure on a second rejection;
- float ball renders its close failure from shared feedback while preserving an
  independent expansion error;
- neither component retains a separate close-error source of truth;
- reduced-motion behavior remains unchanged.

## Windows Verification Gates

Run the complete source and Windows verification sequence from a fresh debug
binary. A focused test pass does not reopen release.

1. Run focused Rust/controller/bridge/frontend tests and full local CI:
   shared Rust, Tauri, frontend, format, both Clippy manifests, boundary guard,
   and production frontend build.
2. Build a fresh debug binary and record its hash, size, timestamp, and exact
   launched path.
3. Re-prove the decisive weekly taskbar row, hidden 318x40 helper, safe native
   slot, no `5H`, and peer isolation.
4. Re-prove 0% and 80% taskbar opacity through DOM, computed style, stable
   geometry, screen-composited captures, hashes, and material pixel change.
5. Re-prove unlocked frontend close and exact-root native `WM_CLOSE`.
6. Acquire and independently verify an exclusive diagnostic writer. For the
   taskbar close:
   - rollback must restore visible/helper windows and persisted `true`;
   - the rebuilt visible root must expose `data-error=true`, retry title, and
     live-region text;
   - after verified lock release, retry must destroy both windows and persist
     `false`.
7. Run the complete isolated float-ball proof: collapsed/expanded content,
   click versus drag, geometry, close, 0/80 screen composition, and Dark theme.
8. Under a verified exclusive writer, repeat the float-ball close rollback:
   rebuilt or surviving float UI must display its close error; after release,
   retry must destroy it and persist `false`.
9. Restore all four surface fields, release the writer, stop only the debug
   app, and confirm no app-under-test, proof DevTools, or writer remains.

Use Computer Use first for targetable interactions. If a no-activate window is
not targetable after the documented retries, use the approved separate-turn
DPI-aware Win32/CDP/screen-composited fallback and state that limitation.

## Release Boundary

Task 5, production NSIS, current-user installation, post-install proof, push,
and any release claim remain prohibited until every automated and Windows gate
above is PASS.

Any failed or inconclusive required item produces a truthful STOP. Evidence
must distinguish product failure, tool limitation, and a gate that was not run.
Invalid or privacy-unsafe screenshots are not committed.

## Acceptance Criteria

- Both status surfaces recover close failure after WebView recreation.
- A surviving status surface receives the same feedback without remounting.
- Taskbar locked rollback visibly exposes the fixed-size retry state and then
  succeeds after lock release.
- Float-ball locked rollback visibly exposes its close error and then succeeds
  after lock release.
- Existing lifecycle, geometry, proof, theme, settings, and privacy invariants
  remain unchanged.
- Full local CI and fresh Windows proof pass with no required inconclusive item.
- Only then may the existing release/install plan resume.
