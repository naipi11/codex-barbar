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
        std::fs::write(
            self.codex_home.join("manifest.json"),
            serde_json::to_string(&manifest).map_err(|_| RuntimeHomeError::Io)?,
        )
        .map_err(|_| RuntimeHomeError::Io)
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
        std::fs::write(
            codex_home.join("config.toml"),
            "cli_auth_credentials_store = \"file\"\n",
        )
        .map_err(|_| RuntimeHomeError::Io)?;
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
            std::fs::write(
                codex_home.join("config.toml"),
                "cli_auth_credentials_store = \"file\"\n",
            )
            .map_err(|_| RuntimeHomeError::Io)?;
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
        let entries = std::fs::read_dir(&self.runtime_root).map_err(|_| RuntimeHomeError::Io)?;
        for entry in entries.flatten() {
            let profile_dir = entry.path();
            if !profile_dir.is_dir() {
                continue;
            }
            let sessions = std::fs::read_dir(&profile_dir).map_err(|_| RuntimeHomeError::Io)?;
            for session in sessions.flatten() {
                let codex_home = session.path();
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

    #[cfg(not(windows))]
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

    #[cfg(not(windows))]
    pub(crate) fn verify_protected_directory(_path: &Path) -> Result<(), RuntimeHomeError> {
        Ok(())
    }
}

pub(crate) fn remove_tree_no_follow(path: &Path) -> Result<(), RuntimeHomeError> {
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
}
