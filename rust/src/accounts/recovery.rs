//! Startup recovery for interrupted runtime and Vault lifecycle stages.
//!
//! The runtime manifest is intentionally token-free: it stores only profile
//! and session identity plus lifecycle state. Paths are always derived from
//! the trusted runtime root and the manifest session ID; reparse points and
//! out-of-root paths are never traversed.

use uuid::Uuid;

use crate::accounts::vault::CredentialVault;
use crate::core::ProfileId;

use super::credential_bundle::collect_bundle;
use super::runtime_home::{
    RuntimeHomeError, RuntimeHomeManager, RuntimeState, remove_tree_no_follow,
};
use super::windows_acl::is_reparse_point;

pub const RUNTIME_MANIFEST_FORMAT: &str = "codex-barbar-runtime";
pub const RUNTIME_MANIFEST_VERSION: u32 = 1;

/// Token-free manifest persisted inside each managed runtime session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub format: String,
    pub version: u32,
    pub session_id: Uuid,
    pub profile_id: ProfileId,
    pub base_vault_generation: Option<u64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub state: RuntimeState,
}

/// What recovery did for one interrupted session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryActionTaken {
    Cleaned,
    KeptVault,
    Resealed,
}

/// Redacted outcome for one recovered runtime session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOutcome {
    pub profile_id: ProfileId,
    pub session_id: Uuid,
    pub action: RecoveryActionTaken,
}

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("runtime recovery failed")]
    Runtime(#[from] RuntimeHomeError),
    #[error("vault recovery failed")]
    Vault(#[from] crate::accounts::vault::crypto::VaultError),
}

pub struct AccountRecovery {
    runtime_homes: RuntimeHomeManager,
    vault: std::sync::Arc<CredentialVault>,
}

impl AccountRecovery {
    pub fn new(runtime_homes: RuntimeHomeManager, vault: std::sync::Arc<CredentialVault>) -> Self {
        Self {
            runtime_homes,
            vault,
        }
    }

    /// Recover every interrupted runtime session:
    ///
    /// - A newer final Vault wins over an older runtime.
    /// - Active/ReadyToSeal sessions at the same base generation are sealed
    ///   as `base + 1`; a new pending session may seal generation 1.
    /// - Invalid credentials never overwrite a valid Vault.
    /// - Preparing/LoggingIn sessions are cleaned without sealing.
    pub fn recover(&self) -> Result<Vec<RecoveryOutcome>, RecoveryError> {
        let mut outcomes = Vec::new();
        for candidate in self.runtime_homes.scan_recovery_candidates()? {
            let metadata = std::fs::symlink_metadata(&candidate.codex_home)
                .map_err(|_| RuntimeHomeError::Io)?;
            if is_reparse_point(&metadata) {
                return Err(RuntimeHomeError::ReparsePointRejected.into());
            }

            let final_generation = self
                .vault
                .inspect(candidate.profile_id)?
                .map(|info| info.generation);
            let bundle_result = collect_bundle(&candidate.codex_home, candidate.profile_id);
            // A bundle with no non-empty credential files is invalid and must
            // never overwrite a valid Vault.
            let valid_bundle = bundle_result
                .ok()
                .filter(|bundle| bundle.files.iter().any(|file| !file.contents.is_empty()));

            let action = match candidate.base_vault_generation {
                Some(base) => match final_generation {
                    Some(generation) if generation > base => RecoveryActionTaken::KeptVault,
                    Some(generation) if generation == base => {
                        if matches!(
                            candidate.state,
                            RuntimeState::Active | RuntimeState::ReadyToSeal
                        ) && let Some(mut bundle) = valid_bundle
                        {
                            match self.vault.seal_expected(
                                candidate.profile_id,
                                Some(base),
                                &mut bundle,
                            ) {
                                Ok(_) => RecoveryActionTaken::Resealed,
                                Err(_) => RecoveryActionTaken::KeptVault,
                            }
                        } else {
                            RecoveryActionTaken::KeptVault
                        }
                    }
                    _ => RecoveryActionTaken::KeptVault,
                },
                None => {
                    if matches!(
                        candidate.state,
                        RuntimeState::Active | RuntimeState::ReadyToSeal
                    ) && final_generation.is_none()
                        && let Some(mut bundle) = valid_bundle
                    {
                        match self
                            .vault
                            .seal_expected(candidate.profile_id, Some(0), &mut bundle)
                        {
                            Ok(_) => RecoveryActionTaken::Resealed,
                            Err(_) => RecoveryActionTaken::KeptVault,
                        }
                    } else {
                        RecoveryActionTaken::KeptVault
                    }
                }
            };

            // Runtime directories are cleaned in every terminal state; a
            // failed cleanup must not destroy a valid Vault.
            remove_tree_no_follow(&candidate.codex_home)?;
            outcomes.push(RecoveryOutcome {
                profile_id: candidate.profile_id,
                session_id: candidate.session_id,
                action,
            });
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::accounts::runtime_home::RuntimeHomeManager;
    use crate::accounts::secret_bytes::SensitiveBytes;
    use crate::accounts::vault::CredentialVault;
    use crate::accounts::vault::envelope::{
        CredentialFile, ManagedCredentialBundle, PrivateProfileMetadata,
    };
    use crate::accounts::vault::store::TestProtector;
    use crate::core::AuthMode;

    use super::*;

    const TEST_REFRESH_TOKEN: &[u8] = b"refresh-token-that-must-never-leak";

    struct RecoveryFixture {
        _dir: TempDir,
        runtime_root: PathBuf,
        vault_root: PathBuf,
        vault: std::sync::Arc<CredentialVault>,
        recovery: AccountRecovery,
    }

    impl RecoveryFixture {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let runtime_root = dir.path().join("runtime");
            let vault_root = dir.path().join("vault");
            let protector = Arc::new(TestProtector::default());
            let vault = std::sync::Arc::new(CredentialVault::new(vault_root.clone(), protector));
            let manager = RuntimeHomeManager::new(runtime_root.clone());
            let recovery = AccountRecovery::new(manager, vault.clone());
            Self {
                _dir: dir,
                runtime_root,
                vault_root,
                vault,
                recovery,
            }
        }

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

        fn seal_generation_one(&self) -> u64 {
            let mut bundle = Self::bundle(b"first");
            self.vault
                .seal_expected(Uuid::nil(), None, &mut bundle)
                .unwrap()
                .generation
        }

        fn active_runtime(
            &self,
            base_generation: u64,
        ) -> crate::accounts::runtime_home::ManagedRuntimeHome {
            let manager = RuntimeHomeManager::new(self.runtime_root.clone());
            let bundle = Self::bundle(b"first");
            let runtime = manager
                .restore(Uuid::nil(), &bundle, base_generation)
                .unwrap();
            std::fs::write(runtime.codex_home().join("auth.json"), TEST_REFRESH_TOKEN).unwrap();
            runtime.set_state(RuntimeState::Active).unwrap();
            runtime
        }

        fn read_final_vault_bytes(&self) -> Vec<u8> {
            std::fs::read(
                self.vault_root
                    .join("00000000-0000-0000-0000-000000000000.dpapi"),
            )
            .unwrap()
        }

        fn create_invalid_active_runtime(&self) {
            let manager = RuntimeHomeManager::new(self.runtime_root.clone());
            let bundle = Self::bundle(b"first");
            let runtime = manager.restore(Uuid::nil(), &bundle, 1).unwrap();
            // Empty credentials are invalid and must not overwrite the Vault.
            std::fs::write(runtime.codex_home().join("auth.json"), b"").unwrap();
            runtime.set_state(RuntimeState::Active).unwrap();
        }
    }

    #[test]
    fn recovery_seals_newer_runtime_without_losing_old_vault() {
        let fixture = RecoveryFixture::new();
        let old = fixture.seal_generation_one();
        let runtime = fixture.active_runtime(old);
        let report = fixture.recovery.recover().unwrap();
        assert_eq!(
            fixture.vault.unseal(Uuid::nil()).unwrap().1.generation,
            old + 1
        );
        assert!(!runtime.codex_home().exists());
        assert_eq!(
            report
                .iter()
                .filter(|outcome| outcome.action == RecoveryActionTaken::Resealed)
                .count(),
            1
        );
    }

    #[test]
    fn invalid_runtime_never_overwrites_valid_vault() {
        let fixture = RecoveryFixture::new();
        fixture.seal_generation_one();
        let before = fixture.read_final_vault_bytes();
        fixture.create_invalid_active_runtime();
        fixture.recovery.recover().unwrap();
        assert_eq!(fixture.read_final_vault_bytes(), before);
    }
}
