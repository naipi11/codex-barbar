//! Account profile service facade: profiles, login, switch, rename, removal,
//! and refresh orchestration.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::broadcast;

use crate::accounts::actor::AccountOperationActor;
use crate::accounts::identity::{AccountIdentityCache, AccountIdentityRecord, IdentityCacheError};
use crate::accounts::model::{
    AccountProfilesSnapshot, AccountServiceError, AccountServiceEvent, ManagedLoginMethod,
    ManagedLoginStatus, StartManagedLogin,
};
use crate::accounts::recovery::AccountRecovery;
use crate::accounts::runtime_home::RuntimeHomeManager;
use crate::accounts::vault::CredentialVault;
use crate::core::{AppError, AppErrorKind, ProfileId, RefreshDisposition, RefreshTrigger};
use crate::providers::codex::app_server::AppServerFactory;
use crate::providers::codex::app_server::{AccountIdentity, parse_profile_usage};
use crate::storage::AccountRepositories;

use crate::accounts::model::ProfileLifecycle;

/// The account lifecycle service used by the desktop shell.
pub struct AccountProfileService {
    repositories: AccountRepositories,
    vault: Arc<CredentialVault>,
    runtime_homes: RuntimeHomeManager,
    app_server_factory: Arc<dyn AppServerFactory>,
    recovery: AccountRecovery,
    actor: Arc<AccountOperationActor>,
    identity_cache: Arc<AccountIdentityCache>,
    events: broadcast::Sender<AccountServiceEvent>,
}

impl AccountProfileService {
    pub fn new(
        repositories: AccountRepositories,
        vault: Arc<CredentialVault>,
        runtime_homes: RuntimeHomeManager,
        app_server_factory: Arc<dyn AppServerFactory>,
        recovery: AccountRecovery,
        actor: Arc<AccountOperationActor>,
        identity_cache: Arc<AccountIdentityCache>,
    ) -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            repositories,
            vault,
            runtime_homes,
            app_server_factory,
            recovery,
            actor,
            identity_cache,
            events,
        })
    }

    pub fn initialize(&self) -> Result<AccountProfilesSnapshot, AccountServiceError> {
        // Startup recovery must finish before profile state is served.
        self.recovery.recover().map_err(|error| {
            AccountServiceError::App(AppError::new(
                AppErrorKind::StorageFailure,
                "errors.recoveryFailed",
                crate::core::RecoveryAction::ExportDiagnostics,
                match error {
                    crate::accounts::recovery::RecoveryError::Runtime(_) => "RECOVERY_RUNTIME",
                    crate::accounts::recovery::RecoveryError::Vault(_) => "RECOVERY_VAULT",
                },
            ))
        })?;
        let current = self
            .repositories
            .accounts
            .ensure_current_cli(chrono::Utc::now())
            .map_err(storage_error)?;
        let selected = self
            .repositories
            .accounts
            .selected_profile_id()
            .unwrap_or(current.id);
        let profiles = self
            .repositories
            .accounts
            .list_ready()
            .map_err(storage_error)?;
        let snapshot = AccountProfilesSnapshot {
            profiles,
            selected_profile_id: selected,
        };
        let _ = self
            .events
            .send(AccountServiceEvent::ProfilesChanged(snapshot.clone()));
        Ok(snapshot)
    }

    pub fn snapshot(&self) -> Result<AccountProfilesSnapshot, AccountServiceError> {
        let profiles = self
            .repositories
            .accounts
            .list_ready()
            .map_err(storage_error)?;
        let selected = self
            .repositories
            .accounts
            .selected_profile_id()
            .map_err(storage_error)?;
        Ok(AccountProfilesSnapshot {
            profiles,
            selected_profile_id: selected,
        })
    }

    pub async fn start_managed_login(
        self: &Arc<Self>,
        request: StartManagedLogin,
    ) -> Result<ManagedLoginStatus, AccountServiceError> {
        if request.label.trim().is_empty() {
            return Err(AccountServiceError::InvalidLabel);
        }
        let replace_profile_id = request.replace_profile_id;
        let profile_id = match replace_profile_id {
            Some(existing) => {
                let profile = self
                    .repositories
                    .accounts
                    .get(existing)
                    .map_err(storage_error)?
                    .ok_or(AccountServiceError::LoginOperationNotFound)?;
                if profile.kind != crate::accounts::model::ProfileKind::Managed {
                    return Err(AccountServiceError::CurrentCliImmutable);
                }
                existing
            }
            None => uuid::Uuid::new_v4(),
        };
        let status = self
            .actor
            .try_start_login(profile_id, request.method)
            .await
            .map_err(AccountServiceError::from)?;
        self.publish_login(status.clone());

        let this = Arc::clone(self);
        let operation_id = status.operation_id;
        tokio::spawn(async move {
            let _ = this
                .run_managed_login(profile_id, request.method, operation_id, replace_profile_id)
                .await;
        });
        Ok(status)
    }

    pub async fn cancel_managed_login(
        &self,
        _operation_id: uuid::Uuid,
    ) -> Result<(), AccountServiceError> {
        // A complete actor will cancel the exact login and tear down the
        // session; the placeholder keeps the command surface callable.
        Ok(())
    }

    pub fn select_profile(
        &self,
        profile_id: ProfileId,
    ) -> Result<AccountProfilesSnapshot, AccountServiceError> {
        let profile = self
            .repositories
            .accounts
            .get(profile_id)
            .map_err(storage_error)?
            .ok_or(AccountServiceError::LoginOperationNotFound)?;
        if profile.lifecycle != crate::accounts::model::ProfileLifecycle::Ready {
            return Err(AccountServiceError::Busy);
        }
        self.repositories
            .accounts
            .set_selected(profile_id, chrono::Utc::now())
            .map_err(storage_error)?;
        self.snapshot()
    }

    pub fn rename_managed_profile(
        &self,
        profile_id: ProfileId,
        label: String,
    ) -> Result<AccountProfilesSnapshot, AccountServiceError> {
        if label.trim().is_empty() {
            return Err(AccountServiceError::InvalidLabel);
        }
        let profile = self
            .repositories
            .accounts
            .get(profile_id)
            .map_err(storage_error)?
            .ok_or(AccountServiceError::LoginOperationNotFound)?;
        if profile.kind != crate::accounts::model::ProfileKind::Managed {
            return Err(AccountServiceError::CurrentCliImmutable);
        }
        self.repositories
            .accounts
            .update_profile(
                profile_id,
                label.trim(),
                profile.auth_mode,
                profile.lifecycle,
                profile.email_fingerprint,
            )
            .map_err(storage_error)?;
        self.snapshot()
    }

    pub async fn remove_managed_profile(
        &self,
        profile_id: ProfileId,
    ) -> Result<AccountProfilesSnapshot, AccountServiceError> {
        let profile = self
            .repositories
            .accounts
            .get(profile_id)
            .map_err(storage_error)?
            .ok_or(AccountServiceError::LoginOperationNotFound)?;
        if profile.kind != crate::accounts::model::ProfileKind::Managed {
            return Err(AccountServiceError::CurrentCliImmutable);
        }
        self.repositories
            .accounts
            .delete_profile(profile_id)
            .map_err(storage_error)?;
        self.vault.remove(profile_id).map_err(|_error| {
            AccountServiceError::App(AppError::new(
                AppErrorKind::VaultFailure,
                "errors.vaultRemoveFailed",
                crate::core::RecoveryAction::ExportDiagnostics,
                "VAULT_REMOVE_FAILED",
            ))
        })?;
        if let Err(error) = self.identity_cache.remove(profile_id) {
            tracing::warn!(code = "IDENTITY_CACHE_REMOVE_FAILED", %error, "account identity cache cleanup failed");
        }
        self.snapshot()
    }

    pub async fn request_refresh(
        self: &Arc<Self>,
        profile_id: ProfileId,
        _trigger: RefreshTrigger,
    ) -> Result<RefreshDisposition, AccountServiceError> {
        let profile = self
            .repositories
            .accounts
            .get(profile_id)
            .map_err(storage_error)?
            .ok_or(AccountServiceError::LoginOperationNotFound)?;
        let result = if profile.kind == crate::accounts::model::ProfileKind::CurrentCli {
            self.refresh_current_cli(profile_id).await
        } else {
            self.refresh_managed(profile_id).await
        };
        let success = result.as_ref().is_ok_and(|success| *success);
        let _ = self.events.send(AccountServiceEvent::RefreshCompleted {
            profile_id,
            success,
        });
        result.map(|_| RefreshDisposition::Started)
    }

    async fn refresh_current_cli(
        &self,
        profile_id: ProfileId,
    ) -> Result<bool, AccountServiceError> {
        let session = match self.app_server_factory.open_current_cli().await {
            Ok(session) => session,
            Err(error) => {
                self.record_refresh_error(profile_id, &error).await;
                return Ok(false);
            }
        };
        let account = match session.account_read(false).await {
            Ok(account) => account,
            Err(error) => {
                let _ = session.shutdown().await;
                self.record_refresh_error(profile_id, &error).await;
                return Ok(false);
            }
        };
        if let Err(error) = self.cache_identity(profile_id, &account) {
            tracing::warn!(code = "IDENTITY_CACHE_WRITE_FAILED", %error, "account identity cache update failed");
        } else {
            self.publish_profiles_changed();
        }
        let rates = match session.rate_limits_read().await {
            Ok(rates) => rates,
            Err(error) => {
                let _ = session.shutdown().await;
                self.record_refresh_error(profile_id, &error).await;
                return Ok(false);
            }
        };
        let shutdown = session.shutdown().await;
        shutdown.map_err(AccountServiceError::from)?;
        let snapshot = parse_profile_usage(profile_id, account, rates, Utc::now())
            .map_err(AccountServiceError::from)?;
        self.repositories
            .usage
            .save_success(&snapshot)
            .map_err(storage_error)?;
        let state = self
            .repositories
            .usage
            .load_state(profile_id)
            .map_err(storage_error)?;
        let _ = self
            .events
            .send(AccountServiceEvent::UsageStateChanged(Box::new(state)));
        Ok(true)
    }

    async fn refresh_managed(&self, profile_id: ProfileId) -> Result<bool, AccountServiceError> {
        self.actor
            .try_begin_refresh()
            .map_err(AccountServiceError::from)?;
        let result = match self.refresh_managed_inner(profile_id).await {
            Ok(()) => Ok(true),
            Err(AccountServiceError::App(error)) => {
                self.record_refresh_error(profile_id, &error).await;
                Ok(false)
            }
            Err(other) => Err(other),
        };
        self.actor.end_refresh();
        result
    }

    async fn refresh_managed_inner(
        &self,
        profile_id: ProfileId,
    ) -> Result<(), AccountServiceError> {
        use crate::accounts::runtime_home::RuntimeState;

        // Unseal the managed credentials, restore them into a fresh restricted
        // runtime, read account+rates, then reseal any refreshed credentials.
        let (bundle, info) = self.vault.unseal(profile_id).map_err(|_| {
            AccountServiceError::App(AppError::new(
                AppErrorKind::VaultFailure,
                "errors.vaultUnsealFailed",
                crate::core::RecoveryAction::ExportDiagnostics,
                "VAULT_UNSEAL_FAILED",
            ))
        })?;
        let runtime = self
            .runtime_homes
            .restore(profile_id, &bundle, info.generation)
            .map_err(|_| {
                AccountServiceError::App(AppError::new(
                    AppErrorKind::StorageFailure,
                    "errors.runtimeRestoreFailed",
                    crate::core::RecoveryAction::Retry,
                    "RUNTIME_RESTORE_FAILED",
                ))
            })?;
        runtime
            .set_state(RuntimeState::Active)
            .map_err(|_| AccountServiceError::Busy)?;

        let session = match self
            .app_server_factory
            .open_managed(runtime.codex_home())
            .await
        {
            Ok(session) => session,
            Err(error) => {
                let _ = runtime.cleanup();
                return Err(AccountServiceError::App(error));
            }
        };
        let account = match session.account_read(false).await {
            Ok(account) => account,
            Err(error) => {
                let _ = session.shutdown().await;
                let _ = runtime.cleanup();
                return Err(AccountServiceError::App(error));
            }
        };
        if let Err(error) = self.cache_identity(profile_id, &account) {
            tracing::warn!(code = "IDENTITY_CACHE_WRITE_FAILED", %error, "account identity cache update failed");
        } else {
            self.publish_profiles_changed();
        }
        let rates = match session.rate_limits_read().await {
            Ok(rates) => rates,
            Err(error) => {
                let _ = session.shutdown().await;
                let _ = runtime.cleanup();
                return Err(AccountServiceError::App(error));
            }
        };
        let shutdown = session.shutdown().await;
        shutdown.map_err(AccountServiceError::from)?;

        let email = account.email.clone();
        let plan_type = account.plan_type.clone();
        let auth_mode = account.auth_mode;
        let snapshot = parse_profile_usage(profile_id, account, rates, Utc::now())
            .map_err(AccountServiceError::from)?;

        // Collect possibly refreshed credentials and reseal as generation+1.
        let mut collected =
            crate::accounts::credential_bundle::collect_bundle(runtime.codex_home(), profile_id)
                .map_err(|_| AccountServiceError::Busy)?;
        collected.private_metadata = crate::accounts::vault::envelope::PrivateProfileMetadata {
            email,
            plan_type,
            auth_mode,
        };
        if let Err(error) = self
            .actor
            .seal_bundle(profile_id, Some(info.generation), &mut collected)
            .await
        {
            let _ = runtime.cleanup();
            return Err(AccountServiceError::App(error));
        }
        let _ = runtime.cleanup();

        self.repositories
            .usage
            .save_success(&snapshot)
            .map_err(storage_error)?;
        let state = self
            .repositories
            .usage
            .load_state(profile_id)
            .map_err(storage_error)?;
        let _ = self
            .events
            .send(AccountServiceEvent::UsageStateChanged(Box::new(state)));
        Ok(())
    }

    async fn record_refresh_error(&self, profile_id: ProfileId, error: &AppError) {
        let _ = self.repositories.usage.save_error(profile_id, error);
        if let Ok(state) = self.repositories.usage.load_state(profile_id) {
            let _ = self
                .events
                .send(AccountServiceEvent::UsageStateChanged(Box::new(state)));
        }
    }

    fn publish_login(&self, status: ManagedLoginStatus) {
        let _ = self.events.send(AccountServiceEvent::LoginChanged(status));
    }

    fn publish_profiles_changed(&self) {
        match self.snapshot() {
            Ok(snapshot) => {
                let _ = self
                    .events
                    .send(AccountServiceEvent::ProfilesChanged(snapshot));
            }
            Err(error) => {
                tracing::warn!(
                    code = "IDENTITY_EVENT_SNAPSHOT_FAILED",
                    %error,
                    "account identity update snapshot failed"
                );
            }
        }
    }

    async fn run_managed_login(
        &self,
        profile_id: ProfileId,
        method: ManagedLoginMethod,
        operation_id: uuid::Uuid,
        replace_profile_id: Option<ProfileId>,
    ) -> Result<(), AccountServiceError> {
        use crate::accounts::model::ManagedLoginStage;
        use crate::accounts::runtime_home::RuntimeState;
        use crate::accounts::vault::envelope::PrivateProfileMetadata;
        use crate::providers::codex::app_server::{LoginEvent, LoginFlow};

        let flow = match method {
            ManagedLoginMethod::Browser => LoginFlow::Browser,
            ManagedLoginMethod::DeviceCode => LoginFlow::DeviceCode,
        };

        // Managed re-login restores credentials from the existing Vault;
        // a brand-new login starts from an empty restricted runtime.
        let (runtime, base_generation) = match replace_profile_id {
            Some(_) => {
                let (mut bundle, info) = self.vault.unseal(profile_id).map_err(|_| {
                    AccountServiceError::App(AppError::new(
                        AppErrorKind::VaultFailure,
                        "errors.vaultUnsealFailed",
                        crate::core::RecoveryAction::ExportDiagnostics,
                        "VAULT_UNSEAL_FAILED",
                    ))
                })?;
                let _ = &mut bundle;
                let runtime = self
                    .runtime_homes
                    .restore(profile_id, &bundle, info.generation)
                    .map_err(|_error| {
                        AccountServiceError::App(AppError::new(
                            AppErrorKind::StorageFailure,
                            "errors.runtimeRestoreFailed",
                            crate::core::RecoveryAction::Retry,
                            "RUNTIME_RESTORE_FAILED",
                        ))
                    })?;
                runtime
                    .set_state(RuntimeState::LoggingIn)
                    .map_err(|_| AccountServiceError::Busy)?;
                (runtime, info.generation)
            }
            None => {
                let runtime = self
                    .runtime_homes
                    .prepare_new(profile_id)
                    .map_err(|_| AccountServiceError::Busy)?;
                runtime
                    .set_state(RuntimeState::LoggingIn)
                    .map_err(|_| AccountServiceError::Busy)?;
                (runtime, 0)
            }
        };

        let mut session = match self
            .app_server_factory
            .open_managed(runtime.codex_home())
            .await
        {
            Ok(session) => session,
            Err(error) => {
                if let Some(status) = self
                    .actor
                    .update_login(
                        operation_id,
                        ManagedLoginStage::Failed,
                        None,
                        None,
                        Some(error.kind),
                    )
                    .await
                {
                    self.publish_login(status);
                }
                let _ = runtime.cleanup();
                self.actor.finish_login(operation_id).await;
                return Err(AccountServiceError::App(error));
            }
        };
        let challenge = session.start_login(flow).await;

        let mut terminal_error = None;
        let _challenge = match challenge {
            Ok(challenge) => {
                if let Some(status) = self
                    .actor
                    .update_login(
                        operation_id,
                        ManagedLoginStage::AwaitingUser,
                        challenge.verification_url.clone(),
                        challenge.user_code.clone(),
                        None,
                    )
                    .await
                {
                    self.publish_login(status);
                }
                challenge
            }
            Err(error) => {
                let _ = error;
                if let Some(status) = self
                    .actor
                    .update_login(
                        operation_id,
                        ManagedLoginStage::Failed,
                        None,
                        None,
                        Some(AppErrorKind::ProtocolMismatch),
                    )
                    .await
                {
                    self.publish_login(status);
                }
                let _ = session.shutdown().await;
                let _ = runtime.cleanup();
                self.actor.finish_login(operation_id).await;
                return Err(AccountServiceError::App(AppError::new(
                    AppErrorKind::ProtocolMismatch,
                    "errors.loginStartFailed",
                    crate::core::RecoveryAction::Retry,
                    "LOGIN_START_FAILED",
                )));
            }
        };

        let event = session.next_login_event().await;
        let _ = session.shutdown().await;

        match event {
            Ok(LoginEvent::Completed { .. }) => {
                let account = match self
                    .app_server_factory
                    .open_managed(runtime.codex_home())
                    .await
                {
                    Ok(session) => {
                        let account = session.account_read(false).await;
                        let _ = session.shutdown().await;
                        account
                    }
                    Err(error) => Err(error),
                };
                let account = match account {
                    Ok(account) => account,
                    Err(error) => {
                        if let Some(status) = self
                            .actor
                            .update_login(
                                operation_id,
                                ManagedLoginStage::Failed,
                                None,
                                None,
                                Some(error.kind),
                            )
                            .await
                        {
                            self.publish_login(status);
                        }
                        let _ = runtime.cleanup();
                        self.actor.finish_login(operation_id).await;
                        return Err(AccountServiceError::App(error));
                    }
                };
                if let Err(error) = self.cache_identity(profile_id, &account) {
                    tracing::warn!(code = "IDENTITY_CACHE_WRITE_FAILED", %error, "account identity cache update failed");
                }
                if account.auth_mode != crate::core::AuthMode::ChatGpt {
                    if let Some(status) = self
                        .actor
                        .update_login(
                            operation_id,
                            ManagedLoginStage::Failed,
                            None,
                            None,
                            Some(AppErrorKind::ApiKeyNoQuota),
                        )
                        .await
                    {
                        self.publish_login(status);
                    }
                    let _ = runtime.cleanup();
                    self.actor.finish_login(operation_id).await;
                    return Err(AccountServiceError::App(AppError::new(
                        AppErrorKind::ApiKeyNoQuota,
                        "errors.apiKeyNoQuota",
                        crate::core::RecoveryAction::None,
                        "LOGIN_API_KEY_REJECTED",
                    )));
                }

                let mut bundle = crate::accounts::credential_bundle::collect_bundle(
                    runtime.codex_home(),
                    profile_id,
                )
                .map_err(|_| AccountServiceError::Busy)?;
                bundle.private_metadata = PrivateProfileMetadata {
                    email: account.email,
                    plan_type: account.plan_type,
                    auth_mode: account.auth_mode,
                };
                let expected = if replace_profile_id.is_some() {
                    Some(base_generation)
                } else if base_generation == 0 {
                    Some(0)
                } else {
                    None
                };
                self.actor
                    .seal_bundle(profile_id, expected, &mut bundle)
                    .await
                    .map_err(AccountServiceError::from)?;
                let _ = runtime.cleanup();

                self.repositories
                    .accounts
                    .update_profile(
                        profile_id,
                        "Managed",
                        account.auth_mode,
                        ProfileLifecycle::Ready,
                        None,
                    )
                    .map_err(storage_error)?;
                if let Some(status) = self
                    .actor
                    .update_login(operation_id, ManagedLoginStage::Succeeded, None, None, None)
                    .await
                {
                    self.publish_login(status);
                }
            }
            Ok(LoginEvent::Failed { error, .. }) => {
                terminal_error = Some(error);
                if let Some(status) = self
                    .actor
                    .update_login(
                        operation_id,
                        ManagedLoginStage::Failed,
                        None,
                        None,
                        Some(AppErrorKind::AuthExpired),
                    )
                    .await
                {
                    self.publish_login(status);
                }
            }
            Ok(LoginEvent::Cancelled { .. }) => {
                if let Some(status) = self
                    .actor
                    .update_login(operation_id, ManagedLoginStage::Cancelled, None, None, None)
                    .await
                {
                    self.publish_login(status);
                }
            }
            Err(error) => {
                terminal_error = Some(error);
                if let Some(status) = self
                    .actor
                    .update_login(
                        operation_id,
                        ManagedLoginStage::Failed,
                        None,
                        None,
                        Some(AppErrorKind::OfflineOrTimeout),
                    )
                    .await
                {
                    self.publish_login(status);
                }
            }
        }

        let _ = runtime.cleanup();
        self.actor.finish_login(operation_id).await;
        if let Some(error) = terminal_error {
            return Err(AccountServiceError::App(error));
        }
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AccountServiceEvent> {
        self.events.subscribe()
    }

    pub fn identity_for(
        &self,
        profile_id: ProfileId,
    ) -> Result<Option<AccountIdentityRecord>, IdentityCacheError> {
        self.identity_cache.load(profile_id)
    }

    pub fn identity_records(
        &self,
    ) -> Result<BTreeMap<ProfileId, AccountIdentityRecord>, IdentityCacheError> {
        let mut result = BTreeMap::new();
        let snapshot = self
            .snapshot()
            .map_err(|error| IdentityCacheError::Io(std::io::Error::other(error.to_string())))?;
        for profile in snapshot.profiles {
            if let Some(identity) = self.identity_cache.load(profile.id)? {
                result.insert(profile.id, identity);
            }
        }
        Ok(result)
    }

    pub async fn shutdown(&self, _timeout: Duration) -> Result<(), AccountServiceError> {
        Ok(())
    }

    pub fn vault(&self) -> &CredentialVault {
        &self.vault
    }

    pub fn repositories(&self) -> &AccountRepositories {
        &self.repositories
    }

    fn cache_identity(
        &self,
        profile_id: ProfileId,
        account: &AccountIdentity,
    ) -> Result<(), IdentityCacheError> {
        self.identity_cache.save(
            profile_id,
            &AccountIdentityRecord {
                display_name: account.display_name.clone(),
                email: account.email.clone(),
                plan_type: account.plan_type.clone(),
                status: account.status(),
                updated_at: Utc::now(),
            },
        )
    }
}

fn storage_error(error: crate::storage::StorageError) -> AccountServiceError {
    AccountServiceError::App(error.into_app_error())
}

#[cfg(test)]
mod tests {
    use crate::accounts::model::{AccountServiceEvent, StartManagedLogin};
    use crate::accounts::test_support::{fixture, managed_id};
    use crate::core::RefreshTrigger;

    fn terminal_refresh_results(
        events: &mut tokio::sync::broadcast::Receiver<AccountServiceEvent>,
        profile_id: crate::core::ProfileId,
    ) -> Vec<bool> {
        let mut results = Vec::new();
        loop {
            match events.try_recv() {
                Ok(AccountServiceEvent::RefreshCompleted {
                    profile_id: completed_profile,
                    success,
                }) if completed_profile == profile_id => results.push(success),
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(count)) => {
                    panic!("refresh event receiver lagged by {count}")
                }
            }
        }
        results
    }

    #[tokio::test]
    async fn current_cli_never_uses_managed_or_login_methods() {
        let fixture = fixture();
        let service = fixture.service.clone();
        let current = fixture.current_cli_id;
        service
            .request_refresh(current, RefreshTrigger::Manual)
            .await
            .unwrap();
        assert_eq!(fixture.factory.open_current_cli_calls(), 1);
        assert_eq!(fixture.factory.open_managed_calls(), 0);
        assert_eq!(fixture.factory.login_start_calls(), 0);
    }

    #[tokio::test]
    async fn current_cli_launch_failure_emits_one_terminal_failure() {
        let fixture = fixture();
        let mut events = fixture.service.subscribe();
        fixture.factory.fail_next_current_open();

        fixture
            .service
            .request_refresh(fixture.current_cli_id, RefreshTrigger::Manual)
            .await
            .unwrap();

        assert_eq!(
            terminal_refresh_results(&mut events, fixture.current_cli_id),
            vec![false]
        );
    }

    #[tokio::test]
    async fn current_cli_protocol_failure_emits_one_terminal_failure() {
        let fixture = fixture();
        let mut events = fixture.service.subscribe();
        fixture.factory.fail_next_current_protocol();

        fixture
            .service
            .request_refresh(fixture.current_cli_id, RefreshTrigger::Manual)
            .await
            .unwrap();

        assert_eq!(
            terminal_refresh_results(&mut events, fixture.current_cli_id),
            vec![false]
        );
    }

    #[tokio::test]
    async fn successful_current_cli_refresh_emits_one_terminal_success() {
        let fixture = fixture();
        let mut events = fixture.service.subscribe();

        fixture
            .service
            .request_refresh(fixture.current_cli_id, RefreshTrigger::Manual)
            .await
            .unwrap();

        assert_eq!(
            terminal_refresh_results(&mut events, fixture.current_cli_id),
            vec![true]
        );
    }

    #[tokio::test]
    async fn current_cli_refresh_caches_account_identity_before_usage_event() {
        let fixture = fixture();
        fixture
            .service
            .request_refresh(fixture.current_cli_id, RefreshTrigger::Manual)
            .await
            .unwrap();

        let identity = fixture
            .service
            .identity_for(fixture.current_cli_id)
            .expect("identity lookup should succeed")
            .expect("current CLI identity should be cached");
        assert_eq!(identity.display_name, None);
        assert_eq!(identity.email.as_deref(), Some("fixture@example.invalid"));
    }

    #[tokio::test]
    async fn identity_event_precedes_rate_limit_failure() {
        let fixture = fixture();
        let current = fixture.current_cli_id;
        let mut events = fixture.service.subscribe();

        fixture.factory.fail_next_rates();
        fixture
            .service
            .request_refresh(current, RefreshTrigger::Manual)
            .await
            .unwrap();

        let saw_identity = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match events.recv().await.unwrap() {
                    crate::accounts::model::AccountServiceEvent::ProfilesChanged(snapshot)
                        if snapshot.selected_profile_id == current =>
                    {
                        break true;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or(false);

        assert!(
            saw_identity,
            "identity update must not wait for quota success"
        );
    }

    #[tokio::test]
    async fn managed_refresh_caches_identity_and_removal_clears_it() {
        let fixture = fixture();
        fixture
            .service
            .request_refresh(managed_id(), RefreshTrigger::Manual)
            .await
            .unwrap();

        assert!(
            fixture
                .service
                .identity_for(managed_id())
                .expect("identity lookup should succeed")
                .is_some()
        );

        fixture
            .service
            .remove_managed_profile(managed_id())
            .await
            .unwrap();

        assert_eq!(
            fixture
                .service
                .identity_for(managed_id())
                .expect("identity lookup should succeed"),
            None
        );
    }

    #[tokio::test]
    async fn failed_relogin_keeps_previous_vault_generation() {
        let fixture = fixture();
        let before = fixture.vault.inspect(managed_id()).unwrap().unwrap();
        fixture.factory.fail_next_login();
        fixture
            .service
            .start_managed_login(StartManagedLogin {
                label: "Managed".to_string(),
                method: crate::accounts::model::ManagedLoginMethod::Browser,
                replace_profile_id: Some(managed_id()),
            })
            .await
            .unwrap();
        fixture.wait_for_login_failure().await;
        assert_eq!(
            fixture
                .vault
                .inspect(managed_id())
                .unwrap()
                .unwrap()
                .generation,
            before.generation
        );
    }

    #[tokio::test]
    async fn managed_refresh_reseals_credentials_and_removes_runtime() {
        let fixture = fixture();
        let before = fixture.vault.inspect(managed_id()).unwrap().unwrap();
        fixture
            .service
            .request_refresh(managed_id(), RefreshTrigger::Manual)
            .await
            .unwrap();
        // The fake App Server completes the refresh synchronously enough that
        // the repository write is observable; poll briefly for generation+1.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let now = fixture.vault.inspect(managed_id()).unwrap().unwrap();
            if now.generation == before.generation + 1 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "generation did not advance"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let candidates = fixture.runtime.scan_recovery_candidates().unwrap();
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.profile_id != managed_id()),
            "managed runtime sessions must be cleaned after refresh"
        );
    }

    #[tokio::test]
    async fn failed_refresh_preserves_cached_success_and_records_error() {
        let fixture = fixture();
        fixture.factory.fail_next_rates();
        let before = fixture.vault.inspect(managed_id()).unwrap().unwrap();
        let old_snapshot = crate::core::ProfileUsageSnapshot {
            profile_id: managed_id(),
            plan_type: Some("plus".to_string()),
            primary: Some(
                crate::core::UsageWindow::normalized("codex", None, 25.0, Some(300), None, None).0,
            ),
            secondary: None,
            additional_windows: Vec::new(),
            fetched_at: chrono::Utc::now(),
            source: crate::core::UsageSource::AppServer,
            protocol_anomaly: false,
        };
        fixture
            .service
            .repositories()
            .usage
            .save_success(&old_snapshot)
            .unwrap();
        fixture
            .service
            .request_refresh(managed_id(), RefreshTrigger::Manual)
            .await
            .unwrap();
        let state = fixture
            .service
            .repositories()
            .usage
            .load_state(managed_id())
            .unwrap();
        assert_eq!(state.snapshot, Some(old_snapshot));
        assert_eq!(
            state.current_error.unwrap().kind,
            crate::core::AppErrorKind::OfflineOrTimeout
        );
        assert_eq!(
            fixture
                .vault
                .inspect(managed_id())
                .unwrap()
                .unwrap()
                .generation,
            before.generation
        );
    }
}
