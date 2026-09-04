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

`cargo check --manifest-path rust/Cargo.toml --all-targets --target
x86_64-unknown-linux-gnu` could not complete because this host lacks
`x86_64-linux-gnu-gcc` required by `ring`; no cross-compiler was installed or
changed. Ubuntu CI remains the target-build gate.

## Deliberately excluded

The pre-existing, concurrently edited files
`rust/src/providers/codex/app_server/job.rs` and
`rust/src/providers/codex/app_server/process.rs` are process-group work and
are intentionally not included in this change.
