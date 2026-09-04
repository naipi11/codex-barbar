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

## Follow-up lifecycle hardening

The initial group-only cleanup could leave two holes: Drop had no bounded
reaper for a subreaper-adopted zombie, and a descendant that called `setsid`
or `setpgid` was no longer addressed by a negative-PGID signal. Linux cleanup
now snapshots the App Server parent/child tree from procfs as identities of
`(pid, start_time)`, then refreshes that snapshot while shutdown is pending.
Detached descendants are signalled and reaped by their exact identity in
addition to the original process group; a reused PID is never acted on.

Drop snapshots the same bounded ownership set, sends KILL, and starts a
250 ms named reaper thread. The thread only waits on those exact descendants,
so Drop does not block and cannot consume an unrelated child. The added Linux
regressions prove that (a) a shell-exited, subreaper-adopted background child
is fully absent after Drop (not merely a zombie), and (b) a `setsid` child
that escapes the original PGID is absent after shutdown.

## Follow-up ownership ledger

The next review identified a pre-shutdown gap: a detached child can make its
shell leader exit before shutdown begins, at which point neither the old
leader's procfs children nor its old PGID is safe to rediscover. Linux now
starts a per-App-Server ledger monitor immediately after spawn. It records
descendants while the leader's `(pid, start_time)` identity is still valid.
After the leader disappears, cleanup may only signal or reap ledger identities
whose start times still match; it does not broadcast to the historical PGID.

Every group signal now requires that the original leader identity still exists
and still owns the original group. This rejects a reused leader PID or a
changed/reused PGID. Drop itself no longer performs procfs I/O or signalling:
it only queues a 250 ms bounded worker, and the worker performs capture,
identity-validated KILL, and reap work. New Linux regressions release a shell
only after its detached child is observed in the early ledger, prove that the
leader exits before shutdown, and assert that cleanup removes the child.

## Verification

- `cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process -- --test-threads=1` — passed on Windows (13 tests; Linux-only cases correctly cfg-gated).
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` — passed on Windows (529 passed, 1 ignored).
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` — passed on Windows.
- `cargo fmt --all -- --check` — passed.
- `cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process -- --test-threads=1` — rerun after the follow-up; passed on Windows (13 tests; Linux-only cases cfg-gated).
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` — rerun after the follow-up; passed on Windows (530 passed, 1 ignored).
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` — rerun after the follow-up; passed on Windows.
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` — rerun after the ownership-ledger follow-up; passed on Windows (530 passed, 1 ignored).
- `cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process -- --test-threads=1` — rerun after the ownership-ledger follow-up; passed on Windows (13 tests; Linux-only ledger and Drop cases cfg-gated).
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` and `cargo fmt --all -- --check` — rerun after the ownership-ledger follow-up; passed on Windows.
- `cargo test --manifest-path rust/Cargo.toml --target x86_64-unknown-linux-gnu providers::codex::app_server::process --no-run` could not link because this Windows host lacks `x86_64-linux-gnu-gcc` (the `ring` build script fails before the crate compiles). Docker Desktop's Linux engine is also unavailable. Therefore only Ubuntu CI can execute the Linux-only regression tests; Windows verification is not presented as Linux proof.
