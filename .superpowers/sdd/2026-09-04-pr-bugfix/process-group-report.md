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

## Final dedicated-supervisor hardening

The ledger design still could not close two kernel races. A child could fork,
call `setsid`/`setpgid`, and make the shell leader exit before any polling
thread observed it. Also, checking `/proc/<pid>/stat` and then calling
`kill(pid, ...)` left a check/use window in which the numeric PID could cease
to name the checked process. Faster polling and additional start-time checks
cannot remove either race.

Linux now inserts a dedicated supervisor process between Tokio and the real
App Server without adding a dependency or changing the validated production
command. `Command::pre_exec` gives the outer child a private process group,
marks it as a child subreaper, resets `SIGCHLD` to the normal waitable
disposition, then forks once. The inner child immediately returns to
`Command` and `execve`s the original App Server with its original stdio. The
outer child closes every unrelated descriptor and remains a single-threaded,
allocation-free supervisor. A nonblocking `SOCK_CLOEXEC` Unix socket performs
the spawn handshake and is the parent's exact ownership capability.

When the App Server leader or another intermediate parent exits, every
surviving orphan is adopted by this per-App-Server subreaper even if it has
changed its session or process group. On shutdown the supervisor repeatedly
reads only its current direct children, signals them before calling `waitpid`,
then reaps and repeats so newly adopted descendants become the next pass's
targets. Because a terminated direct child remains an unreaped zombie during
the signal pass, its PID cannot be reused between discovery and signalling.
Linux cleanup never broadcasts to a historical PID or PGID. If the current
direct-child view is unavailable, cleanup fails closed and does not substitute
a stale identifier.

`Drop` no longer traverses procfs, locks a ledger, creates a thread, signals a
numeric PID/PGID, or waits. It only removes and closes the supervisor control
socket. EOF wakes the already-running supervisor. Explicit shutdown waits at
most the requested grace period plus the existing 250 ms post-request bound.
The supervisor uses 8 bounded TERM passes, then keeps signalling current
direct children with KILL and reaping until `waitpid` reports `ECHILD`. Its
lifetime is deliberately not capped by the caller's deadline: a deep chain can
reveal one adopted child per pass, and a temporarily uninterruptible child must
remain owned until it can exit. A stopped supervisor therefore cannot add
synchronous Drop latency. Windows Job Object creation, assignment, and
kill-on-close behavior are unchanged.

### Final Linux regression coverage

- The process-group test reads the real inner App Server PID and proves both
  that it is distinct from the supervisor and that it starts in the
  supervisor's private group.
- An invalid inner executable must return from spawn within two seconds,
  covering the double-fork exec-error handshake rather than accepting a
  parent/supervisor deadlock.
- The leader-loss regression performs `setsid`, reports the exact leader and
  child identities, and exits in the same shell command. It has no ledger
  polling handshake. Shutdown must remove the original detached identity while
  an independently spawned `sleep` retains the same `(pid, start_time)`.
- The existing escaped-group regression still requires the `setsid` child and
  original process group to disappear.
- The Drop regression creates 128 descendants, stops the supervisor with
  `SIGSTOP`, and requires Drop to return in under 100 ms before resuming the
  supervisor. This proves the caller does not synchronously perform procfs,
  signalling, or reaping even when the cleanup process cannot run.
- A 48-process parent chain exceeds the removed 8 TERM + 17 KILL limit. The
  caller must still return in under one second, while the supervisor continues
  adopting, killing, and reaping until the exact leaf identity and private
  process group both disappear.
- Exact `/proc/<pid>/stat` identity disappearance is required after cleanup;
  a surviving zombie does not satisfy the assertion.

## Final verification (2026-09-04)

- `cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process -- --test-threads=1` — Windows: 13 passed, 0 failed; Linux-only tests were cfg-gated.
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` — Windows: library 530 passed, 0 failed, 1 ignored; App Server contract 17 passed; icon assets 3 passed; doc tests 0 failed.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` — passed on Windows.
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml -- --test-threads=1` — Windows: 264 passed, 0 failed.
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings` — passed on Windows.
- `cargo fmt --all -- --check` — passed.
- The Linux-only `job.rs` implementation was parsed and type-checked with the
  installed `x86_64-unknown-linux-gnu` standard library using `rustc --emit
  metadata -D warnings`; the same isolated target slice passed
  `clippy-driver -D warnings`. This checks the Linux FFI/types without linking
  or claiming runtime proof.
- `cargo test --manifest-path rust/Cargo.toml --target
  x86_64-unknown-linux-gnu providers::codex::app_server::process --no-run` —
  blocked before compiling `codexbar`: `ring` could not find
  `x86_64-linux-gnu-gcc`.
- `docker info` — unavailable: the Docker Desktop Linux engine named pipe does
  not exist. The only registered WSL distribution is the Docker Desktop
  utility distro, not Ubuntu.

The Linux-only process, detached-child, stalled-Drop, and zombie regressions
were therefore not executed on this Windows host. Ubuntu 24.04 CI must run the
focused process tests (serially) and the ordinary Linux Rust gates before this
change is treated as Ubuntu runtime proof.

## PR #33 Ubuntu shared-clippy follow-up

Ubuntu run `33872381891` reached the shared Rust crate and reported three
Linux-cfg lint failures in `process.rs`: a single-arm timeout `match`, a
needless tail `return` in Drop, and an unused test-only `kill` declaration.
The platform timeout paths are now separated with `#[cfg]`, Linux Drop ends
directly after closing the supervisor socket, and the stale declaration was
removed. These are control-flow/lint-only changes; supervisor ownership,
stdio, the socket protocol, process-group assertions, and Windows Job Object
behavior are unchanged.

Post-fix Windows verification:

- Focused process tests: 13 passed, 0 failed (Linux-only tests cfg-gated).
- Shared Rust tests: 530 passed, 0 failed, 1 ignored; App Server contract 17
  passed; icon assets 3 passed; doc tests 0 failed.
- Desktop Tauri Rust tests: 264 passed, 0 failed.
- Shared and desktop `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

This Windows host still cannot execute the Linux-only cases. The next Ubuntu
PR run must confirm that shared clippy is clear and execute the Linux process
regressions.

## PR #33 detached-fixture follow-up

Ubuntu run `33874033848` showed that the shell command `setsid sleep ... &`
reports the wrapper PID in that environment; that PID was still in the
supervisor process group, so the escaped-group test failed before exercising
cleanup. The production supervisor was not implicated, and the independent
immediate-detach regression passed in the same run.

The escaped-group fixture now runs `/usr/bin/python3 -c`, forks once, and has
the child itself call `os.setsid()` before printing its own PID and sleeping.
The test requires both `getpgid(child) != supervisor_pgid` and
`getpgid(child) == child`, then requires the exact `(pid, start_time)` identity
and the supervisor process group to disappear within their existing bounds.
The Python parent waits for the child, so shutdown still has to kill the
leader, adopt the detached child, kill it, and reap the complete tree. The
unused PID-only wait helper was removed; no production code changed.

Post-fixture Windows verification was rerun: shared Rust 530 passed with 1
ignored, App Server contract 17 passed, icon assets 3 passed, desktop Tauri
Rust 264 passed, both Rust clippy commands passed with `-D warnings`, and
format/diff checks passed. Linux execution still requires the next Ubuntu PR
run.
