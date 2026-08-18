# codex-barbar V1 Phase 1 App Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the private Codex HTTP/auth-file implementation with a safe, supervised, capability-checked `codex app-server` stdio client that reads CurrentCli identity and quota without mutating the user's CLI account.

**Architecture:** Place every experimental App Server concern under `rust/src/providers/codex/app_server/`: executable resolution, JSONL codec, tolerant wire models, supervised child process, request correlation, typed sessions, and compatibility probes. Public product code sees only `AppError`, account identity, `ProfileUsageSnapshot`, and the capability-limited `CurrentCliSession`/`ManagedSession` wrappers. A PowerShell fake server drives deterministic process contracts on Windows; production never launches a shell.

**Tech Stack:** Rust stable edition 2024, Tokio process/io/sync/time, serde/serde_json, thiserror, async-trait, existing `windows` crate, PowerShell 5.1+ fake server, official Codex App Server schema generation.

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

| Path | Responsibility after Phase 1 |
|---|---|
| `rust/src/core/app_error.rs` | Stable product error kinds, recovery actions, and redacted diagnostics |
| `rust/src/core/profile_usage.rs` | Profile-scoped quota windows and snapshots |
| `rust/src/providers/codex/app_server/discovery.rs` | Safe native/npm Codex command resolution and version probe |
| `rust/src/providers/codex/app_server/codec.rs` | 1 MiB bounded JSONL framing |
| `rust/src/providers/codex/app_server/protocol.rs` | Request/response/notification envelopes and typed params |
| `rust/src/providers/codex/app_server/model.rs` | Tolerant App Server account/rate-limit wire models and mapping |
| `rust/src/providers/codex/app_server/job.rs` | Windows Job Object process-tree ownership |
| `rust/src/providers/codex/app_server/process.rs` | Fixed-argument, minimal-environment child creation and stream drains |
| `rust/src/providers/codex/app_server/client.rs` | Initialize, correlation, timeouts, notification dispatch, shutdown |
| `rust/src/providers/codex/app_server/session.rs` | Read-only CurrentCli and login-capable Managed type boundaries |
| `rust/tests/fixtures/fake_codex_app_server.ps1` | Deterministic process protocol fixture; never shipped |
| `rust/src/providers/codex/mod.rs` | Codex Provider facade backed only by App Server |
| `scripts/codex-app-server-smoke.ps1` | Real local, redacted, read-only compatibility smoke |
| `docs/compatibility/codex-app-server.md` | Tested Codex versions and Windows install forms |

## Test Support Contract

- `ResolverFixture` and `NpmFixture` live in `discovery.rs` behind `#[cfg(test)]`; each owns a `tempfile::TempDir`, writes only synthetic executables/layouts, and exposes the exact paths used by its assertions.
- `fixture_client` in `rust/tests/codex_app_server_contract.rs` launches `fake_codex_app_server.ps1` with one fixed `FakeServerMode`; it never reads the user's Codex installation or credentials.
- `command_fixture`, fake Job children, and `fixture_current_cli_session` live in their owning module's `#[cfg(test)]` block and use only fixed test environment maps.
- `FakeAppServerFactory` lives in the Codex provider test module and records method calls without starting a process. `probe_with_email_and_path` lives in the smoke example's test module and returns synthetic identity/path data.
- No helper named in this phase is production-exported. Every helper has a deterministic constructor in the file named above before its first test is compiled.

### Task 1: Add stable product errors and profile usage models

**Files:**
- Create: `rust/src/core/app_error.rs`
- Create: `rust/src/core/profile_usage.rs`
- Modify: `rust/src/core/mod.rs`
- Test: inline tests in both new files

**Interfaces:**
- Consumes: `uuid::Uuid`, `chrono::DateTime<Utc>`, serde.
- Produces: roadmap contracts `AppErrorKind`, `AppError`, `RecoveryAction`, `UsageWindow`, `ProfileUsageSnapshot`, and `UsageSource`.

- [ ] **Step 1: Write failing error and quota-invariant tests**

```rust
#[test]
fn error_serialization_contains_only_stable_redacted_fields() {
    let error = AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.protocolMismatch",
        RecoveryAction::InstallTestedCodex,
        "APP_SERVER_REQUIRED_FIELD_MISSING",
    );
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(value["kind"], "protocolMismatch");
    assert!(value.get("rawLine").is_none());
    assert!(value.get("source").is_none());
}

#[test]
fn usage_window_clamps_and_derives_remaining() {
    let (window, anomaly) = UsageWindow::normalized("codex", None, 127.5, Some(300), None, None);
    assert_eq!(window.used_percent, 100.0);
    assert_eq!(window.remaining_percent, 0.0);
    assert!(anomaly);
}
```

- [ ] **Step 2: Run the focused tests and verify missing-type failure**

```powershell
cargo test --manifest-path rust/Cargo.toml error_serialization_contains_only_stable_redacted_fields
cargo test --manifest-path rust/Cargo.toml usage_window_clamps_and_derives_remaining
```

Expected: FAIL because the new modules and constructors do not exist.

- [ ] **Step 3: Implement the exact public enums and finite-value constructor**

```rust
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

impl UsageWindow {
    pub fn normalized(
        limit_id: impl Into<String>,
        label: Option<String>,
        raw_used: f64,
        duration: Option<u64>,
        resets_at: Option<DateTime<Utc>>,
        reached_type: Option<String>,
    ) -> (Self, bool) {
        let anomaly = !raw_used.is_finite() || !(0.0..=100.0).contains(&raw_used);
        let used = if raw_used.is_finite() { raw_used.clamp(0.0, 100.0) } else { 0.0 };
        (Self {
            limit_id: limit_id.into(), label, used_percent: used,
            remaining_percent: 100.0 - used,
            window_duration_minutes: duration, resets_at, reached_type,
        }, anomaly)
    }
}
```

Implement all field names exactly as frozen in the roadmap. `AppError` gets constructor helpers but no field capable of carrying raw protocol text.

- [ ] **Step 4: Run focused tests and shared model regressions**

```powershell
cargo test --manifest-path rust/Cargo.toml error_serialization_contains_only_stable_redacted_fields
cargo test --manifest-path rust/Cargo.toml usage_window_clamps_and_derives_remaining
cargo test --manifest-path rust/Cargo.toml core::rate_window
```

Expected: all commands exit `0`.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/core/app_error.rs rust/src/core/profile_usage.rs rust/src/core/mod.rs
git commit -m "Add V1 error and usage models"
```

### Task 2: Resolve native Codex executables without PATH/CWD injection

**Files:**
- Create: `rust/src/providers/codex/app_server/mod.rs`
- Create: `rust/src/providers/codex/app_server/discovery.rs`
- Modify: `rust/src/providers/codex/mod.rs`
- Modify: `rust/Cargo.toml`
- Test: `rust/src/providers/codex/app_server/discovery.rs` inline tests

**Interfaces:**
- Consumes: optional user override, explicit PATH/PATHEXT snapshot, known native install roots.
- Produces: `CodexCommandResolver::resolve(&ResolveRequest) -> Result<ResolvedCodexCommand, AppError>` and read-only accessors on `ResolvedCodexCommand`.

- [ ] **Step 1: Write failing precedence and injection tests**

```rust
#[test]
fn absolute_override_precedes_path_and_known_locations() {
    let fixture = ResolverFixture::with_native_exes();
    let result = fixture.resolve(Some(fixture.override_exe()), fixture.path_value()).unwrap();
    assert_eq!(result.program(), fixture.override_exe().canonicalize().unwrap());
    assert_eq!(result.installation(), CodexInstallation::NativeExe);
}

#[test]
fn empty_path_segment_never_selects_current_directory() {
    let fixture = ResolverFixture::with_cwd_exe();
    let error = fixture.resolve(None, OsString::from(";C:\\missing")).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::CodexNotFound);
}

#[test]
fn relative_override_is_rejected() {
    let error = fixture().resolve(Some(PathBuf::from("codex.exe")), OsString::new()).unwrap_err();
    assert_eq!(error.diagnostic_code, "CODEX_OVERRIDE_NOT_ABSOLUTE");
}
```

- [ ] **Step 2: Run the tests and verify missing resolver failure**

```powershell
cargo test --manifest-path rust/Cargo.toml absolute_override_precedes_path_and_known_locations
cargo test --manifest-path rust/Cargo.toml empty_path_segment_never_selects_current_directory
cargo test --manifest-path rust/Cargo.toml relative_override_is_rejected
```

Expected: FAIL because the App Server discovery module is absent.

- [ ] **Step 3: Implement fail-closed native resolution**

```rust
pub struct ResolvedCodexCommand {
    program: PathBuf,
    args_prefix: Vec<OsString>,
    version: Option<String>,
    installation: CodexInstallation,
}

impl ResolvedCodexCommand {
    pub fn program(&self) -> &Path { &self.program }
    pub fn args_prefix(&self) -> &[OsString] { &self.args_prefix }
    pub fn version(&self) -> Option<&str> { self.version.as_deref() }
    pub fn installation(&self) -> CodexInstallation { self.installation }
}
```

Resolution order is override, non-empty PATH/PATHEXT entries in declared order, then this ordered native-candidate list: `%LOCALAPPDATA%\Programs\OpenAI Codex\codex.exe`, `%LOCALAPPDATA%\Programs\Codex\codex.exe`, and `%LOCALAPPDATA%\Microsoft\WindowsApps\codex.exe`. Canonicalize each candidate, require a regular file, reject a reparse point on Windows, never prepend the current directory, and never invoke a shell. The WindowsApps alias is selected only if it satisfies the same checks and a direct fixed-argument `--version` probe succeeds; do not enumerate or traverse `%ProgramFiles%\WindowsApps`. The only accepted native suffix is `.exe`; `%APPDATA%\npm\codex.cmd` and other `.cmd` hints are delegated to Task 3.

- [ ] **Step 4: Run resolver tests and Clippy**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::discovery::tests -- --nocapture
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

Expected: resolver tests pass, including canonicalization, regular-file, reparse-point, PATHEXT ordering, access-denied, and missing-file cases.

- [ ] **Step 5: Commit**

```powershell
git add rust/Cargo.toml rust/src/providers/codex/mod.rs rust/src/providers/codex/app_server
git commit -m "Resolve native Codex commands safely"
```

### Task 3: Verify npm wrappers and record Windows installation compatibility

**Files:**
- Modify: `rust/src/providers/codex/app_server/discovery.rs`
- Create: `rust/src/providers/codex/app_server/fixtures/npm-codex.cmd`
- Create: `rust/src/providers/codex/app_server/fixtures/npm-layout/node_modules/@openai/codex/bin/codex.js`
- Create: `docs/research/2026-08-03-codex-windows-launch-compatibility.md`
- Test: `rust/src/providers/codex/app_server/discovery.rs` inline tests

**Interfaces:**
- Consumes: a `.cmd` location hint; canonical Node executable and `@openai/codex` entry files.
- Produces: `CodexInstallation::VerifiedNpmLayout` with `program = node.exe` and one absolute JS-entry prefix argument; arbitrary batch files remain unexecutable.

- [ ] **Step 1: Add failing official-layout and malicious-wrapper tests**

```rust
#[test]
fn cmd_hint_resolves_to_node_and_official_package_entry() {
    let layout = NpmFixture::official();
    let resolved = resolver().resolve_override(layout.cmd()).unwrap();
    assert_eq!(resolved.installation(), CodexInstallation::VerifiedNpmLayout);
    assert_eq!(resolved.program(), layout.node_exe().canonicalize().unwrap());
    assert_eq!(resolved.args_prefix(), &[layout.entry().canonicalize().unwrap().into_os_string()]);
}

#[test]
fn arbitrary_batch_content_is_never_executed() {
    let layout = NpmFixture::malicious("powershell -EncodedCommand AAAA");
    let error = resolver().resolve_override(layout.cmd()).unwrap_err();
    assert_eq!(error.diagnostic_code, "CODEX_WRAPPER_UNSUPPORTED");
    assert_eq!(layout.execution_count(), 0);
}
```

- [ ] **Step 2: Run tests and verify `.cmd` rejection**

```powershell
cargo test --manifest-path rust/Cargo.toml cmd_hint_resolves_to_node_and_official_package_entry
cargo test --manifest-path rust/Cargo.toml arbitrary_batch_content_is_never_executed
```

Expected: official layout test fails before support is added; malicious wrapper is never launched.

- [ ] **Step 3: Implement verified-layout resolution without interpreting the batch file**

For a `.cmd` hint, require all of the following:

```text
shim is an absolute regular non-reparse file
shim size is <= 64 KiB
shim text matches the checked-in official npm fixture after CRLF normalization
node.exe is adjacent or safely resolved through the Task 2 resolver
node_modules\@openai\codex\bin\codex.js is an absolute regular non-reparse file
the package entry remains below the shim directory after canonicalization
```

Ignore the wrapper at launch time. Directly spawn the resolved `node.exe`, pass the absolute JS entry as the first argument, then append `--version` or `app-server` as fixed arguments. Any failed condition maps to `UnsupportedCodexVersion` / `CODEX_WRAPPER_UNSUPPORTED`.

- [ ] **Step 4: Run the real-machine compatibility probe and write the evidence**

```powershell
Get-Command codex -All | Select-Object CommandType,Name,Source,Path
where.exe codex
Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WindowsApps" -Filter 'codex*' -ErrorAction SilentlyContinue | Select-Object FullName,Length,Attributes
```

For every discovered form, test direct `--version` through the resolver, not by shell name. Record Windows version, Codex location class (`native`, `store`, `npm`), direct-launch result, sanitized error category, and selected resolution. The record must explicitly preserve the observed `Access is denied` result for an inaccessible Windows Store resource if it recurs; it must not claim support merely because `Get-Command` found a path.

- [ ] **Step 5: Run all discovery tests and commit**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::discovery::tests -- --nocapture
git add rust/src/providers/codex/app_server/discovery.rs rust/src/providers/codex/app_server/fixtures docs/research/2026-08-03-codex-windows-launch-compatibility.md
git commit -m "Verify Codex Windows launch layouts"
```

### Task 4: Implement bounded JSONL framing and protocol envelopes

**Files:**
- Create: `rust/src/providers/codex/app_server/codec.rs`
- Create: `rust/src/providers/codex/app_server/protocol.rs`
- Modify: `rust/src/providers/codex/app_server/mod.rs`
- Test: inline tests in both new files

**Interfaces:**
- Consumes: async stdout bytes and serializable request params.
- Produces: `read_jsonl_message`, `encode_request`, `IncomingMessage`, `RpcId`, and `MAX_JSONL_BYTES = 1_048_576`.

- [ ] **Step 1: Write failing line-limit and tolerant-envelope tests**

```rust
#[tokio::test]
async fn rejects_a_line_larger_than_one_mib_before_json_parse() {
    let input = vec![b'a'; MAX_JSONL_BYTES + 1];
    let error = read_jsonl_message(BufReader::new(input.as_slice())).await.unwrap_err();
    assert_eq!(error.diagnostic_code, "APP_SERVER_LINE_TOO_LARGE");
}

#[test]
fn response_ignores_unknown_fields_but_requires_id() {
    let incoming = parse_incoming(br#"{"id":7,"result":{},"futureField":true}"#).unwrap();
    assert!(matches!(incoming, IncomingMessage::Response { id: RpcId(7), .. }));
    assert_eq!(
        parse_incoming(br#"{"result":{}}"#).unwrap_err().diagnostic_code,
        "APP_SERVER_REQUIRED_FIELD_MISSING"
    );
}
```

- [ ] **Step 2: Run focused tests and verify missing codec failure**

```powershell
cargo test --manifest-path rust/Cargo.toml rejects_a_line_larger_than_one_mib_before_json_parse
cargo test --manifest-path rust/Cargo.toml response_ignores_unknown_fields_but_requires_id
```

Expected: FAIL because the codec and envelopes do not exist.

- [ ] **Step 3: Implement the bounded reader and tagged incoming classification**

```rust
pub const MAX_JSONL_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RpcId(pub u64);

pub enum IncomingMessage {
    Response { id: RpcId, result: Value },
    Error { id: RpcId, error: RpcErrorBody },
    Notification { method: String, params: Value },
    ServerRequest { id: RpcId, method: String, params: Value },
}
```

Use `AsyncBufRead::fill_buf`/`consume` so the buffer is capped before allocation exceeds `MAX_JSONL_BYTES + 1`. Accept CRLF and LF, reject EOF with a non-empty unterminated line as `APP_SERVER_TRUNCATED_LINE`, reject invalid UTF-8/JSON, ignore unknown fields, and never attach the raw line to an error or trace event.

- [ ] **Step 4: Run codec tests including boundary values**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::codec::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::protocol::tests -- --nocapture
```

Expected: empty, valid, CRLF, exactly-1-MiB, oversized, invalid JSON, invalid UTF-8, truncated, response, error, notification, and server-request cases pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/providers/codex/app_server/codec.rs rust/src/providers/codex/app_server/protocol.rs rust/src/providers/codex/app_server/mod.rs
git commit -m "Add bounded App Server protocol framing"
```

### Task 5: Parse account and rate-limit responses tolerantly

**Files:**
- Create: `rust/src/providers/codex/app_server/model.rs`
- Create: `rust/src/providers/codex/app_server/fixtures/account-chatgpt.json`
- Create: `rust/src/providers/codex/app_server/fixtures/account-api-key.json`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-by-id.json`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-legacy.json`
- Create: `rust/src/providers/codex/app_server/fixtures/rate-limits-anomaly.json`
- Modify: `rust/src/providers/codex/app_server/mod.rs`
- Test: `rust/src/providers/codex/app_server/model.rs` inline tests

**Interfaces:**
- Consumes: `account/read` and `account/rateLimits/read` result values.
- Produces: `AccountIdentity`, `AuthMode`, `ParsedRateLimits`, and `parse_profile_usage(profile_id, account, rates, fetched_at)`.

- [ ] **Step 1: Write failing bucket-selection and anomaly tests**

```rust
#[test]
fn selects_named_codex_bucket_without_object_order_dependency() {
    let value = fixture("rate-limits-by-id.json");
    let parsed = ParsedRateLimits::from_value(value).unwrap();
    assert_eq!(parsed.selected_limit_id.as_deref(), Some("codex"));
    assert_eq!(parsed.primary.as_ref().unwrap().window_duration_minutes, Some(300));
    assert_eq!(parsed.secondary.as_ref().unwrap().window_duration_minutes, Some(10_080));
}

#[test]
fn clamps_abnormal_percent_and_marks_protocol_anomaly() {
    let parsed = ParsedRateLimits::from_value(fixture("rate-limits-anomaly.json")).unwrap();
    assert_eq!(parsed.primary.as_ref().unwrap().used_percent, 100.0);
    assert!(parsed.protocol_anomaly);
}

#[test]
fn api_key_identity_maps_to_no_quota() {
    let account = AccountIdentity::from_value(fixture("account-api-key.json")).unwrap();
    let error = parse_profile_usage(id(), account, ParsedRateLimits::empty(), Utc::now()).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::ApiKeyNoQuota);
}
```

- [ ] **Step 2: Run tests and verify missing mapper failure**

```powershell
cargo test --manifest-path rust/Cargo.toml selects_named_codex_bucket_without_object_order_dependency
cargo test --manifest-path rust/Cargo.toml clamps_abnormal_percent_and_marks_protocol_anomaly
cargo test --manifest-path rust/Cargo.toml api_key_identity_maps_to_no_quota
```

Expected: FAIL because the wire models and fixtures are absent.

- [ ] **Step 3: Implement exact mapping rules**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountIdentity {
    pub auth_mode: AuthMode,
    pub email: Option<String>,
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRateLimits {
    pub selected_limit_id: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub additional_windows: Vec<UsageWindow>,
    pub protocol_anomaly: bool,
}

pub fn parse_profile_usage(
    profile_id: ProfileId,
    account: AccountIdentity,
    rates: ParsedRateLimits,
    fetched_at: DateTime<Utc>,
) -> Result<ProfileUsageSnapshot, AppError>;

let selected = wire.rate_limits_by_limit_id
    .as_ref()
    .and_then(|limits| limits.get("codex"))
    .or(wire.rate_limits.as_ref())
    .ok_or_else(AppError::not_signed_in_or_no_limits)?;
```

Accept documented camelCase fields plus unknown fields. Parse percent only from a JSON number or a finite numeric string; invalid values omit that window and set `protocol_anomaly`. Parse `resetsAt` as Unix seconds. Dynamically label 300 minutes as localization key `usage.window.fiveHours`, 10,080 as `usage.window.weekly`, and other durations as `usage.window.durationMinutes`; do not hard-code primary/secondary meanings by object order. Preserve additional buckets/windows in `additional_windows`.

- [ ] **Step 4: Run the complete fixture matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model::tests -- --nocapture
```

Expected: ChatGPT, API key, missing account, named bucket, compatibility field, extra fields, missing window, numeric string, out-of-range, invalid numeric, expired reset, and additional-window tests pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/providers/codex/app_server/model.rs rust/src/providers/codex/app_server/fixtures rust/src/providers/codex/app_server/mod.rs
git commit -m "Parse Codex App Server account and quota data"
```

### Task 6: Own the child process tree with a Windows Job Object

**Files:**
- Create: `rust/src/providers/codex/app_server/job.rs`
- Create: `rust/src/providers/codex/app_server/process.rs`
- Modify: `rust/src/providers/codex/app_server/mod.rs`
- Modify: `rust/Cargo.toml`
- Test: inline tests in `job.rs` and `process.rs`

**Interfaces:**
- Consumes: validated `ResolvedCodexCommand`, fixed `AppServerProfileEnv`, fixed `AppServerLaunchMode::Stdio`.
- Produces: `SupervisedAppServerProcess::spawn`, piped stdin/stdout/stderr, and `shutdown(Duration::from_secs(3))`.

- [ ] **Step 1: Write failing fixed-command/environment tests**

```rust
#[test]
fn app_server_arguments_are_fixed_os_strings() {
    let spec = AppServerSpawnSpec::current_cli(command_fixture());
    let mut expected = command_fixture().args_prefix().to_vec();
    expected.push(OsString::from("app-server"));
    assert_eq!(spec.arguments(), expected.as_slice());
}

#[test]
fn managed_environment_clears_auth_overrides_only_in_child() {
    let before = std::env::var_os("OPENAI_API_KEY");
    let env = ChildEnvironment::managed(Path::new(r"C:\safe\profile")).unwrap();
    assert_eq!(env.get("CODEX_HOME"), Some(OsStr::new(r"C:\safe\profile")));
    for key in AUTH_OVERRIDE_ENV_NAMES { assert!(env.is_removed(key)); }
    assert_eq!(std::env::var_os("OPENAI_API_KEY"), before);
}
```

- [ ] **Step 2: Run focused tests and verify missing supervisor failure**

```powershell
cargo test --manifest-path rust/Cargo.toml app_server_arguments_are_fixed_os_strings
cargo test --manifest-path rust/Cargo.toml managed_environment_clears_auth_overrides_only_in_child
```

Expected: FAIL because the spawn specification does not exist.

- [ ] **Step 3: Enable only the required existing Windows crate features and implement supervision**

Add these feature strings to the existing `windows = "0.58"` dependency:

```toml
"Win32_System_JobObjects",
"Win32_System_Threading",
"Win32_Storage_FileSystem",
```

Create the child with piped stdin/stdout/stderr and `CREATE_NO_WINDOW`. Create a Job Object, set `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, assign the child process immediately, and retain the Job handle until process shutdown. Drain stdout through the codec task and stderr through a line-capped redactor task concurrently. The common inherited environment allowlist is exactly `SystemRoot`, `WINDIR`, `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`, `LOCALAPPDATA`, `APPDATA`, `TEMP`, `TMP`, `PATH`, `PATHEXT`, `SSL_CERT_FILE`, and `SSL_CERT_DIR`; keys are matched case-insensitively and absent values stay absent. `CurrentCli` additionally preserves existing `OPENAI_API_KEY`, `CODEX_API_KEY`, `CODEX_ACCESS_TOKEN`, `OPENAI_ACCESS_TOKEN`, `OPENAI_ORGANIZATION`, and `OPENAI_PROJECT`, plus `CODEX_HOME` only when it canonicalizes to an absolute ordinary non-reparse directory; absent `CODEX_HOME` stays absent so Codex uses its normal default. It never inherits `OPENAI_BASE_URL`. Managed launch sets its one validated isolated `CODEX_HOME` and explicitly removes all seven authentication/base-URL override names above. CurrentCli uses canonical `AppPaths.root` as working directory; Managed uses its guarded runtime root. No UI-provided key, value, argument, working directory, or environment name enters the specification, and tests use synthetic values only.

- [ ] **Step 4: Test process-tree cleanup on Windows**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::job::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::process::tests -- --nocapture
```

Expected: normal exit, 3-second graceful timeout, forced tree termination, stdout drain, stderr drain, and child-only environment tests pass; no console window is observed in the Windows test.

- [ ] **Step 5: Commit**

```powershell
git add rust/Cargo.toml Cargo.lock rust/src/providers/codex/app_server/job.rs rust/src/providers/codex/app_server/process.rs rust/src/providers/codex/app_server/mod.rs
git commit -m "Supervise the Codex App Server process tree"
```

### Task 7: Correlate RPC requests, notifications, timeouts, and shutdown

**Files:**
- Create: `rust/src/providers/codex/app_server/client.rs`
- Create: `rust/tests/fixtures/fake_codex_app_server.ps1`
- Create: `rust/tests/codex_app_server_contract.rs`
- Modify: `rust/src/providers/codex/app_server/mod.rs`

**Interfaces:**
- Consumes: `SupervisedAppServerProcess` and protocol codec.
- Produces: `CodexAppServerClient::connect`, `request`, `subscribe_notifications`, metrics, and bounded `shutdown`.

- [ ] **Step 1: Add failing normal/interleaved contract tests**

```rust
#[tokio::test]
async fn initialize_precedes_initialized_notification_and_requests_correlate() {
    let client = fixture_client("normal").await;
    assert_eq!(client.metrics().initialized_notifications, 1);
    let account = client.request("account/read", json!({ "refreshToken": false })).await.unwrap();
    assert_eq!(account["account"]["type"], "chatgpt");
}

#[tokio::test]
async fn interleaved_unknown_notification_does_not_steal_response() {
    let client = fixture_client("interleaved").await;
    let value = client.request("account/rateLimits/read", json!({})).await.unwrap();
    assert!(value.get("rateLimitsByLimitId").is_some());
    assert_eq!(client.metrics().unknown_notifications, 1);
}
```

- [ ] **Step 2: Run the integration test and verify missing client failure**

```powershell
cargo test --manifest-path rust/Cargo.toml --test codex_app_server_contract -- --nocapture
```

Expected: FAIL because `CodexAppServerClient` and the fake server do not exist.

- [ ] **Step 3: Implement the client state machine**

```rust
pub const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
pub const RPC_TIMEOUT: Duration = Duration::from_secs(20);
pub const REFRESH_BUDGET: Duration = Duration::from_secs(30);
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
```

Use an `AtomicU64` starting at 1 and a `Mutex<HashMap<RpcId, oneshot::Sender<_>>>`. Start reader and stderr tasks before sending initialize. Send initialize with `experimentalApi: false`, await its matching response within 10 seconds, then write an `initialized` notification with no request ID. Every post-initialize RPC has an independent 20-second timeout; the Phase-1 provider/service wraps the complete account-plus-rate-limit operation in a 30-second budget. Unknown notifications increment a counter only. Unknown/duplicate response IDs become a protocol metric and do not satisfy another request. On EOF/crash, fail every pending sender with one redacted error. Closing first drops stdin, waits 3 seconds, then closes the Job handle to kill the process tree.

- [ ] **Step 4: Run the full fake-server matrix**

```powershell
cargo test --manifest-path rust/Cargo.toml --test codex_app_server_contract -- --nocapture
```

Expected: fixtures `normal`, `interleaved`, `out-of-order`, `unknown-notification`, `duplicate-id`, `invalid-json`, `truncated`, `oversized`, `initialize-timeout`, `rpc-timeout`, `crash`, and `refuse-exit` all produce the asserted result/error without hanging beyond their bounded test timeout.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/providers/codex/app_server/client.rs rust/src/providers/codex/app_server/mod.rs rust/tests/fixtures/fake_codex_app_server.ps1 rust/tests/codex_app_server_contract.rs
git commit -m "Add the Codex App Server stdio client"
```

### Task 8: Expose capability-limited CurrentCli and Managed sessions

**Files:**
- Create: `rust/src/providers/codex/app_server/session.rs`
- Modify: `rust/src/providers/codex/app_server/mod.rs`
- Modify: `rust/tests/codex_app_server_contract.rs`
- Test: inline tests in `session.rs`

**Interfaces:**
- Consumes: `CodexAppServerClient`, `AccountIdentity`, `ParsedRateLimits`.
- Produces: `AppServerFactory`, `LocalAppServerFactory`, `CurrentCliSession`, `ManagedSession`, `LoginFlow`, `LoginChallenge`, and `LoginEvent`.

- [ ] **Step 1: Add failing capability and compile-boundary tests**

```rust
#[tokio::test]
async fn current_cli_uses_only_read_methods() {
    let (session, calls) = fixture_current_cli_session().await;
    session.account_read(false).await.unwrap();
    session.rate_limits_read().await.unwrap();
    session.shutdown().await.unwrap();
    assert_eq!(calls.methods(), ["initialize", "initialized", "account/read", "account/rateLimits/read"]);
}

#[test]
fn initialize_params_keep_experimental_api_disabled() {
    let params = InitializeParams::v1();
    assert_eq!(serde_json::to_value(params).unwrap()["experimentalApi"], false);
}
```

Add a `trybuild`-free source-shape assertion that reads `session.rs` and confirms the `impl CurrentCliSession` block contains none of `start_login`, `cancel_login`, `logout`, `delete`, or `write_config`; do not add a new test dependency.

- [ ] **Step 2: Run the focused tests and verify missing session types**

```powershell
cargo test --manifest-path rust/Cargo.toml current_cli_uses_only_read_methods
cargo test --manifest-path rust/Cargo.toml initialize_params_keep_experimental_api_disabled
```

Expected: FAIL because typed sessions are absent.

- [ ] **Step 3: Implement the exact type capability split**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFlow { Browser, DeviceCode }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginChallenge {
    pub login_id: String,
    pub authorization_url: Option<String>,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LoginEvent {
    Completed { login_id: String },
    Failed { login_id: String, error: AppError },
    Cancelled { login_id: String },
}

#[async_trait]
pub trait AppServerFactory: Send + Sync {
    async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError>;
    async fn open_managed(&self, codex_home: &Path) -> Result<ManagedSession, AppError>;
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

Validate every `authorization_url`/`verification_url` inside Rust: scheme must be `https`, host must be exactly `auth.openai.com`, username/password/fragment must be absent, and the device-code path must be exactly `/codex/device`. A changed host/path fails as `UnsupportedCodexVersion` with `CODEX_LOGIN_URL_UNSUPPORTED` and must be added only through a reviewed tested-version update, never by accepting arbitrary OpenAI subdomains. Map JSON-RPC method-not-found for a required read method to `UnsupportedCodexVersion`; auth absence to `NotSignedIn`; API key identity without quota to `ApiKeyNoQuota`; timeout/EOF to `OfflineOrTimeout`; and incompatible shapes to `ProtocolMismatch`. A Managed browser login uses `chatgpt`; its fallback uses `chatgptDeviceCode`. Do not implement `chatgptAuthTokens` or `account/logout`.

- [ ] **Step 4: Run capability, login, cancel, and isolation contracts**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::session::tests -- --nocapture
cargo test --manifest-path rust/Cargo.toml --test codex_app_server_contract -- --nocapture
```

Expected: CurrentCli call allowlist, missing-method mapping, browser login, device-code login, cancel, failed login, and two different Managed `CODEX_HOME`/child environment assertions pass.

- [ ] **Step 5: Commit**

```powershell
git add rust/src/providers/codex/app_server/session.rs rust/src/providers/codex/app_server/mod.rs rust/tests/codex_app_server_contract.rs
git commit -m "Add capability-safe Codex App Server sessions"
```

### Task 9: Cut the Codex provider over and delete the private endpoint path

**Files:**
- Delete: `rust/src/providers/codex/api.rs`
- Replace: `rust/src/providers/codex/mod.rs`
- Modify: `rust/src/core/provider_factory.rs`
- Modify: `rust/src/core/provider.rs`
- Modify: `rust/src/settings.rs`
- Modify: `rust/src/settings/raw.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/app.rs`
- Modify: `scripts/assert-v1-boundaries.ps1`
- Test: `rust/src/providers/codex/mod.rs` inline tests

**Interfaces:**
- Consumes: `Arc<dyn AppServerFactory>`, `CurrentCliSession`, and profile usage mapper.
- Produces: `CodexProvider::with_app_server(factory)` and the Provider-trait CurrentCli fetch backed only by App Server.

- [ ] **Step 1: Add a failing no-private-path test**

```rust
#[test]
fn codex_provider_fetches_current_cli_through_app_server() {
    let factory = Arc::new(FakeAppServerFactory::chatgpt_with_quota(25.0));
    let provider = CodexProvider::with_app_server(factory.clone());
    let result = tokio_test::block_on(provider.fetch_usage(&FetchContext::default())).unwrap();
    assert_eq!(result.usage.primary.remaining_percent(), 75.0);
    assert_eq!(factory.current_cli_sessions_opened(), 1);
    assert_eq!(factory.http_requests(), 0);
}
```

Extend `scripts/assert-v1-boundaries.ps1` to scan all files below `rust/src/providers/codex` for these case-insensitive patterns:

```text
/wham/
Authorization: Bearer
reqwest::Client
read_to_string.*auth.json
```

- [ ] **Step 2: Run the test and boundary scan before cutover**

```powershell
cargo test --manifest-path rust/Cargo.toml codex_provider_fetches_current_cli_through_app_server
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
```

Expected: provider test fails; boundary scan fails on the current private endpoint implementation.

- [ ] **Step 3: Replace the provider implementation without a fallback**

The default constructor receives `LocalAppServerFactory`; tests inject a fake. Fetch opens `CurrentCliSession`, calls `account_read(false)` and `rate_limits_read()` inside the 30-second refresh budget, maps to `ProfileUsageSnapshot`, then adapts to the retained `ProviderFetchResult` during the migration. Delete private response/request structs, direct `auth.json` reads, bearer construction, `/wham/usage`, and reset-credit calls. Remove Codex cookie/API-key/source settings that could reactivate those paths.

- [ ] **Step 4: Run focused, boundary, and shared regressions**

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex -- --nocapture
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
cargo test --manifest-path rust/Cargo.toml
```

Expected: all commands exit `0`; no private Codex network implementation remains in the source tree.

- [ ] **Step 5: Commit**

```powershell
git add -A rust/src/providers/codex rust/src/core/provider_factory.rs rust/src/core/provider.rs rust/src/settings.rs rust/src/settings/raw.rs apps/desktop-tauri/src-tauri/src/commands/app.rs scripts/assert-v1-boundaries.ps1
git commit -m "Use App Server for Codex usage"
```

### Task 10: Generate schema references and maintain the tested-version matrix

**Files:**
- Create: `rust/examples/codex_app_server_smoke.rs`
- Create: `scripts/codex-app-server-smoke.ps1`
- Create: `rust/src/providers/codex/app_server/schema/README.md`
- Create: `docs/compatibility/codex-app-server.md`
- Modify: `.github/workflows/pr-check.yml`

**Interfaces:**
- Consumes: real resolved Codex command and generated App Server schema output.
- Produces: a redacted read-only smoke summary and a per-version compatibility record; no credentials enter Git or CI.

- [ ] **Step 1: Add the smoke output redaction test**

```rust
#[test]
fn smoke_summary_omits_identity_and_paths() {
    let summary = SmokeSummary::from_probe(probe_with_email_and_path());
    let json = serde_json::to_string(&summary).unwrap();
    assert!(!json.contains('@'));
    assert!(!json.contains("Users\\"));
    assert!(!json.to_ascii_lowercase().contains("token"));
    assert_eq!(summary.experimental_api, false);
}
```

- [ ] **Step 2: Run the focused test and verify missing smoke helper**

```powershell
cargo test --manifest-path rust/Cargo.toml smoke_summary_omits_identity_and_paths
```

Expected: FAIL because the example/helper is absent.

- [ ] **Step 3: Implement the read-only smoke and schema capture**

The PowerShell script creates `$SchemaTemp = Join-Path ([IO.Path]::GetTempPath()) ("codex-barbar-schema-" + [guid]::NewGuid().ToString('N'))`, resolves Codex through the Rust example, and invokes the resolved program directly with its trusted prefix followed by `app-server generate-json-schema --out $SchemaTemp`. It derives `$CodexVersionSlug` from the exact probed version by replacing every character outside `[A-Za-z0-9._-]` with `_`, copies the generated schemas under `rust/src/providers/codex/app_server/schema/${CodexVersionSlug}/`, records the unsanitized probed version plus SHA-256 hashes in `manifest.json`, and removes `$SchemaTemp` in `finally`. Then it opens only `CurrentCliSession`, performs initialize/account/rates, and emits the following complete output shape (the literal version is a synthetic schema example):

```json
{
  "codexVersion": "codex-cli 0.0.0-test",
  "installation": "nativeExe",
  "initialized": true,
  "accountState": "signedIn",
  "rateLimitsMethod": "available",
  "experimentalApi": false,
  "errorKind": null
}
```

The actual implementation substitutes the probed version and enum values; the schema above defines the complete output keys. It never outputs email, account ID, quota values, token, full path, raw RPC, or environment variables. CI runs the fake-server contract suite; a real signed-in smoke is local-only.

- [ ] **Step 4: Run a local real-Codex probe and record its truthful result**

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/codex-app-server-smoke.ps1
```

Expected: exit `0` with a redacted summary for a compatible installation, or a bounded nonzero result with `CodexNotFound`/`UnsupportedCodexVersion` for an inaccessible/unsupported installation. Add the exact version, installation form, account store mode, initialize/read/rates result, date, and issue note to `docs/compatibility/codex-app-server.md`. Do not mark an untested combination compatible.

- [ ] **Step 5: Run Phase-1 verification and commit**

```powershell
cargo fmt --all --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo test --manifest-path rust/Cargo.toml --test codex_app_server_contract -- --nocapture
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/assert-v1-boundaries.ps1
git diff --check
git add rust/examples/codex_app_server_smoke.rs scripts/codex-app-server-smoke.ps1 rust/src/providers/codex/app_server/schema docs/compatibility/codex-app-server.md .github/workflows/pr-check.yml
git commit -m "Document Codex App Server compatibility"
```

Expected: automated gates exit `0`; local real-Codex status is recorded without credentials.

## Phase 1 Exit Gate

- Native and verified npm layouts resolve to canonical direct-launch programs; `.cmd` is never executed and current-directory PATH injection is covered.
- Windows Store/access-denied behavior is recorded honestly, not treated as a runnable command.
- JSONL is limited to 1 MiB and handles notifications, out-of-order responses, malformed input, timeout, crash, and refused shutdown without hanging.
- `initialize` always uses `experimentalApi: false` and sends `initialized` only after a matched successful response.
- `CurrentCliSession` exposes only account read, rate-limit read, and shutdown; it has no login/logout/delete/config-write path.
- Account/quota mapping prefers `rateLimitsByLimitId["codex"]`, falls back to `rateLimits`, clamps finite display data, flags anomalies, and distinguishes API-key no-quota.
- `rust/src/providers/codex/api.rs`, private `/wham/*`, direct auth-file reads, bearer construction, and any private fallback are absent.
- Fake-server contracts pass in CI; real-version compatibility claims are limited to entries with recorded local evidence.
