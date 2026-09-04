//! Verified npm `@openai/codex` layout resolution.
//!
//! A `.cmd` shim is never executed. Instead the shim text must match the
//! checked-in official npm fixture (after CRLF normalization), `node.exe` is
//! resolved natively, and the package entry `node_modules\@openai\codex\
//! bin\codex.js` must canonicalize to a regular, non-reparse file that stays
//! below the shim's install tree.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::core::{AppError, AppErrorKind, RecoveryAction};

use super::discovery::{CodexInstallation, ResolvedCodexCommand, is_reparse};

/// Size cap for a `.cmd` shim we are willing to read and compare.
const MAX_SHIM_BYTES: u64 = 64 * 1024;
const MAX_JSON_BYTES: u64 = 64 * 1024;
const PLATFORM_PACKAGE: &str = "@openai/codex-win32-x64";
const PLATFORM_SUFFIX: &str = "-win32-x64";
const VENDOR_TARGET: &str = "x86_64-pc-windows-msvc";

#[derive(Deserialize)]
struct RootPackage {
    name: String,
    version: String,
    #[serde(rename = "optionalDependencies", default)]
    optional_dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct PlatformPackage {
    name: String,
    version: String,
    os: Vec<String>,
    cpu: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeLayout {
    layout_version: u32,
    version: String,
    target: String,
    variant: String,
    entrypoint: String,
}

/// The checked-in official npm shim (fixture), used as the allow-list.
const OFFICIAL_SHIM: &str = include_str!("fixtures/npm-codex.cmd");

fn wrapper_unsupported() -> AppError {
    AppError::new(
        AppErrorKind::UnsupportedCodexVersion,
        "errors.codexWrapperUnsupported",
        RecoveryAction::InstallTestedCodex,
        "CODEX_WRAPPER_UNSUPPORTED",
    )
}

fn normalize_crlf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Resolve a `.cmd` hint to `node.exe <absolute entry.js>` when, and only
/// when, the layout matches the official npm package layout.
pub fn resolve_npm_shim(shim: &Path) -> Result<ResolvedCodexCommand, AppError> {
    if !shim.is_absolute() {
        return Err(wrapper_unsupported());
    }
    let source_metadata = std::fs::symlink_metadata(shim).map_err(|_| wrapper_unsupported())?;
    if !source_metadata.is_file() || is_reparse(&source_metadata) {
        return Err(wrapper_unsupported());
    }
    let canonical_shim = shim.canonicalize().map_err(|_| wrapper_unsupported())?;
    let metadata = std::fs::symlink_metadata(&canonical_shim).map_err(|_| wrapper_unsupported())?;
    if !metadata.is_file() || is_reparse(&metadata) || metadata.len() > MAX_SHIM_BYTES {
        return Err(wrapper_unsupported());
    }
    let text = std::fs::read_to_string(&canonical_shim).map_err(|_| wrapper_unsupported())?;
    if normalize_crlf(&text) != normalize_crlf(OFFICIAL_SHIM) {
        return Err(wrapper_unsupported());
    }

    // Official npm layout: <prefix>\codex.cmd sits next to
    // <prefix>\node_modules\@openai\codex\bin\codex.js.
    let prefix = canonical_shim
        .parent()
        .ok_or_else(wrapper_unsupported)?
        .to_path_buf();
    let entry = prefix.join(r"node_modules\@openai\codex\bin\codex.js");
    let entry_source_metadata =
        std::fs::symlink_metadata(&entry).map_err(|_| wrapper_unsupported())?;
    if !entry_source_metadata.is_file() || is_reparse(&entry_source_metadata) {
        return Err(wrapper_unsupported());
    }
    let canonical_entry = entry.canonicalize().map_err(|_| wrapper_unsupported())?;
    let entry_meta =
        std::fs::symlink_metadata(&canonical_entry).map_err(|_| wrapper_unsupported())?;
    if !entry_meta.is_file() || is_reparse(&entry_meta) {
        return Err(wrapper_unsupported());
    }
    // The entry must remain below the shim's install tree after
    // canonicalization (defends against junction escapes).
    if !canonical_entry.starts_with(&prefix) {
        return Err(wrapper_unsupported());
    }

    // Resolve node.exe natively: sibling of the shim first, then the normal
    // fail-closed resolver over the ambient PATH snapshot.
    let node = resolve_node(&prefix)?;
    Ok(ResolvedCodexCommand::from_parts(
        node,
        vec![canonical_entry.into_os_string()],
        CodexInstallation::VerifiedNpmLayout,
    ))
}

/// Resolve a Linux npm global-bin symlink only when it points at the exact
/// package entry beneath the same prefix. The symlink itself is never
/// executed, and an adjacent, validated `node` executable launches the fixed
/// JavaScript entry directly.
#[cfg(target_os = "linux")]
pub(crate) fn resolve_linux_npm_layout(shim: &Path) -> Result<ResolvedCodexCommand, AppError> {
    use std::os::unix::fs::PermissionsExt;

    if !shim.is_absolute() || shim.file_name().is_none_or(|name| name != "codex") {
        return Err(wrapper_unsupported());
    }
    let shim_metadata = std::fs::symlink_metadata(shim).map_err(|_| wrapper_unsupported())?;
    if !shim_metadata.file_type().is_symlink() {
        return Err(wrapper_unsupported());
    }

    let bin = shim.parent().ok_or_else(wrapper_unsupported)?;
    if bin.file_name().is_none_or(|name| name != "bin") {
        return Err(wrapper_unsupported());
    }
    let prefix = canonical_directory(bin.parent().ok_or_else(wrapper_unsupported)?)?;
    let expected_entry = canonical_regular_file(
        &prefix.join("lib/node_modules/@openai/codex/bin/codex.js"),
        &prefix,
    )?;
    if shim.canonicalize().map_err(|_| wrapper_unsupported())? != expected_entry {
        return Err(wrapper_unsupported());
    }

    let node = canonical_regular_file(&prefix.join("bin/node"), &prefix)?;
    let node_metadata = std::fs::metadata(&node).map_err(|_| wrapper_unsupported())?;
    if node_metadata.permissions().mode() & 0o111 == 0 {
        return Err(wrapper_unsupported());
    }

    Ok(ResolvedCodexCommand::from_parts(
        node,
        vec![expected_entry.into_os_string()],
        CodexInstallation::VerifiedNpmLayout,
    ))
}

/// Resolve the native Windows x64 executable from a verified official npm
/// prefix. No wrapper text is read or executed by this path.
#[allow(dead_code)]
pub(crate) fn resolve_official_native_from_prefix(
    prefix: &Path,
) -> Result<ResolvedCodexCommand, AppError> {
    resolve_official_native_from_prefix_impl(prefix, &verify_openai_signature)
}

/// The verifier is an internal seam for deterministic package-layout tests.
/// Production always supplies [`verify_openai_signature`].
#[cfg(test)]
pub(super) fn resolve_official_native_from_prefix_with_verifier(
    prefix: &Path,
    verify_signature: &dyn Fn(&Path) -> bool,
) -> Result<ResolvedCodexCommand, AppError> {
    resolve_official_native_from_prefix_impl(prefix, verify_signature)
}

fn resolve_official_native_from_prefix_impl(
    prefix: &Path,
    verify_signature: &dyn Fn(&Path) -> bool,
) -> Result<ResolvedCodexCommand, AppError> {
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err(wrapper_unsupported());
    }

    let prefix = canonical_directory(prefix)?;
    let root_dir = prefix.join(r"node_modules\@openai\codex");
    let root: RootPackage = read_verified_json(&root_dir.join("package.json"), &prefix)?;
    if root.name != "@openai/codex" {
        return Err(wrapper_unsupported());
    }

    let expected_platform_version = format!("{}{}", root.version, PLATFORM_SUFFIX);
    if let Some(dependency) = root.optional_dependencies.get(PLATFORM_PACKAGE)
        && dependency != &format!("npm:@openai/codex@{expected_platform_version}")
    {
        return Err(wrapper_unsupported());
    }

    let platform_dir = root_dir.join(r"node_modules\@openai\codex-win32-x64");
    let platform: PlatformPackage =
        read_verified_json(&platform_dir.join("package.json"), &prefix)?;
    if platform.name != "@openai/codex"
        || platform.version != expected_platform_version
        || platform.os != ["win32"]
        || platform.cpu != ["x64"]
    {
        return Err(wrapper_unsupported());
    }

    let layout_path = platform_dir.join(r"vendor\x86_64-pc-windows-msvc\codex-package.json");
    let layout: NativeLayout = read_verified_json(&layout_path, &prefix)?;
    if layout.layout_version != 1
        || layout.version != root.version
        || layout.target != VENDOR_TARGET
        || layout.variant != "codex"
        || layout.entrypoint.replace('/', "\\") != r"bin\codex.exe"
    {
        return Err(wrapper_unsupported());
    }

    let layout_dir = layout_path.parent().ok_or_else(wrapper_unsupported)?;
    let exe = canonical_regular_file(&layout_dir.join(&layout.entrypoint), &prefix)?;
    if extension_is_not_exe(&exe) || !verify_signature(&exe) {
        return Err(wrapper_unsupported());
    }
    Ok(ResolvedCodexCommand::from_parts(
        exe,
        vec![],
        CodexInstallation::VerifiedNpmLayout,
    ))
}

fn canonical_directory(path: &Path) -> Result<PathBuf, AppError> {
    let source = std::fs::symlink_metadata(path).map_err(|_| wrapper_unsupported())?;
    if !source.is_dir() || is_reparse(&source) {
        return Err(wrapper_unsupported());
    }
    let canonical = path.canonicalize().map_err(|_| wrapper_unsupported())?;
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| wrapper_unsupported())?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(wrapper_unsupported());
    }
    Ok(canonical)
}

fn read_verified_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    prefix: &Path,
) -> Result<T, AppError> {
    let canonical = canonical_regular_file(path, prefix)?;
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| wrapper_unsupported())?;
    if metadata.len() > MAX_JSON_BYTES {
        return Err(wrapper_unsupported());
    }
    let content = std::fs::read(&canonical).map_err(|_| wrapper_unsupported())?;
    serde_json::from_slice(&content).map_err(|_| wrapper_unsupported())
}

fn canonical_regular_file(path: &Path, prefix: &Path) -> Result<PathBuf, AppError> {
    reject_reparse_segments(path, prefix)?;
    let source = std::fs::symlink_metadata(path).map_err(|_| wrapper_unsupported())?;
    if !source.is_file() || is_reparse(&source) {
        return Err(wrapper_unsupported());
    }
    let canonical = path.canonicalize().map_err(|_| wrapper_unsupported())?;
    if !canonical.starts_with(prefix) {
        return Err(wrapper_unsupported());
    }
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| wrapper_unsupported())?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(wrapper_unsupported());
    }
    Ok(canonical)
}

/// Reject reparse points in every directory segment below the trusted prefix.
/// Endpoint-only checks would let a junction be hidden by canonicalization.
fn reject_reparse_segments(path: &Path, prefix: &Path) -> Result<(), AppError> {
    let relative = path
        .strip_prefix(prefix)
        .map_err(|_| wrapper_unsupported())?;
    let mut current = prefix.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current).map_err(|_| wrapper_unsupported())?;
        if is_reparse(&metadata) {
            return Err(wrapper_unsupported());
        }
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn verify_openai_signature(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_CHAIN, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE, WinVerifyTrustEx,
    };

    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: windows::core::PCWSTR(path_wide.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_REVOCATION_CHECK_CHAIN,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let trusted = unsafe { WinVerifyTrustEx(HWND::default(), &mut action, &mut data) == 0 };
    let publisher = if trusted {
        unsafe { signer_publisher(&data) }
    } else {
        None
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe {
        let _ = WinVerifyTrustEx(HWND::default(), &mut action, &mut data);
    }
    publisher.as_deref().is_some_and(is_openai_publisher)
}

#[cfg(windows)]
fn is_openai_publisher(publisher: &str) -> bool {
    publisher == "OpenAI OpCo, LLC"
}

#[cfg(windows)]
unsafe fn signer_publisher(
    data: &windows::Win32::Security::WinTrust::WINTRUST_DATA,
) -> Option<String> {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Security::Cryptography::{
        CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW,
    };
    use windows::Win32::Security::WinTrust::{
        WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData,
    };

    // The state handle remains live until the caller performs CLOSE below.
    let provider = unsafe { WTHelperProvDataFromStateData(data.hWVTStateData) };
    if provider.is_null() {
        return None;
    }
    let signer = unsafe { WTHelperGetProvSignerFromChain(provider, 0, BOOL(0), 0) };
    if signer.is_null() || unsafe { (*signer).pasCertChain.is_null() } {
        return None;
    }
    let certificate = unsafe { (*(*signer).pasCertChain).pCert };
    if certificate.is_null() {
        return None;
    }
    let length =
        unsafe { CertGetNameStringW(certificate, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, None) };
    if length <= 1 {
        return None;
    }
    let mut subject = vec![0_u16; length as usize];
    let written = unsafe {
        CertGetNameStringW(
            certificate,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut subject),
        )
    };
    if written != length || subject.last() != Some(&0) {
        return None;
    }
    String::from_utf16(&subject[..subject.len() - 1]).ok()
}

#[cfg(not(windows))]
pub(super) fn verify_openai_signature(_: &Path) -> bool {
    false
}

fn extension_is_not_exe(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
}

fn resolve_node(prefix: &Path) -> Result<PathBuf, AppError> {
    let sibling = prefix.join("node.exe");
    if let Ok(source_metadata) = std::fs::symlink_metadata(&sibling)
        && source_metadata.is_file()
        && !is_reparse(&source_metadata)
        && let Ok(canonical) = sibling.canonicalize()
        && let Ok(meta) = std::fs::symlink_metadata(&canonical)
        && meta.is_file()
        && !is_reparse(&meta)
    {
        return Ok(canonical);
    }
    // Fall back to the ambient PATH through the fail-closed resolver.
    if let Some(path) = std::env::var_os("PATH") {
        for segment in std::env::split_paths(&path) {
            if segment.as_os_str().is_empty() {
                continue;
            }
            let candidate = segment.join("node.exe");
            if let Ok(source_metadata) = std::fs::symlink_metadata(&candidate)
                && source_metadata.is_file()
                && !is_reparse(&source_metadata)
                && let Ok(canonical) = candidate.canonicalize()
                && let Ok(meta) = std::fs::symlink_metadata(&canonical)
                && meta.is_file()
                && !is_reparse(&meta)
            {
                return Ok(canonical);
            }
        }
    }
    Err(wrapper_unsupported())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Synthetic official npm layout under a TempDir.
    pub struct NpmFixture {
        dir: tempfile::TempDir,
    }

    impl NpmFixture {
        fn new() -> Self {
            Self {
                dir: tempfile::TempDir::new().unwrap(),
            }
        }

        /// Official layout: `<root>\npm\codex.cmd` (official shim text),
        /// `<root>\npm\node.exe`, and the package entry file.
        fn official() -> Self {
            let f = Self::new();
            let npm = f.dir.path().join("npm");
            fs::create_dir_all(npm.join(r"node_modules\@openai\codex\bin")).unwrap();
            fs::write(npm.join("codex.cmd"), OFFICIAL_SHIM).unwrap();
            fs::write(npm.join("node.exe"), b"MZ").unwrap();
            fs::write(
                npm.join(r"node_modules\@openai\codex\bin\codex.js"),
                "// entry\n",
            )
            .unwrap();
            Self { dir: f.dir }
        }

        /// Layout whose shim contains arbitrary (malicious) batch content.
        fn malicious(content: &str) -> Self {
            let f = Self::new();
            let npm = f.dir.path().join("npm");
            fs::create_dir_all(&npm).unwrap();
            fs::write(npm.join("codex.cmd"), content).unwrap();
            Self { dir: f.dir }
        }

        /// An arbitrary `.cmd` wrapper beside the official native package
        /// layout shipped by `@openai/codex` on Windows x64.
        fn official_native_with_wrapper(wrapper: &str) -> Self {
            let f = Self::new();
            let npm = f.dir.path().join("npm");
            let root_package = npm.join(r"node_modules\@openai\codex");
            let platform_package = root_package.join(r"node_modules\@openai\codex-win32-x64");
            let vendor = platform_package.join(r"vendor\x86_64-pc-windows-msvc");
            fs::create_dir_all(vendor.join("bin")).unwrap();
            fs::write(npm.join("codex.cmd"), wrapper).unwrap();
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
            fs::write(vendor.join("bin").join("codex.exe"), b"MZ").unwrap();
            Self { dir: f.dir }
        }

        #[cfg(windows)]
        fn root(&self) -> &Path {
            self.dir.path()
        }

        fn npm_prefix(&self) -> PathBuf {
            self.dir.path().join("npm")
        }

        #[cfg(windows)]
        fn native_exe(&self) -> PathBuf {
            self.npm_prefix().join(
                r"node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe",
            )
        }

        fn write_root_version(&self, version: &str) {
            fs::write(
                self.npm_prefix().join(r"node_modules\@openai\codex\package.json"),
                format!(
                    r#"{{"name":"@openai/codex","version":"{version}","optionalDependencies":{{"@openai/codex-win32-x64":"npm:@openai/codex@0.146.0-win32-x64"}}}}"#
                ),
            )
            .unwrap();
        }

        fn write_layout_version(&self, version: &str) {
            self.mutate_layout(NativeFixtureMutation::Version(version));
        }

        fn mutate_layout(&self, mutation: NativeFixtureMutation<'_>) {
            let (target, variant, entrypoint, version) = match mutation {
                NativeFixtureMutation::Target(target) => {
                    (target, "codex", "bin/codex.exe", "0.146.0")
                }
                NativeFixtureMutation::Variant(variant) => (
                    "x86_64-pc-windows-msvc",
                    variant,
                    "bin/codex.exe",
                    "0.146.0",
                ),
                NativeFixtureMutation::Entrypoint(entrypoint) => {
                    ("x86_64-pc-windows-msvc", "codex", entrypoint, "0.146.0")
                }
                NativeFixtureMutation::Version(version) => {
                    ("x86_64-pc-windows-msvc", "codex", "bin/codex.exe", version)
                }
            };
            fs::write(
                self.npm_prefix().join(
                    r"node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\codex-package.json",
                ),
                format!(
                    r#"{{"layoutVersion":1,"version":"{version}","target":"{target}","variant":"{variant}","entrypoint":"{entrypoint}","resourcesDir":"codex-resources","pathDir":"codex-path"}}"#
                ),
            )
            .unwrap();
        }

        fn cmd(&self) -> PathBuf {
            self.dir.path().join("npm").join("codex.cmd")
        }

        fn node_exe(&self) -> PathBuf {
            self.dir.path().join("npm").join("node.exe")
        }

        fn entry(&self) -> PathBuf {
            self.dir
                .path()
                .join("npm")
                .join(r"node_modules\@openai\codex\bin\codex.js")
        }
    }

    enum NativeFixtureMutation<'a> {
        Target(&'a str),
        Variant(&'a str),
        Entrypoint(&'a str),
        Version(&'a str),
    }

    #[cfg(windows)]
    #[test]
    fn rejected_wrapper_can_resolve_verified_official_native_package() {
        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\necho wrapper\r\n");
        let resolved =
            resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                .unwrap();
        assert_eq!(
            resolved.program(),
            fixture.native_exe().canonicalize().unwrap()
        );
        assert!(resolved.args_prefix().is_empty());
        assert_eq!(
            resolved.installation(),
            CodexInstallation::VerifiedNpmLayout
        );
    }

    #[test]
    fn native_package_rejects_root_and_layout_version_mismatch() {
        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
        fixture.write_root_version("0.145.0");
        assert!(
            resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                .is_err()
        );
        fixture.write_root_version("0.146.0");
        fixture.write_layout_version("0.145.0");
        assert!(
            resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                .is_err()
        );
    }

    #[test]
    fn native_package_rejects_target_variant_and_entrypoint_mismatch() {
        for mutation in [
            NativeFixtureMutation::Target("aarch64-pc-windows-msvc"),
            NativeFixtureMutation::Variant("other"),
            NativeFixtureMutation::Entrypoint("../bin/codex.exe"),
        ] {
            let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
            fixture.mutate_layout(mutation);
            assert!(
                resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                    .is_err()
            );
        }
    }

    #[test]
    fn unsigned_native_package_is_rejected_by_production_verifier() {
        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
        let error = resolve_official_native_from_prefix(&fixture.npm_prefix()).unwrap_err();
        assert_eq!(error.diagnostic_code, "CODEX_WRAPPER_UNSUPPORTED");
    }

    #[cfg(windows)]
    #[test]
    fn native_package_rejects_reparse_escape() {
        use std::os::windows::fs::symlink_file;

        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
        let outside = fixture.root().join("outside-codex.exe");
        fs::write(&outside, b"MZ").unwrap();
        fs::remove_file(fixture.native_exe()).unwrap();
        if let Err(error) = symlink_file(&outside, fixture.native_exe()) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("skipping reparse-point test: symbolic-link privilege unavailable");
                return;
            }
            panic!("failed to create test symlink: {error}");
        }
        assert!(
            resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                .is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    fn intermediate_reparse_directory_is_rejected() {
        use std::os::windows::fs::symlink_dir;

        let fixture = NpmFixture::official_native_with_wrapper("@echo off\r\n");
        let platform = fixture
            .npm_prefix()
            .join(r"node_modules\@openai\codex\node_modules\@openai\codex-win32-x64");
        let outside = fixture.root().join("outside-platform");
        fs::rename(&platform, &outside).unwrap();
        if let Err(error) = symlink_dir(&outside, &platform) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("skipping reparse-point test: symbolic-link privilege unavailable");
                return;
            }
            panic!("failed to create test directory symlink: {error}");
        }
        assert!(
            resolve_official_native_from_prefix_with_verifier(&fixture.npm_prefix(), &|_| true)
                .is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    fn production_verifier_rejects_non_openai_signed_windows_binary() {
        let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
        let notepad = PathBuf::from(system_root).join(r"System32\notepad.exe");
        assert!(notepad.exists(), "Windows notepad.exe must be present");
        assert!(!verify_openai_signature(&notepad));
    }

    #[cfg(windows)]
    #[test]
    fn publisher_allowlist_requires_the_exact_openai_subject() {
        assert!(is_openai_publisher("OpenAI OpCo, LLC"));
        for publisher in [
            "Microsoft Corporation",
            "OpenAI OpCo, LLC ",
            " OpenAI OpCo, LLC",
            "openai OpCo, LLC",
            "OpenAI OpCo, L.L.C.",
        ] {
            assert!(!is_openai_publisher(publisher), "must reject {publisher:?}");
        }
    }

    #[test]
    fn cmd_hint_resolves_to_node_and_official_package_entry() {
        let layout = NpmFixture::official();
        let resolved = resolve_npm_shim(&layout.cmd()).unwrap();
        assert_eq!(
            resolved.installation(),
            CodexInstallation::VerifiedNpmLayout
        );
        assert_eq!(
            resolved.program(),
            layout.node_exe().canonicalize().unwrap()
        );
        assert_eq!(
            resolved.args_prefix(),
            &[layout.entry().canonicalize().unwrap().into_os_string()]
        );
    }

    #[test]
    fn arbitrary_batch_content_is_never_executed() {
        let layout = NpmFixture::malicious("powershell -EncodedCommand AAAA");
        let error = resolve_npm_shim(&layout.cmd()).unwrap_err();
        assert_eq!(error.diagnostic_code, "CODEX_WRAPPER_UNSUPPORTED");
        assert_eq!(error.kind, AppErrorKind::UnsupportedCodexVersion);
    }

    #[test]
    fn shim_with_missing_entry_is_rejected() {
        let f = NpmFixture::official();
        fs::remove_file(f.entry()).unwrap();
        let error = resolve_npm_shim(&f.cmd()).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::UnsupportedCodexVersion);
    }

    #[test]
    fn crlf_shim_matches_lf_fixture() {
        let f = NpmFixture::official();
        let lf = normalize_crlf(OFFICIAL_SHIM);
        let crlf = lf.replace('\n', "\r\n");
        fs::write(f.cmd(), crlf).unwrap();
        assert!(resolve_npm_shim(&f.cmd()).is_ok());
    }

    #[test]
    fn relative_shim_is_rejected() {
        let error = resolve_npm_shim(Path::new("codex.cmd")).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::UnsupportedCodexVersion);
    }

    #[test]
    fn resolver_dispatches_cmd_override_to_npm_verification() {
        let layout = NpmFixture::official();
        let resolved = super::super::discovery::CodexCommandResolver::new()
            .resolve_override(&layout.cmd())
            .unwrap();
        assert_eq!(
            resolved.installation(),
            CodexInstallation::VerifiedNpmLayout
        );
    }

    #[test]
    fn resolver_rejects_malicious_cmd_override() {
        let layout = NpmFixture::malicious("@echo off\r\necho pwned\r\n");
        let error = super::super::discovery::CodexCommandResolver::new()
            .resolve_override(&layout.cmd())
            .unwrap_err();
        assert_eq!(error.diagnostic_code, "CODEX_WRAPPER_UNSUPPORTED");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn verified_linux_npm_layout_resolves_its_node_entry_without_a_shell() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let fixture = tempfile::TempDir::new().unwrap();
        let prefix = fixture.path().join("prefix");
        let bin = prefix.join("bin");
        let entry = prefix.join("lib/node_modules/@openai/codex/bin/codex.js");
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::write(&entry, "// verified entry\n").unwrap();
        let node = bin.join("node");
        fs::write(&node, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&node).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&node, permissions).unwrap();
        let shim = bin.join("codex");
        symlink("../lib/node_modules/@openai/codex/bin/codex.js", &shim).unwrap();

        let resolved = resolve_linux_npm_layout(&shim).unwrap();

        assert_eq!(
            resolved.installation(),
            CodexInstallation::VerifiedNpmLayout
        );
        assert_eq!(resolved.program(), node.canonicalize().unwrap());
        assert_eq!(
            resolved.args_prefix(),
            &[entry.canonicalize().unwrap().into_os_string()]
        );
    }

    #[cfg(windows)]
    #[test]
    fn launch_arguments_strip_extended_length_prefix() {
        let layout = NpmFixture::official();
        let resolved = resolve_npm_shim(&layout.cmd()).unwrap();
        let entry = layout.entry().canonicalize().unwrap();
        // Canonicalization on Windows returns the `\\?\` extended form, which
        // must never be handed to node.exe as a script argument.
        assert!(
            entry.to_string_lossy().starts_with(r"\\?\"),
            "test precondition: canonical entry uses extended prefix"
        );
        let args = resolved.launch_args_prefix();
        assert_eq!(args.len(), 1);
        let stripped = super::super::discovery::strip_extended_prefix(&entry.to_string_lossy());
        assert_eq!(args[0].to_string_lossy(), stripped);
        assert!(!args[0].to_string_lossy().starts_with(r"\\?\"));
    }
}
