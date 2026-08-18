//! Codex provider backed exclusively by the supervised `codex app-server`.
//!
//! The provider deliberately has no HTTP client, auth-file reader, bearer
//! construction, or private endpoint fallback. The App Server owns the
//! credential/network boundary; this facade only maps its typed read results
//! into the legacy `ProviderFetchResult` shape retained during migration.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::core::{
    AppError, AppErrorKind, FetchContext, ProfileUsageSnapshot, Provider, ProviderError,
    ProviderFetchResult, ProviderId, ProviderMetadata, RateWindow, SourceMode, UsageSnapshot,
    UsageWindow,
};

pub mod app_server;

use app_server::{
    AppServerFactory, CodexCommandResolver, LocalAppServerFactory, ResolveRequest,
    parse_profile_usage,
};

/// Codex provider for fetching AI usage limits through the official App Server.
pub struct CodexProvider {
    metadata: ProviderMetadata,
    app_server: Arc<dyn AppServerFactory>,
}

impl CodexProvider {
    pub fn new() -> Self {
        Self::with_app_server(Arc::new(LocalAppServerFactory::default()))
    }

    /// Inject a supervised App Server factory for deterministic tests and
    /// future account-service composition.
    pub fn with_app_server(factory: Arc<dyn AppServerFactory>) -> Self {
        Self {
            metadata: Self::metadata(),
            app_server: factory,
        }
    }

    fn metadata() -> ProviderMetadata {
        ProviderMetadata {
            id: ProviderId::Codex,
            display_name: "Codex",
            session_label: "Session",
            weekly_label: "Weekly",
            supports_opus: false,
            supports_credits: false,
            default_enabled: true,
            is_primary: true,
            dashboard_url: Some("https://chatgpt.com/codex/settings/usage"),
            status_page_url: Some("https://status.openai.com"),
        }
    }
}

impl Default for CodexProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn fetch_usage(&self, _ctx: &FetchContext) -> Result<ProviderFetchResult, ProviderError> {
        let session = self
            .app_server
            .open_current_cli()
            .await
            .map_err(map_app_error)?;

        let read_result = match tokio::time::timeout(Duration::from_secs(30), async {
            let account = session.account_read(false).await?;
            let rate_limits = session.rate_limits_read().await?;
            Ok::<_, AppError>((account, rate_limits))
        })
        .await
        {
            Ok(result) => result.map_err(map_app_error),
            Err(_) => Err(map_app_error(AppError::new(
                AppErrorKind::OfflineOrTimeout,
                "errors.offlineOrTimeout",
                crate::core::RecoveryAction::Retry,
                "APP_SERVER_REFRESH_TIMEOUT",
            ))),
        };

        // Always close the supervised process, even when the read timed out or
        // the App Server returned a protocol error.
        let shutdown_result = session.shutdown().await.map_err(map_app_error);
        let (account, rate_limits) = read_result?;
        shutdown_result?;

        let email = account.email.clone();
        let snapshot = parse_profile_usage(Uuid::nil(), account, rate_limits, Utc::now())
            .map_err(map_app_error)?;
        Ok(provider_result_from_snapshot(snapshot, email))
    }

    fn available_sources(&self) -> Vec<SourceMode> {
        // `Cli` means the supervised App Server process. OAuth/web modes are
        // intentionally absent so persisted legacy settings cannot select a
        // private HTTP or cookie path.
        vec![SourceMode::Auto, SourceMode::Cli]
    }

    fn supports_oauth(&self) -> bool {
        false
    }

    fn supports_cli(&self) -> bool {
        true
    }

    fn detect_version(&self) -> Option<String> {
        let request = ResolveRequest {
            override_path: None,
            path: std::env::var_os("PATH"),
            pathext: std::env::var_os("PATHEXT"),
        };
        let command = CodexCommandResolver::new().resolve(&request).ok()?;
        if let Some(version) = command.version() {
            return Some(version.to_owned());
        }
        app_server::discovery::probe_version(&command)
    }
}

fn provider_result_from_snapshot(
    snapshot: ProfileUsageSnapshot,
    email: Option<String>,
) -> ProviderFetchResult {
    let primary_wire = snapshot
        .primary
        .clone()
        .or_else(|| snapshot.secondary.clone());
    let primary = primary_wire
        .as_ref()
        .map(rate_window_from_usage)
        .unwrap_or_default();

    let mut usage = UsageSnapshot::new(primary);
    usage.updated_at = snapshot.fetched_at;
    if snapshot.primary.is_some()
        && let Some(secondary) = snapshot.secondary.as_ref()
    {
        usage = usage.with_secondary(rate_window_from_usage(secondary));
    }
    for window in &snapshot.additional_windows {
        let title = window
            .label
            .clone()
            .unwrap_or_else(|| window.limit_id.clone());
        usage = usage.with_extra_rate_window(
            window.limit_id.clone(),
            title,
            rate_window_from_usage(window),
        );
    }
    if let Some(email) = email {
        usage = usage.with_email(email);
    }
    if let Some(plan_type) = snapshot.plan_type {
        usage = usage.with_login_method(format!("ChatGPT {plan_type}"));
    }
    ProviderFetchResult::new(usage, "appServer")
}

fn rate_window_from_usage(window: &UsageWindow) -> RateWindow {
    RateWindow::with_details(
        window.used_percent,
        window
            .window_duration_minutes
            .and_then(|minutes| u32::try_from(minutes).ok()),
        window.resets_at,
        None,
    )
}

fn map_app_error(error: AppError) -> ProviderError {
    let diagnostic = if error.diagnostic_code.is_empty() {
        "APP_SERVER_ERROR"
    } else {
        error.diagnostic_code.as_str()
    };
    match error.kind {
        AppErrorKind::CodexNotFound => ProviderError::NotInstalled(
            "Codex App Server executable was not found or could not be launched.".to_string(),
        ),
        AppErrorKind::NotSignedIn | AppErrorKind::AuthExpired => ProviderError::AuthRequired,
        AppErrorKind::ApiKeyNoQuota => {
            ProviderError::Other("Codex API-key accounts do not expose ChatGPT quota.".to_string())
        }
        AppErrorKind::OfflineOrTimeout => ProviderError::Timeout,
        AppErrorKind::UnsupportedCodexVersion => {
            ProviderError::Other(format!("Codex App Server is unsupported ({diagnostic})."))
        }
        AppErrorKind::ProtocolMismatch => ProviderError::Parse(format!(
            "Codex App Server protocol mismatch ({diagnostic})."
        )),
        AppErrorKind::RateLimited => ProviderError::Other(format!(
            "Codex App Server rate limited the request ({diagnostic})."
        )),
        AppErrorKind::VaultFailure | AppErrorKind::StorageFailure => ProviderError::Other(format!(
            "Codex App Server local storage failed ({diagnostic})."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::core::AppError;
    use crate::providers::codex::app_server::{
        AppServerFactory, AppServerSpawnSpec, CodexAppServerClient, CurrentCliSession,
        FakeServerMode, ManagedSession,
    };

    struct FakeAppServerFactory {
        current_cli_sessions: AtomicUsize,
    }

    impl FakeAppServerFactory {
        fn chatgpt_with_quota(_used_percent: f64) -> Self {
            Self {
                current_cli_sessions: AtomicUsize::new(0),
            }
        }

        fn current_cli_sessions_opened(&self) -> usize {
            self.current_cli_sessions.load(Ordering::Relaxed)
        }

        fn http_requests(&self) -> usize {
            0
        }
    }

    #[async_trait::async_trait]
    impl AppServerFactory for FakeAppServerFactory {
        async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError> {
            self.current_cli_sessions.fetch_add(1, Ordering::Relaxed);
            let spec = AppServerSpawnSpec::test_fixture(FakeServerMode::Normal)?;
            let client = CodexAppServerClient::connect(spec).await?;
            Ok(CurrentCliSession::from_client(client))
        }

        async fn open_managed(&self, _codex_home: &Path) -> Result<ManagedSession, AppError> {
            let spec = AppServerSpawnSpec::test_fixture(FakeServerMode::Normal)?;
            let client = CodexAppServerClient::connect(spec).await?;
            Ok(ManagedSession::from_client(client))
        }
    }

    #[tokio::test]
    async fn codex_provider_fetches_current_cli_through_app_server() {
        let factory = Arc::new(FakeAppServerFactory::chatgpt_with_quota(25.0));
        let provider = CodexProvider::with_app_server(factory.clone());
        let result = provider
            .fetch_usage(&FetchContext::default())
            .await
            .unwrap();
        assert_eq!(result.usage.primary.remaining_percent(), 75.0);
        assert_eq!(factory.current_cli_sessions_opened(), 1);
        assert_eq!(factory.http_requests(), 0);
    }
}
