//! Deterministic account-service fixture for lifecycle tests.
//!
//! Compiled only with `#[cfg(test)]`. Uses temporary directories, synthetic
//! credentials, and a fake App Server factory; never reads the user's Codex
//! installation or credentials.

use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use tempfile::TempDir;
use uuid::Uuid;

use crate::accounts::actor::AccountOperationActor;
use crate::accounts::identity::AccountIdentityCache;
use crate::accounts::model::{AccountServiceEvent, ManagedLoginStage, StartManagedLogin};
use crate::accounts::recovery::AccountRecovery;
use crate::accounts::runtime_home::RuntimeHomeManager;
use crate::accounts::secret_bytes::SensitiveBytes;
use crate::accounts::service::AccountProfileService;
use crate::accounts::vault::CredentialVault;
use crate::accounts::vault::envelope::{
    CredentialFile, ManagedCredentialBundle, PrivateProfileMetadata,
};
use crate::accounts::vault::store::TestProtector;
use crate::core::{AppError, AuthMode, ProfileId};
use crate::providers::codex::app_server::{
    AppServerFactory, AppServerSpawnSpec, CodexAppServerClient, CurrentCliSession, FakeServerMode,
    ManagedSession,
};
use crate::storage::AccountRepositories;

pub fn managed_id() -> ProfileId {
    Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0)
}

/// Fake factory that records which capabilities were exercised. Sessions are
/// backed by the deterministic PowerShell fake App Server fixture.
#[derive(Debug, Default)]
pub struct FakeAppServerFactory {
    open_current_cli_calls: AtomicUsize,
    open_managed_calls: AtomicUsize,
    login_start_calls: AtomicUsize,
    next_mode: std::sync::Mutex<Option<FakeServerMode>>,
    next_current_mode: std::sync::Mutex<Option<FakeServerMode>>,
    fail_current_open: AtomicBool,
}

impl FakeAppServerFactory {
    pub fn open_current_cli_calls(&self) -> usize {
        self.open_current_cli_calls.load(Ordering::Relaxed)
    }

    pub fn open_managed_calls(&self) -> usize {
        self.open_managed_calls.load(Ordering::Relaxed)
    }

    pub fn login_start_calls(&self) -> usize {
        self.login_start_calls.load(Ordering::Relaxed)
    }

    pub fn fail_next_login(&self) {
        *self.next_mode.lock().unwrap() = Some(FakeServerMode::LoginFailed);
    }

    pub fn fail_next_rates(&self) {
        *self.next_mode.lock().unwrap() = Some(FakeServerMode::Crash);
    }

    pub fn fail_next_current_open(&self) {
        self.fail_current_open.store(true, Ordering::Relaxed);
    }

    pub fn fail_next_current_protocol(&self) {
        *self.next_current_mode.lock().unwrap() = Some(FakeServerMode::InvalidJson);
    }

    fn take_mode(&self) -> FakeServerMode {
        self.next_mode
            .lock()
            .unwrap()
            .take()
            .unwrap_or(FakeServerMode::Normal)
    }
}

#[async_trait]
impl AppServerFactory for FakeAppServerFactory {
    async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError> {
        self.open_current_cli_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_current_open.swap(false, Ordering::Relaxed) {
            return Err(fake_session_error(AppError::new(
                crate::core::AppErrorKind::OfflineOrTimeout,
                "errors.fakeSession",
                crate::core::RecoveryAction::Retry,
                "FAKE_CURRENT_OPEN",
            )));
        }
        let mode = self
            .next_current_mode
            .lock()
            .unwrap()
            .take()
            .unwrap_or(FakeServerMode::Normal);
        let spec = AppServerSpawnSpec::test_fixture(mode).map_err(fake_session_error)?;
        let client = CodexAppServerClient::connect(spec)
            .await
            .map_err(fake_session_error)?;
        Ok(CurrentCliSession::from_client(client))
    }

    async fn open_managed(&self, _codex_home: &Path) -> Result<ManagedSession, AppError> {
        self.open_managed_calls.fetch_add(1, Ordering::Relaxed);
        let mode = self.take_mode();
        let spec = AppServerSpawnSpec::test_fixture(mode).map_err(fake_session_error)?;
        let client = CodexAppServerClient::connect(spec)
            .await
            .map_err(fake_session_error)?;
        Ok(ManagedSession::from_client(client))
    }
}

fn fake_session_error(_error: AppError) -> AppError {
    AppError::new(
        crate::core::AppErrorKind::ProtocolMismatch,
        "errors.fakeSession",
        crate::core::RecoveryAction::Retry,
        "FAKE_SESSION",
    )
}

/// Account-service fixture: temp SQLite, DPAPI-backed Vault, runtime
/// manager, recovery, actor, and service.
pub struct AccountServiceFixture {
    pub _dir: TempDir,
    pub vault: Arc<CredentialVault>,
    pub factory: Arc<FakeAppServerFactory>,
    pub runtime: RuntimeHomeManager,
    pub identity_cache: Arc<AccountIdentityCache>,
    pub current_cli_id: ProfileId,
    pub service: Arc<AccountProfileService>,
}

impl AccountServiceFixture {
    pub async fn wait_for_login_failure(&self) {
        let mut events = self.service.subscribe();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
                Ok(Ok(AccountServiceEvent::LoginChanged(status)))
                    if status.stage == ManagedLoginStage::Failed =>
                {
                    return;
                }
                _ => {}
            }
        }
        panic!("login did not reach Failed within the wait window");
    }
}

pub fn fixture() -> AccountServiceFixture {
    let dir = TempDir::new().unwrap();
    let repositories = AccountRepositories::open(&dir.path().join("data.db")).expect("temp db");
    let vault = Arc::new(CredentialVault::new(
        dir.path().join("vault"),
        Arc::new(TestProtector::default()),
    ));
    let runtime_root = dir.path().join("runtime");
    let runtime = RuntimeHomeManager::new(runtime_root.clone());
    let factory = Arc::new(FakeAppServerFactory::default());
    let recovery = AccountRecovery::new(
        RuntimeHomeManager::new(runtime_root.clone()),
        Arc::clone(&vault),
    );
    let actor = Arc::new(AccountOperationActor::new(Arc::clone(&vault)));
    let identity_cache = Arc::new(AccountIdentityCache::new(
        dir.path().join("identity").join("profiles.json"),
    ));
    let service = AccountProfileService::new(
        repositories,
        Arc::clone(&vault),
        runtime,
        factory.clone(),
        recovery,
        actor,
        Arc::clone(&identity_cache),
    );
    let current_cli_id = service
        .initialize()
        .expect("initialize")
        .selected_profile_id;

    // Seed a Managed profile row so re-login tests have a target.
    service
        .repositories()
        .accounts
        .insert_pending(managed_id(), "Managed".to_string(), chrono::Utc::now())
        .expect("seed managed");
    service
        .repositories()
        .accounts
        .update_profile(
            managed_id(),
            "Managed",
            AuthMode::ChatGpt,
            crate::accounts::model::ProfileLifecycle::Ready,
            None,
        )
        .expect("seed ready");

    // Seed a Vault so re-login can inspect a prior generation.
    let mut bundle = ManagedCredentialBundle {
        files: vec![CredentialFile {
            relative_path: "auth.json".to_string(),
            contents: SensitiveBytes::new(b"first".to_vec()),
        }],
        private_metadata: PrivateProfileMetadata {
            email: None,
            plan_type: None,
            auth_mode: AuthMode::ChatGpt,
        },
    };
    vault
        .seal_expected(managed_id(), None, &mut bundle)
        .expect("seed vault");

    AccountServiceFixture {
        _dir: dir,
        vault,
        factory,
        runtime: RuntimeHomeManager::new(runtime_root),
        identity_cache,
        current_cli_id,
        service,
    }
}

/// Keep `StartManagedLogin` import referenced so clippy stays quiet when the
/// fixture helper grows in later tasks.
#[allow(dead_code)]
fn _start_request(label: String) -> StartManagedLogin {
    StartManagedLogin {
        label,
        method: crate::accounts::model::ManagedLoginMethod::Browser,
        replace_profile_id: None,
    }
}
