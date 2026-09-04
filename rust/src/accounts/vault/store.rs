//! Atomic vault publishing and recovery (implemented in Task 3).

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::core::ProfileId;

use super::crypto::{CredentialProtector, VaultError};
use super::envelope::{ManagedCredentialBundle, VaultEnvelope};

fn remove_file_if_present(path: &std::path::Path) -> Result<(), VaultError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(VaultError::Io),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultInfo {
    pub profile_id: ProfileId,
    pub generation: u64,
    pub sealed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultRecovery {
    KeptFinal,
    PromotedTemp,
    RestoredBackup,
}

pub struct CredentialVault {
    vault_root: PathBuf,
    protector: Arc<dyn CredentialProtector>,
    #[cfg(test)]
    fault: std::sync::Mutex<Option<VaultWriteStep>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultWriteStep {
    TempFlushed,
    Published,
    FinalVerified,
}

impl CredentialVault {
    pub fn new(vault_root: PathBuf, protector: Arc<dyn CredentialProtector>) -> Self {
        Self {
            vault_root,
            protector,
            #[cfg(test)]
            fault: std::sync::Mutex::new(None),
        }
    }

    pub fn seal_expected(
        &self,
        profile_id: ProfileId,
        expected_generation: Option<u64>,
        bundle: &mut ManagedCredentialBundle,
    ) -> Result<VaultInfo, VaultError> {
        let final_path = self.final_path(profile_id);
        let previous_raw = std::fs::read(&final_path).ok();
        let existing = self.read_envelope_for_seal(&final_path, profile_id, expected_generation)?;
        let previous_secret = match self.read_envelope(&final_path) {
            Ok(Some(envelope)) => {
                let ciphertext = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &envelope.ciphertext_base64,
                )
                .map_err(|_| VaultError::InvalidEnvelope)?;
                Some(
                    self.protector
                        .unprotect_current_user(profile_id, &ciphertext)?,
                )
            }
            Ok(None) | Err(VaultError::InvalidEnvelope) => None,
            Err(error) => return Err(error),
        };
        let next_generation = match (expected_generation, existing.as_ref()) {
            (Some(expected), Some(envelope)) if envelope.generation == expected => expected + 1,
            (Some(0), None) => 1,
            (Some(_), _) => return Err(VaultError::GenerationConflict),
            (None, Some(envelope)) => envelope.generation + 1,
            (None, None) => 1,
        };

        let plaintext = super::envelope::encode_bundle(bundle)?;
        let ciphertext = self
            .protector
            .protect_current_user(profile_id, plaintext.as_slice())?;
        let envelope = VaultEnvelope::new(profile_id, next_generation, Utc::now(), &ciphertext);
        let json = serde_json::to_string(&envelope).map_err(|_| VaultError::Io)?;

        let temp_path = self.random_temp_path(profile_id);
        let backup_path = self.backup_path(profile_id);
        if let Err(error) = self.write_temp(&temp_path, json.as_bytes()) {
            let rollback_error = self
                .rollback_after_failure(
                    profile_id,
                    previous_raw.as_deref(),
                    previous_secret.as_ref(),
                )
                .err();
            let _ = std::fs::remove_file(&temp_path);
            return Err(rollback_error.unwrap_or(error));
        }
        #[cfg(test)]
        if let Err(error) = self.maybe_fail(VaultWriteStep::TempFlushed) {
            let rollback_error = self
                .rollback_after_failure(
                    profile_id,
                    previous_raw.as_deref(),
                    previous_secret.as_ref(),
                )
                .err();
            let _ = std::fs::remove_file(&temp_path);
            return Err(rollback_error.unwrap_or(error));
        }

        let publish_result = self.publish(&temp_path, &final_path, &backup_path);
        #[cfg(test)]
        let publish_result =
            publish_result.and_then(|_| self.maybe_fail(VaultWriteStep::Published));
        if let Err(error) = publish_result {
            let rollback_error = self
                .rollback_after_failure(
                    profile_id,
                    previous_raw.as_deref(),
                    previous_secret.as_ref(),
                )
                .err();
            let _ = std::fs::remove_file(&temp_path);
            return Err(rollback_error.unwrap_or(error));
        }

        let written = match self.read_envelope(&final_path)?.ok_or(VaultError::Io) {
            Ok(value) => value,
            Err(error) => {
                let rollback_error = self
                    .rollback_after_failure(
                        profile_id,
                        previous_raw.as_deref(),
                        previous_secret.as_ref(),
                    )
                    .err();
                return Err(rollback_error.unwrap_or(error));
            }
        };
        if written.profile_id != profile_id || written.generation != next_generation {
            let rollback_error = self
                .rollback_after_failure(
                    profile_id,
                    previous_raw.as_deref(),
                    previous_secret.as_ref(),
                )
                .err();
            return Err(rollback_error.unwrap_or(VaultError::InvalidEnvelope));
        }
        #[cfg(test)]
        if let Err(error) = self.maybe_fail(VaultWriteStep::FinalVerified) {
            let rollback_error = self
                .rollback_after_failure(
                    profile_id,
                    previous_raw.as_deref(),
                    previous_secret.as_ref(),
                )
                .err();
            return Err(rollback_error.unwrap_or(error));
        }
        let _ = std::fs::remove_file(&backup_path);
        Ok(VaultInfo {
            profile_id,
            generation: next_generation,
            sealed_at: written.sealed_at,
        })
    }

    pub fn unseal(
        &self,
        profile_id: ProfileId,
    ) -> Result<(ManagedCredentialBundle, VaultInfo), VaultError> {
        let envelope = self
            .read_envelope(&self.final_path(profile_id))?
            .ok_or(VaultError::InvalidEnvelope)?;
        if envelope.profile_id != profile_id {
            return Err(VaultError::WrongProfile);
        }
        let ciphertext = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &envelope.ciphertext_base64,
        )
        .map_err(|_| VaultError::InvalidEnvelope)?;
        let plaintext = self
            .protector
            .unprotect_current_user(profile_id, &ciphertext)?;
        let bundle = super::envelope::decode_bundle(plaintext.as_slice())?;
        Ok((
            bundle,
            VaultInfo {
                profile_id,
                generation: envelope.generation,
                sealed_at: envelope.sealed_at,
            },
        ))
    }

    pub fn inspect(&self, profile_id: ProfileId) -> Result<Option<VaultInfo>, VaultError> {
        Ok(self
            .read_envelope(&self.final_path(profile_id))?
            .map(|envelope| VaultInfo {
                profile_id: envelope.profile_id,
                generation: envelope.generation,
                sealed_at: envelope.sealed_at,
            }))
    }

    /// Return the generation of a legacy envelope that this platform cannot
    /// unseal, allowing an interactive replacement login to advance it safely.
    #[cfg(target_os = "linux")]
    pub fn legacy_replacement_generation(
        &self,
        profile_id: ProfileId,
    ) -> Result<Option<u64>, VaultError> {
        let path = self.final_path(profile_id);
        let Ok(contents) = std::fs::read(path) else {
            return Ok(None);
        };
        let Ok(envelope) = serde_json::from_slice::<VaultEnvelope>(&contents) else {
            return Ok(None);
        };
        if envelope.format == super::envelope::VAULT_FORMAT
            && envelope.version == super::envelope::VAULT_VERSION
            && envelope.protection == "windows-dpapi-current-user"
            && envelope.profile_id == profile_id
            && base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &envelope.ciphertext_base64,
            )
            .is_ok_and(|bytes| !bytes.is_empty())
        {
            Ok(Some(envelope.generation))
        } else {
            Ok(None)
        }
    }

    pub fn recover_atomic_artifacts(&self) -> Result<Vec<VaultRecovery>, VaultError> {
        let mut actions = Vec::new();
        let mut profiles = std::collections::BTreeSet::new();
        if let Ok(entries) = std::fs::read_dir(&self.vault_root) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    let stem = name
                        .strip_suffix(".dpapi")
                        .or_else(|| name.strip_suffix(".bak"))
                        .or_else(|| name.split_once(".tmp.").map(|(head, _)| head));
                    if let Some(stem) = stem
                        && let Ok(profile_id) = Uuid::parse_str(stem)
                    {
                        profiles.insert(profile_id);
                    }
                }
            }
        }
        for profile_id in profiles {
            let final_path = self.final_path(profile_id);
            let temp_path = self.find_temp(profile_id);
            let backup_path = self.backup_path(profile_id);
            let final_envelope = self.read_envelope_lenient(&final_path);
            if let Some(envelope) = final_envelope
                && envelope.profile_id == profile_id
            {
                actions.push(VaultRecovery::KeptFinal);
                if let Some(temp) = &temp_path {
                    let _ = std::fs::remove_file(temp);
                }
                let _ = std::fs::remove_file(&backup_path);
                continue;
            }
            if let Some(temp_path) = &temp_path
                && let Some(temp_envelope) = self.read_envelope_lenient(temp_path)
                && temp_envelope.profile_id == profile_id
            {
                self.publish(temp_path, &final_path, &backup_path)
                    .map_err(|_| VaultError::Io)?;
                actions.push(VaultRecovery::PromotedTemp);
                continue;
            }
            if let Some(backup_envelope) = self.read_envelope_lenient(&backup_path)
                && backup_envelope.profile_id == profile_id
            {
                std::fs::copy(&backup_path, &final_path).map_err(|_| VaultError::Io)?;
                actions.push(VaultRecovery::RestoredBackup);
                let _ = std::fs::remove_file(&backup_path);
                continue;
            }
            return Err(VaultError::Io);
        }
        Ok(actions)
    }

    pub fn remove(&self, profile_id: ProfileId) -> Result<(), VaultError> {
        self.protector.remove_current_user(profile_id)?;
        remove_file_if_present(&self.final_path(profile_id))?;
        for temp in self.find_temps_checked(profile_id)? {
            remove_file_if_present(&temp)?;
        }
        remove_file_if_present(&self.backup_path(profile_id))?;
        Ok(())
    }

    fn final_path(&self, profile_id: ProfileId) -> PathBuf {
        self.vault_root.join(format!("{profile_id}.dpapi"))
    }

    fn backup_path(&self, profile_id: ProfileId) -> PathBuf {
        self.vault_root.join(format!("{profile_id}.bak"))
    }

    fn random_temp_path(&self, profile_id: ProfileId) -> PathBuf {
        self.vault_root
            .join(format!("{profile_id}.tmp.{}", Uuid::new_v4()))
    }

    fn find_temp(&self, profile_id: ProfileId) -> Option<PathBuf> {
        let prefix = format!("{profile_id}.tmp.");
        let entries = std::fs::read_dir(&self.vault_root).ok()?;
        entries.flatten().map(|entry| entry.path()).find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
    }

    fn find_temps_checked(&self, profile_id: ProfileId) -> Result<Vec<PathBuf>, VaultError> {
        let prefix = format!("{profile_id}.tmp.");
        let entries = match std::fs::read_dir(&self.vault_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(VaultError::Io),
        };
        let mut matches = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| VaultError::Io)?;
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
            {
                matches.push(path);
            }
        }
        Ok(matches)
    }

    fn read_envelope(&self, path: &std::path::Path) -> Result<Option<VaultEnvelope>, VaultError> {
        let Ok(contents) = std::fs::read(path) else {
            return Ok(None);
        };
        let envelope: VaultEnvelope =
            serde_json::from_slice(&contents).map_err(|_| VaultError::InvalidEnvelope)?;
        envelope.validate()?;
        Ok(Some(envelope))
    }

    fn read_envelope_for_seal(
        &self,
        path: &std::path::Path,
        profile_id: ProfileId,
        expected_generation: Option<u64>,
    ) -> Result<Option<VaultEnvelope>, VaultError> {
        #[cfg(not(target_os = "linux"))]
        let _ = (profile_id, expected_generation);
        match self.read_envelope(path) {
            Ok(envelope) => Ok(envelope),
            Err(VaultError::InvalidEnvelope) => {
                #[cfg(target_os = "linux")]
                {
                    let Ok(contents) = std::fs::read(path) else {
                        return Ok(None);
                    };
                    let Ok(envelope) = serde_json::from_slice::<VaultEnvelope>(&contents) else {
                        return Err(VaultError::InvalidEnvelope);
                    };
                    if envelope.format == super::envelope::VAULT_FORMAT
                        && envelope.version == super::envelope::VAULT_VERSION
                        && envelope.protection == "windows-dpapi-current-user"
                        && envelope.profile_id == profile_id
                        && expected_generation == Some(envelope.generation)
                        && base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            &envelope.ciphertext_base64,
                        )
                        .is_ok_and(|bytes| !bytes.is_empty())
                    {
                        return Ok(Some(envelope));
                    }
                }
                Err(VaultError::InvalidEnvelope)
            }
            Err(error) => Err(error),
        }
    }

    fn rollback_after_failure(
        &self,
        profile_id: ProfileId,
        previous_raw: Option<&[u8]>,
        previous_secret: Option<&crate::accounts::secret_bytes::SensitiveBytes>,
    ) -> Result<(), VaultError> {
        let mut failure = None;
        match previous_secret {
            Some(secret) => {
                if let Err(error) = self
                    .protector
                    .protect_current_user(profile_id, secret.as_slice())
                {
                    failure = Some(error);
                }
            }
            None => {
                if let Err(error) = self.protector.remove_current_user(profile_id) {
                    failure = Some(error);
                }
            }
        }
        match previous_raw {
            Some(raw) => {
                if std::fs::write(self.final_path(profile_id), raw).is_err() {
                    failure = Some(VaultError::Io);
                }
            }
            None => {
                if let Err(error) = remove_file_if_present(&self.final_path(profile_id)) {
                    failure = Some(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn read_envelope_lenient(&self, path: &std::path::Path) -> Option<VaultEnvelope> {
        self.read_envelope(path).ok().flatten()
    }

    fn write_temp(&self, path: &std::path::Path, bytes: &[u8]) -> Result<(), VaultError> {
        std::fs::create_dir_all(&self.vault_root).map_err(|_| VaultError::Io)?;
        let mut file = std::fs::File::create(path).map_err(|_| VaultError::Io)?;
        std::io::Write::write_all(&mut file, bytes).map_err(|_| VaultError::Io)?;
        std::io::Write::flush(&mut file).map_err(|_| VaultError::Io)?;
        file.sync_all().map_err(|_| VaultError::Io)?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::Storage::FileSystem::FlushFileBuffers;
            let handle = HANDLE(file.as_raw_handle() as _);
            unsafe { FlushFileBuffers(handle) }.map_err(|_| VaultError::Io)?;
        }
        Ok(())
    }

    fn publish(
        &self,
        temp_path: &std::path::Path,
        final_path: &std::path::Path,
        backup_path: &std::path::Path,
    ) -> Result<(), VaultError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::Storage::FileSystem::{
                MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACEFILE_WRITE_THROUGH, ReplaceFileW,
            };
            use windows::core::PCWSTR;

            fn wide(path: &std::path::Path) -> Vec<u16> {
                path.as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect()
            }
            let final_exists = final_path.exists();
            if final_exists {
                let _ = std::fs::remove_file(backup_path);
                let final_wide = wide(final_path);
                let temp_wide = wide(temp_path);
                let backup_wide = wide(backup_path);
                unsafe {
                    ReplaceFileW(
                        PCWSTR::from_raw(final_wide.as_ptr()),
                        PCWSTR::from_raw(temp_wide.as_ptr()),
                        PCWSTR::from_raw(backup_wide.as_ptr()),
                        REPLACEFILE_WRITE_THROUGH,
                        None,
                        None,
                    )
                }
                .map_err(|_| VaultError::Io)?;
            } else {
                let temp_wide = wide(temp_path);
                let final_wide = wide(final_path);
                unsafe {
                    MoveFileExW(
                        PCWSTR::from_raw(temp_wide.as_ptr()),
                        PCWSTR::from_raw(final_wide.as_ptr()),
                        MOVEFILE_WRITE_THROUGH,
                    )
                }
                .map_err(|_| VaultError::Io)?;
            }
        }
        #[cfg(not(windows))]
        {
            // Backups are a ReplaceFileW recovery mechanism; POSIX rename is atomic.
            let _ = backup_path;
            std::fs::rename(temp_path, final_path).map_err(|_| VaultError::Io)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn maybe_fail(&self, step: VaultWriteStep) -> Result<(), VaultError> {
        let mut fault = self.fault.lock().unwrap();
        if *fault == Some(step) {
            *fault = None;
            return Err(VaultError::Io);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn test_vault_fixture() -> (CredentialVault, Arc<TestProtector>) {
    let dir = tempfile::tempdir().unwrap();
    let protector = Arc::new(TestProtector::default());
    let vault = CredentialVault::new(dir.path().to_path_buf(), protector.clone());
    (vault, protector)
}

#[cfg(test)]
pub(crate) fn test_vault_fixture_with_remove_tracking()
-> (CredentialVault, Arc<RemoveTrackingProtector>) {
    let dir = tempfile::tempdir().unwrap();
    let protector = Arc::new(RemoveTrackingProtector::default());
    let vault = CredentialVault::new(dir.path().to_path_buf(), protector.clone());
    (vault, protector)
}

#[cfg(test)]
#[derive(Default)]
pub struct TestProtector {
    #[allow(dead_code)]
    pub calls: std::sync::atomic::AtomicUsize,
}

#[cfg(all(test, target_os = "linux"))]
#[derive(Default)]
struct LinuxMarkerProtector;

#[cfg(all(test, target_os = "linux"))]
impl CredentialProtector for LinuxMarkerProtector {
    fn protect_current_user(
        &self,
        profile_id: ProfileId,
        _plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Ok(super::crypto::secret_service_marker(profile_id))
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        _ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        Err(VaultError::UnprotectFailed)
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct RemoveTrackingProtector {
    removed: std::sync::Mutex<Vec<ProfileId>>,
}

#[cfg(test)]
#[derive(Default)]
struct StatefulProtector {
    secret: std::sync::Mutex<Option<Vec<u8>>>,
}

#[cfg(test)]
struct FailingRemoveProtector {
    fail_once: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
struct RollbackFailProtector {
    calls: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl Default for RollbackFailProtector {
    fn default() -> Self {
        Self {
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
impl CredentialProtector for RollbackFailProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        _plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
        if call == 3 {
            Err(VaultError::SecretServiceLocked)
        } else {
            Ok(b"opaque-marker".to_vec())
        }
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        _ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        Ok(crate::accounts::secret_bytes::SensitiveBytes::new(
            b"previous".to_vec(),
        ))
    }
}

#[cfg(test)]
impl Default for FailingRemoveProtector {
    fn default() -> Self {
        Self {
            fail_once: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

#[cfg(test)]
impl CredentialProtector for FailingRemoveProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        Ok(crate::accounts::secret_bytes::SensitiveBytes::new(
            ciphertext.to_vec(),
        ))
    }

    fn remove_current_user(&self, _profile_id: ProfileId) -> Result<(), VaultError> {
        if self
            .fail_once
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            Err(VaultError::SecretServiceUnavailable)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
impl CredentialProtector for StatefulProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        *self.secret.lock().unwrap() = Some(plaintext.to_vec());
        Ok(b"opaque-marker".to_vec())
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        _ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        self.secret
            .lock()
            .unwrap()
            .clone()
            .map(crate::accounts::secret_bytes::SensitiveBytes::new)
            .ok_or(VaultError::UnprotectFailed)
    }

    fn remove_current_user(&self, _profile_id: ProfileId) -> Result<(), VaultError> {
        *self.secret.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
impl RemoveTrackingProtector {
    pub fn removed_profiles(&self) -> Vec<ProfileId> {
        self.removed.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl CredentialProtector for RemoveTrackingProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        Ok(crate::accounts::secret_bytes::SensitiveBytes::new(
            ciphertext.to_vec(),
        ))
    }

    fn remove_current_user(&self, profile_id: ProfileId) -> Result<(), VaultError> {
        self.removed.lock().unwrap().push(profile_id);
        Ok(())
    }
}

#[cfg(test)]
impl CredentialProtector for TestProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(plaintext.to_vec())
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        ciphertext: &[u8],
    ) -> Result<crate::accounts::secret_bytes::SensitiveBytes, VaultError> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(crate::accounts::secret_bytes::SensitiveBytes::new(
            ciphertext.to_vec(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::secret_bytes::SensitiveBytes;
    use crate::accounts::vault::envelope::{CredentialFile, PrivateProfileMetadata};
    use crate::core::AuthMode;

    fn bundle(token: &[u8]) -> ManagedCredentialBundle {
        ManagedCredentialBundle {
            files: vec![CredentialFile {
                relative_path: "auth.json".to_string(),
                contents: SensitiveBytes::new(token.to_vec()),
            }],
            private_metadata: PrivateProfileMetadata {
                email: Some("user@example.com".to_string()),
                plan_type: Some("plus".to_string()),
                auth_mode: AuthMode::ChatGpt,
            },
        }
    }

    #[test]
    fn crash_after_temp_flush_preserves_old_final() {
        let (vault, _) = test_vault_fixture();
        let mut first = bundle(b"first");
        let old = vault.seal_expected(Uuid::nil(), None, &mut first).unwrap();
        vault
            .fault
            .lock()
            .unwrap()
            .replace(VaultWriteStep::TempFlushed);
        let mut second = bundle(b"second");
        assert!(
            vault
                .seal_expected(Uuid::nil(), Some(old.generation), &mut second)
                .is_err()
        );
        vault.recover_atomic_artifacts().unwrap();
        let info = vault.inspect(Uuid::nil()).unwrap().unwrap();
        assert_eq!(info.generation, old.generation);
    }

    #[test]
    fn corrupt_final_recovers_valid_backup() {
        let (vault, _) = test_vault_fixture();
        let mut first = bundle(b"first");
        let gen1 = vault.seal_expected(Uuid::nil(), None, &mut first).unwrap();
        let final_path = vault.final_path(Uuid::nil());
        let backup_path = vault.backup_path(Uuid::nil());
        std::fs::copy(&final_path, &backup_path).unwrap();
        std::fs::write(&final_path, b"corrupt").unwrap();

        let actions = vault.recover_atomic_artifacts().unwrap();
        assert!(actions.contains(&VaultRecovery::RestoredBackup));
        let info = vault.inspect(Uuid::nil()).unwrap().unwrap();
        assert_eq!(info.generation, gen1.generation);
    }

    #[cfg(windows)]
    #[test]
    fn published_vault_keeps_previous_generation_in_backup_until_commit() {
        let (vault, _) = test_vault_fixture();
        let mut first = bundle(b"first");
        let gen1 = vault.seal_expected(Uuid::nil(), None, &mut first).unwrap();

        // ReplaceFileW leaves the previous final at the backup path. Exercise
        // that Windows-specific crash window separately from portable recovery.
        vault
            .fault
            .lock()
            .unwrap()
            .replace(VaultWriteStep::Published);
        let mut second = bundle(b"second");
        assert!(
            vault
                .seal_expected(Uuid::nil(), Some(gen1.generation), &mut second)
                .is_err()
        );
        let backup = vault.backup_path(Uuid::nil());
        assert!(backup.is_file());
        assert_eq!(
            vault.read_envelope(&backup).unwrap().unwrap().generation,
            gen1.generation
        );
    }

    #[test]
    fn removing_a_vault_deletes_the_protector_entry() {
        let (vault, protector) = test_vault_fixture_with_remove_tracking();
        vault.remove(Uuid::nil()).unwrap();
        assert_eq!(protector.removed_profiles(), vec![Uuid::nil()]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_local_envelope_contains_only_opaque_marker() {
        let dir = tempfile::tempdir().unwrap();
        let protector = Arc::new(LinuxMarkerProtector);
        let vault = CredentialVault::new(dir.path().to_path_buf(), protector);
        let secret = b"sk-test-token-that-must-never-leak";
        let mut bundle = bundle(secret);
        vault.seal_expected(Uuid::nil(), None, &mut bundle).unwrap();

        let path = dir.path().join(format!("{}.dpapi", Uuid::nil()));
        let raw = std::fs::read_to_string(path).unwrap();
        assert!(!raw.contains("sk-test-token"));
        let envelope: VaultEnvelope = serde_json::from_str(&raw).unwrap();
        let payload = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            envelope.ciphertext_base64,
        )
        .unwrap();
        assert_eq!(
            payload,
            crate::accounts::vault::crypto::secret_service_marker(Uuid::nil())
        );
    }

    #[test]
    fn filesystem_failure_after_secret_update_rolls_back_keyring_secret() {
        let dir = tempfile::tempdir().unwrap();
        let protector = Arc::new(StatefulProtector::default());
        let vault = CredentialVault::new(dir.path().to_path_buf(), protector.clone());
        let mut first = bundle(b"first-secret");
        let first_info = vault.seal_expected(Uuid::nil(), None, &mut first).unwrap();
        vault
            .fault
            .lock()
            .unwrap()
            .replace(VaultWriteStep::TempFlushed);
        let mut second = bundle(b"second-secret");
        assert!(
            vault
                .seal_expected(Uuid::nil(), Some(first_info.generation), &mut second)
                .is_err()
        );
        let (restored, info) = vault.unseal(Uuid::nil()).unwrap();
        assert_eq!(info.generation, first_info.generation);
        assert_eq!(restored.files[0].contents.as_slice(), b"first-secret");
    }

    #[test]
    fn protector_removal_failure_keeps_local_envelope_for_retry() {
        let dir = tempfile::tempdir().unwrap();
        let vault = CredentialVault::new(
            dir.path().to_path_buf(),
            Arc::new(FailingRemoveProtector::default()),
        );
        let mut bundle = bundle(b"secret");
        vault.seal_expected(Uuid::nil(), None, &mut bundle).unwrap();
        assert!(vault.remove(Uuid::nil()).is_err());
        assert!(dir.path().join(format!("{}.dpapi", Uuid::nil())).exists());
        vault.remove(Uuid::nil()).unwrap();
        assert!(!dir.path().join(format!("{}.dpapi", Uuid::nil())).exists());
    }

    #[test]
    fn removing_a_vault_deletes_all_temp_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let vault =
            CredentialVault::new(dir.path().to_path_buf(), Arc::new(TestProtector::default()));
        std::fs::create_dir_all(dir.path()).unwrap();
        for suffix in ["one", "two"] {
            std::fs::write(
                dir.path().join(format!("{}.tmp.{suffix}", Uuid::nil())),
                b"stale",
            )
            .unwrap();
        }
        vault.remove(Uuid::nil()).unwrap();
        assert!(!dir.path().join(format!("{}.tmp.one", Uuid::nil())).exists());
        assert!(!dir.path().join(format!("{}.tmp.two", Uuid::nil())).exists());
    }

    #[test]
    fn rollback_failure_is_reported_instead_of_claiming_safe_commit() {
        let dir = tempfile::tempdir().unwrap();
        let protector = Arc::new(RollbackFailProtector::default());
        let vault = CredentialVault::new(dir.path().to_path_buf(), protector);
        let mut first = bundle(b"first");
        let info = vault.seal_expected(Uuid::nil(), None, &mut first).unwrap();
        vault
            .fault
            .lock()
            .unwrap()
            .replace(VaultWriteStep::TempFlushed);
        let mut second = bundle(b"second");
        assert_eq!(
            vault.seal_expected(Uuid::nil(), Some(info.generation), &mut second),
            Err(VaultError::SecretServiceLocked)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn corrupt_legacy_base64_cannot_enter_replacement_path() {
        let dir = tempfile::tempdir().unwrap();
        let vault =
            CredentialVault::new(dir.path().to_path_buf(), Arc::new(TestProtector::default()));
        let raw = serde_json::json!({
            "format": crate::accounts::vault::envelope::VAULT_FORMAT,
            "version": crate::accounts::vault::envelope::VAULT_VERSION,
            "protection": "windows-dpapi-current-user",
            "profile_id": Uuid::nil(),
            "generation": 1,
            "sealed_at": Utc::now(),
            "ciphertext_base64": "not-base64"
        });
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(
            dir.path().join(format!("{}.dpapi", Uuid::nil())),
            serde_json::to_vec(&raw).unwrap(),
        )
        .unwrap();
        assert_eq!(
            vault.legacy_replacement_generation(Uuid::nil()).unwrap(),
            None
        );
        let mut bundle = bundle(b"new");
        assert_eq!(
            vault.seal_expected(Uuid::nil(), Some(1), &mut bundle),
            Err(VaultError::InvalidEnvelope)
        );
    }
}
