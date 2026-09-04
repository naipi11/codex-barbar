# Task 4 report — managed-login prerequisite and XDG autostart/locale

## Commits

- Prerequisite: `cb19fa1dee5f5adf96f4e7e437d0b1658c1b24f6` — `Fix managed login terminal lifecycle`
- Task 4 implementation: `027e9179132db15891e9a8ae4c136750301a25c4` — `Add Linux XDG autostart support`

## Managed-login prerequisite

- Replaced scattered terminal handling with one state-machine exit. Every path now
  attempts runtime cleanup before terminal completion, publishes its terminal
  status while the actor is still active, and only then calls `finish_login`.
- `Ready` and `Succeeded` are reachable only after runtime cleanup and profile
  persistence both succeed. A cleanup failure publishes `Failed` with
  `StorageFailure`; a primary App Server/vault error retains its original kind.
- `Cancelled` is a distinct non-success terminal state and no longer reaches an
  auth-mode `expect` or panics. If its runtime cleanup fails, the cleanup failure
  is surfaced as `Failed` because the credential-bearing runtime may remain.
- The spawned wrapper no longer overwrites active errors with a generic
  `VaultFailure`; it only records the already-published terminal failure.
- Added deterministic cleanup-failure finalization coverage and a deterministic
  cancelled fake App Server mode for both PowerShell and Python fixtures.
- Managed refresh cleanup handling was left intact.

## Task 4 implementation

- Added the fixed Linux XDG entry
  `config/autostart/com.naipi11.codexbarbar.desktop`.
- Enabling validates an absolute Linux path whose file name is exactly
  `codex-barbar`, emits a Desktop Entry `Exec` token plus `--background` without
  a shell, writes and syncs a same-directory temporary file, then atomically
  renames it into place.
- Disabling removes only that fixed file, treats absence as success, and leaves
  sibling autostart entries untouched.
- Added a typed platform `autostart` facade: Linux routes to XDG and Windows
  continues to route to the existing HKCU Run implementation without changing
  its registry code.
- Added a typed platform `system_locale::language()` facade returning
  `LanguagePreference`. Linux follows non-empty `LC_ALL`, `LC_MESSAGES`, then
  `LANG`, maps Simplified Chinese variants to `ZhCn`, and otherwise selects
  `EnUs`. Windows wraps the existing user-default locale logic without changing
  its mapping.
- Routed desktop startup reconciliation, settings updates, and tray system
  language selection through the platform facades. No dependencies were added.

## RED evidence

- `cargo test --manifest-path rust/Cargo.toml accounts::service::tests:: -- --test-threads=1`
  failed before the prerequisite implementation because
  `ManagedLoginTerminal` and `finish_managed_login` did not exist.
- `cargo test --manifest-path rust/Cargo.toml platform::linux::autostart`
  failed before Task 4 implementation because the Linux autostart and locale
  functions/types and the Windows typed locale facade did not exist.

## GREEN evidence on the Windows host

- `cargo test --manifest-path rust/Cargo.toml accounts:: -- --test-threads=1`:
  67 passed, 0 failed.
- `cargo test --manifest-path rust/Cargo.toml platform::`: 21 passed, 0 failed.
  This includes 5 Linux autostart fixture tests, 3 Linux locale fixture tests,
  the existing 2 Windows autostart tests, and the Windows locale facade test.
- `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1`: library
  518 passed / 1 ignored, contract integration 17 passed, icon integration 3
  passed, and doc tests 0; no failures.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`:
  passed.
- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`: passed.
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`:
  254 passed, 0 failed.
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`:
  passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed before the Task 4
  implementation commit.

## Linux verification boundary

- `x86_64-unknown-linux-gnu` is installed, but
  `cargo check --manifest-path rust/Cargo.toml --target x86_64-unknown-linux-gnu`
  cannot progress past `ring` on this Windows host because
  `x86_64-linux-gnu-gcc` is unavailable. Docker has no reachable Linux engine.
- Consequently, Linux compilation, an installed executable path, XDG desktop
  startup behavior, environment locale behavior in a real session, and desktop
  integration remain unverified here. Ubuntu CI and the real GNOME acceptance
  pass are still required; the Windows fixture results are not claimed as Linux
  runtime proof.

## Fix round 1

- Kept the approved fixed filename `com.naipi11.codexbarbar.desktop`; the review
  proposal to rename it was rejected by the ledger/spec.
- Managed login now returns the original `session.start_login()` `AppError`
  after attempting shutdown. A deterministic login-start process-exit fixture
  proves `OfflineOrTimeout`, `Retry`, and `APP_SERVER_EOF` are not rewritten as
  a protocol mismatch.
- Added `runtimeCleanupFailed` to the managed-login status contract from the
  actor through the Tauri DTO and TypeScript event validator. When a primary
  login error and cleanup failure occur together, the returned primary
  `AppError` and terminal `errorKind` remain unchanged while the typed flag is
  true. Logs use only the fixed `RUNTIME_CLEANUP_FAILED_AFTER_LOGIN_ERROR` code.
- Corrected Linux `Exec` serialization to apply both Desktop Entry string-value
  escaping and Exec argument quoting. Spaces, literal backslashes, double
  quotes, and dollar signs round-trip through deterministic expected strings;
  executable paths containing `=` are rejected as invalid by the Freedesktop
  grammar. No shell is introduced.
- Linux locale selection now strips encoding/modifier suffixes and matches
  complete language/script/region tokens. `zh_CN`, `zh_SG`, and `zh_Hans`
  variants select Simplified Chinese; partial tokens such as `zh_CNfoo` and
  `zh_Hansard` do not.

### Fix-round RED evidence

- `managed_login_preserves_non_protocol_start_error` failed with actual
  `ProtocolMismatch` versus expected `OfflineOrTimeout` before the change.
- `primary_and_cleanup_failure_preserve_primary_and_flag_runtime_risk` failed
  to compile because the typed cleanup-risk field did not exist.
- `desktop_exec_serialization_preserves_reserved_literal_path_characters`
  failed because only one escaping layer was emitted for reserved characters.
- Linux locale tests failed both recognition of `zh_SG` and rejection of
  partial `zh_CNfoo`/`zh_Hansard` tokens.

### Fix-round GREEN evidence

- Account service tests: 18 passed; full account tests: 69 passed.
- Platform tests: 23 passed.
- Rust and desktop clippy with `-D warnings`: passed.
- Desktop Rust check and 254 tests: passed.
- Frontend Vitest: 297 passed; TypeScript/Vite production build: passed. The
  existing `App.test.tsx` React `act(...)` warning remains non-fatal and is
  unrelated to this fix round.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Linux-native execution limits remain unchanged from the section above.
