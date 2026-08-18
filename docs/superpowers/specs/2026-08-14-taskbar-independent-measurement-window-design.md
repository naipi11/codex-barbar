# Taskbar Independent Measurement Window Design

## Status

Approved design for the next taskbar-width architecture iteration.

This document supersedes the in-page measurement-replica architecture in
`2026-08-14-taskbar-unconstrained-measurement-design.md`. That earlier design
and its failed Windows proof remain historical evidence. They must not be used
as passing evidence for this design.

## Problem

The visible `taskbar-status` WebView is resized from measurements taken inside
the same WebView. Even after moving an inert replica off-screen, the native
viewport still constrained the effective layout. A fresh Windows build produced
a 197x40 logical window that showed `ProofU` and only the beginning of `周 98%`;
the reset date and close button were absent.

The frontend therefore cannot use any DOM inside the visible taskbar WebView as
the authority for its own native width.

## Decision

Create a second Tauri WebView window dedicated to width measurement.

- Visible window label: `taskbar-status`.
- Measurement window label: `taskbar-status-measure`.
- Visible window purpose: render and interact with the taskbar capsule only.
- Measurement window purpose: render identical geometry inside an independent
  318x40 logical viewport and submit the measured width.

The measurement window is created only while taskbar status is enabled and is
destroyed with it. It is permanently hidden, non-focusable, absent from the
taskbar, and pinned to `Theme::Dark`.

## First-Gate Windows Probe

Before the production lifecycle is changed, a minimal Windows probe must prove
that a Tauri WebView created with `visible(false)` still:

1. loads the React route;
2. performs CSS layout;
3. runs `ResizeObserver`;
4. measures the complete weekly geometry inside a 318x40 viewport; and
5. submits a width that lets the visible surface show the complete weekly row.

If the hidden WebView does not produce reliable layout, implementation stops.
The fallback architecture would require a separately approved design using a
visible but off-screen no-activate window. The implementation must not silently
switch to that architecture or add another recursive geometry patch.

## Window Architecture

### Visible taskbar window

The existing `taskbar-status` window remains the user-facing surface. It:

- starts at the safe fallback width of 318 logical pixels;
- renders only the visible `TaskbarStatusContents` mode;
- never owns a measurement ref;
- never calls the width bridge;
- retains no-activate, tool-window, layered, transparent, skip-taskbar,
  always-on-top, non-focusable, and fixed dark-theme behavior;
- remains 40 logical pixels tall; and
- is positioned by the existing taskbar-safe native slot calculation.

### Independent measurement window

The new `taskbar-status-measure` window:

- has a fixed 318x40 logical viewport;
- is created with `visible(false)`;
- is non-focusable, undecorated, non-resizable, non-maximizable,
  non-minimizable, non-closable, skip-taskbar, and `Theme::Dark`;
- never becomes visible, foreground, interactive, or user-closeable;
- renders only the measurement mode of the shared taskbar contents;
- contains no click, drag, close, focus, or navigation handlers;
- is `aria-hidden`, inert, and removed from tab order;
- uses intrinsic `max-content` geometry capped at 318 logical pixels; and
- is the only frontend source for taskbar content width.

The independent 318-pixel viewport is the isolation boundary. The measurement
root must not inherit width constraints from the visible taskbar WebView.

## Shared Presentation Model

`TaskbarStatusContents` remains the single markup path for:

```text
avatar | compact identity | real quota metrics | nearest reset | close
```

The visible and measurement routes must derive the same display properties from
the same status-surface model. They must not duplicate metric ordering, reset
selection, identity truncation, band selection, or label formatting.

The visible mode owns semantic buttons and handlers. The measurement mode owns
no handlers or accessible controls. Child test IDs remain visible-mode only so
tests do not encounter duplicate identities.

## Data Flow

1. Settings enable taskbar status.
2. Rust marks the taskbar surface enabled and creates the visible window at
   318x40.
3. Rust creates the hidden 318x40 measurement window.
4. Both routes obtain the same bootstrap snapshot and subscribe to the same
   status/settings events.
5. The measurement route renders shared geometry and observes its root.
6. The observer reads border-box and scroll width, rejects invalid values,
   rounds once to an integer, and calls `set_taskbar_status_width`.
7. Rust verifies that the caller is `taskbar-status-measure` and that taskbar
   status is still enabled.
8. Rust clamps the width to the inclusive 104 through 318 range and applies the
   existing native resize/reposition transaction.
9. Repeated widths are ignored. Later identity, quota, locale, font, reset, or
   settings changes trigger a new measurement.

The existing single-request-in-flight, latest-width queue remains in the
measurement route. No visible-window measurement fallback is allowed.

## Lifecycle and Recovery

### Enable

The visible window is required. Failure to create or position it fails the
enable operation through existing stable errors.

The measurement window is recoverable. If it cannot be created, the visible
surface remains enabled and usable at 318 pixels. Rust records a stable tracing
diagnostic, and the existing two-second status-surface monitor retries creation.

### Disable and close

Shutdown order is fixed:

1. destroy the measurement window;
2. close the visible window.

If measurement-window destruction fails, the visible window remains and the
enable state rolls back. If measurement destruction succeeds but visible-window
closure fails, enable state rolls back and the monitor recreates the measurement
window. Successful shutdown clears both cached handles while preserving normal
settings persistence semantics.

### Unexpected destruction

Unexpected destruction of the measurement window clears only its cached handle.
It does not disable the user's taskbar status. The monitor recreates it while the
surface remains enabled.

Unexpected destruction or native close of the visible window continues through
the typed status-surface controller. A stale width request received while the
surface is disabled is rejected without mutating the stored logical width.

## Width Authority and Failure Semantics

- Minimum supported width: 104 logical pixels.
- Maximum width: 318 logical pixels.
- Initial and first-failure fallback: 318 logical pixels.
- Height: 40 logical pixels.

Missing `ResizeObserver`, zero/NaN/infinite measurement, route failure, hidden
window creation failure, unauthorized caller, or bridge rejection must not
replace the safe fallback with a narrower inferred width.

After the first confirmed resize, failures preserve the last confirmed native
width. Native resize and reposition failures continue to use the existing Rust
transaction and rollback behavior.

Frontend code emits no console diagnostics. Rust uses stable, non-sensitive
`tracing` codes for measurement-window creation, lifecycle, and authorization
failures. No tokens, cookies, profile identities, raw bridge payloads, or private
paths may be logged.

## Width Command Authorization

The existing `set_taskbar_status_width` command remains the typed bridge, but it
must validate:

- invoking window label is exactly `taskbar-status-measure`;
- taskbar status is currently enabled; and
- the submitted value reaches the existing clamp/transaction boundary.

Calls from `taskbar-status`, `main`, `settings`, `float-ball`, unknown labels, or
the disabled state return a stable error and perform no resize, reposition, or
state mutation.

## Close-Failure Interaction

Long close-error text no longer participates in taskbar geometry.

When closing the taskbar surface fails:

- the close button enters a red error state;
- it performs one short shake animation unless reduced motion is requested;
- its title becomes `关闭失败，点击重试`;
- an `aria-live` message announces the failure; and
- the button remains enabled and retries the same typed close path.

The quota fields stay rendered and unchanged. The error state is fixed-size and
does not require a measurement-window event.

## Components and Ownership

Expected production boundaries:

- `taskbar_overlay/window.rs`: visible and measurement window labels, routes,
  builders, and fixed dimensions.
- `taskbar_overlay/mod.rs`: two cached handles, enable/disable ordering,
  recovery, monitor reconciliation, and width authorization state.
- `commands/status_surfaces.rs`: typed command caller-label and enabled-state
  validation.
- `status_surfaces.rs` and `main.rs`: destroyed-window routing that distinguishes
  the measurement helper from the user-facing status surface.
- `App.tsx`: route `taskbar-status-measure` to a measurement-only surface.
- `TaskbarStatus.tsx`: visible controller only.
- `TaskbarStatusMeasure.tsx`: measurement controller only.
- `TaskbarStatusContents.tsx`: shared geometry and visible/measurement semantics.
- `useTaskbarStatusWidth.ts`: replica-root measurement and serialized submission.

Exact filenames may follow current module conventions, but responsibilities may
not be merged back into the visible-window measurement path.

## Testing

### Rust tests

Cover:

- stable labels, routes, 318x40 measurement viewport, and dark-theme builder
  contract;
- visible-required and measurement-recoverable enable behavior;
- measurement-first shutdown and rollback for each failure point;
- unexpected measurement destruction and monitor recreation;
- caller-label allow/deny matrix;
- disabled-state width rejection without mutation;
- existing 104 through 318 clamp and resize/reposition rollback; and
- absence of dependency, theme, monitor-period, or taskbar-positioning changes.

### Frontend tests

Cover:

- `App` routing for both labels;
- visible route contains one visible geometry root and no measurement root;
- measurement route contains one inert measurement root and no visible root;
- shared field order and text for weekly and multi-window quota fixtures;
- only the measurement route creates `ResizeObserver` and invokes the width
  bridge;
- invalid measurement, missing observer, queueing, dedupe, retry, and unmount;
- close error red state, tooltip, retry, `aria-live`, and reduced motion; and
- no root opacity, quota-band, identity, reset, or accessibility regressions.

### Windows proof

After fresh debug build, prove:

- the measurement WebView is present but `IsWindowVisible` is false;
- it does not appear in the taskbar, foreground, Alt-Tab surface, or screenshots;
- the visible window shows `ProofU | 周 98% | 8/20 | ×` with no `5H`;
- logical height is exactly 40;
- normal weekly width is within 104 through 318 and strictly below 318;
- the rectangle remains in the taskbar-safe slot; and
- disabling the feature destroys both windows.

CUA Driver is the preferred proof tool. If unavailable, the documented
Win32/UIA plus `PrintWindow` fallback must record the limitation.

## Release Gate

Production NSIS build and current-user installation remain blocked until the
fresh weekly Windows proof passes every field, width, height, visibility, focus,
and safe-placement criterion.

Only after that gate passes may the workflow continue with opacity, float-ball,
Settings, close persistence, resolver/live quota, release artifact, and install
verification.

## Out of Scope

- Provider parsing or refresh behavior.
- Account identity or settings schema changes.
- Float-ball layout or lifecycle changes.
- OpenCodex wrapper or official Codex resolver changes.
- Installer path, Winget metadata, or release version changes.
- New dependencies or raw frontend window permissions.
- A visible off-screen measurement window without a separate approved design.

## Acceptance Criteria

- A hidden independent WebView, not the visible taskbar WebView, is the sole
  frontend width authority.
- The hidden-window probe passes before production lifecycle changes proceed.
- Visible and measurement routes use one shared geometry component and one
  derived presentation model.
- The helper exists only while taskbar status is enabled and is destroyed first.
- Measurement failure preserves a functional 318-pixel visible surface.
- Only the measurement window may submit width while enabled.
- Close failure uses a fixed-size retry state rather than inline geometry text.
- Fresh Windows proof shows the complete weekly sequence with no fabricated 5H.
- Normal weekly width is below 318 and height is 40.
- The helper remains hidden, non-focusable, absent from taskbar/Alt-Tab, and
  pinned to dark theme.
- Full Rust, Tauri, frontend, formatting, clippy, production frontend build, and
  Windows proof gates pass before release or installation.
