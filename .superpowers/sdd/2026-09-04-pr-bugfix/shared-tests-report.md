# Shared Rust Ubuntu test fixes

## Scope

Addressed the non-process-group failures from PR #33 Ubuntu job `100994232956`
(run `33863892866`). The separately failing Unix process-group test was not
modified.

## Root causes and fixes

- Credential-bundle validation relied on host-native `Path::is_absolute`, so a
  Windows drive or UNC path was ordinary text on Linux. It now rejects volume
  prefixes and leading backslashes before native path parsing, and the test
  covers a UNC path.
- The vault recovery test assumed Windows `ReplaceFileW` creates a backup.
  Portable recovery coverage now creates a valid backup explicitly; the
  `ReplaceFileW` crash-window assertion remains Windows-only.
- App-path, Codex-session, data-cleanup, and start-at-login tests no longer
  encode Windows separators in fixtures. The autostart command-string tests
  remain Windows-only because their absolute-path contract is Windows-specific.
  The portable data-cleanup test now supplies a native absolute noncanonical
  target, while Windows UNC, drive-path, and separator coverage is retained
  behind `cfg(windows)`.
- Backup names used only a millisecond timestamp, permitting overwrites during
  rapid migrations. A UUID suffix makes every snapshot distinct while the
  timestamp prefix preserves chronological retention ordering.

## Verification

- `cargo fmt --all`
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1`
  - 529 passed, 0 failed, 1 ignored (Windows host)
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
  - passed
- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`
  - passed
- `git diff --check`
  - passed

## Follow-up review correction

The original data-cleanup fixture still used a Windows literal for its
noncanonical-target assertion. It now uses a native absolute sibling target
on every platform and keeps the literal assertion behind `cfg(windows)`. The
app-path test similarly keeps its Windows absolute/separator assertion while
using a native Unix expectation elsewhere, and Codex-session Windows UNC and
drive-path checks are restored under `cfg(windows)`.

Follow-up verification reran the full shared suite: 530 passed, 0 failed, 1
ignored; shared clippy, `cargo fmt --check`, and `git diff --check` passed.

`cargo check --manifest-path rust/Cargo.toml --all-targets --target
x86_64-unknown-linux-gnu` could not complete because this host lacks
`x86_64-linux-gnu-gcc` required by `ring`; no cross-compiler was installed or
changed. Ubuntu CI remains the target-build gate.

## Deliberately excluded

The pre-existing, concurrently edited files
`rust/src/providers/codex/app_server/job.rs` and
`rust/src/providers/codex/app_server/process.rs` are process-group work and
are intentionally not included in this change.
