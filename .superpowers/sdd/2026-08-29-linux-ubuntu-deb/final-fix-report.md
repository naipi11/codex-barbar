# Linux Debian final-review fix report

Branch: `codex/linux-deb`  
Review baseline: `91922f18`  
Implementation head: `21cfc88d`

No push, tag, GitHub Release, package publication, dependency addition, or
external repository mutation was performed.

## Commits

1. `0c57eae364123ecacc673fc4277c0f705ed24201` — Harden Linux credentials and managed login
2. `e1b11ca5bfcb18171d90b72f294046156e74ea2f` — Make Linux capabilities runtime authoritative
3. `21cfc88d098d86d2b4838b6e0a48d6aee79e17bd` — Bind Linux release artifacts to candidate commit

## Critical findings

### 1. Linux runtime credential permissions and symlink safety — fixed in code; Ubuntu execution pending

- Linux runtime, profile, session, and nested credential directories are
  created/restricted to `0700` and verified after creation.
- Runtime manifests, `config.toml`, and restored credential files are written
  through owner-only `0600` temporary files and atomic rename, then verified.
- Creation, collection, recovery scanning, and recursive cleanup reject a
  symlink in the target or any ancestor. Cleanup has a Linux regression proving
  a symlinked profile ancestor cannot delete the external target.
- `collect_bundle` verifies every directory and file mode before the bundle can
  reach `CredentialVault::seal_expected` / Secret Service.
- Windows DACL/DPAPI code paths remain selected by the existing Windows cfg.
- Added Linux-only tests for `0700`/`0600`, nested credentials, loose-mode
  rejection, symlinked creation ancestors, and no-follow cleanup.

These Linux-only tests could not execute on this Windows host. The attempted
Linux target check stopped in the pre-existing `ring` build script before
compiling this crate because `x86_64-linux-gnu-gcc` is unavailable. Ubuntu CI
must compile and run them.

### 2. Non-Windows plaintext secure-file fallback — fixed in code; Secret Service runtime pending

- Linux secret-bearing files now store their complete UTF-8 value in the
  existing `keyring` Secret Service backend. The local file contains only
  `format`, `version`, `protection=linux-secret-service`, and a path-derived
  SHA-256 opaque identifier.
- Linux secret reads reject raw plaintext, foreign DPAPI wrappers, unknown
  wrappers, and path-mismatched markers. Secret writes accept only a missing
  file or the matching Linux marker; existing plaintext/foreign/unreadable
  files are not rewritten or treated as migration input.
- A failed marker write compensates by restoring/deleting the Secret Service
  value. Error text is fixed and contains no keyring payload.
- `settings/api_keys.rs`, `settings/manual_cookies.rs`, and
  `core/token_accounts.rs` retain their existing call sites but now receive the
  Linux fail-closed/Secret Service behavior from `secure_file`.
- Non-secret preferences and identity metadata use an explicitly named
  owner-only public-file API on Linux, so Secret Service unavailability does
  not lock the active SQLite `AppSettings` or ordinary preferences.
- Legacy `settings.json` inline proxy passwords, manual cookies, and API tokens
  are removed from the Linux public representation; attempts to save new
  inline credentials there fail with `io::ErrorKind::Unsupported`. Dedicated
  secret stores remain the supported path.
- Windows continues to use the existing DPAPI wrapper. Other unsupported
  platforms retain their previous behavior outside the Linux release scope.

Linux tests cover plaintext rejection/no-rewrite, marker path binding and lack
of secret bytes, owner-only non-secret files, and inline-settings scrubbing.
They require Ubuntu CI. A real Secret Service round trip and locked/unavailable
collection behavior remain mandatory Ubuntu desktop acceptance checks.

### 3. Fresh managed-login lifecycle and cancellation — fixed and exercised on Windows

- A fresh operation acquires the actor, inserts its requested label as a
  `Pending` managed profile, and carries that exact label to the final `Ready`
  update.
- `AccountRepository::update_profile` now rejects a zero-row update with
  `DB_PROFILE_UPDATE_MISSING`; success can no longer be published after a
  missing update.
- The actor owns an operation-specific cancellation watch channel. A wrong or
  post-commit operation id is rejected. Before the vault commit point, cancel
  shuts down/cancels the matching App Server login, cleans the runtime, removes
  new-profile vault/identity state and the pending row, publishes `Cancelled`,
  then releases actor state.
- Failure follows the same compensating cleanup. Vault-cleanup failure is
  surfaced instead of silently deleting the recovery handle.
- Success updates the row, publishes terminal success while the actor is still
  present, releases the actor, then publishes `ProfilesChanged`.
- Existing managed re-login keeps its prior profile/vault on pre-commit
  failure.

RED evidence was observed before implementation for the missing-row update,
fresh empty-repository success, and exact cancellation. Final account service
suite: 21 passed, 0 failed. The full shared suite also includes these tests.

### 4. Frozen command allowlist — fixed and exercised

- Added registered `get_platform_capabilities` to the intentional V1 allowlist.
- The mismatch diagnostic now joins `Compare-Object`'s actual `InputObject`
  values instead of rendering empty object strings.
- Before the fix, `assert-v1-boundaries.ps1` failed with the empty
  `missing=[] extra=[]` diagnostic. Final boundary and release-policy scripts
  both exit 0.

## Important findings

### 5. Runtime capability contract and Accounts UI — fixed in code; Linux desktop probe pending

- `NotificationCapabilityStatus` has one backend type. The production platform
  snapshot composes the existing runtime notification probe with
  `keyring::Entry::store_status()` (which initializes/checks the Secret Service
  store without creating, reading, or deleting a credential).
- Both `get_platform_capabilities` and bootstrap use that runtime snapshot.
- The managed-login command independently fails closed with
  `MANAGED_CREDENTIALS_UNAVAILABLE` when secure storage is absent.
- Settings defaults to an unavailable `other` capability set until bootstrap,
  rather than briefly assuming Windows support.
- Accounts disables the label and add-login controls and displays the existing
  keyring explanation when `managedCredentials` is false. Frontend unit and
  Settings-integration regressions cover the behavior.
- Linux notification copy now uses the Linux locale facade and desktop-neutral
  copy. Windows retains Windows-specific settings/test copy. Corrupted Chinese
  pricing notification strings were repaired and covered.

The backend mapping test and command fail-closed test pass on Windows. Actual
GNOME/KDE D-Bus notification and Secret Service availability transitions must
still be exercised on Ubuntu.

### 6. Exact release provenance — fixed and exercised without publishing

- `aggregate-release-assets.mjs` requires a validated 40-hex expected commit
  and requires both target manifests' `commit` fields to equal it.
- Release aggregation passes the validated `github.sha` through an environment
  variable. The mismatch/missing-commit Node regression is green.
- Before the only `gh release create`, the publish job fetches the exact remote
  `refs/tags/v<version>`, dereferences it to a commit, and requires equality
  with `GITHUB_SHA`. Build-only workflow dispatch remains available; a
  publish-draft dispatch must have the exact remote tag.
- The static workflow guard enforces the matching condition, tag commands,
  ordering, sole draft creation, commit argument, and both build dependencies.

No tag fetch, release creation, or other release side effect was executed
locally.

### 7. Linux npm Codex process activity — fixed and exercised

- Process discovery recognizes the exact resolver argv relationship only:
  absolute canonical-shaped `<prefix>/bin/node` followed immediately by
  `<prefix>/lib/node_modules/@openai/codex/bin/codex.js`.
- Relative node paths, arbitrary scripts, and mismatched prefixes remain idle;
  descendants of the verified root remain included.
- The focused proc reader suite passes 5/5, including the verified npm fixture
  and negative node cases.

### 8. Ayatana build contract — fixed and policy guarded

- Both `linux-check` and `linux-build` set
  `TAURI_LINUX_AYATANA_APPINDICATOR: "1"` at job scope.
- `assert-release-workflow.ps1` requires the variable in both Ubuntu jobs.

## Minor triage

- Taskbar measurement route: fixed. It waits for bootstrap and renders/measures
  only when `taskbarStatus` is available; regression is green.
- `/proc` cadence: safely mitigated from a synchronous traversal every 250 ms
  to once per second on Linux while retaining the Windows cadence. It remains
  a synchronous best-effort probe; profile before further complexity.
- Task 2 report wording: fixed to state macOS/BSD code is retained but was not
  executed or proved on this Windows host.
- Notification locale/copy: fixed as described under Important 5.
- Windows PowerShell 5.1 BOM: `windows-release-build.ps1` now writes manifest
  and rewritten SBOM JSON with `UTF8Encoding(false)`; the script parses under
  Windows PowerShell 5.1. A full Windows release staging run was not repeated.

## Final verification

The following commands were run against the final implementation changes
before the three commits:

- `cargo fmt --all -- --check` — exit 0.
- `git diff --check` — exit 0.
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`
  — exit 0.
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`
  — exit 0.
- `cargo test --quiet --manifest-path rust/Cargo.toml -- --test-threads=1`
  — exit 0: library 528 passed / 1 ignored; App Server contract 17 passed;
  icon assets 3 passed; doc tests 0.
- `cargo test --quiet --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`
  — exit 0: 264 passed.
- `pnpm exec vitest run` from `apps/desktop-tauri` — exit 0: 39 files,
  303 tests passed.
- `pnpm run build` from `apps/desktop-tauri` — exit 0 (`tsc --noEmit` and
  Vite production build).
- `pnpm run tauri:build:debug` from `apps/desktop-tauri` — exit 0; rebuilt
  `target/debug/codex-barbar.exe` and
  `target/debug/bundle/nsis/codex-barbar_1.1.0_x64-setup.exe`.
- `node --test scripts/aggregate-release-assets.test.mjs` — exit 0: 4 passed.
- `node --check scripts/aggregate-release-assets.mjs` and test module — exit 0.
- `powershell.exe ... scripts/assert-v1-boundaries.ps1` — exit 0.
- `powershell.exe ... scripts/assert-release-workflow.ps1` — exit 0.
- Windows PowerShell 5.1 AST parse of the two guards and
  `windows-release-build.ps1` — exit 0.

Attempted but not passed:

- `cargo check --manifest-path rust/Cargo.toml --target x86_64-unknown-linux-gnu --lib`
  — exit 1 in `ring v0.17.14` before this crate compiled:
  `ToolNotFound: failed to find tool x86_64-linux-gnu-gcc`.

Not available on this Windows host and therefore not claimed: Ubuntu Rust/Tauri
compile, Linux-only unit execution, Debian build/control inspection, GNOME or
KDE launch, Wayland/X11 behavior, D-Bus notifications, Secret Service
roundtrip/lock handling, and dpkg install/remove acceptance.

## Windows CUA regression proof

The debug binary was rebuilt, the installed single instance was stopped, and
the new binary was launched with `CODEXBAR_PROOF_MODE=settings:providers`.
Cua Driver 0.22.0 enumerated the exact debug PID/window and captured the
Accounts and Notifications surfaces. Accounts showed the selected profile and
enabled Windows managed-login entry; Notifications retained the Windows copy
and current controls. The debug process was then stopped and the original
installed executable was relaunched (PID 22832 at handoff).

- `final-settings-providers.png` — SHA-256
  `adbfc5d2df6d23be2f54137c032d60d67f5c408e899d117f0eebe45b67090d71`
- `final-settings-notifications.png` — SHA-256
  `f4bf97c6d565ec72a2c3c3df4ddda51d01d6c507a6f2c36dbd5e543e97314f5f`

This is Windows regression evidence only. The disabled Secret Service UI and
Linux measurement-route absence remain Ubuntu acceptance items.
