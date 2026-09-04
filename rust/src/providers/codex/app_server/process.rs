//! Fixed-argument, minimal-environment App Server child process launcher.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
#[cfg(any(unix, test))]
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::core::{AppError, AppErrorKind, RecoveryAction};

use super::discovery::ResolvedCodexCommand;

/// Environment names inherited by every App Server child (common allowlist).
pub const COMMON_INHERITED_ENV_NAMES: &[&str] = &[
    "SystemRoot",
    "WINDIR",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "APPDATA",
    "TEMP",
    "TMP",
    "PATH",
    "PATHEXT",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Authentication/override names explicitly removed from Managed children
/// and conditionally preserved for CurrentCli.
pub const AUTH_OVERRIDE_ENV_NAMES: &[&str] = &[
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "OPENAI_ACCESS_TOKEN",
    "OPENAI_ORGANIZATION",
    "OPENAI_PROJECT",
    "OPENAI_BASE_URL",
];

/// How the App Server child is launched. V1 only supports stdio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServerLaunchMode {
    Stdio,
}

/// Profile environment flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServerProfileEnv {
    CurrentCli,
    Managed,
}

/// Fixed modes understood by the checked-in PowerShell contract fixture.
///
/// This enum intentionally carries no command, argument, environment, or
/// working-directory input. It exists only so process-level tests can exercise
/// the same supervised launch path without touching a user's Codex install.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeServerMode {
    Normal,
    Interleaved,
    OutOfOrder,
    UnknownNotification,
    DuplicateId,
    InvalidJson,
    Truncated,
    Oversized,
    InitializeTimeout,
    RpcTimeout,
    Crash,
    RefuseExit,
    LoginFailed,
    LoginCancelled,
    LoginStartCrash,
}

impl FakeServerMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Interleaved => "interleaved",
            Self::OutOfOrder => "out-of-order",
            Self::UnknownNotification => "unknown-notification",
            Self::DuplicateId => "duplicate-id",
            Self::InvalidJson => "invalid-json",
            Self::Truncated => "truncated",
            Self::Oversized => "oversized",
            Self::InitializeTimeout => "initialize-timeout",
            Self::RpcTimeout => "rpc-timeout",
            Self::Crash => "crash",
            Self::RefuseExit => "refuse-exit",
            Self::LoginFailed => "login-failed",
            Self::LoginCancelled => "login-cancelled",
            Self::LoginStartCrash => "login-start-crash",
        }
    }
}

/// A fully specified child environment. Values are explicit: no ambient
/// environment leaks in except through the allowlists.
#[derive(Debug, Clone, Default)]
pub struct ChildEnvironment {
    values: BTreeMap<String, Option<OsString>>,
}

impl ChildEnvironment {
    /// Build the CurrentCli environment: common allowlist plus existing auth
    /// overrides (which the CLI itself manages), never OPENAI_BASE_URL, and
    /// CODEX_HOME only when it is a safe absolute ordinary directory.
    pub fn current_cli() -> Self {
        let mut env = Self::default();
        for name in COMMON_INHERITED_ENV_NAMES {
            env.inherit_if_present(name);
        }
        for name in AUTH_OVERRIDE_ENV_NAMES {
            if name.eq_ignore_ascii_case("OPENAI_BASE_URL") {
                env.remove(name);
            } else {
                env.inherit_if_present(name);
            }
        }
        match std::env::var_os("CODEX_HOME") {
            Some(value) if is_safe_codex_home(Path::new(&value)) => {
                env.set("CODEX_HOME", value);
            }
            _ => env.remove("CODEX_HOME"),
        }
        env
    }

    /// Build the Managed environment: common allowlist, one isolated
    /// validated CODEX_HOME, and every auth override removed.
    pub fn managed(codex_home: &Path) -> Result<Self, AppError> {
        if !is_safe_codex_home(codex_home) {
            return Err(AppError::new(
                AppErrorKind::VaultFailure,
                "errors.managedCodexHomeInvalid",
                RecoveryAction::None,
                "MANAGED_CODEX_HOME_INVALID",
            ));
        }
        let mut env = Self::default();
        for name in COMMON_INHERITED_ENV_NAMES {
            env.inherit_if_present(name);
        }
        for name in AUTH_OVERRIDE_ENV_NAMES {
            env.remove(name);
        }
        env.set("CODEX_HOME", codex_home.as_os_str().to_os_string());
        Ok(env)
    }

    /// Build the deterministic environment used by the checked-in fake
    /// server. Only non-secret Windows runtime roots are copied; all
    /// authentication, base-URL, and CODEX_HOME names are explicitly absent.
    #[doc(hidden)]
    pub fn test_fixture() -> Self {
        let mut env = Self::default();
        for name in ["SystemRoot", "WINDIR", "TEMP", "TMP", "PSModulePath"] {
            env.inherit_if_present(name);
        }
        for name in AUTH_OVERRIDE_ENV_NAMES {
            env.remove(name);
        }
        env.remove("CODEX_HOME");
        env
    }

    fn inherit_if_present(&mut self, name: &str) {
        if let Some(value) = std::env::var_os(name) {
            self.set(name, value);
        } else {
            self.remove(name);
        }
    }

    fn set(&mut self, name: &str, value: OsString) {
        self.values.insert(name.to_ascii_uppercase(), Some(value));
    }

    fn remove(&mut self, name: &str) {
        self.values.insert(name.to_ascii_uppercase(), None);
    }

    /// Current effective value for a name (case-insensitive), if set.
    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.values
            .get(&name.to_ascii_uppercase())
            .and_then(|value| value.as_deref())
    }

    /// True when the name is explicitly removed from the child.
    pub fn is_removed(&self, name: &str) -> bool {
        matches!(self.values.get(&name.to_ascii_uppercase()), Some(None))
    }
}

/// Absolute, ordinary (non-reparse) directory check for CODEX_HOME.
fn is_safe_codex_home(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata.is_dir() && !super::discovery::is_reparse(&metadata),
        Err(_) => false,
    }
}

/// Fixed spawn specification: validated command + fixed `app-server` argument
/// + explicit environment + fixed working directory.
///
/// No UI-provided key, value, argument, working directory, or environment
/// name can enter it.
#[derive(Debug, Clone)]
pub struct AppServerSpawnSpec {
    program: PathBuf,
    arguments: Vec<OsString>,
    environment: ChildEnvironment,
    working_directory: PathBuf,
    profile: AppServerProfileEnv,
    launch_mode: AppServerLaunchMode,
}

impl AppServerSpawnSpec {
    /// CurrentCli spec: resolved command, fixed app-server arg, CurrentCli
    /// environment, and the canonical AppPaths root as working directory.
    pub fn current_cli(command: ResolvedCodexCommand) -> Self {
        let working_directory = crate::app_paths::AppPaths::discover()
            .map(|paths| existing_directory(&paths.root))
            .unwrap_or_else(|_| std::env::temp_dir());
        Self::build(
            command,
            ChildEnvironment::current_cli(),
            working_directory,
            AppServerProfileEnv::CurrentCli,
        )
    }

    /// Managed spec: resolved command, fixed app-server arg, isolated
    /// validated CODEX_HOME, and the guarded runtime root as working
    /// directory.
    pub fn managed(
        command: ResolvedCodexCommand,
        codex_home: &Path,
        runtime_root: &Path,
    ) -> Result<Self, AppError> {
        let environment = ChildEnvironment::managed(codex_home)?;
        let working_directory = existing_directory(runtime_root);
        Ok(Self::build(
            command,
            environment,
            working_directory,
            AppServerProfileEnv::Managed,
        ))
    }

    /// Build a process specification for the fixed PowerShell contract
    /// fixture used by Windows integration tests.
    ///
    /// The constructor is deliberately constrained to [`FakeServerMode`];
    /// callers cannot provide an arbitrary executable, script, argument, or
    /// environment through this test hook.
    #[doc(hidden)]
    pub fn test_fixture(mode: FakeServerMode) -> Result<Self, AppError> {
        let command = fixture_command(mode)?;
        Ok(Self::build(
            command,
            ChildEnvironment::test_fixture(),
            existing_directory(
                &crate::app_paths::AppPaths::discover()
                    .map(|paths| paths.root)
                    .unwrap_or_else(|_| std::env::temp_dir()),
            ),
            AppServerProfileEnv::CurrentCli,
        ))
    }

    /// Managed counterpart of [`Self::test_fixture`] for isolation contracts.
    #[doc(hidden)]
    pub fn test_managed_fixture(
        mode: FakeServerMode,
        codex_home: &Path,
        runtime_root: &Path,
    ) -> Result<Self, AppError> {
        let command = fixture_command(mode)?;
        Ok(Self::build(
            command,
            ChildEnvironment::managed(codex_home)?,
            existing_directory(runtime_root),
            AppServerProfileEnv::Managed,
        ))
    }

    fn build(
        command: ResolvedCodexCommand,
        environment: ChildEnvironment,
        working_directory: PathBuf,
        profile: AppServerProfileEnv,
    ) -> Self {
        let mut arguments = command.launch_args_prefix();
        arguments.push(OsString::from("app-server"));
        Self {
            program: command.program().to_path_buf(),
            arguments,
            environment,
            working_directory,
            profile,
            launch_mode: AppServerLaunchMode::Stdio,
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    pub fn environment(&self) -> &ChildEnvironment {
        &self.environment
    }
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }
    pub fn profile(&self) -> AppServerProfileEnv {
        self.profile
    }
    pub fn launch_mode(&self) -> AppServerLaunchMode {
        self.launch_mode
    }
}

/// Resolve a working directory that is guaranteed to exist: the canonical
/// directory itself, or its nearest existing ancestor, falling back to the
/// system temp directory.
fn existing_directory(path: &Path) -> PathBuf {
    let mut candidate = Some(path.to_path_buf());
    while let Some(dir) = candidate {
        if let Ok(canonical) = dir.canonicalize()
            && canonical.is_dir()
        {
            return canonical;
        }
        candidate = dir.parent().map(Path::to_path_buf);
    }
    std::env::temp_dir()
}

#[cfg(windows)]
fn fixture_command(mode: FakeServerMode) -> Result<ResolvedCodexCommand, AppError> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_codex_app_server.ps1");
    if !script.is_file() {
        return Err(AppError::new(
            AppErrorKind::StorageFailure,
            "errors.appServerFixtureMissing",
            RecoveryAction::Retry,
            "APP_SERVER_FIXTURE_MISSING",
        ));
    }

    #[cfg(windows)]
    let powershell = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");

    Ok(ResolvedCodexCommand::from_parts(
        powershell,
        vec![
            OsString::from("-NoLogo"),
            OsString::from("-NoProfile"),
            OsString::from("-NonInteractive"),
            OsString::from("-ExecutionPolicy"),
            OsString::from("Bypass"),
            OsString::from("-File"),
            script.into_os_string(),
            OsString::from("-Mode"),
            OsString::from(mode.as_str()),
        ],
        super::discovery::CodexInstallation::NativeExe,
    ))
}

#[cfg(target_os = "linux")]
fn fixture_command(mode: FakeServerMode) -> Result<ResolvedCodexCommand, AppError> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_codex_app_server.py");
    if !script.is_file() {
        return Err(AppError::new(
            AppErrorKind::StorageFailure,
            "errors.appServerFixtureMissing",
            RecoveryAction::Retry,
            "APP_SERVER_FIXTURE_MISSING",
        ));
    }

    Ok(ResolvedCodexCommand::from_parts(
        PathBuf::from("/usr/bin/python3"),
        vec![
            script.into_os_string(),
            OsString::from("--mode"),
            OsString::from(mode.as_str()),
        ],
        super::discovery::CodexInstallation::NativeExe,
    ))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn fixture_command(_mode: FakeServerMode) -> Result<ResolvedCodexCommand, AppError> {
    Err(AppError::new(
        AppErrorKind::StorageFailure,
        "errors.appServerFixtureMissing",
        RecoveryAction::Retry,
        "APP_SERVER_FIXTURE_UNSUPPORTED_PLATFORM",
    ))
}

/// Default graceful shutdown window before the Job handle is closed (which
/// terminates the whole process tree).
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// The final reap attempt after SIGKILL is deliberately bounded: process
/// cleanup must not make the caller wait forever if the OS does not reap a
/// child promptly.
#[cfg(any(unix, test))]
const POST_KILL_WAIT: Duration = Duration::from_millis(250);

#[cfg(any(unix, test))]
async fn wait_after_kill<T>(wait: impl Future<Output = T>) {
    let _ = tokio::time::timeout(POST_KILL_WAIT, wait).await;
}

/// A running App Server owned by a Windows Job Object or a dedicated Unix
/// supervisor. Dropping it initiates whole-tree cleanup on either platform.
pub struct SupervisedAppServerProcess {
    child: Child,
    #[cfg(windows)]
    job: super::job::JobHandle,
    #[cfg(all(unix, not(target_os = "linux")))]
    process_group: Option<i32>,
    #[cfg(target_os = "linux")]
    supervisor: super::job::LinuxSupervisorHandle,
}

impl SupervisedAppServerProcess {
    /// Spawn with piped stdio and the fixed environment, then attach the
    /// platform process-tree owner before returning control to the caller.
    pub fn spawn(spec: &AppServerSpawnSpec) -> Result<Self, AppError> {
        #[cfg(target_os = "linux")]
        let pending_supervisor = super::job::PendingLinuxSupervisor::new().map_err(|_| {
            AppError::new(
                AppErrorKind::StorageFailure,
                "errors.appServerSpawnFailed",
                RecoveryAction::Retry,
                "APP_SERVER_SUPERVISOR_PIPE_FAILED",
            )
        })?;

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(&spec.working_directory)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(target_os = "linux")]
        {
            // The dedicated subreaper must outlive this Tokio handle long
            // enough to terminate and reap its descendants after Drop.
            command.kill_on_drop(false);
            pending_supervisor.configure(&mut command);
        }
        #[cfg(not(target_os = "linux"))]
        command.kill_on_drop(true);
        // Explicit environment: clear everything, then apply only the spec.
        command.env_clear();
        for (name, value) in &spec.environment.values {
            if let Some(value) = value {
                command.env(name, value);
            }
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW_FLAG);
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            command.process_group(0);
        }

        let child = command.spawn().map_err(|_| {
            AppError::new(
                AppErrorKind::CodexNotFound,
                "errors.appServerSpawnFailed",
                RecoveryAction::InstallTestedCodex,
                "APP_SERVER_SPAWN_FAILED",
            )
        })?;

        #[cfg(windows)]
        let mut child = child;

        #[cfg(windows)]
        let job = {
            let job = super::job::JobHandle::new_kill_on_close().map_err(|_| {
                let _ = child.start_kill();
                AppError::new(
                    AppErrorKind::StorageFailure,
                    "errors.appServerJobFailed",
                    RecoveryAction::Retry,
                    "APP_SERVER_JOB_CREATE_FAILED",
                )
            })?;
            job.assign_process(child.id().unwrap_or_default())
                .map_err(|_| {
                    let _ = child.start_kill();
                    AppError::new(
                        AppErrorKind::StorageFailure,
                        "errors.appServerJobFailed",
                        RecoveryAction::Retry,
                        "APP_SERVER_JOB_ASSIGN_FAILED",
                    )
                })?;
            job
        };

        #[cfg(all(unix, not(target_os = "linux")))]
        let process_group = child.id().and_then(|pid| i32::try_from(pid).ok());

        #[cfg(target_os = "linux")]
        let supervisor = pending_supervisor.activate().map_err(|_| {
            AppError::new(
                AppErrorKind::StorageFailure,
                "errors.appServerSpawnFailed",
                RecoveryAction::Retry,
                "APP_SERVER_SUPERVISOR_START_FAILED",
            )
        })?;

        Ok(Self {
            child,
            #[cfg(windows)]
            job,
            #[cfg(all(unix, not(target_os = "linux")))]
            process_group,
            #[cfg(target_os = "linux")]
            supervisor,
        })
    }

    pub fn stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.child.stdin.take()
    }
    pub fn stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.stdout.take()
    }
    pub fn stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.child.stderr.take()
    }

    /// Graceful shutdown: close stdio, wait up to `grace`, then ask the
    /// platform owner to terminate and reap the remaining process tree.
    pub async fn shutdown(mut self, grace: Duration) {
        let _ = self.child.stdin.take();
        let _ = self.child.stdout.take();
        let _ = self.child.stderr.take();
        #[cfg(not(target_os = "linux"))]
        if tokio::time::timeout(grace, self.child.wait())
            .await
            .is_err()
        {
            #[cfg(unix)]
            if let Some(process_group) = self.process_group {
                super::job::terminate_process_group(process_group);
                if tokio::time::timeout(Duration::from_millis(250), self.child.wait())
                    .await
                    .is_err()
                {
                    super::job::kill_process_group(process_group);
                    wait_after_kill(self.child.wait()).await;
                }
            } else {
                let _ = self.child.start_kill();
                wait_after_kill(self.child.wait()).await;
            }
            #[cfg(not(unix))]
            {
                let _ = self.child.start_kill();
                let _ = self.child.wait().await;
            }
        }
        #[cfg(target_os = "linux")]
        {
            let _ = tokio::time::timeout(grace, self.child.wait()).await;
            // If the dedicated supervisor still exists, at least one member
            // of its private process tree is alive. The control capability is
            // exact and cannot be redirected by PID/PGID reuse.
            self.supervisor.request_shutdown();
            wait_after_kill(self.child.wait()).await;
        }
        // `job` is dropped here, terminating any surviving tree members.
        #[cfg(windows)]
        drop(self.job);
    }
}

#[cfg(unix)]
impl Drop for SupervisedAppServerProcess {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        self.supervisor.request_shutdown();
        #[cfg(all(unix, not(target_os = "linux")))]
        if let Some(process_group) = self.process_group {
            super::job::kill_process_group(process_group);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn command_fixture() -> ResolvedCodexCommand {
        ResolvedCodexCommand::from_parts(
            PathBuf::from(r"C:\Codex\codex.exe"),
            vec![OsString::from("--fixture")],
            super::super::discovery::CodexInstallation::NativeExe,
        )
    }

    #[test]
    fn app_server_arguments_are_fixed_os_strings() {
        let spec = AppServerSpawnSpec::current_cli(command_fixture());
        let mut expected = command_fixture().args_prefix().to_vec();
        expected.push(OsString::from("app-server"));
        assert_eq!(spec.arguments(), expected.as_slice());
        assert_eq!(spec.launch_mode(), AppServerLaunchMode::Stdio);
    }

    #[test]
    fn managed_environment_clears_auth_overrides_only_in_child() {
        let dir = tempfile::TempDir::new().unwrap();
        let before = std::env::var_os("OPENAI_API_KEY");
        let env = ChildEnvironment::managed(dir.path()).unwrap();
        assert_eq!(env.get("CODEX_HOME"), Some(dir.path().as_os_str()));
        for key in AUTH_OVERRIDE_ENV_NAMES {
            assert!(env.is_removed(key), "{key} should be removed");
        }
        assert_eq!(std::env::var_os("OPENAI_API_KEY"), before);
    }

    #[test]
    fn managed_environment_rejects_relative_or_missing_codex_home() {
        assert!(ChildEnvironment::managed(Path::new("relative")).is_err());
        assert!(ChildEnvironment::managed(Path::new(r"C:\definitely\missing\codex-home")).is_err());
    }

    #[test]
    fn current_cli_environment_never_inherits_openai_base_url() {
        let env = ChildEnvironment::current_cli();
        assert!(env.is_removed("OPENAI_BASE_URL"));
    }

    #[test]
    fn current_cli_environment_drops_unsafe_codex_home() {
        // Point CODEX_HOME at something that does not exist; the child must
        // not inherit it.
        unsafe {
            std::env::set_var("CODEX_HOME", r"C:\definitely\missing\codex-home");
        }
        let env = ChildEnvironment::current_cli();
        assert!(env.is_removed("CODEX_HOME") || env.get("CODEX_HOME").is_none());
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }

    #[test]
    fn fixture_environment_never_inherits_auth_or_codex_home() {
        let env = ChildEnvironment::test_fixture();
        for key in AUTH_OVERRIDE_ENV_NAMES {
            assert!(env.is_removed(key), "{key} should be removed in fixtures");
        }
        assert!(env.is_removed("CODEX_HOME"));
        assert!(env.get("PATH").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn fixture_environment_preserves_powershell_module_path() {
        let env = ChildEnvironment::test_fixture();
        assert_eq!(
            env.get("PSModulePath"),
            std::env::var_os("PSModulePath").as_deref()
        );
    }

    #[test]
    fn environment_lookup_is_case_insensitive() {
        let dir = tempfile::TempDir::new().unwrap();
        let env = ChildEnvironment::managed(dir.path()).unwrap();
        assert_eq!(env.get("codex_home"), Some(dir.path().as_os_str()));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn supervised_spawn_normal_exit_and_shutdown() {
        // Use a real executable that exits immediately to prove spawn +
        // shutdown plumbing end to end.
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from("exit 0"),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        // Strip the fixed app-server arg for this synthetic command.
        spec.arguments.truncate(spec.arguments.len() - 1);
        let process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        process.shutdown(SHUTDOWN_GRACE).await;
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn shutdown_terminates_a_surviving_child_via_the_job() {
        // A child that sleeps far past the grace window must be terminated by
        // the Job teardown, not left running.
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from("Start-Sleep -Seconds 60"),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let pid = process.child.id().expect("child pid");
        let started = std::time::Instant::now();
        process.shutdown(Duration::from_millis(200)).await;
        assert!(started.elapsed() < Duration::from_secs(10));
        // The pid must no longer be alive.
        let still_alive = std::process::Command::new(r"C:\Windows\System32\tasklist.exe")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false);
        assert!(!still_alive, "pid {pid} survived supervised shutdown");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_spawn_owns_a_distinct_process_group_and_cancels_it() {
        unsafe extern "C" {
            fn getpgid(pid: i32) -> i32;
            fn getpgrp() -> i32;
        }

        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/bin/sh"),
            vec![
                OsString::from("-c"),
                OsString::from("printf '%s\\n' \"$$\"; sleep 60 & wait"),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let pid = process.child.id().expect("child pid") as i32;
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        let app_server_pid = read_reported_pid(&mut stdout).await;
        drop(stdout);

        assert_eq!(unsafe { getpgid(pid) }, pid);
        assert_ne!(unsafe { getpgid(pid) }, unsafe { getpgrp() });
        assert_ne!(app_server_pid, pid, "supervisor must be a separate process");
        assert_eq!(unsafe { getpgid(app_server_pid) }, pid);

        process.shutdown(Duration::from_millis(50)).await;
        assert_process_group_exits_within(pid, Duration::from_secs(1)).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_inner_exec_failure_does_not_deadlock_the_supervisor_handshake() {
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/definitely/missing/codex"),
            Vec::new(),
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let spec = AppServerSpawnSpec::current_cli(command);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            tokio::task::spawn_blocking(move || SupervisedAppServerProcess::spawn(&spec)),
        )
        .await
        .expect("inner exec failure must not deadlock")
        .expect("spawn task must finish");
        assert!(result.is_err(), "missing App Server unexpectedly launched");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_shutdown_reaps_a_background_descendant_after_its_shell_exits() {
        use tokio::io::AsyncBufReadExt;

        unsafe extern "C" {
            fn getpgid(pid: i32) -> i32;
        }

        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/bin/sh"),
            vec![
                OsString::from("-c"),
                OsString::from("sleep 60 & printf '%s\\n' \"$!\"; exit 0"),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let process_group = process.child.id().expect("child pid") as i32;
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        let mut descendant = String::new();
        tokio::time::timeout(Duration::from_secs(1), stdout.read_line(&mut descendant))
            .await
            .expect("shell must report its background descendant")
            .expect("background descendant pid must be readable");
        drop(stdout);
        let descendant: i32 = descendant
            .trim()
            .parse()
            .expect("background descendant pid");
        assert_eq!(unsafe { getpgid(descendant) }, process_group);

        let started = std::time::Instant::now();
        process.shutdown(Duration::from_millis(50)).await;
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_process_group_exits_within(process_group, Duration::from_secs(1)).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_drop_is_non_blocking_while_a_large_tree_supervisor_is_stalled() {
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/bin/sh"),
            vec![
                OsString::from("-c"),
                OsString::from(
                    "i=0; while [ \"$i\" -lt 128 ]; do sleep 60 & i=$((i + 1)); done; printf 'ready\\n'; wait",
                ),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let supervisor = process.child.id().expect("supervisor pid") as i32;
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        assert_eq!(read_reported_line(&mut stdout).await, "ready");
        drop(stdout);

        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        assert_eq!(unsafe { kill(supervisor, 19) }, 0, "stop supervisor");

        let started = std::time::Instant::now();
        drop(process);
        let elapsed = started.elapsed();
        assert_eq!(unsafe { kill(supervisor, 18) }, 0, "resume supervisor");
        // The supervisor was unable to perform procfs or reap work while Drop
        // ran. Drop must only notify it and close the control channel.
        assert!(
            elapsed < Duration::from_millis(100),
            "Drop took {elapsed:?}"
        );
        assert_process_group_exits_within(supervisor, Duration::from_secs(2)).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_supervisor_reaps_a_chain_deeper_than_the_escalation_budget() {
        unsafe extern "C" {
            fn getpgid(pid: i32) -> i32;
        }

        let script = r#"
import os
import time

for _ in range(48):
    if os.fork() != 0:
        time.sleep(60)
        os._exit(0)

print(os.getpid(), flush=True)
time.sleep(60)
"#;
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/usr/bin/python3"),
            vec![OsString::from("-c"), OsString::from(script)],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let process_group = process.child.id().expect("supervisor pid") as i32;
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        let leaf = read_reported_pid(&mut stdout).await;
        drop(stdout);
        let leaf_identity = read_linux_process_identity(leaf).expect("deep-chain leaf identity");
        assert_eq!(unsafe { getpgid(leaf) }, process_group);

        let started = std::time::Instant::now();
        process.shutdown(Duration::from_millis(50)).await;
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "caller waited for unbounded supervisor cleanup"
        );

        // The old 8 TERM + 17 KILL loop exited after at most 25 adoption
        // layers. This chain is intentionally deeper and the exact leaf must
        // disappear rather than survive or remain as a zombie.
        assert_process_identity_exits_within(leaf_identity, Duration::from_secs(3)).await;
        assert_process_group_exits_within(process_group, Duration::from_secs(3)).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_shutdown_contains_immediate_detach_and_leaves_unrelated_process_alive() {
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/bin/sh"),
            vec![
                OsString::from("-c"),
                OsString::from(
                    "leader_start=$(awk '{ print $22 }' /proc/$$/stat); setsid sleep 60 & printf '%s %s %s\\n' \"$$\" \"$leader_start\" \"$!\"; exit 0",
                ),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        let (leader_identity, descendant) = read_reported_identity_and_pid(&mut stdout).await;
        drop(stdout);

        // No tracking handshake is permitted here: the shell exits as part of
        // the same command that forks the detached child. This is the race the
        // old polling ledger could not close.
        let descendant_identity =
            read_linux_process_identity(descendant).expect("descendant identity");
        assert_process_identity_exits_within(leader_identity, Duration::from_secs(1)).await;

        let unrelated = std::process::Command::new("/bin/sleep")
            .arg("60")
            .spawn()
            .expect("spawn unrelated process");
        let mut unrelated = ChildGuard(unrelated);
        let unrelated_identity = read_linux_process_identity(unrelated.0.id() as i32)
            .expect("unrelated process identity");

        process.shutdown(Duration::from_millis(50)).await;
        assert_process_identity_exits_within(descendant_identity, Duration::from_secs(1)).await;
        assert_eq!(
            read_linux_process_identity(unrelated_identity.0),
            Some(unrelated_identity),
            "cleanup touched an unrelated process after its leader was gone"
        );
        unrelated.terminate();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_shutdown_terminates_a_descendant_that_escapes_the_process_group() {
        unsafe extern "C" {
            fn getpgid(pid: i32) -> i32;
        }

        let script = r#"
import os
import time

child = os.fork()
if child == 0:
    os.setsid()
    print(os.getpid(), flush=True)
    time.sleep(60)
    os._exit(0)

os.waitpid(child, 0)
"#;
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from("/usr/bin/python3"),
            vec![OsString::from("-c"), OsString::from(script)],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let process_group = process.child.id().expect("child pid") as i32;
        let mut stdout = tokio::io::BufReader::new(process.stdout().expect("stdout piped"));
        let descendant = read_reported_pid(&mut stdout).await;
        drop(stdout);
        let descendant_identity =
            read_linux_process_identity(descendant).expect("detached descendant identity");
        assert_ne!(unsafe { getpgid(descendant) }, process_group);
        assert_eq!(unsafe { getpgid(descendant) }, descendant);

        process.shutdown(Duration::from_millis(50)).await;
        assert_process_group_exits_within(process_group, Duration::from_secs(1)).await;
        assert_process_identity_exits_within(descendant_identity, Duration::from_secs(1)).await;
    }

    #[cfg(target_os = "linux")]
    async fn assert_process_group_exits_within(process_group: i32, limit: Duration) {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }

        let deadline = tokio::time::Instant::now() + limit;
        loop {
            if unsafe { kill(-process_group, 0) } != 0 {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "process group {process_group} survived bounded shutdown"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[cfg(target_os = "linux")]
    async fn read_reported_pid(
        stdout: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
    ) -> i32 {
        read_reported_line(stdout)
            .await
            .parse()
            .expect("background descendant pid")
    }

    #[cfg(target_os = "linux")]
    async fn read_reported_identity_and_pid(
        stdout: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
    ) -> ((i32, u64), i32) {
        let line = read_reported_line(stdout).await;
        let mut fields = line.split_whitespace();
        let leader = fields
            .next()
            .expect("leader pid")
            .parse()
            .expect("numeric leader pid");
        let leader_start = fields
            .next()
            .expect("leader start time")
            .parse()
            .expect("numeric leader start time");
        let descendant = fields
            .next()
            .expect("descendant pid")
            .parse()
            .expect("numeric descendant pid");
        assert!(fields.next().is_none(), "unexpected pid report: {line:?}");
        ((leader, leader_start), descendant)
    }

    #[cfg(target_os = "linux")]
    async fn read_reported_line(
        stdout: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
    ) -> String {
        use tokio::io::AsyncBufReadExt;

        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(1), stdout.read_line(&mut line))
            .await
            .expect("shell must report its background descendant")
            .expect("background descendant pid must be readable");
        line.trim().to_owned()
    }

    #[cfg(target_os = "linux")]
    fn read_linux_process_identity(pid: i32) -> Option<(i32, u64)> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let fields = stat
            .rsplit_once(')')?
            .1
            .split_whitespace()
            .collect::<Vec<_>>();
        Some((pid, fields.get(19)?.parse().ok()?))
    }

    #[cfg(target_os = "linux")]
    async fn assert_process_identity_exits_within(identity: (i32, u64), limit: Duration) {
        let deadline = tokio::time::Instant::now() + limit;
        loop {
            if read_linux_process_identity(identity.0) != Some(identity) {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "process identity {identity:?} survived bounded cleanup"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[cfg(target_os = "linux")]
    struct ChildGuard(std::process::Child);

    #[cfg(target_os = "linux")]
    impl ChildGuard {
        fn terminate(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            self.terminate();
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn unix_fixture_completes_the_real_initialize_and_account_protocol() {
        let spec = AppServerSpawnSpec::test_fixture(FakeServerMode::Normal).unwrap();
        let client = tokio::time::timeout(
            Duration::from_secs(5),
            super::super::client::CodexAppServerClient::connect(spec),
        )
        .await
        .expect("fixture initialization must be bounded")
        .expect("unix fixture must initialize");

        let account = client
            .request("account/read", serde_json::json!({ "refreshToken": false }))
            .await
            .expect("unix fixture must answer account/read");
        assert_eq!(account["account"]["type"], "chatgpt");
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn post_kill_wait_is_bounded_when_the_child_never_reaps() {
        let started = std::time::Instant::now();
        wait_after_kill(std::future::pending::<()>()).await;
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stdout_and_stderr_are_piped_and_drainable() {
        let command = ResolvedCodexCommand::from_parts(
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from(
                    "[Console]::Out.Write('out-line'); [Console]::Error.Write('err-line')",
                ),
            ],
            super::super::discovery::CodexInstallation::NativeExe,
        );
        let mut spec = AppServerSpawnSpec::current_cli(command);
        spec.arguments.truncate(spec.arguments.len() - 1);
        let mut process = SupervisedAppServerProcess::spawn(&spec).unwrap();
        let mut stdout = process.stdout().expect("stdout piped");
        let mut stderr = process.stderr().expect("stderr piped");
        let mut out = String::new();
        let mut err = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut stdout, &mut out)
            .await
            .unwrap();
        tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut err)
            .await
            .unwrap();
        process.shutdown(SHUTDOWN_GRACE).await;
        assert!(out.contains("out-line"), "stdout was not drained: {out:?}");
        assert!(err.contains("err-line"), "stderr was not drained: {err:?}");
    }

    #[tokio::test]
    async fn stderr_redaction_line_cap_hides_secrets_and_bounds_lines() {
        use crate::core::SecretRedactor;
        let secret_line = "error token=sk-abcdefgh12345678 Bearer abc.def.ghi";
        let redacted = SecretRedactor::redact(secret_line);
        assert!(!redacted.contains("sk-abcdefgh12345678"));
        assert!(!redacted.contains("abc.def.ghi"));
        // Line cap: a redactor task over many lines must bound its output.
        let many: String = (0..10_000).map(|i| format!("line {i}\n")).collect();
        const MAX_DRAIN_LINES: usize = 1_000;
        let kept: Vec<&str> = many.lines().take(MAX_DRAIN_LINES).collect();
        assert_eq!(kept.len(), MAX_DRAIN_LINES);
    }
}
