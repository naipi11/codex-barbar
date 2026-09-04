//! Isolated, restricted Managed `CODEX_HOME` directories.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::accounts::vault::ManagedCredentialBundle;
use crate::core::ProfileId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeHomeError {
    #[error("unsafe bundle path")]
    UnsafeBundlePath,
    #[error("reparse point rejected")]
    ReparsePointRejected,
    #[error("runtime directory already exists")]
    AlreadyExists,
    #[error("protected directory creation failed")]
    AclFailed,
    #[error("runtime directory verification failed")]
    VerificationFailed,
    #[error("io error")]
    Io,
}

/// One active Managed runtime directory for a profile.
pub struct ManagedRuntimeHome {
    codex_home: PathBuf,
    session_id: Uuid,
    profile_id: ProfileId,
    base_vault_generation: Option<u64>,
    created_at: DateTime<Utc>,
}

impl ManagedRuntimeHome {
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn base_vault_generation(&self) -> Option<u64> {
        self.base_vault_generation
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn cleanup(&self) -> Result<(), RuntimeHomeError> {
        super::runtime_home::remove_tree_no_follow(&self.codex_home)
    }

    /// Write the token-free runtime manifest that makes this session
    /// recoverable after a crash. Never includes paths or credentials.
    pub(crate) fn write_manifest(&self, state: RuntimeState) -> Result<(), RuntimeHomeError> {
        let manifest = serde_json::json!({
            "format": "codex-barbar-runtime",
            "version": 1,
            "sessionId": self.session_id.to_string(),
            "profileId": self.profile_id.to_string(),
            "baseVaultGeneration": self.base_vault_generation,
            "createdAt": self.created_at.to_rfc3339(),
            "state": state.as_str(),
        });
        write_restricted_file(
            self.codex_home.join("manifest.json"),
            serde_json::to_vec(&manifest).map_err(|_| RuntimeHomeError::Io)?,
        )
    }

    #[allow(dead_code)] // lifecycle hook consumed by the login/refresh actor
    pub(crate) fn set_state(&self, state: RuntimeState) -> Result<(), RuntimeHomeError> {
        self.write_manifest(state)
    }
}

/// Recovery metadata found on disk for one profile.
pub struct RecoveryCandidate {
    pub session_id: Uuid,
    pub profile_id: ProfileId,
    pub codex_home: PathBuf,
    pub base_vault_generation: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub state: RuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeState {
    Preparing,
    LoggingIn,
    Active,
    ReadyToSeal,
}

impl RuntimeState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::LoggingIn => "loggingIn",
            Self::Active => "active",
            Self::ReadyToSeal => "readyToSeal",
        }
    }
}

pub struct RuntimeHomeManager {
    runtime_root: PathBuf,
}

impl RuntimeHomeManager {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn prepare_new(
        &self,
        profile_id: ProfileId,
    ) -> Result<ManagedRuntimeHome, RuntimeHomeError> {
        let session_id = Uuid::new_v4();
        let codex_home = self.session_path(profile_id, session_id);
        let _guard = Self::create_guarded_dir(&codex_home)?;
        write_restricted_file(
            codex_home.join("config.toml"),
            b"cli_auth_credentials_store = \"file\"\n",
        )?;
        let home = ManagedRuntimeHome {
            codex_home,
            session_id,
            profile_id,
            base_vault_generation: None,
            created_at: Utc::now(),
        };
        home.write_manifest(RuntimeState::Preparing)?;
        Ok(home)
    }

    pub fn restore(
        &self,
        profile_id: ProfileId,
        bundle: &ManagedCredentialBundle,
        base_generation: u64,
    ) -> Result<ManagedRuntimeHome, RuntimeHomeError> {
        let session_id = Uuid::new_v4();
        let codex_home = self.session_path(profile_id, session_id);
        let _guard = Self::create_guarded_dir(&codex_home)?;
        let result = (|| {
            for file in &bundle.files {
                super::credential_bundle::restore_entry(&codex_home, file)?;
            }
            write_restricted_file(
                codex_home.join("config.toml"),
                b"cli_auth_credentials_store = \"file\"\n",
            )?;
            let home = ManagedRuntimeHome {
                codex_home: codex_home.clone(),
                session_id,
                profile_id,
                base_vault_generation: Some(base_generation),
                created_at: Utc::now(),
            };
            home.write_manifest(RuntimeState::Active)?;
            Ok(home)
        })();
        match result {
            Ok(home) => Ok(home),
            Err(error) => {
                remove_tree_no_follow(&codex_home)?;
                Err(error)
            }
        }
    }

    pub fn scan_recovery_candidates(&self) -> Result<Vec<RecoveryCandidate>, RuntimeHomeError> {
        let mut candidates = Vec::new();
        if !self.runtime_root.exists() {
            return Ok(candidates);
        }
        verify_restricted_directory(&self.runtime_root)?;
        let entries = std::fs::read_dir(&self.runtime_root).map_err(|_| RuntimeHomeError::Io)?;
        for entry in entries.flatten() {
            let profile_dir = entry.path();
            let profile_metadata =
                std::fs::symlink_metadata(&profile_dir).map_err(|_| RuntimeHomeError::Io)?;
            if super::windows_acl::is_reparse_point(&profile_metadata) {
                return Err(RuntimeHomeError::ReparsePointRejected);
            }
            if !profile_metadata.is_dir() {
                continue;
            }
            verify_restricted_directory(&profile_dir)?;
            let sessions = std::fs::read_dir(&profile_dir).map_err(|_| RuntimeHomeError::Io)?;
            for session in sessions.flatten() {
                let codex_home = session.path();
                let session_metadata =
                    std::fs::symlink_metadata(&codex_home).map_err(|_| RuntimeHomeError::Io)?;
                if super::windows_acl::is_reparse_point(&session_metadata) {
                    return Err(RuntimeHomeError::ReparsePointRejected);
                }
                if !session_metadata.is_dir() {
                    continue;
                }
                verify_restricted_directory(&codex_home)?;
                let manifest_path = codex_home.join("manifest.json");
                let Ok(contents) = std::fs::read_to_string(&manifest_path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
                    continue;
                };
                let Ok(session_id) = value
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .ok_or(())
                else {
                    continue;
                };
                let Ok(profile_id) = value
                    .get("profileId")
                    .and_then(|v| v.as_str())
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .ok_or(())
                else {
                    continue;
                };
                candidates.push(RecoveryCandidate {
                    session_id,
                    profile_id,
                    codex_home,
                    base_vault_generation: value
                        .get("baseVaultGeneration")
                        .and_then(|v| v.as_u64()),
                    created_at: value
                        .get("createdAt")
                        .and_then(|v| v.as_str())
                        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default(),
                    state: value
                        .get("state")
                        .and_then(|v| v.as_str())
                        .and_then(|s| match s {
                            "preparing" => Some(RuntimeState::Preparing),
                            "loggingIn" => Some(RuntimeState::LoggingIn),
                            "active" => Some(RuntimeState::Active),
                            "readyToSeal" => Some(RuntimeState::ReadyToSeal),
                            _ => None,
                        })
                        .unwrap_or(RuntimeState::Preparing),
                });
            }
        }
        Ok(candidates)
    }

    fn session_path(&self, profile_id: ProfileId, session_id: Uuid) -> PathBuf {
        self.runtime_root
            .join(profile_id.to_string())
            .join(session_id.to_string())
    }

    #[cfg(windows)]
    pub(crate) fn create_guarded_dir(
        path: &Path,
    ) -> Result<super::windows_acl::GuardedRuntimeDir, RuntimeHomeError> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{CloseHandle, GENERIC_ALL, HLOCAL, LocalFree};
        use windows::Win32::Security::Authorization::{
            ConvertStringSidToSidW, EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE,
            SetEntriesInAclW, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_IS_WELL_KNOWN_GROUP,
            TRUSTEE_W,
        };
        use windows::Win32::Security::{
            ACL, CopySid, GetLengthSid, GetTokenInformation, InitializeSecurityDescriptor,
            PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
            SECURITY_DESCRIPTOR, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
            TOKEN_QUERY, TOKEN_USER, TokenUser,
        };
        use windows::Win32::Storage::FileSystem::CreateDirectoryW;
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        if path.exists() {
            return Err(RuntimeHomeError::AlreadyExists);
        }
        std::fs::create_dir_all(path.parent().ok_or(RuntimeHomeError::Io)?)
            .map_err(|_| RuntimeHomeError::Io)?;

        // Current process user SID.
        let mut token = windows::Win32::Foundation::HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(|error| {
                tracing::debug!(target: "codexbar::accounts", error = %error, "OpenProcessToken failed");
                RuntimeHomeError::AclFailed
            })?;
        let mut length = 0u32;
        let query_result = unsafe {
            GetTokenInformation(token, TokenUser, Some(std::ptr::null_mut()), 0, &mut length)
        };
        if let Err(error) = query_result {
            // Win32 reports ERROR_INSUFFICIENT_BUFFER (0x7A) when the caller
            // asks only for the required size; that is the expected query
            // path, not a failure.
            if error.code().0 as u32 != 0x8007_007A {
                tracing::debug!(target: "codexbar::accounts", error = %error, "GetTokenInformation query failed");
                return Err(RuntimeHomeError::AclFailed);
            }
        }
        let mut buffer = vec![0u8; length as usize];
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr() as *mut _),
                length,
                &mut length,
            )
        }
        .map_err(|error| {
            tracing::debug!(target: "codexbar::accounts", error = %error, "GetTokenInformation fill failed");
            RuntimeHomeError::AclFailed
        })?;
        let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let user_sid = user.User.Sid;
        let user_len = unsafe { GetLengthSid(user_sid) };
        let mut user_sid_copy = vec![0u8; user_len as usize];
        unsafe { CopySid(user_len, PSID(user_sid_copy.as_mut_ptr() as _), user_sid) }.map_err(
            |error| {
                tracing::debug!(target: "codexbar::accounts", error = %error, "CopySid failed");
                RuntimeHomeError::AclFailed
            },
        )?;
        let _ = unsafe { CloseHandle(token) };

        // SYSTEM SID. Keep the UTF-16 buffer alive for the whole call; a
        // temporary Vec would be dropped before ConvertStringSidToSidW runs.
        let system_sid_wide: Vec<u16> = "S-1-5-18\0".encode_utf16().collect();
        let system_sid_text = windows::core::PCWSTR::from_raw(system_sid_wide.as_ptr());
        let mut system_sid: PSID = PSID(std::ptr::null_mut());
        unsafe { ConvertStringSidToSidW(system_sid_text, &mut system_sid) }
            .map_err(|error| {
                tracing::debug!(target: "codexbar::accounts", error = %error, "ConvertStringSidToSidW failed");
                RuntimeHomeError::AclFailed
            })?;

        let entries = [
            EXPLICIT_ACCESS_W {
                grfAccessPermissions: GENERIC_ALL.0,
                grfAccessMode: GRANT_ACCESS,
                grfInheritance: windows::Win32::Security::ACE_FLAGS(0x3), // CONTAINER|OBJECT inherit
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: std::ptr::null_mut(),
                    MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_USER,
                    ptstrName: windows::core::PWSTR(user_sid_copy.as_mut_ptr() as *mut _),
                },
            },
            EXPLICIT_ACCESS_W {
                grfAccessPermissions: GENERIC_ALL.0,
                grfAccessMode: GRANT_ACCESS,
                grfInheritance: windows::Win32::Security::ACE_FLAGS(0x3),
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: std::ptr::null_mut(),
                    MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_WELL_KNOWN_GROUP,
                    ptstrName: windows::core::PWSTR(system_sid.0 as *mut u16),
                },
            },
        ];
        let mut new_acl: *mut ACL = std::ptr::null_mut();
        let acl_result = unsafe { SetEntriesInAclW(Some(&entries), None, &mut new_acl) };
        if acl_result != windows::Win32::Foundation::WIN32_ERROR(0) {
            tracing::debug!(target: "codexbar::accounts", error = acl_result.0, "SetEntriesInAclW failed");
            return Err(RuntimeHomeError::AclFailed);
        }
        let mut sd = SECURITY_DESCRIPTOR::default();
        unsafe { InitializeSecurityDescriptor(PSECURITY_DESCRIPTOR((&mut sd) as *mut _ as _), 1) }
            .map_err(|error| {
                tracing::debug!(target: "codexbar::accounts", error = %error, "InitializeSecurityDescriptor failed");
                RuntimeHomeError::AclFailed
            })?;
        unsafe {
            SetSecurityDescriptorDacl(PSECURITY_DESCRIPTOR((&mut sd) as *mut _ as _), true, Some(new_acl), false)
        }
        .map_err(|error| {
            tracing::debug!(target: "codexbar::accounts", error = %error, "SetSecurityDescriptorDacl failed");
            RuntimeHomeError::AclFailed
        })?;
        unsafe {
            SetSecurityDescriptorControl(PSECURITY_DESCRIPTOR((&mut sd) as *mut _ as _), SE_DACL_PROTECTED, SE_DACL_PROTECTED)
        }
        .map_err(|error| {
            tracing::debug!(target: "codexbar::accounts", error = %error, "SetSecurityDescriptorControl failed");
            RuntimeHomeError::AclFailed
        })?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: (&mut sd) as *mut _ as *mut _,
            bInheritHandle: false.into(),
        };
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe { CreateDirectoryW(windows::core::PCWSTR::from_raw(wide.as_ptr()), Some(&sa)) }
            .map_err(|error| {
                tracing::debug!(target: "codexbar::accounts", error = %error, "CreateDirectoryW failed");
                RuntimeHomeError::AclFailed
            })?;
        unsafe {
            let _ = LocalFree(HLOCAL(new_acl as _));
            let _ = LocalFree(HLOCAL(system_sid.0 as _));
        }
        Ok(super::windows_acl::GuardedRuntimeDir {
            path: path.to_path_buf(),
        })
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn create_guarded_dir(
        path: &Path,
    ) -> Result<super::windows_acl::GuardedRuntimeDir, RuntimeHomeError> {
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            return Err(if metadata.file_type().is_symlink() {
                RuntimeHomeError::ReparsePointRejected
            } else {
                RuntimeHomeError::AlreadyExists
            });
        }
        let profile_root = path.parent().ok_or(RuntimeHomeError::Io)?;
        let runtime_root = profile_root.parent().ok_or(RuntimeHomeError::Io)?;
        reject_symlinked_ancestors(path)?;
        ensure_restricted_directory(runtime_root)?;
        ensure_restricted_directory(profile_root)?;
        ensure_restricted_directory(path)?;
        Self::verify_protected_directory(runtime_root)?;
        Self::verify_protected_directory(profile_root)?;
        Self::verify_protected_directory(path)?;
        Ok(super::windows_acl::GuardedRuntimeDir {
            path: path.to_path_buf(),
        })
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub(crate) fn create_guarded_dir(
        path: &Path,
    ) -> Result<super::windows_acl::GuardedRuntimeDir, RuntimeHomeError> {
        std::fs::create_dir_all(path).map_err(|_| RuntimeHomeError::Io)?;
        Ok(super::windows_acl::GuardedRuntimeDir {
            path: path.to_path_buf(),
        })
    }

    #[cfg(windows)]
    pub(crate) fn verify_protected_directory(path: &Path) -> Result<(), RuntimeHomeError> {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HLOCAL, LocalFree};
        use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
        use windows::Win32::Security::{
            ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION, GetAclInformation,
            GetSecurityDescriptorControl, PROTECTED_DACL_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
        };
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        };
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                windows::core::PCWSTR::from_raw(wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )
        }
        .map_err(|_| RuntimeHomeError::VerificationFailed)?;
        let mut dacl: *mut windows::Win32::Security::ACL = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        let result = unsafe {
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut dacl),
                None,
                Some(&mut sd),
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        if result != windows::Win32::Foundation::WIN32_ERROR(0) {
            return Err(RuntimeHomeError::VerificationFailed);
        }
        let mut control = 0u16;
        let mut revision = 0u32;
        unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) }
            .map_err(|_| RuntimeHomeError::VerificationFailed)?;
        if control & SE_DACL_PROTECTED.0 == 0 {
            return Err(RuntimeHomeError::VerificationFailed);
        }
        let mut info = ACL_SIZE_INFORMATION::default();
        unsafe {
            GetAclInformation(
                dacl,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        }
        .map_err(|_| RuntimeHomeError::VerificationFailed)?;
        if info.AceCount != 2 {
            return Err(RuntimeHomeError::VerificationFailed);
        }
        unsafe { LocalFree(HLOCAL(sd.0 as _)) };
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn verify_protected_directory(path: &Path) -> Result<(), RuntimeHomeError> {
        verify_restricted_directory(path)
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub(crate) fn verify_protected_directory(_path: &Path) -> Result<(), RuntimeHomeError> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn reject_symlinked_ancestors(path: &Path) -> Result<(), RuntimeHomeError> {
    for ancestor in path.ancestors().filter(|path| !path.as_os_str().is_empty()) {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RuntimeHomeError::ReparsePointRejected);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(RuntimeHomeError::Io),
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn reject_symlinked_ancestors(_path: &Path) -> Result<(), RuntimeHomeError> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn ensure_restricted_directory(path: &Path) -> Result<(), RuntimeHomeError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    reject_symlinked_ancestors(path)?;
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        match std::fs::symlink_metadata(cursor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(RuntimeHomeError::ReparsePointRejected);
                }
                if !metadata.is_dir() {
                    return Err(RuntimeHomeError::VerificationFailed);
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(cursor.to_path_buf());
                cursor = cursor.parent().ok_or(RuntimeHomeError::Io)?;
            }
            Err(_) => return Err(RuntimeHomeError::Io),
        }
    }
    for directory in missing.into_iter().rev() {
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(&directory)
            .map_err(|_| RuntimeHomeError::Io)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| RuntimeHomeError::AclFailed)?;
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| RuntimeHomeError::AclFailed)?;
    verify_restricted_directory(path)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn ensure_restricted_directory(path: &Path) -> Result<(), RuntimeHomeError> {
    std::fs::create_dir_all(path).map_err(|_| RuntimeHomeError::Io)
}

#[cfg(target_os = "linux")]
pub(crate) fn verify_restricted_directory(path: &Path) -> Result<(), RuntimeHomeError> {
    use std::os::unix::fs::PermissionsExt;

    reject_symlinked_ancestors(path)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| RuntimeHomeError::Io)?;
    if metadata.file_type().is_symlink() {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    if !metadata.is_dir() || metadata.permissions().mode() & 0o777 != 0o700 {
        return Err(RuntimeHomeError::VerificationFailed);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn verify_restricted_directory(_path: &Path) -> Result<(), RuntimeHomeError> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn verify_restricted_file(path: &Path) -> Result<(), RuntimeHomeError> {
    use std::os::unix::fs::PermissionsExt;

    reject_symlinked_ancestors(path)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| RuntimeHomeError::Io)?;
    if metadata.file_type().is_symlink() {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(RuntimeHomeError::VerificationFailed);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn verify_restricted_file(_path: &Path) -> Result<(), RuntimeHomeError> {
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn write_restricted_file(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), RuntimeHomeError> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let path = path.as_ref();
    let parent = path.parent().ok_or(RuntimeHomeError::Io)?;
    verify_restricted_directory(parent)?;
    reject_symlinked_ancestors(path)?;
    let temporary = parent.join(format!(".codexbar-private-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| RuntimeHomeError::Io)?;
        file.write_all(contents.as_ref())
            .map_err(|_| RuntimeHomeError::Io)?;
        file.flush().map_err(|_| RuntimeHomeError::Io)?;
        file.sync_all().map_err(|_| RuntimeHomeError::Io)?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| RuntimeHomeError::AclFailed)?;
        verify_restricted_file(&temporary)?;
        reject_symlinked_ancestors(parent)?;
        std::fs::rename(&temporary, path).map_err(|_| RuntimeHomeError::Io)?;
        verify_restricted_file(path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn write_restricted_file(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), RuntimeHomeError> {
    std::fs::write(path, contents).map_err(|_| RuntimeHomeError::Io)
}

pub(crate) fn remove_tree_no_follow(path: &Path) -> Result<(), RuntimeHomeError> {
    reject_symlinked_ancestors(path)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| RuntimeHomeError::Io)?;
    if super::windows_acl::is_reparse_point(&metadata) {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|_| RuntimeHomeError::Io)? {
            let entry = entry.map_err(|_| RuntimeHomeError::Io)?;
            remove_tree_no_follow(&entry.path())?;
        }
        std::fs::remove_dir(path).map_err(|_| RuntimeHomeError::Io)
    } else {
        std::fs::remove_file(path).map_err(|_| RuntimeHomeError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::secret_bytes::SensitiveBytes;
    use crate::accounts::vault::{CredentialFile, PrivateProfileMetadata};

    fn manager() -> RuntimeHomeManager {
        let dir = tempfile::tempdir().unwrap();
        RuntimeHomeManager::new(dir.path().join("runtime"))
    }

    #[test]
    fn managed_homes_are_distinct_and_force_file_auth() {
        let a = manager().prepare_new(Uuid::nil()).unwrap();
        let b = manager().prepare_new(Uuid::new_v4()).unwrap();
        assert_ne!(a.codex_home(), b.codex_home());
        assert_eq!(
            std::fs::read_to_string(a.codex_home().join("config.toml")).unwrap(),
            "cli_auth_credentials_store = \"file\"\n"
        );
    }

    #[test]
    fn restore_failure_removes_partial_credential_runtime() {
        let root = tempfile::tempdir().unwrap();
        let manager = RuntimeHomeManager::new(root.path().join("runtime"));
        let bundle = ManagedCredentialBundle {
            files: vec![
                CredentialFile {
                    relative_path: "auth.json".to_string(),
                    contents: SensitiveBytes::new(b"secret".to_vec()),
                },
                CredentialFile {
                    relative_path: "../escape".to_string(),
                    contents: SensitiveBytes::new(b"bad".to_vec()),
                },
            ],
            private_metadata: PrivateProfileMetadata {
                email: None,
                plan_type: None,
                auth_mode: crate::core::AuthMode::ChatGpt,
            },
        };

        assert!(manager.restore(Uuid::nil(), &bundle, 1).is_err());
        let profile_root = root.path().join("runtime").join(Uuid::nil().to_string());
        assert!(
            std::fs::read_dir(profile_root).map_or(true, |mut entries| entries.next().is_none())
        );
    }

    #[cfg(target_os = "linux")]
    fn mode(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::symlink_metadata(path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runtime_directories_and_files_are_owner_only() {
        let root = tempfile::tempdir().unwrap();
        let runtime_root = root.path().join("runtime");
        let profile_id = Uuid::new_v4();
        let manager = RuntimeHomeManager::new(runtime_root.clone());

        let home = manager.prepare_new(profile_id).unwrap();

        assert_eq!(mode(&runtime_root), 0o700);
        assert_eq!(mode(&runtime_root.join(profile_id.to_string())), 0o700);
        assert_eq!(mode(home.codex_home()), 0o700);
        assert_eq!(mode(&home.codex_home().join("config.toml")), 0o600);
        assert_eq!(mode(&home.codex_home().join("manifest.json")), 0o600);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_restore_makes_nested_credentials_private_and_rejects_loose_modes() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let manager = RuntimeHomeManager::new(root.path().join("runtime"));
        let profile_id = Uuid::new_v4();
        let bundle = ManagedCredentialBundle {
            files: vec![CredentialFile {
                relative_path: "nested/auth.json".to_string(),
                contents: SensitiveBytes::new(b"secret".to_vec()),
            }],
            private_metadata: PrivateProfileMetadata {
                email: None,
                plan_type: None,
                auth_mode: crate::core::AuthMode::ChatGpt,
            },
        };
        let home = manager.restore(profile_id, &bundle, 1).unwrap();
        let nested = home.codex_home().join("nested");
        let auth = nested.join("auth.json");
        assert_eq!(mode(&nested), 0o700);
        assert_eq!(mode(&auth), 0o600);

        std::fs::set_permissions(&auth, std::fs::Permissions::from_mode(0o644)).unwrap();
        let error =
            match crate::accounts::credential_bundle::collect_bundle(home.codex_home(), profile_id)
            {
                Ok(_) => panic!("loose credential mode was accepted"),
                Err(error) => error,
            };
        assert_eq!(error, RuntimeHomeError::VerificationFailed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runtime_creation_rejects_a_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let linked = root.path().join("linked");
        symlink(&real, &linked).unwrap();
        let manager = RuntimeHomeManager::new(linked.join("runtime"));

        let error = match manager.prepare_new(Uuid::new_v4()) {
            Ok(_) => panic!("symlinked runtime ancestor was accepted"),
            Err(error) => error,
        };
        assert_eq!(error, RuntimeHomeError::ReparsePointRejected);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_cleanup_does_not_follow_a_symlinked_profile_ancestor() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside_profile = root.path().join("outside-profile");
        let outside_session = outside_profile.join("session");
        std::fs::create_dir_all(&outside_session).unwrap();
        let outside_secret = outside_session.join("auth.json");
        std::fs::write(&outside_secret, b"secret").unwrap();
        let linked_profile = root.path().join("linked-profile");
        symlink(&outside_profile, &linked_profile).unwrap();

        assert_eq!(
            remove_tree_no_follow(&linked_profile.join("session")),
            Err(RuntimeHomeError::ReparsePointRejected)
        );
        assert!(outside_secret.exists());
    }
}
