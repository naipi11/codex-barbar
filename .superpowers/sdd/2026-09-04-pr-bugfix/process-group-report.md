# Linux App Server process-group cleanup

## Root cause

`SupervisedAppServerProcess::shutdown` only terminated the Unix process group
when its tracked leader exceeded the grace period. A shell leader can exit
normally while a background App Server descendant is still running, so that
path skipped bounded group cleanup. The old immediate `kill(-pgid, 0)` test
also treated a zombie member as a live process group member.

## Change

On Linux, the desktop process enables `PR_SET_CHILD_SUBREAPER` before it
spawns an App Server. When a shell leader exits ahead of a background child,
the desktop process adopts that child. Shutdown now always sends TERM to the
owned process group, polls it for at most 250 ms while reaping adopted group
children, then sends KILL and polls for another bounded 250 ms if needed.
Tokio continues to reap the tracked leader; the manual `waitpid(-pgid,
WNOHANG)` loop runs only after Tokio has observed that leader, preventing a
double-reap race. Windows Job Object ownership is unchanged.

The Linux regression tests cover both a shell that waits for its background
child and one that exits immediately after reporting that child. Both assert a
distinct group and use a bounded condition wait for the complete group to
disappear.

## Verification

- `cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process -- --test-threads=1` — passed on Windows (13 tests; Linux-only cases correctly cfg-gated).
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` — passed on Windows (529 passed, 1 ignored).
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` — passed on Windows.
- `cargo fmt --all -- --check` — passed.
- `cargo test --manifest-path rust/Cargo.toml --target x86_64-unknown-linux-gnu providers::codex::app_server::process --no-run` could not link because this Windows host lacks `x86_64-linux-gnu-gcc` (the `ring` build script fails before the crate compiles). Docker Desktop's Linux engine is also unavailable. Therefore only Ubuntu CI can execute the Linux-only regression tests; Windows verification is not presented as Linux proof.
