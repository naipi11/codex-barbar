# Task 4 Report: Safe Dynamic Taskbar Window Sizing

## Result

- Added a typed `set_taskbar_status_width` command and frontend bridge; the browser remains unable to use raw window-resize permissions.
- Taskbar content width is rounded to the nearest logical pixel, rejects non-finite and non-positive input to the 104px minimum, and is bounded to 104–318px. The default is 168px and the shared logical height is 40px.
- The taskbar overlay uses its current logical width for native window creation and all horizontal/vertical safe-slot computation. Resize failure returns `TASKBAR_STATUS_RESIZE_FAILED`; state changes only after native resize succeeds, and is restored when a following reposition fails.
- Added `useTaskbarStatusWidth`, which observes only the supplied content wrapper, prefers `borderBoxSize`, falls back to `getBoundingClientRect()`, deduplicates rounded and in-flight widths, retries a failed command on later observations, and disconnects on cleanup. Task 5 will attach this hook to `TaskbarStatus`.
- Expanded `scripts/assert-v1-boundaries.ps1`'s exact registered-command allowlist for the new, narrowly scoped Rust command. This is necessary to keep the guard synchronized with `main.rs`; it does not broaden capabilities or add raw frontend window access.

## TDD evidence

1. Added clamp/slot, typed-bridge, and deterministic `ResizeObserver` tests first.
2. RED run failed as expected because `clamp_logical_width`, `setTaskbarStatusWidth`, and the hook did not exist.
3. A second RED test caught duplicate asynchronous width submissions; implementation now tracks pending width and clears it on failure for retry.
4. Positioning-contract tests then caught stale 160px/48px constants; positioning now consumes the new shared minimum and height constants.

## Verification

- `cargo fmt --all -- --check`
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` — 120 passed
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`
- `pnpm --dir apps/desktop-tauri test` — 24 files, 119 passed
- `pnpm --dir apps/desktop-tauri run build`
- `scripts/assert-v1-boundaries.ps1`
- `git diff --check`

No fresh desktop/CUA proof was collected: Task 4 adds native sizing infrastructure and a standalone hook but deliberately does not attach it to the rendered TaskbarStatus surface until Task 5.

## Fix round 1

- The native width update is now a compensating transaction. After a successful resize followed by reposition failure, it restores the prior native size and repositions the prior slot. If compensation resize itself fails, state remains at the requested width because that is the only truthful native-size state; the stored slot is invalidated. All transaction failure paths return `TASKBAR_STATUS_RESIZE_FAILED`.
- The ResizeObserver hook now serializes one native command at a time and coalesces observations to the latest rounded width. Completion drains only the latest queued width; a rejection needs a subsequent observation to retry, and unmount prevents queued dispatches.
- Added deterministic fake-operation tests for native success, initial resize failure, reposition failure plus successful compensation, and compensation failure. Added deferred-promise hook tests for serialized latest-width dispatch, reject-then-observe retry, and unmount while a request is pending.

Fix-round verification: focused taskbar tests (19 passed), full Tauri tests (124 passed), Tauri clippy with `-D warnings`, full frontend tests (24 files, 120 passed), frontend production build, `cargo fmt --all -- --check`, and `git diff --check`.

## Fix round 2

- A failed submitted width now blocks only that submitted value. If a newer value was observed while it was in flight, the hook immediately drains that newer queued value; the same failed width still waits for a later observation before retrying.
- Restored deterministic coverage for preferring `borderBoxSize` and using `getBoundingClientRect()` when the observed box lacks a finite width.

Fix-round verification: focused hook tests and full frontend tests (24 files, 123 passed), frontend production build, and `git diff --check`. Rust was unchanged in this round.
