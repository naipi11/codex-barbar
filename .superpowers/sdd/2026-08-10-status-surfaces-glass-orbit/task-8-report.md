# Task 8 Report: Expandable Glass Orbit Float Ball

## RED

Before production changes, added the typed expanded-state bridge expectation, fake-timer expansion hook tests, and FloatBall hover, drag, close-error, and target-isolation assertions.

`pnpm --dir apps/desktop-tauri test -- src/hooks/useFloatBallExpansion.test.tsx src/surfaces/FloatBall.test.tsx src/types/bridge.test.ts` exited 1 as expected. The new hook module and `setFloatBallExpanded` export were absent; the old FloatBall DOM had no expanded card, shell, or independent close control.

## GREEN

- Added `useFloatBallExpansion` with exact 180ms expansion and 120ms collapse scheduling, native error state, pending-timer cancellation, and unmount cleanup.
- Added the typed `set_float_ball_expanded` frontend bridge and exposed it from `useStatusSurface`.
- Rebuilt FloatBall as a non-button shell with a drag-owning body button and permanent sibling close button. Drag threshold crossing awaits `collapseNow()` before native `startDragging`; click compatibility suppression remains in place.
- Added 88x88 collapsed orbit and 260x148 expanded quota-card contracts, dual quota/reset display, identity, updated text, exact status colors, status-point/track refresh breathing, and reduced-motion suppression.
- Close errors render `关闭失败，请重试`; expansion errors render `悬浮球尺寸切换失败`.

## Verification

- Focused test command: 23 files, 103 tests passed.
- `pnpm --dir apps/desktop-tauri test`: 23 files, 103 tests passed.
- `pnpm --dir apps/desktop-tauri run build`: passed (`tsc --noEmit` and Vite production build).
- `git diff --check`: passed with no whitespace errors.

## Self-review

- Body and X are sibling button targets; close stops propagation and does not open the panel.
- Drag tests prove false expansion command precedes `startDragging` and panel opening is suppressed.
- Timer tests cover exact expansion boundary, cancel-on-reenter behavior, failure copy, and cleanup.
- No dependencies or unrelated production files were changed.

## Concerns

- No CUA/native proof was performed. Task 9 must rebuild and validate the native window resize, hover, drag, and visual behavior on Windows.
