# Task 7 report: platform-aware React settings and surfaces

Date: 2026-09-04

## Implemented

- Threaded `BootstrapDto.platform` through `Settings` to the General and Taskbar & Float Ball tabs. Existing fixtures now carry a complete platform capability object, including the App and Settings test bootstrap fixtures.
- On unsupported platforms, the Taskbar & Float Ball tab omits only the taskbar-status group. It leaves the floating-ball controls intact when `floatingBall` is supported and does not submit a taskbar-status patch or mutate the persisted setting.
- Added typed English and Simplified Chinese availability copy for taskbar, tray, keyring, and the Wayland floating-ball fallback. Unsupported tray/keyring capability notices are surfaced in General; the Wayland notice is scoped to the Linux floating-ball group.
- Added route/surface guards after bootstrap: unsupported taskbar routes render no taskbar UI, and unsupported floating-ball routes render no float-ball UI. The float-ball guard uses a child component so React hook order remains stable after the asynchronous bootstrap arrives.
- Documented that `BootstrapDto.platform.notifications` is static platform support; the existing notification bridge remains the source of runtime D-Bus session availability. The Notifications UI already uses that dynamic capability instead of the bootstrap value.
- Windows defaults retain the prior UI shape and persistence paths.

## Test-first evidence

- RED: the focused Settings/Taskbar/App suite failed because the Linux fixture still rendered the taskbar fieldset; the expected taskbar-absence assertions failed.
- RED: the floating-ball capability test timed out because the unsupported surface still rendered.
- GREEN initially exposed a hook-order error caused by returning from `FloatBall` before its later hooks. Moving those hooks into a capability-gated child fixed the root cause.

## Verification

- `corepack pnpm@10.18.1 --dir apps/desktop-tauri test` — 39 files, 300 tests passed.
- `corepack pnpm@10.18.1 --dir apps/desktop-tauri run build` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` — 260 passed.
- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` — passed.
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings` — passed.
- `git diff --check` — passed. Git emitted only existing LF-to-CRLF conversion notices.

## Linux validation boundary

This Windows host verified the typed frontend contract and native Windows desktop Rust manifest only. It did not run an Ubuntu build or a Wayland/GNOME session. Ubuntu 24.04 CI and a real GNOME session still need to confirm that taskbar/measurement windows are absent, the floating ball remains draggable, and the Wayland always-on-top fallback copy matches observed compositor behavior.
