# Tauri Linux Clippy repair report

## Scope

- Worktree: `C:\Users\stack\Documents\codex-barbar\.worktrees\linux-deb`
- Branch: `codex/linux-deb`
- Source failure: PR #33 run `33861589229`, Ubuntu job `100987020335`, step `Tauri Rust clippy`
- Remote state: unchanged; no push or GitHub mutation was performed.

## Root cause

The frontend ordering repair allowed `tauri::generate_context!()` to compile, which exposed the next Linux gate: the Tauri binary module graph still compiled Windows-only notification, foreground/fullscreen, taskbar-overlay, and Win32 geometry helpers. They have real Windows consumers but no Linux production consumers, so `cargo clippy --all-targets -- -D warnings` reported one unused import and 41 dead-code diagnostics. The test binary kept many of the items live through unit tests, which is why its shorter diagnostic set differed from the production binary.

## Repair

- Notification code now compiles the PowerShell registration/probe/toast scripts, exit-code mapping, and Windows setting abstractions only for Windows or their platform-neutral unit tests. `DesktopNotificationSink` contains only the transport for the current production target (plus the explicit unsupported test/fallback variant), preserving the Linux `notify-rust` sink and Windows toast behavior without cross-target transport stubs.
- Windows notification Settings launching, taskbar proof helpers, foreground scheduling, fullscreen probing, monitor intervals, and measurement-window deferred-state helpers now use item-level target/test gates matching their actual consumers.
- `ReconcileCause` retains the cross-platform `ShellChanged` path used to restore/reposition the Linux float ball, while Windows-only foreground, periodic, and fullscreen causes compile only on Windows. The non-Windows monitor remains a no-op.
- Taskbar Win32 discovery and positioning modules compile for Windows production and for unit tests, not the Linux production binary. Shared label, authorization, width clamping, and unsupported-runtime behavior remain available on Linux. Three non-Windows taskbar methods with no generic caller were removed; the generic taskbar no-op/unsupported methods used by status reconciliation remain intact.
- Windows taskbar route/dimension constants and taskbar subtraction geometry compile for Windows or their existing unit tests. No `allow(dead_code)` suppression was added.

The existing cfg-focused tests continue to assert that Linux does not create taskbar or measurement windows, Linux notification capability uses the desktop-notification path, and Windows-only positioning/discovery logic remains covered as pure unit-test code.

## Verification

Passed on this Windows host after rebuilding the frontend distribution:

```text
pnpm --dir apps/desktop-tauri run build
# TypeScript passed; Vite built dist/ (79 modules)

cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
# 264 passed; 0 failed
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings

cargo test --manifest-path rust/Cargo.toml
# 528 library tests passed; 1 ignored; 17 contract tests and 3 icon tests passed
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings

cargo fmt --all
git diff --check
```

The successful Tauri check/test/Clippy runs occurred with `apps/desktop-tauri/dist/index.html` present, so the local Windows build also exercised `tauri::generate_context!()` after the frontend build.

## Linux verification boundary

`cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --target x86_64-unknown-linux-gnu --all-targets` cannot reach this crate on the Windows host. The installed Rust target stops in the GTK dependency build scripts (`glib-sys`, `gdk-sys`, `gio-sys`, and peers) because `pkg-config` has no Linux sysroot/cross wrapper. Only Docker Desktop's internal WSL distribution is installed, not a usable Ubuntu development environment.

The saved Ubuntu log is therefore authoritative for the target-specific error list. Every reported item is now excluded from the Linux production binary or retained only where a Linux test consumes it. There are no remaining known Linux-only warnings in that log, but a native Ubuntu rerun is still required to prove that no later warnings were hidden behind the original 42-error cutoff.
