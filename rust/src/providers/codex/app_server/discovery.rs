//! Fail-closed resolution of native Codex executables.
//!
//! Resolution never prepends the current directory, never invokes a shell,
//! canonicalizes every candidate, requires a regular file, and rejects
//! reparse points (symlinks/junctions) on Windows. The only accepted native
//! suffix is `.exe`; `.cmd` npm shims are verified against the checked-in
//! official fixture and launched as `node.exe <entry>` instead of executing
//! batch content.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::core::{AppError, AppErrorKind, RecoveryAction};

/// How the resolved Codex command is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexInstallation {
    /// A native `codex.exe`.
    NativeExe,
    /// The WindowsApps app-execution alias (only after the same checks plus a
    /// successful direct `--version` probe).
    StoreAlias,
    /// An npm `@openai/codex` layout whose `.cmd` shim matched the official
    /// fixture; launched as `node.exe <absolute entry.js>`.
    VerifiedNpmLayout,
}

/// A fully resolved, launch-ready Codex command.
#[derive(Debug, Clone)]
pub struct ResolvedCodexCommand {
    program: PathBuf,
    args_prefix: Vec<OsString>,
    version: Option<String>,
    installation: CodexInstallation,
}

impl ResolvedCodexCommand {
    pub fn program(&self) -> &Path {
        &self.program
    }
    /// Program path in the form accepted by process-creation APIs (strips the
    /// Win32 extended-length prefix used by `canonicalize`).
    pub fn launch_program(&self) -> PathBuf {
        launch_path(&self.program)
    }
    pub fn args_prefix(&self) -> &[OsString] {
        &self.args_prefix
    }
    /// Arguments in the form accepted by process-creation APIs. Canonical
    /// Windows paths carry the `\\?\` extended-length prefix, which child
    /// programs such as node.exe do not reliably accept as an argument.
    pub fn launch_args_prefix(&self) -> Vec<OsString> {
        self.args_prefix
            .iter()
            .map(|arg| {
                arg.to_str()
                    .map(|text| OsString::from(strip_extended_prefix(text)))
                    .unwrap_or_else(|| arg.clone())
            })
            .collect()
    }
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    pub fn installation(&self) -> CodexInstallation {
        self.installation
    }
    #[allow(dead_code)]
    pub(crate) fn set_version(&mut self, version: Option<String>) {
        self.version = version;
    }

    pub(crate) fn from_parts(
        program: PathBuf,
        args_prefix: Vec<OsString>,
        installation: CodexInstallation,
    ) -> Self {
        Self {
            program,
            args_prefix,
            version: None,
            installation,
        }
    }
}

/// Inputs to one resolution pass.
#[derive(Debug, Clone, Default)]
pub struct ResolveRequest {
    /// User-configured absolute path override (highest precedence).
    pub override_path: Option<PathBuf>,
    /// Explicit PATH snapshot; resolution never reads the process PATH itself.
    pub path: Option<OsString>,
    /// Explicit PATHEXT snapshot (e.g. `.EXE;.CMD`); defaults to `.EXE`.
    pub pathext: Option<OsString>,
}

/// Resolver for Codex commands. Stateless; all inputs come through
/// [`ResolveRequest`] so tests fully control the environment.
pub struct CodexCommandResolver;

impl CodexCommandResolver {
    pub fn new() -> Self {
        Self
    }

    /// Resolve in precedence order: absolute override, then non-empty PATH
    /// entries in declared order, then the known native install roots.
    pub fn resolve(&self, request: &ResolveRequest) -> Result<ResolvedCodexCommand, AppError> {
        if let Some(override_path) = &request.override_path {
            return self.resolve_override(override_path);
        }
        if let Some(path) = &request.path
            && let Some(found) = self.resolve_in_path(path, request.pathext.as_ref())?
        {
            return Ok(found);
        }
        for candidate in known_native_candidates() {
            if let Some(cmd) = verify_native_exe(&candidate)? {
                let installation = if is_windows_apps_alias(&candidate) {
                    // The Store alias must additionally survive a direct
                    // fixed-argument `--version` probe before it is selected.
                    match probe_version(&cmd) {
                        Some(version) => {
                            let mut cmd = cmd;
                            cmd.installation = CodexInstallation::StoreAlias;
                            cmd.version = Some(version);
                            return Ok(cmd);
                        }
                        None => continue,
                    }
                } else {
                    CodexInstallation::NativeExe
                };
                let mut cmd = cmd;
                cmd.installation = installation;
                return Ok(cmd);
            }
        }
        Err(AppError::new(
            AppErrorKind::CodexNotFound,
            "errors.codexNotFound",
            RecoveryAction::InstallTestedCodex,
            "CODEX_NOT_FOUND",
        ))
    }

    /// Resolve an explicit user override. Must be absolute; `.exe` files go
    /// through the native checks, `.cmd` shims through the npm verification.
    pub fn resolve_override(&self, override_path: &Path) -> Result<ResolvedCodexCommand, AppError> {
        if !override_path.is_absolute() {
            return Err(AppError::new(
                AppErrorKind::CodexNotFound,
                "errors.codexOverrideNotAbsolute",
                RecoveryAction::InstallTestedCodex,
                "CODEX_OVERRIDE_NOT_ABSOLUTE",
            ));
        }
        #[cfg(target_os = "linux")]
        if override_path
            .file_name()
            .is_some_and(|name| name == "codex")
        {
            if let Some(command) = verify_linux_codex(override_path)? {
                return Ok(command);
            }
            return crate::providers::codex::app_server::npm::resolve_linux_npm_layout(
                override_path,
            )
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::CodexNotFound,
                    "errors.codexNotFound",
                    RecoveryAction::InstallTestedCodex,
                    "CODEX_OVERRIDE_NOT_USABLE",
                )
            });
        }

        match extension_lower(override_path).as_deref() {
            Some("exe") => verify_native_exe(override_path)?.ok_or_else(|| {
                AppError::new(
                    AppErrorKind::CodexNotFound,
                    "errors.codexNotFound",
                    RecoveryAction::InstallTestedCodex,
                    "CODEX_OVERRIDE_NOT_USABLE",
                )
            }),
            Some("cmd") => {
                crate::providers::codex::app_server::npm::resolve_npm_shim(override_path)
            }
            _ => Err(AppError::new(
                AppErrorKind::CodexNotFound,
                "errors.codexNotFound",
                RecoveryAction::InstallTestedCodex,
                "CODEX_OVERRIDE_UNSUPPORTED_SUFFIX",
            )),
        }
    }

    /// Search an explicit PATH snapshot. Empty segments are skipped outright;
    /// the current directory is never implied.
    fn resolve_in_path(
        &self,
        path: &OsString,
        pathext: Option<&OsString>,
    ) -> Result<Option<ResolvedCodexCommand>, AppError> {
        #[cfg(target_os = "linux")]
        {
            // PATHEXT is a Windows-only lookup convention; Linux probes bare commands.
            let _ = pathext;
            for segment in std::env::split_paths(path) {
                if segment.as_os_str().is_empty() {
                    continue;
                }
                if let Some(command) = verify_linux_codex(&segment.join("codex"))? {
                    return Ok(Some(command));
                }
                if let Ok(command) =
                    crate::providers::codex::app_server::npm::resolve_linux_npm_layout(
                        &segment.join("codex"),
                    )
                {
                    return Ok(Some(command));
                }
            }
            Ok(None)
        }

        #[cfg(not(target_os = "linux"))]
        {
            let exts = pathext_extensions(pathext);
            for segment in std::env::split_paths(path) {
                if segment.as_os_str().is_empty() {
                    continue;
                }
                for ext in &exts {
                    let candidate = segment.join(format!("codex.{ext}"));
                    match ext.as_str() {
                        "exe" => {
                            if let Some(cmd) = verify_native_exe(&candidate)? {
                                return Ok(Some(cmd));
                            }
                        }
                        "cmd" => {
                            if let Ok(cmd) =
                                crate::providers::codex::app_server::npm::resolve_npm_shim(
                                    &candidate,
                                )
                            {
                                return Ok(Some(cmd));
                            }
                            if let Ok(cmd) = crate::providers::codex::app_server::npm::resolve_official_native_from_prefix(&segment)
                        {
                            return Ok(Some(cmd));
                        }
                        }
                        _ => {}
                    }
                }
            }
            Ok(None)
        }
    }

    #[cfg(all(test, windows))]
    fn resolve_in_path_with_native_verifier(
        &self,
        path: &OsString,
        pathext: Option<&OsString>,
        verify_signature: &dyn Fn(&Path) -> bool,
    ) -> Result<Option<ResolvedCodexCommand>, AppError> {
        let exts = pathext_extensions(pathext);
        for segment in std::env::split_paths(path) {
            if segment.as_os_str().is_empty() {
                continue;
            }
            for ext in &exts {
                let candidate = segment.join(format!("codex.{ext}"));
                match ext.as_str() {
                    "exe" => {
                        if let Some(cmd) = verify_native_exe(&candidate)? {
                            return Ok(Some(cmd));
                        }
                    }
                    "cmd" => {
                        // Only a verified official npm layout resolves; a
                        // mismatch is not fatal to the rest of the search.
                        if let Ok(cmd) =
                            crate::providers::codex::app_server::npm::resolve_npm_shim(&candidate)
                        {
                            return Ok(Some(cmd));
                        }
                        if let Ok(cmd) = crate::providers::codex::app_server::npm::resolve_official_native_from_prefix_with_verifier(&segment, verify_signature)
                        {
                            return Ok(Some(cmd));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }
}

impl Default for CodexCommandResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Ordered known native install roots.
fn known_native_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Vec::new()
    }

    #[cfg(not(target_os = "linux"))]
    {
        let mut out = Vec::new();
        if let Some(local) = dirs::data_local_dir() {
            out.push(local.join(r"Programs\OpenAI Codex\codex.exe"));
            out.push(local.join(r"Programs\Codex\codex.exe"));
            out.push(local.join(r"Microsoft\WindowsApps\codex.exe"));
        }
        out
    }
}

fn is_windows_apps_alias(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
        .contains(r"microsoft\windowsapps\")
}

/// Cross-platform reparse-point check (Windows: real reparse points;
/// elsewhere: symlinks).
#[cfg(windows)]
pub(crate) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
pub(crate) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|e| e.to_ascii_lowercase())
}

/// Parse a PATHEXT snapshot into lowercase extensions without dots.
#[cfg(not(target_os = "linux"))]
fn pathext_extensions(pathext: Option<&OsString>) -> Vec<String> {
    let raw = match pathext.and_then(|p| p.to_str()) {
        Some(raw) if !raw.trim().is_empty() => raw.to_string(),
        _ => ".EXE".to_string(),
    };
    raw.split(';')
        .filter_map(|e| {
            let e = e.trim().trim_start_matches('.').to_ascii_lowercase();
            (!e.is_empty()).then_some(e)
        })
        .collect()
}

/// Canonicalize a native `.exe` candidate and require a regular,
/// non-reparse-point file. Returns `Ok(None)` for simply-absent files.
pub(crate) fn verify_native_exe(path: &Path) -> Result<Option<ResolvedCodexCommand>, AppError> {
    let original_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if is_reparse(&original_metadata) {
        return Ok(None);
    }
    let Ok(canonical) = path.canonicalize() else {
        return Ok(None); // absent or inaccessible candidates are skipped
    };
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| {
        AppError::new(
            AppErrorKind::CodexNotFound,
            "errors.codexNotFound",
            RecoveryAction::InstallTestedCodex,
            "CODEX_CANDIDATE_INACCESSIBLE",
        )
    })?;
    if is_reparse(&metadata) {
        return Ok(None);
    }
    if !metadata.is_file() {
        return Ok(None);
    }
    Ok(Some(ResolvedCodexCommand {
        program: canonical,
        args_prefix: Vec::new(),
        version: None,
        installation: CodexInstallation::NativeExe,
    }))
}

/// Verify a Linux `codex` executable without accepting a shell wrapper or
/// symlink. npm layouts are handled separately by their verified resolver.
#[cfg(target_os = "linux")]
fn verify_linux_codex(path: &Path) -> Result<Option<ResolvedCodexCommand>, AppError> {
    use std::os::unix::fs::PermissionsExt;

    let source = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if !source.is_file() || is_reparse(&source) || source.permissions().mode() & 0o111 == 0 {
        return Ok(None);
    }
    let canonical = path.canonicalize().map_err(|_| {
        AppError::new(
            AppErrorKind::CodexNotFound,
            "errors.codexNotFound",
            RecoveryAction::InstallTestedCodex,
            "CODEX_CANDIDATE_INACCESSIBLE",
        )
    })?;
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| {
        AppError::new(
            AppErrorKind::CodexNotFound,
            "errors.codexNotFound",
            RecoveryAction::InstallTestedCodex,
            "CODEX_CANDIDATE_INACCESSIBLE",
        )
    })?;
    if !metadata.is_file() || is_reparse(&metadata) || metadata.permissions().mode() & 0o111 == 0 {
        return Ok(None);
    }
    Ok(Some(ResolvedCodexCommand::from_parts(
        canonical,
        Vec::new(),
        CodexInstallation::NativeExe,
    )))
}

/// Direct fixed-argument `--version` probe; no shell is involved.
pub fn probe_version(cmd: &ResolvedCodexCommand) -> Option<String> {
    let mut command = std::process::Command::new(launch_path(cmd.program()));
    command
        .args(cmd.launch_args_prefix())
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW_FLAG);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.split_whitespace().last()?.trim().to_string();
    (!version.is_empty()).then_some(version)
}

/// `std::fs::canonicalize` uses the Win32 extended-length prefix on Windows.
/// Rust's process creation path is more portable when it receives the normal
/// drive-letter form, while all filesystem validation still uses canonical
/// paths.
fn launch_path(path: &Path) -> PathBuf {
    PathBuf::from(strip_extended_prefix(&path.to_string_lossy()))
}

pub(crate) fn strip_extended_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Owns a TempDir with synthetic executables and exposes exact paths.
    pub struct ResolverFixture {
        dir: tempfile::TempDir,
    }

    impl ResolverFixture {
        pub fn new() -> Self {
            Self {
                dir: tempfile::TempDir::new().unwrap(),
            }
        }

        pub fn root(&self) -> &Path {
            self.dir.path()
        }

        /// Layout: `bin/codex.exe` on PATH and an override exe elsewhere.
        pub fn with_native_exes() -> Self {
            let f = Self::new();
            fs::create_dir_all(f.root().join("bin")).unwrap();
            fs::write(f.root().join("bin").join("codex.exe"), b"MZ").unwrap();
            let override_exe = f.override_exe();
            fs::create_dir_all(override_exe.parent().unwrap()).unwrap();
            fs::write(&override_exe, b"MZ").unwrap();
            f
        }

        /// Layout: a `codex.exe` in the fixture root (the "cwd"), plus a PATH
        /// whose first segment is empty (current-directory injection attempt).
        pub fn with_cwd_exe() -> Self {
            let f = Self::new();
            fs::write(f.root().join("codex.exe"), b"MZ").unwrap();
            f
        }

        pub fn override_exe(&self) -> PathBuf {
            self.root().join("override").join("codex.exe")
        }

        pub fn path_value(&self) -> OsString {
            let mut value = OsString::new();
            value.push(self.root().join("bin"));
            value
        }

        pub fn resolve(
            &self,
            override_path: Option<PathBuf>,
            path: OsString,
        ) -> Result<ResolvedCodexCommand, AppError> {
            CodexCommandResolver::new().resolve(&ResolveRequest {
                override_path,
                path: Some(path),
                pathext: None,
            })
        }
    }

    #[test]
    fn absolute_override_precedes_path_and_known_locations() {
        let fixture = ResolverFixture::with_native_exes();
        fs::create_dir_all(fixture.override_exe().parent().unwrap()).unwrap();
        fs::write(fixture.override_exe(), b"MZ").unwrap();
        let result = fixture
            .resolve(Some(fixture.override_exe()), fixture.path_value())
            .unwrap();
        assert_eq!(
            result.program(),
            fixture.override_exe().canonicalize().unwrap()
        );
        assert_eq!(result.installation(), CodexInstallation::NativeExe);
    }

    #[test]
    fn empty_path_segment_never_selects_current_directory() {
        let fixture = ResolverFixture::with_cwd_exe();
        // PATH = ";C:\missing": the empty leading segment must not resolve to
        // the process current directory (which contains codex.exe here only in
        // the fixture root, never the actual cwd).
        let search = OsString::from(";C:\\definitely-missing-codex-dir");
        let request = ResolveRequest {
            override_path: None,
            path: Some(search),
            pathext: None,
        };
        // The known native roots likely do not exist on the test host, so a
        // not-found error proves the empty segment was skipped.
        let result = CodexCommandResolver::new().resolve(&request);
        match result {
            Err(e) => assert_eq!(e.kind, AppErrorKind::CodexNotFound),
            Ok(found) => {
                // If a machine-wide install exists, it must never come from
                // the fixture root (the simulated cwd).
                assert!(!found.program().starts_with(fixture.root()));
            }
        }
    }

    #[test]
    fn relative_override_is_rejected() {
        let fixture = ResolverFixture::with_native_exes();
        let error = fixture
            .resolve(Some(PathBuf::from("codex.exe")), OsString::new())
            .unwrap_err();
        assert_eq!(error.kind, AppErrorKind::CodexNotFound);
        assert_eq!(error.diagnostic_code, "CODEX_OVERRIDE_NOT_ABSOLUTE");
    }

    #[cfg(windows)]
    #[test]
    fn path_search_finds_native_exe_in_declared_order() {
        let fixture = ResolverFixture::with_native_exes();
        let found = fixture.resolve(None, fixture.path_value()).unwrap();
        assert_eq!(
            found.program(),
            fixture
                .root()
                .join("bin")
                .join("codex.exe")
                .canonicalize()
                .unwrap()
        );
        assert_eq!(found.installation(), CodexInstallation::NativeExe);
    }

    #[test]
    fn missing_file_candidates_are_skipped_not_fatal() {
        let fixture = ResolverFixture::new();
        fs::create_dir_all(fixture.root().join("bin")).unwrap();
        let result = fixture.resolve(None, fixture.root().join("bin").into_os_string());
        // Nothing to find: either a machine-wide known-root hit or NotFound.
        if let Err(e) = result {
            assert_eq!(e.kind, AppErrorKind::CodexNotFound);
        }
    }

    #[test]
    fn directory_named_codex_exe_is_rejected() {
        let fixture = ResolverFixture::new();
        fs::create_dir_all(fixture.root().join("bin").join("codex.exe")).unwrap();
        let request = ResolveRequest {
            override_path: None,
            path: Some(fixture.root().join("bin").into_os_string()),
            pathext: None,
        };
        let result = CodexCommandResolver::new().resolve(&request);
        if let Err(e) = result {
            assert_eq!(e.kind, AppErrorKind::CodexNotFound);
        }
    }

    #[cfg(windows)]
    #[test]
    fn pathext_is_honored_for_cmd_hints() {
        let exts = pathext_extensions(Some(&OsString::from(".COM;.EXE;.CMD")));
        assert_eq!(exts, vec!["com", "exe", "cmd"]);
        let default = pathext_extensions(None);
        assert_eq!(default, vec!["exe"]);
    }

    #[cfg(not(test))]
    fn resolve_in_path_with_native_verifier(
        &self,
        path: &OsString,
        pathext: Option<&OsString>,
        _: &dyn Fn(&Path) -> bool,
    ) -> Result<Option<ResolvedCodexCommand>, AppError> {
        let exts = pathext_extensions(pathext);
        for segment in std::env::split_paths(path) {
            if segment.as_os_str().is_empty() {
                continue;
            }
            for ext in &exts {
                let candidate = segment.join(format!("codex.{ext}"));
                match ext.as_str() {
                    "exe" => {
                        if let Some(cmd) = verify_native_exe(&candidate)? {
                            return Ok(Some(cmd));
                        }
                    }
                    "cmd" => {
                        if let Ok(cmd) =
                            crate::providers::codex::app_server::npm::resolve_npm_shim(&candidate)
                        {
                            return Ok(Some(cmd));
                        }
                        if let Ok(cmd) = crate::providers::codex::app_server::npm::resolve_official_native_from_prefix(&segment)
                        {
                            return Ok(Some(cmd));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }

    #[cfg(windows)]
    #[test]
    fn path_wrapper_can_fall_back_to_verified_native_package() {
        let fixture = tempfile::TempDir::new().unwrap();
        let prefix = fixture.path().join("npm");
        let root_package = prefix.join(r"node_modules\@openai\codex");
        let platform_package = root_package.join(r"node_modules\@openai\codex-win32-x64");
        let vendor = platform_package.join(r"vendor\x86_64-pc-windows-msvc");
        fs::create_dir_all(vendor.join("bin")).unwrap();
        fs::write(prefix.join("codex.cmd"), "@echo off\r\necho wrapper\r\n").unwrap();
        fs::write(
            root_package.join("package.json"),
            r#"{"name":"@openai/codex","version":"0.146.0","optionalDependencies":{"@openai/codex-win32-x64":"npm:@openai/codex@0.146.0-win32-x64"}}"#,
        )
        .unwrap();
        fs::write(
            platform_package.join("package.json"),
            r#"{"name":"@openai/codex","version":"0.146.0-win32-x64","os":["win32"],"cpu":["x64"]}"#,
        )
        .unwrap();
        fs::write(
            vendor.join("codex-package.json"),
            r#"{"layoutVersion":1,"version":"0.146.0","target":"x86_64-pc-windows-msvc","variant":"codex","entrypoint":"bin/codex.exe","resourcesDir":"codex-resources","pathDir":"codex-path"}"#,
        )
        .unwrap();
        let native = vendor.join("bin").join("codex.exe");
        fs::write(&native, b"MZ").unwrap();

        let found = CodexCommandResolver::new()
            .resolve_in_path_with_native_verifier(
                &prefix.into_os_string(),
                Some(&OsString::from(".CMD")),
                &|_| true,
            )
            .unwrap()
            .unwrap();
        assert_eq!(found.program(), native.canonicalize().unwrap());
        assert!(found.args_prefix().is_empty());
        assert_eq!(found.installation(), CodexInstallation::VerifiedNpmLayout);
    }

    #[cfg(windows)]
    #[test]
    fn reparse_point_candidate_is_rejected() {
        // A symlink named "codex.exe" must be rejected before canonicalize()
        // can resolve it to a regular target. Some CI/dev shells do not grant
        // SeCreateSymbolicLinkPrivilege; in that case the test is skipped.
        let fixture = ResolverFixture::new();
        let target = fixture.root().join("real-codex.exe");
        fs::write(&target, b"MZ").unwrap();
        let bin = fixture.root().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let link = bin.join("codex.exe");
        if let Err(error) = std::os::windows::fs::symlink_file(&target, &link) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("skipping reparse-point test: symbolic-link privilege unavailable");
                return;
            }
            panic!("failed to create test symlink: {error}");
        }
        assert!(verify_native_exe(&link).unwrap().is_none());
    }

    #[test]
    fn unsupported_override_suffix_is_rejected() {
        let fixture = ResolverFixture::new();
        let script = fixture.root().join("codex.ps1");
        fs::write(&script, b"Write-Host hi").unwrap();
        let error = CodexCommandResolver::new()
            .resolve_override(&script)
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "CODEX_OVERRIDE_UNSUPPORTED_SUFFIX");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_resolver_accepts_codex_without_a_windows_suffix() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = tempfile::TempDir::new().unwrap();
        let path = fixture.path().join("codex");
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();

        let command = CodexCommandResolver::new().resolve_override(&path).unwrap();
        assert_eq!(command.launch_program(), path.canonicalize().unwrap());
    }

    /// Real-machine compatibility probe. It is intentionally ignored so the
    /// ordinary unit suite never depends on a user's Codex installation.
    #[test]
    #[ignore = "run explicitly with scripts/codex-app-server-smoke.ps1"]
    fn real_machine_resolver_probe_is_read_only() {
        let resolver = CodexCommandResolver::new();
        let request = ResolveRequest {
            override_path: None,
            path: std::env::var_os("PATH"),
            pathext: std::env::var_os("PATHEXT"),
        };
        let resolved = resolver
            .resolve(&request)
            .expect("Codex command not resolvable");
        let version = probe_version(&resolved);
        println!(
            "installation={:?} program={} version={}",
            resolved.installation(),
            redact_machine_path(&launch_path(resolved.program())),
            version.as_deref().unwrap_or("unavailable")
        );
        assert!(
            version.is_some(),
            "resolved command must pass a direct --version probe"
        );
    }

    fn redact_machine_path(path: &Path) -> String {
        let text = path.to_string_lossy().replace('\\', "/");
        if let Some((_, suffix)) = text.split_once("/Users/") {
            if let Some((_, rest)) = suffix.split_once('/') {
                format!("%USERPROFILE%/{rest}")
            } else {
                "%USERPROFILE%".to_string()
            }
        } else {
            text
        }
    }
}
