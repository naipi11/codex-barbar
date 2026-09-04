# Task 3 report: Linux Secret Service managed credentials

## Delivered scope

- Added the approved `keyring` dependency with the zbus Secret Service
  backend and updated `Cargo.lock`.
- Added `LinuxSecretServiceProtector`, using the fixed service
  `com.naipi11.codexbarbar` and the profile UUID as the keyring username.
  Managed bundle bytes are written/read/deleted with `set_secret`,
  `get_secret`, and `delete_credential`.
- Linux vault envelopes store only the opaque
  `codex-barbar-secret-service:v1:<profile-uuid>` marker. Marker parsing is
  exact and rejects malformed or cross-profile values. Windows retains the
  existing current-user DPAPI protector and envelope format.
- Added a default `CredentialProtector::remove_current_user` hook and made
  vault removal invoke it, with Secret Service errors reduced to coarse
  locked/unavailable results and no secret-bearing diagnostics.
- Added a platform protector factory used by the desktop bootstrap.
- Prevented Linux secure-file callers from rewriting an existing Windows
  DPAPI wrapper as plaintext.
- Linux replacement login now recognizes only a structurally valid legacy
  DPAPI envelope, starts a fresh restricted runtime, and advances its known
  generation; unrelated corruption remains a typed vault failure. Managed
  refresh failures that indicate unreadable credentials request
  reauthentication.
- Vault sealing now compensates filesystem/publish failures in memory by
  restoring the previous keyring secret and local marker (or deleting an
  uncommitted keyring item), preventing marker/Secret-Service divergence.
- Profile removal now deletes vault credentials before the database row, so a
  Secret Service failure leaves the profile and local envelope retryable.
- Added a Linux-only reauthentication migration path for structurally valid
  legacy DPAPI envelopes: replacement login starts with a fresh restricted
  runtime and advances the preserved generation. Other corruption remains
  export-diagnostics only.

## Tests and checks

- `cargo test --manifest-path rust/Cargo.toml accounts::vault` — PASS (10
  passed on Windows; Linux-only tests are cfg-gated).
- `cargo test --manifest-path rust/Cargo.toml secure_file` — PASS (5 passed).
- `cargo test --manifest-path rust/Cargo.toml accounts::service::tests` — PASS
  (14 passed on Windows; Linux-only legacy reauthentication test is cfg-gated).
- Deterministic vault fault tests cover keyring rollback after a filesystem
  failure and retryable protector deletion.
- Linux-only service coverage exercises successful replacement after an
  unreadable legacy envelope (cfg-gated on this Windows host).
- Fix-round 2 added checked local-artifact deletion, explicit compensation
  error propagation, terminal cleanup for early login failures, and strict
  legacy ciphertext validation. Deterministic rollback-failure, retryable
  deletion, and corrupt-base64 regression tests were added.
- Fix-round 3 enumerates and checks every temporary artifact during removal,
  uses a scoped runtime cleanup guard for every managed-login exit path, and
  adds multi-temp deletion coverage. Rollback and local cleanup failures remain
  typed and retryable; original operation errors are preserved.
- Fix-round 4 makes runtime cleanup result-aware on successful login and
  transactionalizes runtime restoration: partial credential directories are
  removed on any restore failure, with cleanup errors surfaced as typed
  storage failures. Added multi-temp and partial-restore regression coverage.
- Fix-round 5 delays profile readiness and terminal `Succeeded` until runtime
  cleanup succeeds, and applies result-aware cleanup to managed refresh.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
  — PASS.
- `cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml` — PASS.
- `cargo fmt --all -- --check` and `git diff --check` — PASS.

## Linux-runtime limitation

Ubuntu Secret Service/D-Bus round-trip tests were not run on this Windows
host. Cross-target compilation is also blocked because
`x86_64-linux-gnu-gcc` is not installed (the `ring` build script stops before
the Linux test binary is produced). No Linux keyring result is claimed; the
round-trip and unavailable/locked behavior must be exercised in Ubuntu CI or
an equivalent Linux environment with a disposable D-Bus Secret Service.
