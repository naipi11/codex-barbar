//! Atomic vault publishing and recovery (implemented in Task 3).

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::core::ProfileId;

use super::crypto::{CredentialProtector, VaultError};
use super::envelope::{ManagedCredentialBundle, VaultEnvelope};

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
        let existing = self.read_envelope(&final_path)?;
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
        self.write_temp(&temp_path, json.as_bytes())?;
        #[cfg(test)]
        self.maybe_fail(VaultWriteStep::TempFlushed)?;

        let publish_result = self.publish(&temp_path, &final_path, &backup_path);
        #[cfg(test)]
        self.maybe_fail(VaultWriteStep::Published)?;
        publish_result?;

        let written = self.read_envelope(&final_path)?.ok_or(VaultError::Io)?;
        if written.profile_id != profile_id || written.generation != next_generation {
            return Err(VaultError::InvalidEnvelope);
        }
        #[cfg(test)]
        self.maybe_fail(VaultWriteStep::FinalVerified)?;
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
        let _ = std::fs::remove_file(self.final_path(profile_id));
        if let Some(temp) = self.find_temp(profile_id) {
            let _ = std::fs::remove_file(temp);
        }
        let _ = std::fs::remove_file(self.backup_path(profile_id));
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

    fn read_envelope(&self, path: &std::path::Path) -> Result<Option<VaultEnvelope>, VaultError> {
        let Ok(contents) = std::fs::read(path) else {
            return Ok(None);
        };
        let envelope: VaultEnvelope =
            serde_json::from_slice(&contents).map_err(|_| VaultError::InvalidEnvelope)?;
        envelope.validate()?;
        Ok(Some(envelope))
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
#[derive(Default)]
pub struct TestProtector {
    #[allow(dead_code)]
    pub calls: std::sync::atomic::AtomicUsize,
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
        // Crash after ReplaceFileW but before backup deletion: final is the
        // new generation and the previous generation remains in the backup.
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
        let final_path = vault.final_path(Uuid::nil());
        std::fs::write(&final_path, b"corrupt").unwrap();
        let actions = vault.recover_atomic_artifacts().unwrap();
        assert!(actions.contains(&VaultRecovery::RestoredBackup));
        let info = vault.inspect(Uuid::nil()).unwrap().unwrap();
        assert_eq!(info.generation, gen1.generation);
    }
}
