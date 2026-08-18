# Taskbar status, float ball, and account identity verification — 2026-08-07

This record covers the Windows implementation on branch
`codex/taskbar-floatball-identity`. The evidence was collected from the fresh
debug build in this worktree and uses only the credential-free proof harness for
the visual surfaces.

## Fresh build

Command:

```powershell
corepack pnpm@10.18.1 run tauri:build:debug
```

Artifacts:

```text
target/debug/codex-barbar.exe
target/debug/bundle/nsis/codex-barbar_1.0.0_x64-setup.exe
```

The build completed with exit code 0. The frontend TypeScript check, Vite
production bundle, Rust compilation, and NSIS bundling all completed.

## Automated gates

The following commands were run from the worktree and completed with exit code
0:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\assert-v1-boundaries.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\local-check.ps1
```

Observed results:

- V1 boundary guard: passed.
- Shared Rust: 264 unit tests passed, 1 ignored; the app-server contract suite
  passed 17 tests.
- Tauri shell: 95 tests passed.
- Frontend: 20 test files and 72 tests passed.
- Frontend production build: passed.

The focused regressions for this feature include the float-ball DPI clamp, the
non-blocking status-surface window callback, pointer-drag protection from
compatibility mouse events, and the current-account selector's concrete
identity fallback.

## Native Windows evidence

The CUA driver was not installed at
`%LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe`. Equivalent
read-only Win32 evidence was therefore used: window enumeration, visibility and
response checks, `GetWindowRect`, DPI/style inspection, and `PrintWindow`
screenshots. Each proof scenario uses synthetic account data and does not read
or display the user's account identity, cookies, or tokens. Normal local
repository and recovery startup still runs, so this proof mode is not a strict
no-I/O storage sandbox.

### Taskbar status overlay

- Window title: `codex-barbar taskbar status`
- Responding: `True`
- Physical rectangle: `1820,1368 - 2210,1440`
- Physical size: `390x72` at 144 DPI (150% scaling)
- Primary work area: `0,0 - 2560,1368`
- Notification area begins at `x=2218`; the overlay leaves an 8 px physical
  clearance and does not cover the notification area.
- Extended styles include `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, and
  `WS_EX_LAYERED`; the overlay is not the foreground window.
- Proof screenshot: [taskbar-status-debug-final.png](windows/2026-08-07/screenshots/taskbar-status-debug-final.png)

### Animated float ball

- Window title: `codex-barbar float ball`
- Responding: `True`
- Physical rectangle: `2440,1248 - 2548,1356`
- Physical size: `108x108` at 144 DPI (the 72 logical px ball)
- Primary work area: `0,0 - 2560,1368`
- The restored position stays inside the work area with the scaled 8 logical px
  margin.
- Extended styles include `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`, and
  `WS_EX_LAYERED`; the window is not the foreground window.
- Proof screenshot: [float-ball-debug-final.png](windows/2026-08-07/screenshots/float-ball-debug-final.png)

Both screenshots show the synthetic identity `Ming Zhao` and a usage
percentage; neither shows the placeholder `Current CLI`.

## Coverage limits

The following still require a real interactive CUA run before a final public
release:

- clicking the taskbar overlay and float ball to open the tray panel;
- dragging the float ball and restoring its position after restart;
- Explorer restart and taskbar auto-hide transitions;
- top, left, and right taskbars;
- multiple monitors and a 200% DPI session;
- keyboard/assistive-technology traversal.

The corresponding geometry, proof-mode, settings, identity parsing, and
thread-safety behavior has deterministic Rust/TypeScript coverage. This record
is an internal implementation verification, not a claim that the full CUA
acceptance matrix is complete.
