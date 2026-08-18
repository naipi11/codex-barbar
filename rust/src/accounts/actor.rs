//! Single-operation owner for Managed login/refresh/switch/removal.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;

use crate::accounts::model::{ManagedLoginMethod, ManagedLoginStage, ManagedLoginStatus};
use crate::accounts::vault::{CredentialVault, ManagedCredentialBundle};
use crate::core::{AppError, ProfileId};

/// A single in-flight login operation tracked by the actor.
#[derive(Debug, Clone)]
pub struct ActiveLogin {
    pub operation_id: uuid::Uuid,
    pub profile_id: ProfileId,
    pub method: ManagedLoginMethod,
    pub stage: ManagedLoginStage,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
    pub error_kind: Option<crate::core::AppErrorKind>,
}

impl ActiveLogin {
    pub fn status(&self) -> ManagedLoginStatus {
        ManagedLoginStatus {
            operation_id: self.operation_id,
            profile_id: self.profile_id,
            stage: self.stage,
            verification_url: self.verification_url.clone(),
            user_code: self.user_code.clone(),
            error_kind: self.error_kind,
        }
    }
}

/// Owns the single permitted Managed operation. Only one plaintext runtime
/// and one App Server may exist at a time across the application.
pub struct AccountOperationActor {
    vault: Arc<CredentialVault>,
    active_login: Mutex<Option<ActiveLogin>>,
    refresh_active: AtomicBool,
}

impl AccountOperationActor {
    pub fn new(vault: Arc<CredentialVault>) -> Self {
        Self {
            vault,
            active_login: Mutex::new(None),
            refresh_active: AtomicBool::new(false),
        }
    }

    pub async fn try_start_login(
        &self,
        profile_id: ProfileId,
        method: ManagedLoginMethod,
    ) -> Result<ManagedLoginStatus, AppError> {
        let mut slot = self.active_login.lock().await;
        if slot.is_some() {
            return Err(AppError::new(
                crate::core::AppErrorKind::AuthExpired,
                "errors.anotherLoginActive",
                crate::core::RecoveryAction::Retry,
                "ACCOUNT_OPERATION_BUSY",
            ));
        }
        let status = ManagedLoginStatus {
            operation_id: uuid::Uuid::new_v4(),
            profile_id,
            stage: ManagedLoginStage::Starting,
            verification_url: None,
            user_code: None,
            error_kind: None,
        };
        *slot = Some(ActiveLogin {
            operation_id: status.operation_id,
            profile_id,
            method,
            stage: ManagedLoginStage::Starting,
            verification_url: None,
            user_code: None,
            error_kind: None,
        });
        Ok(status)
    }

    pub async fn update_login(
        &self,
        operation_id: uuid::Uuid,
        stage: ManagedLoginStage,
        verification_url: Option<String>,
        user_code: Option<String>,
        error_kind: Option<crate::core::AppErrorKind>,
    ) -> Option<ManagedLoginStatus> {
        let mut slot = self.active_login.lock().await;
        let active = slot.as_mut()?;
        if active.operation_id != operation_id {
            return None;
        }
        active.stage = stage;
        active.verification_url = verification_url;
        active.user_code = user_code;
        active.error_kind = error_kind;
        Some(active.status())
    }

    pub async fn finish_login(&self, operation_id: uuid::Uuid) {
        let mut slot = self.active_login.lock().await;
        if slot
            .as_ref()
            .is_some_and(|active| active.operation_id == operation_id)
        {
            *slot = None;
        }
    }

    pub async fn active_login(&self) -> Option<ActiveLogin> {
        self.active_login.lock().await.clone()
    }

    pub async fn is_busy(&self) -> bool {
        self.active_login.lock().await.is_some()
    }

    /// Acquire the single refresh permit. Returns false when a refresh or
    /// login is already active so only one App Server exists at a time.
    pub fn try_begin_refresh(&self) -> Result<(), AppError> {
        if self.refresh_active.swap(true, Ordering::AcqRel) {
            return Err(AppError::new(
                crate::core::AppErrorKind::OfflineOrTimeout,
                "errors.refreshAlreadyActive",
                crate::core::RecoveryAction::Retry,
                "ACCOUNT_REFRESH_BUSY",
            ));
        }
        Ok(())
    }

    pub fn end_refresh(&self) {
        self.refresh_active.store(false, Ordering::Release);
    }

    /// Seal a credential bundle; the actor owns the only write path into the
    /// Vault for Managed profiles.
    pub async fn seal_bundle(
        &self,
        profile_id: ProfileId,
        expected_generation: Option<u64>,
        bundle: &mut ManagedCredentialBundle,
    ) -> Result<(), AppError> {
        self.vault
            .seal_expected(profile_id, expected_generation, bundle)
            .map(|_| ())
            .map_err(|error| {
                AppError::new(
                    crate::core::AppErrorKind::VaultFailure,
                    "errors.vaultSealFailed",
                    crate::core::RecoveryAction::ExportDiagnostics,
                    match error {
                        crate::accounts::vault::crypto::VaultError::GenerationConflict => {
                            "VAULT_GENERATION_CONFLICT"
                        }
                        _ => "VAULT_SEAL_FAILED",
                    },
                )
            })
    }
}
