# Taskbar Unconstrained Measurement Design

**Date:** 2026-08-14  
**Status:** Approved design pending written-spec review  
**Parent spec:** `docs/superpowers/specs/2026-08-14-status-surface-accuracy-density-opacity-design.md`

## Problem

The compact taskbar surface must show, in order, the avatar, six-character identity, real quota cells, nearest reset date, and close action. Its native window must remain 40 logical pixels high and between 104 and 318 logical pixels wide.

The current implementation measures the visible surface from inside a WebView whose viewport is itself controlled by the measured width. On a fresh Windows build this forms a feedback loop:

1. The native window starts narrow.
2. CSS clips or constrains descendants to that viewport.
3. DOM geometry reports the already-clipped width.
4. Rust resizes the native window to the clipped width.
5. The reset date and close action remain unreachable.

Recursive DOM geometry did not break the loop reliably. After four reviewed width commits, a fresh Windows proof still produced a 198x40 window clipped after the weekly percentage. Further recursive measurement patches are prohibited by this design.

## Decision

Use a hidden, unconstrained measurement replica rendered from the same presentational component as the visible taskbar content.

The measurement replica is the sole source of frontend content width. The visible surface is never measured. Rust remains the authority for clamping, native resize, positioning, and transactional rollback.

If measurement or the bridge fails, the native window remains at the accepted safe fallback width of 318 logical pixels. Complete controls take priority over compactness during failure.

## Considered Approaches

### A. Hidden unconstrained replica — selected

Render the same content twice: one visible instance and one inert, invisible, off-screen measurement instance. The replica uses intrinsic sizing outside the visible viewport constraint.

Advantages:

- Uses the browser's real font and CSS layout.
- Breaks the native-window/visible-DOM feedback loop.
- Reacts to identity, quota, reset, locale, and font changes.
- Keeps width logic independent from text-specific formulas.

Cost:

- Requires a shared presentational component and strict semantic isolation for the replica.

### B. Maximum-width native handshake

Start at 318 pixels, measure the visible content, then shrink.

Rejected because subsequent content changes would again measure the visible surface and could re-enter the feedback loop. It also couples native show timing to frontend readiness.

### C. Canvas or formula-based width

Calculate width from text metrics and fixed token sizes.

Rejected because font loading, DPI, letter spacing, localization, and future CSS changes could drift from the rendered geometry.

## Component Architecture

### `TaskbarStatusContents`

Extract one pure presentational component containing the complete geometric sequence:

```text
avatar | compact identity | quota track | nearest reset | close
```

It accepts the already-derived status-surface model plus interaction mode:

- `visible`: semantic buttons, handlers, titles, accessible names, error state, and test IDs.
- `measurement`: identical classes and content geometry, no handlers or test IDs, `aria-hidden`, `inert`, and removed from tab order.

The two modes must not maintain separate markup for the geometric fields. A markup or label change must affect both instances through the shared component.

### Visible surface

The visible taskbar surface:

- fills the current native viewport;
- keeps the quota track as the only overflow-constrained internal region;
- reserves the reset and close columns;
- retains background-only opacity, quota bands, focus styles, and close retry behavior;
- does not provide a ref to the width hook.

### Measurement replica

The measurement replica:

- is positioned off-screen;
- uses `visibility: hidden` and `pointer-events: none`;
- is `aria-hidden` and `inert`;
- uses intrinsic inline sizing (`max-content`) and is not constrained by `html`, `body`, `#root`, or the current native viewport;
- preserves the taskbar's intentional 318-pixel outer cap and 166-pixel quota-track cap;
- is observed by `ResizeObserver`;
- never becomes interactive or visible in screenshots.

## Native Fallback Contract

Introduce an explicit bootstrap/fallback width distinct from the normal compact target:

- minimum: 104 logical pixels;
- maximum and failure fallback: 318 logical pixels;
- height: 40 logical pixels.

The taskbar manager starts at 318 pixels. A successful measurement normally shrinks it immediately. If the measurement environment, observer, bridge, resize, or reposition fails, 318 remains the truthful and functional width.

The existing Rust transaction continues to:

1. resize the native window;
2. update the manager width only after a successful resize;
3. reposition using the new width;
4. compensate to the previous native width when reposition fails;
5. retain the only known truthful width when compensation itself fails.

## Measurement Data Flow

1. `useStatusSurface` produces the authoritative model and settings snapshot.
2. React renders visible and measurement instances from the same props.
3. The measurement hook observes only the replica.
4. It reads the replica border-box or scroll width, rounds once to a logical integer, and submits only changed widths.
5. The existing serialized latest-width queue permits one bridge request in flight and coalesces newer values.
6. Rust clamps to 104 through 318, resizes, and repositions.
7. Content or locale changes update the replica and repeat the process.

The recursive `intrinsicWidth` traversal and its recursive-geometry tests are removed. There is no fallback to measuring the visible surface.

## Failure Handling

- Missing `ResizeObserver`: silently keep 318 pixels. Do not add frontend console logging or a diagnostic bridge command; native resize and positioning failures continue to use Rust `tracing`.
- Invalid or zero replica measurement: ignore it and keep 318 pixels.
- Bridge rejection: keep the last confirmed native width; before the first success this is 318 pixels.
- Repeated unchanged measurement: no bridge call.
- Component unmount: disconnect the observer and suppress queued follow-up calls.
- Measurement replica accidentally focusable or exposed to accessibility: fail component tests.

No error path may replace the safe fallback with a narrower inferred width.

## Test Design

### Frontend unit and component tests

Add RED tests proving:

1. A visible viewport mocked at 168 or 198 pixels does not affect the replica's complete measured width.
2. The replica contains avatar, six-character identity, real metrics, reset date, and close geometry.
3. The replica has no accessible role, focus target, handler, or duplicate test ID.
4. Weekly-only content measures the complete `ProofU | 周 98% | 8/20 | ×` sequence.
5. Many real metrics respect the 318-pixel outer cap and 166-pixel quota-track cap while preserving reset and close.
6. Content growth and genuine shrink produce one serialized latest-width update each.
7. Repeated values are deduplicated; a rejected request cannot overwrite a newer desired width.
8. Missing observer, invalid width, bridge failure, and unmount preserve the 318-pixel safe fallback.

Delete tests that validate recursive descendant arithmetic. They describe the rejected architecture rather than product behavior.

### Rust tests

Preserve transaction and positioning coverage. Update the initialization contract to assert the 318-pixel safe fallback while retaining the 104–318 clamp and 40-pixel height.

### Fresh Windows proof

After a fresh debug build and after stopping the stale single-instance process:

- launch `taskbar-status:weekly`;
- wait for UI settle without relying on a content mutation;
- assert the complete proof sequence is visible: `ProofU`, `周 98%`, `8/20`, and close;
- assert height 40 and width within 104–318;
- assert the rectangle remains taskbar-safe;
- capture a new screenshot and reject all pre-redesign screenshots as historical failure evidence.

Only after this proof passes may Task 9 continue with float-ball/settings proof, release NSIS build, current-user installation, and installed-process verification.

## Scope

Expected implementation scope:

- `apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx`
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.css`
- `apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx`
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.ts`
- `apps/desktop-tauri/src/hooks/useTaskbarStatusWidth.test.tsx`
- taskbar native default/fallback constants and their tests only if required
- Windows verification evidence after the fix passes

No provider, account, quota parsing, settings persistence, float-ball layout, or installer behavior changes belong to this redesign.

## Acceptance Criteria

- The visible surface is never the frontend width source.
- The measurement replica is unconstrained by the current native viewport.
- Visible and measurement geometry share one presentational component.
- The replica is inert, hidden, and absent from accessibility and tab order.
- Recursive DOM-width inference is deleted.
- Measurement failure leaves the functional 318-pixel fallback.
- Weekly proof shows identity, weekly percentage, reset date, and close with no 5H row.
- Width is within 104–318 and height is 40 on a fresh Windows build.
- Existing opacity, band, close, queue, resize rollback, and taskbar-safe positioning tests remain green.
- Release build and installation remain blocked until fresh Windows proof passes.
