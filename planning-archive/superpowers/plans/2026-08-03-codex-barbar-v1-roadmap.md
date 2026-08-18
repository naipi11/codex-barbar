# codex-barbar V1 Roadmap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a secure, Codex-only Windows 11 tray application named `codex-barbar`, from the approved design through reproducible V1 release artifacts.

**Architecture:** Keep the Win-CodexBar Tauri 2 shell and focused shared Rust primitives, but replace the Codex private HTTP path with one typed `codex app-server` stdio adapter. Rust owns processes, profiles, credentials, SQLite, refresh state, diagnostics, and fixed external actions; React receives only redacted DTOs. Managed accounts use isolated `CODEX_HOME` directories while active and strict DPAPI Current User vaults while idle.

**Tech Stack:** Windows 11 x64, Tauri 2, React 18, TypeScript 5.6, Rust stable edition 2024, Tokio, serde/serde_json, rusqlite, Windows API through the existing `windows` crate, Vitest 3, PowerShell 5.1+, GitHub Actions.

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

## 1. Approved Inputs and Execution Boundary

- Approved design: `docs/superpowers/specs/2026-08-03-codex-barbar-v1-design.md`.
- Imported Windows baseline: commit `b167e328147b93f997034a6b50c8b769d2a37f3b`.
- Product repository: `https://github.com/naipi11/codex-barbar.git`.
- Behavior reference: `https://github.com/steipete/CodexBar.git`.
- Current implementation branch at planning time: `codex/design-spec`.
- The approved App Server risk is bounded by an adapter, capability probes, tolerant parsing, a tested-version matrix, and explicit incompatibility UX.
- The implementation must stop for design review if a real Codex build cannot satisfy strict account isolation, CurrentCli read-only behavior, or strict idle-at-rest encryption.

## 2. Plan Suite and Gate Order

Execute these documents in order. A later plan consumes only interfaces committed by the preceding plans.

| Gate | Plan | Working result | Exit evidence |
|---|---|---|---|
| M0 | `2026-08-03-codex-barbar-v1-phase-0-foundation.md` | Branded, Codex-only, buildable desktop baseline with forbidden legacy release paths removed | Both Rust manifests, Vitest/build, identity/source audit, forbidden-surface scan |
| M1 | `2026-08-03-codex-barbar-v1-phase-1-app-server.md` | Safe executable resolution, supervised JSONL client, typed sessions, account/quota parsing, no private endpoint fallback | Unit tests, PowerShell fake-server contract tests, real unsigned-in smoke, version-matrix record |
| M2 | `2026-08-03-codex-barbar-v1-phase-2-account-vault.md` | CurrentCli plus DPAPI Managed profiles, SQLite snapshots, refresh coordination, recovery, removal | DPAPI/DACL/SQLite tests, injected crash matrix, two-profile isolation contract test |
| M3 | `2026-08-03-codex-barbar-v1-phase-3-tray-ui.md` | Complete tray panel, native menu, account/settings flows, localization, accessibility, manual update UX | Vitest/bridge tests and fresh-build CUA screenshots/observations |
| M4–M6 | `2026-08-03-codex-barbar-v1-phase-4-windows-release.md` | Hardened app, diagnostics, NSIS and portable ZIP, hosted gates, documentation, RC evidence | Local checks, clean-machine matrix, SBOM/hash checks, release checklist |

After every task:

1. Review only that task's diff.
2. Run its focused failing/passing test cycle.
3. Run the task's stated regression slice.
4. Commit with the exact scoped message from the phase plan.
5. Do not batch unrelated task commits.

At each phase gate run:

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
git diff --check
```

Expected: every command exits `0`; `git diff --check` prints nothing.

## 3. Canonical Cross-Phase Rust Contracts

These names and field types are frozen for the plan suite. Implementers may add private fields, but changing a public name or semantic requires updating all five phase plans before dependent work proceeds.

```rust
pub type ProfileId = uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    CurrentCli,
    Managed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    Unknown,
    ChatGpt,
    ApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: ProfileId,
    pub kind: ProfileKind,
    pub label: String,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub auth_mode: AuthMode,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_selected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_success_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

`AccountProfile.email` is an in-memory value. It is never a column in SQLite and reaches React only in a redacted profile DTO after the relevant profile metadata has been unsealed.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub limit_id: String,
    pub label: Option<String>,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_duration_minutes: Option<u64>,
    pub resets_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reached_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageSnapshot {
    pub profile_id: ProfileId,
    pub plan_type: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub additional_windows: Vec<UsageWindow>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub source: UsageSource,
    pub protocol_anomaly: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageSource {
    AppServer,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryAction {
    SelectCodexExecutable,
    InstallTestedCodex,
    SignIn,
    ReloginManagedProfile,
    Retry,
    WaitAndRetry,
    ExplainApiBilling,
    ExportDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppErrorKind {
    CodexNotFound,
    UnsupportedCodexVersion,
    NotSignedIn,
    ApiKeyNoQuota,
    AuthExpired,
    OfflineOrTimeout,
    RateLimited,
    ProtocolMismatch,
    VaultFailure,
    StorageFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub user_message_key: String,
    pub action: RecoveryAction,
    pub retry_after: Option<chrono::DateTime<chrono::Utc>>,
    pub diagnostic_code: String,
}
```

The error contains no raw RPC body, token, environment dump, or full user path. `diagnostic_code` is a stable low-cardinality code such as `APP_SERVER_LINE_TOO_LARGE`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageState {
    pub profile_id: ProfileId,
    pub snapshot: Option<ProfileUsageSnapshot>,
    pub current_error: Option<AppError>,
    pub refresh_status: RefreshStatus,
    pub freshness: Freshness,
    pub manual_cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefreshStatus { Idle, Refreshing, Cooldown, Backoff, Blocked }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Freshness { Fresh, Stale, Missing }
```

`snapshot` is the last successful snapshot; `current_error` describes only the latest failed refresh. A failure never fabricates a successful `100% remaining` snapshot.

## 4. Canonical App Server Boundary

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCodexCommand {
    program: std::path::PathBuf,
    args_prefix: Vec<std::ffi::OsString>,
    version: Option<String>,
    installation: CodexInstallation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexInstallation {
    NativeExe,
    VerifiedNpmLayout,
}

impl ResolvedCodexCommand {
    pub fn program(&self) -> &std::path::Path;
    pub fn args_prefix(&self) -> &[std::ffi::OsString];
    pub fn version(&self) -> Option<&str>;
    pub fn installation(&self) -> CodexInstallation;
}
```

- Native: `program` is the canonical absolute `codex.exe`, `args_prefix` is empty.
- Verified npm layout: `program` is canonical absolute `node.exe`, and `args_prefix` contains only the canonical absolute `@openai/codex` JavaScript entry point.
- The final fixed arguments are appended as separate OS strings: `app-server` for runtime and `--version` for version probing.
- A `.cmd` path is never executed. It is accepted only as a location hint for a verified npm layout; failure to prove the layout returns `UnsupportedCodexVersion` with diagnostic code `CODEX_WRAPPER_UNSUPPORTED`.
- The resolver never asks a shell to reinterpret user text.

```rust
pub struct CodexAppServerClient { /* private transport */ }
pub struct CurrentCliSession { /* private client */ }
pub struct ManagedSession { /* private client */ }

#[async_trait::async_trait]
pub trait AppServerFactory: Send + Sync {
    async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError>;
    async fn open_managed(&self, codex_home: &std::path::Path)
        -> Result<ManagedSession, AppError>;
}

impl CurrentCliSession {
    pub async fn account_read(&self, refresh_token: bool) -> Result<AccountIdentity, AppError>;
    pub async fn rate_limits_read(&self) -> Result<ParsedRateLimits, AppError>;
    pub async fn shutdown(self) -> Result<(), AppError>;
}

impl ManagedSession {
    pub async fn account_read(&self, refresh_token: bool) -> Result<AccountIdentity, AppError>;
    pub async fn rate_limits_read(&self) -> Result<ParsedRateLimits, AppError>;
    pub async fn start_login(&self, flow: LoginFlow) -> Result<LoginChallenge, AppError>;
    pub async fn next_login_event(&mut self) -> Result<LoginEvent, AppError>;
    pub async fn cancel_login(&self, login_id: &str) -> Result<(), AppError>;
    pub async fn shutdown(self) -> Result<(), AppError>;
}
```

No conversion from `CurrentCliSession` to `ManagedSession` exists. `account/logout` is absent from both public types.

## 5. Canonical Storage and Vault Boundary

```text
%LOCALAPPDATA%\codex-barbar\
  data\codex-barbar.db
  vault\UUID.dpapi
  runtime\UUID\
  logs\codex-barbar.log*
```

Each `UUID` filename is the canonical lowercase hyphenated UUID of the owning Profile or random runtime session, respectively; callers never provide either path string directly.

```rust
pub trait CredentialProtector: Send + Sync {
    fn protect_current_user(&self, profile_id: ProfileId, plaintext: &[u8])
        -> Result<Vec<u8>, VaultError>;
    fn unprotect_current_user(&self, profile_id: ProfileId, ciphertext: &[u8])
        -> Result<SensitiveBytes, VaultError>;
}

pub struct CredentialVault { /* private paths/protector */ }

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

`SensitiveBytes::drop` overwrites its allocation with zero bytes before release. It is never `Debug`, `Display`, `Serialize`, or part of a Tauri DTO.

```rust
pub struct AccountRepository { /* private AppDatabase */ }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsageCacheKey {
    pub profile_id: ProfileId,
    pub provider_id: ProviderId,
}

impl UsageCacheKey {
    pub const fn codex(profile_id: ProfileId) -> Self;
}

impl AccountRepository {
    fn ensure_current_cli(&self, now: DateTime<Utc>) -> Result<AccountProfile, StorageError>;
    fn list_ready(&self) -> Result<Vec<AccountProfile>, StorageError>;
    fn selected_profile_id(&self) -> Result<ProfileId, StorageError>;
    fn set_selected(&self, profile_id: ProfileId, now: DateTime<Utc>) -> Result<(), StorageError>;
}

pub trait UsageRepository: Send + Sync {
    fn load_state(&self, profile_id: ProfileId) -> Result<ProfileUsageState, StorageError>;
    fn load_all_states(&self) -> Result<Vec<ProfileUsageState>, StorageError>;
    fn save_success(&self, snapshot: &ProfileUsageSnapshot) -> Result<(), StorageError>;
    fn save_error(&self, profile_id: ProfileId, error: &AppError) -> Result<(), StorageError>;
    fn delete_profile(&self, profile_id: ProfileId) -> Result<(), StorageError>;
}
```

SQLite uses WAL and transactions. Migrations create a bounded backup and never auto-rebuild over a failed database.
The SQLite and in-memory cache primary key is `(profile_id, provider_id)` via `UsageCacheKey`; V1 repository convenience methods derive `ProviderId::Codex`, while frontend DTOs omit the redundant provider dimension because no other provider can ship.

## 6. Canonical Refresh Contract

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTrigger {
    Startup,
    Timer,
    PanelOpened,
    Manual,
    ProfileSwitched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshDisposition {
    Started,
    Joined,
    Cooldown { retry_at: DateTime<Utc> },
    Backoff { retry_at: DateTime<Utc> },
    Blocked { error: AppErrorKind },
}

pub trait ProfileRefresher: Send + Sync {
    async fn refresh(&self, profile_id: ProfileId, trigger: RefreshTrigger)
        -> Result<ProfileUsageState, AppError>;
}
```

There is at most one in-flight refresh per profile and at most one active App Server process across the application. Unselected profiles are not polled in V1.

## 7. Canonical Tauri Command Allowlist

The final `invoke_handler` contains only these command names:

```text
get_bootstrap_state
get_settings_snapshot
get_locale_strings
select_profile
refresh_selected_profile
start_managed_login
cancel_managed_login
rename_managed_profile
remove_managed_profile
update_settings
validate_codex_executable
get_diagnostics_summary
export_diagnostics
check_for_updates
open_release_page
open_codex_usage_page
open_settings_window
close_settings_window
dismiss_tray_panel
set_flyout_size
get_current_surface_state
quit_app
```

The frontend bridge exports the same camelCase functions and no generic `openExternalUrl`, `openPath`, HTTP, shell, cookie, API-key, token-account, chart, cost, session, workspace, provider-login, provider-revoke, update-download, or update-apply function.

## 8. Frontend DTO Contract

```ts
export type ProfileKind = "currentCli" | "managed";
export type AuthMode = "unknown" | "chatGpt" | "apiKey";
export type Freshness = "fresh" | "stale" | "missing";
export type RefreshStatus = "idle" | "refreshing" | "cooldown" | "backoff" | "blocked";
export type AppErrorKind =
  | "codexNotFound" | "unsupportedCodexVersion" | "notSignedIn"
  | "apiKeyNoQuota" | "authExpired" | "offlineOrTimeout" | "rateLimited"
  | "protocolMismatch" | "vaultFailure" | "storageFailure";
export type RecoveryAction =
  | "selectCodexExecutable" | "installTestedCodex" | "signIn"
  | "reloginManagedProfile" | "retry" | "waitAndRetry"
  | "explainApiBilling" | "exportDiagnostics";

export interface AppErrorDto {
  kind: AppErrorKind;
  userMessageKey: string;
  action: RecoveryAction;
  retryAfter: string | null;
}

export interface AppSettingsDto {
  autostartEnabled: boolean;
  refreshIntervalSeconds: 0 | 60 | 300 | 900 | 1800;
  displayMode: "remaining" | "used";
  theme: "system" | "light" | "dark";
  language: "system" | "zh-CN" | "en-US";
  codexExecutableOverride: string | null;
}

export interface ProfileSummaryDto {
  id: string;
  kind: ProfileKind;
  label: string;
  email: string | null;
  planType: string | null;
  authMode: AuthMode;
  removable: boolean;
  lastSuccessAt: string | null;
}

export interface UsageWindowDto {
  limitId: string;
  label: string | null;
  usedPercent: number;
  remainingPercent: number;
  windowDurationMinutes: number | null;
  resetsAt: string | null;
  reachedType: string | null;
}

export interface ProfileUsageStateDto {
  profileId: string;
  primary: UsageWindowDto | null;
  secondary: UsageWindowDto | null;
  additionalWindows: UsageWindowDto[];
  fetchedAt: string | null;
  currentError: AppErrorDto | null;
  freshness: Freshness;
  refreshStatus: RefreshStatus;
  manualCooldownUntil: string | null;
  protocolAnomaly: boolean;
}

export interface CodexCompatibilityDto {
  status: "notChecked" | "compatible" | "notFound" | "unsupported";
  installation: "nativeExe" | "verifiedNpmLayout" | null;
  executablePath: string | null;
  version: string | null;
  capabilities: { accountRead: boolean; rateLimitsRead: boolean; managedLogin: boolean };
}

export interface ManagedLoginStateDto {
  operationId: string;
  profileId: string;
  stage: "starting" | "awaitingUser" | "succeeded" | "failed" | "cancelled";
  verificationUrl: string | null;
  userCode: string | null;
  errorKind: AppErrorKind | null;
}

export interface BootstrapDto {
  productName: "codex-barbar";
  version: string;
  settings: AppSettingsDto;
  profiles: ProfileSummaryDto[];
  selectedProfileId: string;
  usageByProfile: Record<string, ProfileUsageStateDto>;
  codex: CodexCompatibilityDto;
}
```

Rust owns `remainingPercent` calculation and finite-number validation. TypeScript formats values, durations, reset time, and localized copy; it does not repair protocol data.

## 9. Verification and Evidence Layout

Record non-secret evidence under these paths:

```text
docs/verification/app-server/${CodexVersionSlug}.md
docs/verification/windows/${ExecutionDate}/cua-observations.md
docs/verification/windows/${ExecutionDate}/screenshots/*.png
docs/verification/release/${ReleaseVersion}/artifact-manifest.json
docs/verification/release/${ReleaseVersion}/clean-machine-matrix.md
```

`CodexVersionSlug` is the exact `codex --version` output normalized to ASCII letters, digits, dots, and hyphens; `ExecutionDate` is the local execution date in `YYYY-MM-DD`; `ReleaseVersion` is read from the four synchronized product manifests.

Evidence must contain tested versions, commands, pass/fail observations, and SHA-256 values. It must not contain email addresses, tokens, full `CODEX_HOME` contents, raw RPC lines, or a user home path. Replace the home prefix with `%USERPROFILE%` and the app root with `%LOCALAPPDATA%\codex-barbar`.

## 10. Spec Coverage Matrix

| Approved design area | Owning phase/tasks | Proof |
|---|---|---|
| Windows 11 x64 product identity and upstream history | Phase 0 Tasks 1–3 | Source audit, manifest/config assertions |
| Codex-only runtime and removed legacy surfaces | Phase 0 Tasks 4–7; Phase 4 Task 3 | Runtime registry test, command/capability scan, frontend build |
| Safe executable discovery and Store/npm/native compatibility | Phase 1 Tasks 2–3 | Resolver unit tests and real-machine compatibility record |
| JSONL limits, request correlation, timeouts, notification handling | Phase 1 Tasks 4, 6–7 | Fake App Server contract suite |
| Capability probing and read-only CurrentCli boundary | Phase 1 Tasks 7–8 | Compile-time API shape and contract assertions |
| Account and quota parsing with anomaly handling | Phase 1 Tasks 5, 9–10 | Fixture matrix tests |
| Removal of private Codex endpoints | Phase 1 Task 9 | Forbidden-string test and source scan |
| Strict DPAPI, DACL, isolated runtime, atomic replacement | Phase 2 Tasks 2–5 | Windows unit tests and injected failures |
| CurrentCli/Managed lifecycle, login, switch, removal | Phase 2 Tasks 6–7, 9 | Service and two-profile contract tests |
| SQLite migration, last-success snapshot, current error separation | Phase 2 Task 1 | Migration/rollback/cache tests |
| Refresh merge, cooldown, staleness, jitter/backoff | Phase 2 Task 8 | Deterministic clock/random tests |
| Dynamic tray icon, tooltip, left flyout, right native menu | Phase 3 Tasks 2–4, 8 | Unit tests plus CUA observations |
| Settings, language, theme, accessibility and scaling | Phase 3 Tasks 5, 7–8 | Vitest, semantic assertions, CUA matrix |
| Manual-only update behavior | Phase 3 Task 6; Phase 4 Task 3 | Network trigger tests and command allowlist |
| Least privilege, logging, diagnostics, clean shutdown | Phase 0 Task 7; Phase 4 Tasks 1–3 | Security scans, child cleanup, diagnostic secret scan |
| NSIS, portable ZIP, hashes, SBOM, CI, docs and RC | Phase 4 Tasks 4–9 | Artifact doctor, hosted gates, clean-machine matrix |
| Release definition and external publish gate | Phase 4 Task 10 | Signed-off release checklist; publication only after authorization |

## 11. Stop Conditions

Pause implementation and return to design review when any of these is observed:

- A supported Codex build requires enabling `experimentalApi` or calling a private quota endpoint.
- App Server cannot read CurrentCli in both documented file and Windows credential-store modes without codex-barbar copying or switching CLI credentials.
- Managed `CODEX_HOME` cannot be forced to file credential storage without affecting the user's main Codex configuration.
- A runtime directory cannot be protected against other ordinary local users or cannot be rejected when it is a reparse point.
- DPAPI Current User protection cannot be completed without machine scope or plaintext fallback.
- The packaged frontend requires arbitrary shell, file, or network permission.
- A new dependency is required and the user has not approved it.
- Real Windows evidence contradicts the approved UI, security, or compatibility guarantees.

## 12. Completion Review

Before calling V1 implementation complete, the executing agent must run `superpowers:verification-before-completion`, inspect the approved Spec section by section, and attach evidence for every row in the coverage matrix. Passing unit tests alone is insufficient for tray, DWM, WebView2, DPI, taskbar-edge, installer, uninstaller, startup, DPAPI, DACL, Job Object, or crash-recovery behavior.

The final code-complete handoff must state separately:

- code and automated-test status;
- real Windows CUA/clean-machine status;
- Codex version matrix status;
- unsigned or Authenticode-signed artifact status;
- whether GitHub publication was authorized and performed.
