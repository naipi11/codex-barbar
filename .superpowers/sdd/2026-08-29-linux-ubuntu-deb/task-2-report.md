# Task 2 report: Linux Codex discovery and process activity

## Delivered scope

- Added `platform::linux::process`, a read-only procfs adapter with an
  injectable `ProcReader` contract (`cmdline`, parent, and children reads).
  Discovery traverses from PID 1, identifies a `codex` root, and preserves its
  entire descendant tree. Permission failures, missing files, and malformed
  procfs fields result in no activity rather than a false busy state.
- Added Linux resolver support for an absolute executable named `codex`, with
  regular-file, non-symlink, and executable-bit validation. PATH resolution
  uses only literal `codex` candidates and skips empty segments.
- Added a strict Linux npm layout branch: only an npm global-bin symlink at
  `prefix/bin/codex` resolving exactly to
  `prefix/lib/node_modules/@openai/codex/bin/codex.js` is accepted. It launches
  the adjacent verified `prefix/bin/node` directly; no shell shim is run.
  Windows `.exe`, `.cmd`, signed-native-package, and Store alias branches
  remain unchanged.
- Made Unix App Server children their own process group. Bounded shutdown now
  sends TERM and then KILL to the group after a short wait, and drop performs
  group cleanup so descendants are not orphaned. Windows Job Object behavior
  is unchanged.
- Changed float-ball Linux motion to consume only shared procfs discovery
  facts. It continues to reuse the existing fast-tier config signal and has no
  Linux process enumeration in the UI shell. Focus error copy is desktop-
  neutral, while non-Windows focus remains unsupported.
- Replaced the non-Windows test-fixture program `powershell.exe` with
  `/bin/false`; the PowerShell fixture is Windows-only and cannot be launched
  as a Linux process.

## Tests added first

- Linux bare `codex` override acceptance.
- ProcReader root plus App Server child discovery.
- ProcReader nested descendant discovery.
- ProcReader permission denial produces idle/no discovery.
- Verified Linux npm symlink layout launches the fixed node entry directly.
- Unix App Server process group differs from the caller and is gone after
  bounded shutdown.

## Verification evidence

- `cargo fmt --all -- --check` completed successfully.
- `cargo test --manifest-path rust/Cargo.toml --lib providers::codex::app_server -- --test-threads=1` completed: 80 passed, 0 failed, 1 pre-existing ignored.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` completed successfully.
- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` completed successfully.
- `git diff --check` completed successfully.

## Linux-runtime limitation

The Windows host has the Linux Rust standard-library target but not
`x86_64-linux-gnu-gcc`, so its cross-target test compilation stops in `ring`.
Docker Desktop's Linux engine was unavailable when first invoked; a local
startup attempt did not provide a completed container test result within this
task window. Consequently the cfg-Linux tests above are committed but require
execution on Ubuntu CI or a functioning Linux container before release.
