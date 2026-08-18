# codex-barbar V1 Phase 2 Account Vault Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add CurrentCli plus safe Managed profiles, strict DPAPI-at-rest credentials, crash-safe isolated `CODEX_HOME` activation, SQLite snapshots, and deterministic refresh coordination without ever switching or copying the user's main CLI credentials.

**Architecture:** A synchronous SQLite repository persists non-secret metadata, settings, last-success snapshots, and current error state. `CredentialVault` stores versioned credential bundles using DPAPI Current User and verified atomic replacement; `RuntimeHomeManager` creates no-follow, current-user/SYSTEM-only activation directories. One `AccountOperationActor` owns login/refresh/switch/remove operations so at most one Managed profile and one App Server exist at once, while `RefreshScheduler` merges triggers and applies cooldown/staleness/backoff rules.

**Tech Stack:** Rust stable edition 2024, Tokio actor/broadcast/time primitives, rusqlite, serde/serde_json, chrono, uuid, sha2, existing `windows` crate DPAPI/DACL/file APIs, Phase-1 `AppServerFactory`.

## Global Constraints

- Supported platform is native Windows 11 23H2 or newer on x64; Windows 10, Windows on ARM, WSL, macOS, and Linux builds are outside V1.
- The shipping product supports Codex only. Claude, Gemini, every other provider, browser-cookie import, generic API-key/token accounts, cost charts, sessions, workspaces, PTY, FloatBar, PopOut, and usage notifications stay outside the release surface.
- Account and quota network access goes only through the official but experimental `codex app-server` stdio JSONL process; `experimentalApi` remains `false`, and private `/wham/*` calls are removed without a release fallback.
- `CurrentCli` is read-only: it may call `initialize`, send `initialized`, call `account/read`, and call `account/rateLimits/read`; it has no login, logout, delete, switch, or configuration-write method.
- Every `Managed` profile uses an isolated `CODEX_HOME`, forces `cli_auth_credentials_store = "file"`, clears authentication override variables in the child only, and keeps idle credentials in strict DPAPI Current User ciphertext.
- DPAPI failure is fatal for the vault operation. There is no Local Machine scope fallback and no plaintext fallback. Vault replacement is temporary-file, flush, then atomic replace.
- The React WebView receives no OAuth token, refresh token, raw `auth.json`, arbitrary filesystem capability, arbitrary shell/process capability, or arbitrary network capability.
- Startup does not check for, download, or apply updates. A user-initiated action may check a public GitHub Release or open the fixed Releases page; no PAT is embedded or requested.
- Default refresh interval is 5 minutes; panel-open refresh threshold is 60 seconds; manual cooldown is 15 seconds; transient backoff is 30 seconds, 2 minutes, 5 minutes, then 15 minutes with ±20% jitter.
- Product defaults are remaining-quota display, system theme, system language, autostart off, no telemetry, and `%LOCALAPPDATA%\codex-barbar` storage for installed and portable builds.
- Toolchain is pnpm 10.18.1, Node 20, Rust stable edition 2024, and target `x86_64-pc-windows-msvc`. Use pnpm only.
- Do not introduce a new third-party crate, package, build tool, hosted service, or telemetry endpoint without explicit user confirmation. Additional features on the already-present `windows` crate must be limited to the Win32 APIs named in the approved design.
- Preserve the complete Win-CodexBar Git history, MIT license, author attribution, `win-upstream`, `mac-reference`, and tag `upstream/win-codexbar-2026-08-03`.
- Every UI/tray/settings change requires a fresh desktop build, termination of the old single instance, and real Windows CUA evidence as described by `AGENTS.md`.
- Do not push, publish a GitHub Release, open a pull request, buy a signing certificate, or contact either upstream repository unless the user explicitly authorizes that external action.

---

## File Responsibility Map

| Path | Responsibility after Phase 2 |
|---|---|
| `rust/src/accounts/model.rs` | Profile, login, lifecycle, service event types |
| `rust/src/storage/database.rs` | Open, WAL, backup, transactional migrations, read-only failure mode |
| `rust/src/storage/account_repository.rs` | Profile metadata and selected-profile transaction rules |
| `rust/src/storage/usage_repository.rs` | Last success and current error as independent persisted values |
| `rust/src/accounts/secret_bytes.rs` | Non-printing, best-effort zeroing byte owner |
| `rust/src/accounts/vault/crypto.rs` | Strict DPAPI Current User protect/unprotect |
| `rust/src/accounts/vault/store.rs` | Versioned envelope, compare-and-swap generation, atomic publish/recovery |
| `rust/src/accounts/windows_acl.rs` | Protected Current User + SYSTEM DACL and reparse checks |
| `rust/src/accounts/runtime_home.rs` | Isolated runtime creation, fixed config, bundle collect/restore, no-follow cleanup |
| `rust/src/accounts/recovery.rs` | Startup recovery for runtime/Vault/SQLite lifecycle checkpoints |
| `rust/src/accounts/actor.rs` | Single active profile/App Server operation owner |
| `rust/src/accounts/service.rs` | Profile API and orchestration facade |
| `rust/src/refresh/scheduler.rs` | Merge, cooldown, panel threshold, interval, staleness, backoff, jitter |
| `apps/desktop-tauri/src-tauri/src/commands/accounts.rs` | Redacted profile/login/refresh Tauri commands |
| `apps/desktop-tauri/src-tauri/src/state.rs` | Shared service/scheduler/repository ownership and bootstrap cache |

## Test Support Contract

- `rust/src/accounts/test_support.rs` is compiled only with `#[cfg(test)]` and owns `fixture`, `profile_id`, `snapshot`, `old_snapshot`, synthetic credential bundles, `FakeAppServerFactory`, fake clocks, and lifecycle fault injection. It uses `tempfile::TempDir`, fixed fake tokens, and never reads `%USERPROFILE%\.codex`.
- Storage-only `test_repositories` and `test_usage_repository` wrappers live in the storage modules' `#[cfg(test)]` blocks and open temporary SQLite files with the production migrations.
- Vault tests inject a `CurrentUserProtector` trait implementation; Windows-native round trips use the real current-user DPAPI only with random synthetic bytes and delete their temporary roots.
- Runtime/DACL tests derive all targets from their temporary root, refuse reparse traversal, and register cleanup before the first assertion. Scheduler tests inject `Clock` and `JitterSource`; they never sleep on wall-clock time.
- Every helper referenced below is introduced in one of these named files before its first test is compiled and is not production-exported.

## Frozen Phase-2 Service Contract

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedLoginMethod { Browser, DeviceCode }

#[derive(Debug, Clone)]
pub struct StartManagedLogin {
    pub label: String,
    pub method: ManagedLoginMethod,
    pub replace_profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedLoginStage { Starting, AwaitingUser, Succeeded, Failed, Cancelled }

#[derive(Debug, Clone)]
pub struct ManagedLoginStatus {
    pub operation_id: Uuid,
    pub profile_id: ProfileId,
    pub stage: ManagedLoginStage,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
    pub error_kind: Option<AppErrorKind>,
}

pub struct AccountProfilesSnapshot {
    pub profiles: Vec<AccountProfile>,
    pub selected_profile_id: ProfileId,
}

#[derive(Debug, Clone)]
pub enum AccountServiceEvent {
    ProfilesChanged(AccountProfilesSnapshot),
    LoginChanged(ManagedLoginStatus),
    SelectedProfileChanged { profile_id: ProfileId },
    UsageStateChanged(ProfileUsageState),
    RefreshStateChanged { profile_id: ProfileId, status: RefreshStatus },
}

#[derive(Debug, thiserror::Error)]
pub enum AccountServiceError {
    #[error("application operation failed")]
    App(AppError),
    #[error("profile label is invalid")]
    InvalidLabel,
    #[error("Current CLI profile is immutable")]
    CurrentCliImmutable,
    #[error("another account operation is active")]
    Busy,
    #[error("account already exists")]
    DuplicateProfile { existing_profile_id: ProfileId },
    #[error("login operation was not found")]
    LoginOperationNotFound,
}

impl From<AppError> for AccountServiceError {
    fn from(error: AppError) -> Self { Self::App(error) }
}

pub struct AccountProfileService { /* private repositories, vault, actor */ }

impl AccountProfileService {
    pub fn new(
        repositories: AccountRepositories,
        vault: CredentialVault,
        runtime_homes: RuntimeHomeManager,
        app_server_factory: Arc<dyn AppServerFactory>,
    ) -> Arc<Self>;
    pub fn initialize(&self) -> Result<AccountProfilesSnapshot, AccountServiceError>;
    pub fn snapshot(&self) -> Result<AccountProfilesSnapshot, AccountServiceError>;
    pub async fn start_managed_login(
        self: &Arc<Self>,
        request: StartManagedLogin,
    ) -> Result<ManagedLoginStatus, AccountServiceError>;
    pub async fn cancel_managed_login(&self, operation_id: Uuid)
        -> Result<(), AccountServiceError>;
    pub fn select_profile(&self, profile_id: ProfileId)
        -> Result<AccountProfilesSnapshot, AccountServiceError>;
    pub fn rename_managed_profile(&self, profile_id: ProfileId, label: String)
        -> Result<AccountProfilesSnapshot, AccountServiceError>;
    pub async fn remove_managed_profile(&self, profile_id: ProfileId)
        -> Result<AccountProfilesSnapshot, AccountServiceError>;
    pub async fn request_refresh(self: &Arc<Self>, profile_id: ProfileId, trigger: RefreshTrigger)
        -> Result<RefreshDisposition, AccountServiceError>;
    pub fn subscribe(&self) -> broadcast::Receiver<AccountServiceEvent>;
    pub async fn shutdown(&self, timeout: Duration) -> Result<(), AccountServiceError>;
}
```

### Task 1: Add Profile models and transactional SQLite repositories

**Files:**
- Create: `rust/src/accounts/mod.rs`
- Create: `rust/src/accounts/model.rs`
- Create: `rust/src/storage/mod.rs`
- Create: `rust/src/storage/database.rs`
- Create: `rust/src/storage/migrations.rs`
- Create: `rust/src/storage/account_repository.rs`
- Create: `rust/src/storage/usage_repository.rs`
- Modify: `rust/src/lib.rs`
- Test: inline tests in the new storage/model modules

**Interfaces:**
- Consumes: roadmap `ProfileId = Uuid`, `ProfileKind`, `AuthMode`, `AccountProfile`, `ProfileUsageSnapshot`, `ProfileUsageState`.
- Produces: `AppDatabase`, `AccountRepository`, `UsageRepository` trait, `SqliteUsageRepository`, `AccountRepositories`, internal `UsageCacheKey { profile_id, provider_id }`, `BootstrapState`, and `DatabaseBootstrap::{Ready, ReadOnlyFailure}`.

- [ ] **Step 1: Write failing uniqueness and last-success/error tests**

```rust
#[test]
fn current_cli_is_unique_and_selected_by_default() {
    let repos = test_repositories();
    let first = repos.accounts.ensure_current_cli(Utc::now()).unwrap();
    let second = repos.accounts.ensure_current_cli(Utc::now()).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(repos.accounts.selected_profile_id().unwrap(), first.id);
    assert_eq!(repos.accounts.list_ready().unwrap().iter().filter(|p| p.kind == ProfileKind::CurrentCli).count(), 1);
}

#[test]
fn refresh_error_does_not_replace_last_success() {
    let repo = test_usage_repository();
    repo.save_success(&snapshot()).unwrap();
    repo.save_error(profile_id(), &offline_error()).unwrap();
    let state = repo.load_state(profile_id()).unwrap();
    assert_eq!(state.snapshot, Some(snapshot()));
    assert_eq!(state.current_error.unwrap().kind, AppErrorKind::OfflineOrTimeout);
}
```

- [ ] **Step 2: Run the tests and verify missing repository failure**

```powershell
cargo test --manifest-path rust/Cargo.toml current_cli_is_unique_and_selected_by_default
cargo test --manifest-path rust/Cargo.toml refresh_error_does_not_replace_last_success
```

Expected: FAIL because accounts/storage modules do not exist.

- [ ] **Step 3: Implement database open, migration, and schema**

Use this schema in migration `1`:

```sql
CREATE TABLE schema_meta (component TEXT PRIMARY KEY, version INTEGER NOT NULL);
CREATE TABLE app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
CREATE TABLE account_profiles (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK(kind IN ('currentCli','managed')),
  label TEXT NOT NULL,
  auth_mode TEXT NOT NULL,
  lifecycle TEXT NOT NULL CHECK(lifecycle IN ('pending','ready','removing')),
  email_fingerprint BLOB,
  created_at INTEGER NOT NULL,
  last_selected_at INTEGER,
  last_success_at INTEGER
);
CREATE UNIQUE INDEX one_current_cli ON account_profiles(kind) WHERE kind = 'currentCli';
CREATE TABLE usage_snapshots (
  profile_id TEXT NOT NULL REFERENCES account_profiles(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL CHECK(provider_id = 'codex'),
  snapshot_json TEXT NOT NULL,
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY(profile_id, provider_id)
);
CREATE TABLE profile_refresh_state (
  profile_id TEXT NOT NULL REFERENCES account_profiles(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL CHECK(provider_id = 'codex'),
  error_json TEXT,
  attempted_at INTEGER,
  PRIMARY KEY(profile_id, provider_id)
);
```

`UsageCacheKey::codex(profile_id)` is the only public constructor in V1 and always supplies existing `ProviderId::Codex`; repository convenience methods accept Profile ID and derive that composite key internally. The in-memory hot cache uses the same key even though the redacted frontend DTO groups the only shipping provider by Profile. Enable `foreign_keys=ON`, `journal_mode=WAL`, and transactions. Before a version-changing migration, checkpoint/close the WAL, copy the database to a timestamped backup beside it, retain the three newest backups, reopen, and run the migration transaction. A migration error rolls back and returns `StorageFailure`; it never deletes/recreates the source database. Email, tokens, raw auth files, and detailed error text never become columns.

`DatabaseBootstrap::Ready(AccountRepositories)` is the only writable mode. `DatabaseBootstrap::ReadOnlyFailure { database_path, backup_path, error }` preserves the original file, exposes only redacted path class/error data to `BootstrapState`, disables profile mutations/refresh, and lets the desktop show the recovery/export-diagnostics UI instead of terminating. Add injected migration-failure and reopen tests proving the source bytes remain unchanged.

- [ ] **Step 4: Run migration, rollback, and repository suites**

```powershell
cargo test --manifest-path rust/Cargo.toml storage::migrations::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml storage::account_repository::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml storage::usage_repository::tests -- --nocapture
```

Expected: fresh migration, idempotent open, backup retention, injected rollback, one CurrentCli, selected profile, cascade delete, cached bootstrap, and success/error separation pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts rust/src/storage rust/src/lib.rs
git commit -m "Add account and usage repositories"
```

### Task 2: Implement strict Current User DPAPI and secret byte ownership

**Files:**
- Create: `rust/src/accounts/secret_bytes.rs`
- Create: `rust/src/accounts/vault/mod.rs`
- Create: `rust/src/accounts/vault/crypto.rs`
- Create: `rust/src/accounts/vault/envelope.rs`
- Modify: `rust/src/accounts/mod.rs`
- Modify: `rust/Cargo.toml`
- Test: inline tests in the new modules

**Interfaces:**
- Consumes: DPAPI `CryptProtectData`/`CryptUnprotectData` from the existing `windows` crate.
- Produces: `SensitiveBytes`, `CredentialProtector`, `WindowsDpapiProtector`, `VaultEnvelope`, `ManagedCredentialBundle`, `PrivateProfileMetadata`, and `VaultError`.

- [ ] **Step 1: Write failing no-fallback and non-printing tests**

```rust
#[test]
fn protect_failure_has_no_machine_or_plaintext_fallback() {
    let protector = FailingCurrentUserProtector::default();
    let error = protector.protect_current_user(profile_id(), TEST_TOKEN).unwrap_err();
    assert!(matches!(error, VaultError::ProtectFailed));
    assert_eq!(protector.calls(), 1);
}

#[test]
fn sensitive_bytes_debug_never_prints_content() {
    let bytes = SensitiveBytes::new(TEST_TOKEN.to_vec());
    assert_eq!(format!("{bytes:?}"), "SensitiveBytes([REDACTED])");
}

#[cfg(windows)]
#[test]
fn dpapi_flags_are_current_user_and_ui_forbidden_only() {
    assert_eq!(DPAPI_FLAGS, CRYPTPROTECT_UI_FORBIDDEN);
    assert_eq!(DPAPI_FLAGS & CRYPTPROTECT_LOCAL_MACHINE, 0);
}
```

- [ ] **Step 2: Run focused tests and verify missing types**

```powershell
cargo test --manifest-path rust/Cargo.toml protect_failure_has_no_machine_or_plaintext_fallback
cargo test --manifest-path rust/Cargo.toml sensitive_bytes_debug_never_prints_content
```

Expected: FAIL because strict vault primitives are absent.

- [ ] **Step 3: Implement strict crypto and versioned envelope**

Extend only the existing `windows` crate features with:

```toml
"Win32_Security_Authorization",
"Win32_Storage_FileSystem",
"Win32_System_Memory",
```

Implement these exact ownership/error types before the DPAPI calls:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    UnsupportedPlatform,
    ProtectFailed,
    UnprotectFailed,
    InvalidEnvelope,
    WrongProfile,
    GenerationConflict,
    Io,
}

pub trait CredentialProtector: Send + Sync {
    fn protect_current_user(&self, profile_id: ProfileId, plaintext: &[u8])
        -> Result<Vec<u8>, VaultError>;
    fn unprotect_current_user(&self, profile_id: ProfileId, ciphertext: &[u8])
        -> Result<SensitiveBytes, VaultError>;
}

pub struct SensitiveBytes { bytes: Vec<u8> }
impl SensitiveBytes {
    pub fn new(bytes: Vec<u8>) -> Self;
    pub fn as_slice(&self) -> &[u8];
}

pub struct CredentialFile {
    pub relative_path: String,
    pub contents: SensitiveBytes,
}

pub struct PrivateProfileMetadata {
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub auth_mode: AuthMode,
}

pub struct ManagedCredentialBundle {
    pub files: Vec<CredentialFile>,
    pub private_metadata: PrivateProfileMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEnvelope {
    pub format: String,
    pub version: u32,
    pub protection: String,
    pub profile_id: ProfileId,
    pub generation: u64,
    pub sealed_at: DateTime<Utc>,
    pub ciphertext_base64: String,
}
```

`WindowsDpapiProtector` calls Current User DPAPI once with `CRYPTPROTECT_UI_FORBIDDEN`; it never sets `CRYPTPROTECT_LOCAL_MACHINE`. Optional entropy is the fixed ASCII domain `codex-barbar/vault/v1` plus profile UUID and is not described as an independent key. `ManagedCredentialBundle` uses a private canonical encoder/decoder inside the Vault module that copies borrowed byte slices into one `SensitiveBytes` plaintext buffer; credential ownership types themselves never implement serde. The outer envelope is JSON containing `format = "codex-barbar-vault"`, `version = 1`, `protection = "windows-dpapi-current-user"`, profile ID, generation, sealed time, and base64 DPAPI ciphertext. Reject every other protection label. `SensitiveBytes` implements a custom redacted `Debug`, no `Display`/`Serialize`, and overwrites its live allocation via volatile writes in `Drop`.

- [ ] **Step 4: Run DPAPI and envelope matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::vault::crypto::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml accounts::vault::envelope::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml accounts::secret_bytes::tests -- --nocapture
```

Expected: same-user round trip, wrong-profile entropy, tampered ciphertext, unknown version, wrong protection label, protect/unprotect failure, and redacted error/debug tests pass on Windows.

- [ ] **Step 5: Commit**

```powershell
git add rust/Cargo.toml Cargo.lock rust/src/accounts/secret_bytes.rs rust/src/accounts/vault rust/src/accounts/mod.rs
git commit -m "Add strict current-user DPAPI vault"
```

### Task 3: Publish Vault files atomically with generation recovery

**Files:**
- Create: `rust/src/accounts/vault/store.rs`
- Modify: `rust/src/accounts/vault/mod.rs`
- Modify: `rust/src/accounts/vault/envelope.rs`
- Test: `rust/src/accounts/vault/store.rs` inline tests

**Interfaces:**
- Consumes: `CredentialProtector`, encrypted `VaultEnvelope`.
- Produces: `CredentialVault::seal_expected`, `unseal`, `inspect`, `recover_atomic_artifacts`, `remove`, and `VaultInfo`.

- [ ] **Step 1: Write failing crash-point tests**

```rust
#[test]
fn crash_after_temp_flush_preserves_old_final() {
    let old = fixture().seal_generation_one();
    fixture().inject(VaultWriteStep::TempFlushed);
    fixture().attempt_generation_two().unwrap_err();
    fixture().vault.recover_atomic_artifacts().unwrap();
    assert_eq!(fixture().vault.unseal(profile_id()).unwrap().1.generation, old.generation);
}

#[test]
fn corrupt_final_recovers_valid_backup() {
    let fixture = vault_with_valid_backup();
    fixture.corrupt_final();
    fixture.vault.recover_atomic_artifacts().unwrap();
    assert_eq!(fixture.vault.unseal(profile_id()).unwrap().1.generation, 1);
}
```

- [ ] **Step 2: Run the tests and verify non-atomic store failure**

```powershell
cargo test --manifest-path rust/Cargo.toml crash_after_temp_flush_preserves_old_final
cargo test --manifest-path rust/Cargo.toml corrupt_final_recovers_valid_backup
```

Expected: FAIL because no atomic vault store exists.

- [ ] **Step 3: Implement compare-and-swap and Windows atomic publish**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultInfo {
    pub profile_id: ProfileId,
    pub generation: u64,
    pub sealed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultRecovery { KeptFinal, PromotedTemp, RestoredBackup }

impl CredentialVault {
    pub fn new(vault_root: PathBuf, protector: Arc<dyn CredentialProtector>) -> Self;
    pub fn seal_expected(
        &self,
        profile_id: ProfileId,
        expected_generation: Option<u64>,
        bundle: &mut ManagedCredentialBundle,
    ) -> Result<VaultInfo, VaultError>;
    pub fn unseal(&self, profile_id: ProfileId)
        -> Result<(ManagedCredentialBundle, VaultInfo), VaultError>;
    pub fn inspect(&self, profile_id: ProfileId) -> Result<Option<VaultInfo>, VaultError>;
    pub fn recover_atomic_artifacts(&self) -> Result<Vec<VaultRecovery>, VaultError>;
    pub fn remove(&self, profile_id: ProfileId) -> Result<(), VaultError>;
}
```

Create a random `.tmp` with `create_new` in the same vault directory, write only DPAPI ciphertext, flush Rust buffers, call `FlushFileBuffers`, then read/decrypt/verify profile and generation. First publish calls `MoveFileExW(temp_path, final_path, MOVEFILE_WRITE_THROUGH)`; an update calls `ReplaceFileW(final_path, temp_path, backup_path, REPLACEFILE_WRITE_THROUGH, None, None)`. Verify the final file before deleting backup. Recovery priority is valid final, verified temp only when final is absent, then valid backup; otherwise return `VaultFailure`. Generation compare-and-swap prevents an old runtime from overwriting newer credentials.

- [ ] **Step 4: Run every injected failure point**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::vault::store::tests -- --nocapture
```

Expected: before-create, after-temp-write, after-flush, after-replace, before-final-verify, and before-backup-delete tests all retain at least one valid ciphertext and no test token appears in the vault directory.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts/vault
git commit -m "Make vault updates atomic"
```

### Task 4: Create restricted, isolated Managed CODEX_HOME directories

**Files:**
- Create: `rust/src/accounts/windows_acl.rs`
- Create: `rust/src/accounts/credential_bundle.rs`
- Create: `rust/src/accounts/runtime_home.rs`
- Modify: `rust/src/accounts/mod.rs`
- Test: inline tests in the new modules

**Interfaces:**
- Consumes: `AppPaths.runtime`, `ManagedCredentialBundle`, profile ID.
- Produces: `RuntimeHomeManager`, `ManagedRuntimeHome`, `GuardedRuntimeDir`, `RecoveryCandidate`, and no-follow bundle collection/restoration.

- [ ] **Step 1: Write failing isolation/config/path tests**

```rust
#[test]
fn managed_homes_are_distinct_and_force_file_auth() {
    let a = manager().prepare_new(profile_a()).unwrap();
    let b = manager().prepare_new(profile_b()).unwrap();
    assert_ne!(a.codex_home(), b.codex_home());
    assert_eq!(
        std::fs::read_to_string(a.codex_home().join("config.toml")).unwrap(),
        "cli_auth_credentials_store = \"file\"\n"
    );
}

#[test]
fn bundle_restore_rejects_parent_absolute_and_reparse_paths() {
    for entry in ["../auth.json", r"C:\outside\auth.json"] {
        assert!(restore_entry(entry).is_err());
    }
    assert!(restore_fake_reparse_entry("auth.json").is_err());
}
```

- [ ] **Step 2: Run focused tests and verify missing runtime manager**

```powershell
cargo test --manifest-path rust/Cargo.toml managed_homes_are_distinct_and_force_file_auth
cargo test --manifest-path rust/Cargo.toml bundle_restore_rejects_parent_absolute_and_reparse_paths
```

Expected: FAIL because runtime/DACL modules are absent.

- [ ] **Step 3: Implement secure creation, fixed config, and no-follow traversal**

Build a protected DACL before `CreateDirectoryW`: inherited ACEs disabled; allow-full-control ACEs exactly for the current process token SID and `S-1-5-18` SYSTEM; no Users, Authenticated Users, Everyone, or Administrators ACE. Re-read and verify the security descriptor after creation. Reject `FILE_ATTRIBUTE_REPARSE_POINT` on the trusted root, session directory, every path component, and every credential file. Create `config.toml` fresh on every activation and never include it or the token-free `manifest.json` in the encrypted credential bundle. Recursively clean without following links.

```rust
impl RuntimeHomeManager {
    pub fn prepare_new(&self, profile_id: ProfileId) -> Result<ManagedRuntimeHome, RuntimeHomeError>;
    pub fn restore(&self, profile_id: ProfileId, bundle: &ManagedCredentialBundle, base_generation: u64)
        -> Result<ManagedRuntimeHome, RuntimeHomeError>;
    pub fn scan_recovery_candidates(&self) -> Result<Vec<RecoveryCandidate>, RuntimeHomeError>;
}
```

- [ ] **Step 4: Run pure and Windows-native ACL tests**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::runtime_home::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml accounts::windows_acl::tests -- --nocapture
```

Expected: exact protected DACL, no inherited ACE, two-profile isolation, fixed file-auth config, relative regular-file bundle, reparse rejection, and no-follow cleanup pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts/windows_acl.rs rust/src/accounts/credential_bundle.rs rust/src/accounts/runtime_home.rs rust/src/accounts/mod.rs
git commit -m "Create isolated managed Codex homes"
```

### Task 5: Recover interrupted runtime and Vault lifecycle stages

**Files:**
- Create: `rust/src/accounts/recovery.rs`
- Modify: `rust/src/accounts/runtime_home.rs`
- Modify: `rust/src/accounts/vault/store.rs`
- Modify: `rust/src/storage/account_repository.rs`
- Test: `rust/src/accounts/recovery.rs` inline tests

**Interfaces:**
- Consumes: token-free runtime manifest, Vault generation, Profile lifecycle row.
- Produces: `RuntimeManifest`, `RuntimeState`, `AccountRecovery::recover`, and redacted `RecoveryOutcome`.

- [ ] **Step 1: Write failing newer-runtime and invalid-runtime tests**

```rust
#[test]
fn recovery_seals_newer_runtime_without_losing_old_vault() {
    let old = fixture().seal_generation_one();
    let runtime = fixture().active_runtime(old.generation);
    fixture().write_refreshed_auth(&runtime, TEST_REFRESH_TOKEN);
    let report = fixture().recovery.recover().unwrap();
    assert_eq!(fixture().vault.unseal(profile_id()).unwrap().1.generation, 2);
    assert!(!runtime.path().exists());
    assert_eq!(report.iter().filter(|r| r.action == RecoveryActionTaken::Resealed).count(), 1);
}

#[test]
fn invalid_runtime_never_overwrites_valid_vault() {
    let before = fixture().read_final_vault_bytes();
    fixture().create_invalid_active_runtime();
    fixture().recovery.recover().unwrap();
    assert_eq!(fixture().read_final_vault_bytes(), before);
}
```

- [ ] **Step 2: Run tests and verify missing recovery state machine**

```powershell
cargo test --manifest-path rust/Cargo.toml recovery_seals_newer_runtime_without_losing_old_vault
cargo test --manifest-path rust/Cargo.toml invalid_runtime_never_overwrites_valid_vault
```

Expected: FAIL because recovery is absent.

- [ ] **Step 3: Implement the exact token-free manifest and rules**

```rust
pub struct RuntimeManifest {
    pub format: String,
    pub version: u32,
    pub session_id: Uuid,
    pub profile_id: ProfileId,
    pub base_vault_generation: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub state: RuntimeState,
}

pub enum RuntimeState { Preparing, LoggingIn, Active, ReadyToSeal }
```

Manifest contains no absolute path or credential. Derive paths only from trusted runtime root plus session ID. A newer final Vault wins over an older runtime; Active/ReadyToSeal at the same base generation may seal `base + 1`; a new pending ready profile may seal generation 1 then promote SQLite; invalid credentials never overwrite a valid Vault; Preparing/LoggingIn pending profiles are cleaned; Removing profiles finish deletion. Reparse/out-of-root paths are never traversed.

- [ ] **Step 4: Run the lifecycle crash matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::recovery::tests -- --nocapture
```

Expected: injected crash after unseal, child start, refreshed token writeback, temp flush, Vault replace, and before SQLite commit always leaves a valid Vault or the previous valid Vault and cleans/reports the runtime safely.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts/recovery.rs rust/src/accounts/runtime_home.rs rust/src/accounts/vault/store.rs rust/src/storage/account_repository.rs
git commit -m "Recover interrupted account sessions"
```

### Task 6: Implement Managed login, rename, switch, re-login, and removal actor

**Files:**
- Create: `rust/src/accounts/actor.rs`
- Create: `rust/src/accounts/service.rs`
- Create: `rust/src/accounts/test_support.rs`
- Modify: `rust/src/accounts/mod.rs`
- Test: inline tests in actor/service

**Interfaces:**
- Consumes: Phase-1 `AppServerFactory`, `CurrentCliSession`, `ManagedSession`, `LoginFlow`, `LoginEvent`; repositories, Vault, runtime manager.
- Produces: the frozen `AccountProfileService`, `StartManagedLogin`, `ManagedLoginStatus`, `ManagedLoginStage`, and `AccountServiceEvent`.

- [ ] **Step 1: Write failing read-only CLI and failed-relogin tests**

```rust
#[tokio::test]
async fn current_cli_never_uses_managed_or_login_methods() {
    let service = fixture().service();
    service.request_refresh(current_cli_id(), RefreshTrigger::Manual).await.unwrap();
    assert_eq!(fixture().factory.open_current_cli_calls(), 1);
    assert_eq!(fixture().factory.open_managed_calls(), 0);
    assert_eq!(fixture().factory.login_start_calls(), 0);
}

#[tokio::test]
async fn failed_relogin_keeps_previous_vault_generation() {
    let before = fixture().vault.inspect(managed_id()).unwrap().unwrap();
    fixture().factory.fail_next_login();
    fixture().service().start_managed_login(relogin_request(managed_id())).await.unwrap();
    fixture().wait_for_login_failure().await;
    assert_eq!(fixture().vault.inspect(managed_id()).unwrap().unwrap().generation, before.generation);
}
```

- [ ] **Step 2: Run tests and verify missing actor/service**

```powershell
cargo test --manifest-path rust/Cargo.toml current_cli_never_uses_managed_or_login_methods
cargo test --manifest-path rust/Cargo.toml failed_relogin_keeps_previous_vault_generation
```

Expected: FAIL because account orchestration is absent.

- [ ] **Step 3: Implement the single-operation actor and login transaction**

The actor uses `tokio::select!` over login event, cancel, a 15-minute login deadline, and shutdown; no long-held `&mut ManagedSession` prevents cancellation. On deadline it sends `account/login/cancel` for that exact login ID before teardown. New login is pending row → restricted empty runtime → `open_managed` → browser `chatgpt` or device-code start → backend validates the exact HTTPS login URL and opens it without exposing generic URL IPC → completion → verify `config.toml` is still exactly `cli_auth_credentials_store = "file"`, `auth.json` and every credential entry are ordinary non-reparse files below the runtime root, and `account_read(false)` reports ChatGPT → normalized-email SHA-256 fingerprint duplicate check → child shutdown → bundle collection → strict seal/readback → runtime cleanup → pending-to-ready transaction → select. A browser failure/timeout returns `Failed` with a fixed “Retry with device code” action; accepting it starts a new `DeviceCode` operation only after the failed browser session is shut down. Failed/cancelled new login deletes pending/runtime; failed re-login leaves the prior Vault. Missing email is allowed only with a unique nonblank label. API-key identity is rejected with `ApiKeyNoQuota`.

Selection publishes the target's cache and transactionally updates only `selected_profile_id` immediately; it never changes any CLI file. The actor then finishes/cancels and seals any old Managed operation before it starts the target refresh, so only one plaintext runtime and App Server exist even during a rapid switch. Removal rejects CurrentCli, marks Managed as Removing, falls back to CurrentCli when selected, stops the operation, removes runtime/Vault/usage, then metadata. Incomplete removal remains recoverable.

- [ ] **Step 4: Run lifecycle and isolation suites**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::actor::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml accounts::service::tests -- --nocapture
```

Expected: browser/device-code success, exact-ID cancel, 15-minute fake-clock timeout, browser-to-device-code retry after teardown, method failure, duplicate email, email-less unique label, API-key reject, busy second login, rename, cache-first select, failed re-login, CurrentCli immutability, selected removal fallback, and token-free events pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts/actor.rs rust/src/accounts/service.rs rust/src/accounts/test_support.rs rust/src/accounts/mod.rs
git commit -m "Add managed account lifecycle service"
```

### Task 7: Route Managed refresh through unseal, App Server, and reseal

**Files:**
- Modify: `rust/src/accounts/actor.rs`
- Modify: `rust/src/accounts/service.rs`
- Modify: `rust/src/storage/usage_repository.rs`
- Test: inline service/repository tests

**Interfaces:**
- Consumes: `request_refresh(profile_id, trigger)`, session account/rates methods.
- Produces: complete CurrentCli/Managed refresh pipelines and persisted `ProfileUsageState`.

- [ ] **Step 1: Write failing reseal and cache-preservation tests**

```rust
#[tokio::test]
async fn managed_refresh_reseals_credentials_and_removes_runtime() {
    let before = fixture().vault.inspect(managed_id()).unwrap().unwrap();
    let disposition = fixture().service.request_refresh(managed_id(), RefreshTrigger::Manual).await.unwrap();
    fixture().wait_for_refresh_complete(managed_id()).await;
    assert_eq!(disposition, RefreshDisposition::Started);
    assert_eq!(fixture().vault.inspect(managed_id()).unwrap().unwrap().generation, before.generation + 1);
    assert!(fixture().runtime.sessions_for(managed_id()).unwrap().is_empty());
}

#[tokio::test]
async fn failed_refresh_preserves_cached_success_and_records_error() {
    fixture().usage.save_success(&old_snapshot()).unwrap();
    fixture().factory.fail_rates(AppErrorKind::OfflineOrTimeout);
    fixture().service.request_refresh(managed_id(), RefreshTrigger::Manual).await.unwrap();
    fixture().wait_for_refresh_complete(managed_id()).await;
    let state = fixture().usage.load_state(managed_id()).unwrap();
    assert_eq!(state.snapshot, Some(old_snapshot()));
    assert_eq!(state.current_error.unwrap().kind, AppErrorKind::OfflineOrTimeout);
}
```

- [ ] **Step 2: Run tests and verify incomplete refresh pipeline**

```powershell
cargo test --manifest-path rust/Cargo.toml managed_refresh_reseals_credentials_and_removes_runtime
cargo test --manifest-path rust/Cargo.toml failed_refresh_preserves_cached_success_and_records_error
```

Expected: FAIL until Managed refresh uses the Vault/runtime path.

- [ ] **Step 3: Implement both exact pipelines**

CurrentCli: `open_current_cli` → `account_read(false)` → rates → on AuthExpired at most one official `account_read(true)` → retry rates once → map/save success → shutdown; it still has no login/logout/config-write method and never touches runtime/Vault. Managed: acquire global actor permit → unseal → create restricted runtime → restore bundle/fixed config → Active → `open_managed` → account/rates → on AuthExpired at most one official `account_read(true)` → retry rates once → shutdown → collect possibly refreshed credentials → seal expected generation + 1 → cleanup → save success. A refresh error saves only current error. A seal/cleanup error keeps old Vault, stops the process, leaves a restricted recovery manifest if cleanup cannot complete, and returns `VaultFailure`.

- [ ] **Step 4: Run current/managed failure matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::service::tests::refresh -- --nocapture
cargo test --manifest-path rust/Cargo.toml storage::usage_repository::tests -- --nocapture
```

Expected: current read-only, managed generation increment, one auth refresh, offline cache, quota success, protocol failure, Vault failure, cleanup recovery, and no parallel child pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/accounts/actor.rs rust/src/accounts/service.rs rust/src/storage/usage_repository.rs
git commit -m "Route profile refresh through account service"
```

### Task 8: Add deterministic refresh merging, cooldown, freshness, and backoff

**Files:**
- Create: `rust/src/refresh/mod.rs`
- Create: `rust/src/refresh/scheduler.rs`
- Create: `rust/src/refresh/policy.rs`
- Modify: `rust/src/lib.rs`
- Test: inline tests in the new modules

**Interfaces:**
- Consumes: `AccountProfileService::request_refresh`, selected profile, settings refresh interval.
- Produces: `RefreshScheduler`, `RefreshPolicy`, `Clock`, `JitterSource`, and roadmap `RefreshTrigger`/`RefreshDisposition`/`Freshness`/`RefreshStatus`.

- [ ] **Step 1: Write failing policy tests with fake time/jitter**

```rust
#[tokio::test]
async fn duplicate_refresh_joins_and_manual_obeys_fifteen_second_cooldown() {
    let scheduler = fixture_scheduler();
    assert_eq!(scheduler.request(id(), RefreshTrigger::Manual).await.unwrap(), RefreshDisposition::Started);
    assert_eq!(scheduler.request(id(), RefreshTrigger::PanelOpened).await.unwrap(), RefreshDisposition::Joined);
    fixture().complete_success(id()).await;
    assert!(matches!(scheduler.request(id(), RefreshTrigger::Manual).await.unwrap(), RefreshDisposition::Cooldown { .. }));
}

#[test]
fn stale_threshold_is_max_of_twice_interval_and_ten_minutes() {
    let policy = RefreshPolicy::new(Duration::from_secs(60));
    assert_eq!(policy.stale_after(), Duration::from_secs(600));
    let policy = RefreshPolicy::new(Duration::from_secs(900));
    assert_eq!(policy.stale_after(), Duration::from_secs(1800));
}
```

- [ ] **Step 2: Run tests and verify scheduler absence**

```powershell
cargo test --manifest-path rust/Cargo.toml duplicate_refresh_joins_and_manual_obeys_fifteen_second_cooldown
cargo test --manifest-path rust/Cargo.toml stale_threshold_is_max_of_twice_interval_and_ten_minutes
```

Expected: FAIL because refresh modules are absent.

- [ ] **Step 3: Implement exact scheduling policy**

Allow intervals `0`, 60, 300, 900, and 1800 seconds; default 300. Startup, timer, panel-open snapshot older than 60 seconds, manual, and profile switch are triggers. One future per profile is shared by duplicate callers; only selected profile gets timer polling. Manual bypasses one backoff but never the 15-second manual cooldown. Transient failures step through 30, 120, 300, 900 seconds with ±20% jitter; use the existing SHA-256 crate over profile ID + attempt + scheduled second to derive bounded deterministic jitter. For `RateLimited`, if `AppError.retry_after` is between 1 second and 24 hours in the future, wait at least `max(policy_backoff, retry_after - now)` and apply only non-negative 0–20% deterministic jitter; absent, past, or over-24-hour values use the normal table. Success resets attempt. Auth/version/Vault/storage failures block until user action. `fresh` lasts two intervals; `stale` begins after `max(2 * interval, 600 seconds)`.

- [ ] **Step 4: Run deterministic scheduler matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml refresh::policy::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml refresh::scheduler::tests -- --nocapture
```

Expected: all triggers, merge, 60-second panel threshold, cooldown, interval-off, selected-only polling, four backoffs, ±20% bounds, server `retry_after`, manual bypass, success reset, block kinds, cancellation, and staleness pass without wall-clock sleeps.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/refresh rust/src/lib.rs
git commit -m "Schedule profile refresh safely"
```

### Task 9: Wire bootstrap, redacted account commands, events, startup recovery, and quit

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/commands/accounts.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/app.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/mod.rs`
- Replace: `apps/desktop-tauri/src-tauri/src/events.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/state.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/main.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/window.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/tray_bridge.rs`
- Test: inline Tauri command/state tests

**Interfaces:**
- Consumes: account service, usage repository, refresh scheduler.
- Produces: `get_bootstrap_state`, `select_profile`, `refresh_selected_profile`, `start_managed_login`, `cancel_managed_login`, `rename_managed_profile`, `remove_managed_profile`, and account/usage events.

- [ ] **Step 1: Write failing bridge secrecy/startup-order tests**

```rust
#[test]
fn bootstrap_bridge_contains_profiles_cache_and_no_secret_fields() {
    let json = serde_json::to_value(bridge_fixture()).unwrap();
    let text = json.to_string().to_ascii_lowercase();
    assert!(json.get("profiles").is_some());
    assert!(json.get("selectedProfileId").is_some());
    for forbidden in ["token", "authjson", "codexhome", "vaultpath"] { assert!(!text.contains(forbidden)); }
}

#[test]
fn startup_recovers_before_tray_and_refresh() {
    let trace = run_startup_with_recording_dependencies();
    assert_eq!(trace, ["logging", "database", "recovery", "cache", "tray", "background-refresh"]);
}
```

- [ ] **Step 2: Run tests and verify incomplete bootstrap**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml bootstrap_bridge_contains_profiles_cache_and_no_secret_fields
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml startup_recovers_before_tray_and_refresh
```

Expected: FAIL because bootstrap/account commands are not wired.

- [ ] **Step 3: Implement the redacted command/event boundary and graceful quit**

`get_bootstrap_state` returns settings, profiles, selected profile, all cached states, Codex compatibility status, and app version. CurrentCli DTO has `removable = false`; email is only included when in-memory metadata is available. Events are fixed names: `accounts-updated`, `account-login-updated`, `selected-profile-changed`, `profile-usage-state-changed`, and `refresh-state-changed`. Select returns the target cache immediately, then requests `ProfileSwitched` refresh. Commands accept UUID/label/login-mode only, not path/env/command args. Login URLs are validated/opened inside Rust; device code is a display string.

Unify every quit entry through `request_graceful_quit`: stop scheduler, cancel login, stop App Server, attempt Managed seal, wait at most 3 seconds, then close Job Object/exit. A timeout leaves a protected manifest for startup recovery.

- [ ] **Step 4: Run command, startup, and shutdown tests**

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::accounts::tests -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml startup -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml shutdown -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
```

Expected: DTO secrecy, CurrentCli immutability, event payloads, cache-first selection, recovery order, single quit coordinator, normal seal, timeout, and forced child cleanup pass.

- [ ] **Step 5: Run the Windows-native credential invariant check**

```powershell
$auth = Join-Path $env:USERPROFILE '.codex\auth.json'
$before = if (Test-Path $auth) { (Get-FileHash -Algorithm SHA256 $auth).Hash } else { $null }
# Exercise add, refresh, switch, and remove only with a disposable Managed account.
$after = if (Test-Path $auth) { (Get-FileHash -Algorithm SHA256 $auth).Hash } else { $null }
if ($before -ne $after) { throw 'Current CLI auth file changed during Managed operations' }
```

Set `$ExecutionDate = Get-Date -Format 'yyyy-MM-dd'`. Also inspect Vault/runtime DACLs, force-kill during each lifecycle checkpoint, relaunch, and record redacted outcomes under `docs/verification/windows/${ExecutionDate}/account-vault.md`.

- [ ] **Step 6: Commit**

```powershell
git add apps/desktop-tauri/src-tauri/src/commands/accounts.rs apps/desktop-tauri/src-tauri/src/commands/app.rs apps/desktop-tauri/src-tauri/src/commands/mod.rs apps/desktop-tauri/src-tauri/src/events.rs apps/desktop-tauri/src-tauri/src/state.rs apps/desktop-tauri/src-tauri/src/main.rs apps/desktop-tauri/src-tauri/src/commands/window.rs apps/desktop-tauri/src-tauri/src/tray_bridge.rs docs/verification/windows
git commit -m "Expose account lifecycle commands"
```

## Phase 2 Exit Gate

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
git diff --check
```

Real Windows evidence must show same-user DPAPI success, different-user decrypt failure, exact Current User + SYSTEM DACL, reparse rejection, two-profile isolation, no idle plaintext runtime, force-kill recovery, and unchanged CurrentCli auth state. Phase 3 adds the visible account UI and its CUA proof.
