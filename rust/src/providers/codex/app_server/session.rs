//! Capability-limited App Server sessions.
//!
//! `CurrentCliSession` is intentionally read-only. `ManagedSession` owns the
//! same transport shape but is the only public type that can start or cancel
//! an account login. Both types keep protocol parsing and URL validation in
//! Rust so a WebView never receives arbitrary process or navigation power.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::app_paths::AppPaths;
use crate::core::{AppError, AppErrorKind, RecoveryAction};

use super::client::{AppServerNotification, CodexAppServerClient};
use super::discovery::{CodexCommandResolver, ResolveRequest};
use super::model::{AccountIdentity, ParsedRateLimits};
use super::process::AppServerSpawnSpec;

/// Login flows supported for a managed profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFlow {
    Browser,
    DeviceCode,
}

/// A validated login challenge returned by the App Server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginChallenge {
    pub login_id: String,
    pub authorization_url: Option<String>,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
}

/// A typed login lifecycle event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginEvent {
    Completed { login_id: String },
    Failed { login_id: String, error: AppError },
    Cancelled { login_id: String },
}

/// Factory boundary used by provider/product code.
#[async_trait]
pub trait AppServerFactory: Send + Sync {
    async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError>;
    async fn open_managed(&self, codex_home: &Path) -> Result<ManagedSession, AppError>;
}

/// Local factory that resolves a tested Codex command and opens one supervised
/// stdio session per profile.
#[derive(Clone)]
pub struct LocalAppServerFactory {
    request_source: Arc<dyn Fn() -> Result<ResolveRequest, AppError> + Send + Sync>,
}

impl std::fmt::Debug for LocalAppServerFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalAppServerFactory")
            .finish_non_exhaustive()
    }
}

impl LocalAppServerFactory {
    pub fn new() -> Self {
        Self::with_resolve_request(ResolveRequest {
            override_path: None,
            path: std::env::var_os("PATH"),
            pathext: std::env::var_os("PATHEXT"),
        })
    }

    pub fn with_resolve_request(request: ResolveRequest) -> Self {
        Self::with_request_source(move || Ok(request.clone()))
    }

    /// Resolve from current persisted settings on each open. A settings error
    /// must propagate rather than silently selecting another CLI/account.
    pub fn with_request_source(
        source: impl Fn() -> Result<ResolveRequest, AppError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            request_source: Arc::new(source),
        }
    }

    fn resolve(&self) -> Result<super::discovery::ResolvedCodexCommand, AppError> {
        let request = (self.request_source)()?;
        CodexCommandResolver::new().resolve(&request)
    }

    fn runtime_root() -> Result<PathBuf, AppError> {
        AppPaths::discover()
            .map(|paths| paths.runtime)
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::StorageFailure,
                    "errors.appPathsUnavailable",
                    RecoveryAction::Retry,
                    "APP_PATHS_UNAVAILABLE",
                )
            })
    }
}

impl Default for LocalAppServerFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppServerFactory for LocalAppServerFactory {
    async fn open_current_cli(&self) -> Result<CurrentCliSession, AppError> {
        let command = self.resolve()?;
        let client =
            CodexAppServerClient::connect(AppServerSpawnSpec::current_cli(command)).await?;
        Ok(CurrentCliSession::from_client(client))
    }

    async fn open_managed(&self, codex_home: &Path) -> Result<ManagedSession, AppError> {
        let command = self.resolve()?;
        let runtime_root = Self::runtime_root()?;
        let spec = AppServerSpawnSpec::managed(command, codex_home, &runtime_root)?;
        let client = CodexAppServerClient::connect(spec).await?;
        Ok(ManagedSession::from_client(client))
    }
}

/// Read-only CurrentCli capability boundary.
#[derive(Clone)]
pub struct CurrentCliSession {
    client: CodexAppServerClient,
}

impl CurrentCliSession {
    /// Construct a read-only session from an already-connected client.
    #[doc(hidden)]
    pub fn from_client(client: CodexAppServerClient) -> Self {
        Self { client }
    }

    pub async fn account_read(&self, refresh_token: bool) -> Result<AccountIdentity, AppError> {
        let value = self
            .client
            .request("account/read", json!({ "refreshToken": refresh_token }))
            .await?;
        AccountIdentity::from_value(value).map_err(map_model_error)
    }

    pub async fn rate_limits_read(&self) -> Result<ParsedRateLimits, AppError> {
        let value = self
            .client
            .request("account/rateLimits/read", json!({}))
            .await?;
        ParsedRateLimits::from_value(value).map_err(map_model_error)
    }

    pub async fn shutdown(self) -> Result<(), AppError> {
        self.client.shutdown().await
    }
}

/// Managed profile capability boundary. Login methods deliberately do not
/// exist on [`CurrentCliSession`].
pub struct ManagedSession {
    client: CodexAppServerClient,
    login_events: broadcast::Receiver<AppServerNotification>,
    cancelled_login_id: Mutex<Option<String>>,
}

impl ManagedSession {
    /// Construct a managed session from an already-connected client.
    #[doc(hidden)]
    pub fn from_client(client: CodexAppServerClient) -> Self {
        let login_events = client.subscribe_notifications();
        Self {
            client,
            login_events,
            cancelled_login_id: Mutex::new(None),
        }
    }

    pub async fn account_read(&self, refresh_token: bool) -> Result<AccountIdentity, AppError> {
        let value = self
            .client
            .request("account/read", json!({ "refreshToken": refresh_token }))
            .await?;
        AccountIdentity::from_value(value).map_err(map_model_error)
    }

    pub async fn rate_limits_read(&self) -> Result<ParsedRateLimits, AppError> {
        let value = self
            .client
            .request("account/rateLimits/read", json!({}))
            .await?;
        ParsedRateLimits::from_value(value).map_err(map_model_error)
    }

    pub async fn start_login(&self, flow: LoginFlow) -> Result<LoginChallenge, AppError> {
        let login_type = match flow {
            LoginFlow::Browser => "chatgpt",
            LoginFlow::DeviceCode => "chatgptDeviceCode",
        };
        let value = self
            .client
            .request("account/login/start", json!({ "type": login_type }))
            .await?;
        parse_login_challenge(value, flow)
    }

    pub async fn next_login_event(&mut self) -> Result<LoginEvent, AppError> {
        loop {
            let notification = tokio::select! {
                biased;
                result = self.login_events.recv() => result,
                error = self.client.disconnected() => return Err(error),
            }
            .map_err(|error| match error {
                broadcast::error::RecvError::Lagged(_) => {
                    session_protocol_error("APP_SERVER_LOGIN_EVENT_LAGGED")
                }
                broadcast::error::RecvError::Closed => {
                    session_offline_error("APP_SERVER_LOGIN_EVENT_STREAM_CLOSED")
                }
            })?;
            if let Some(event) = parse_login_event(notification)? {
                if let LoginEvent::Failed { login_id, .. } = &event {
                    let mut cancelled = self
                        .cancelled_login_id
                        .lock()
                        .expect("login cancellation lock");
                    if cancelled.as_deref() == Some(login_id.as_str()) {
                        *cancelled = None;
                        return Ok(LoginEvent::Cancelled {
                            login_id: login_id.clone(),
                        });
                    }
                }
                return Ok(event);
            }
        }
    }

    pub async fn cancel_login(&self, login_id: &str) -> Result<(), AppError> {
        if login_id.trim().is_empty() {
            return Err(session_protocol_error("APP_SERVER_LOGIN_ID_MISSING"));
        }
        let value = self
            .client
            .request(
                "account/login/cancel",
                json!({ "loginId": login_id.trim() }),
            )
            .await?;
        match value.get("status").and_then(Value::as_str) {
            Some("canceled") => {
                *self
                    .cancelled_login_id
                    .lock()
                    .expect("login cancellation lock") = Some(login_id.trim().to_string());
                Ok(())
            }
            Some("notFound") => Ok(()),
            _ => Err(session_protocol_error("APP_SERVER_LOGIN_CANCEL_INVALID")),
        }
    }

    pub async fn shutdown(self) -> Result<(), AppError> {
        self.client.shutdown().await
    }
}

// Only the official rust-v0.167.0 response variants for the two supported
// interactive flows are accepted. Unknown extra fields remain forward compatible.
#[derive(Deserialize)]
#[serde(tag = "type")]
enum LoginResponse {
    #[serde(rename = "chatgpt", rename_all = "camelCase")]
    Browser { login_id: String, auth_url: String },
    #[serde(rename = "chatgptDeviceCode", rename_all = "camelCase")]
    Device {
        login_id: String,
        verification_url: String,
        user_code: String,
    },
}

fn parse_login_challenge(value: Value, flow: LoginFlow) -> Result<LoginChallenge, AppError> {
    let response: LoginResponse = serde_json::from_value(value)
        .map_err(|_| session_protocol_error("APP_SERVER_LOGIN_CHALLENGE_INVALID"))?;
    let challenge = match (response, flow) {
        (LoginResponse::Browser { login_id, auth_url }, LoginFlow::Browser) => LoginChallenge {
            login_id,
            authorization_url: Some(validate_login_url(&auth_url, false)?),
            verification_url: None,
            user_code: None,
        },
        (
            LoginResponse::Device {
                login_id,
                verification_url,
                user_code,
            },
            LoginFlow::DeviceCode,
        ) => {
            if user_code.trim().is_empty() {
                return Err(session_protocol_error("APP_SERVER_LOGIN_CHALLENGE_INVALID"));
            }
            LoginChallenge {
                login_id,
                authorization_url: None,
                verification_url: Some(validate_login_url(&verification_url, true)?),
                user_code: Some(user_code),
            }
        }
        _ => return Err(session_protocol_error("APP_SERVER_LOGIN_CHALLENGE_INVALID")),
    };
    if challenge.login_id.trim().is_empty() {
        return Err(session_protocol_error("APP_SERVER_LOGIN_ID_MISSING"));
    }
    Ok(challenge)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginCompletedNotification {
    login_id: Option<String>,
    success: bool,
}

fn parse_login_event(notification: AppServerNotification) -> Result<Option<LoginEvent>, AppError> {
    if notification.method != "account/login/completed" {
        return Ok(None);
    }
    let params: LoginCompletedNotification = serde_json::from_value(notification.params)
        .map_err(|_| session_protocol_error("APP_SERVER_LOGIN_EVENT_INVALID"))?;
    // The official notification allows no loginId for non-interactive account
    // updates. Such an event cannot complete this session's interactive login.
    let Some(login_id) = params.login_id else {
        return Ok(None);
    };
    if login_id.trim().is_empty() {
        return Err(session_protocol_error("APP_SERVER_LOGIN_EVENT_ID_MISSING"));
    }
    if params.success {
        Ok(Some(LoginEvent::Completed { login_id }))
    } else {
        Ok(Some(LoginEvent::Failed {
            login_id,
            error: AppError::new(
                AppErrorKind::AuthExpired,
                "errors.loginFailed",
                RecoveryAction::Reauthenticate,
                "APP_SERVER_LOGIN_FAILED",
            ),
        }))
    }
}

fn validate_login_url(raw: &str, require_device_path: bool) -> Result<String, AppError> {
    let parsed = reqwest::Url::parse(raw).map_err(|_| login_url_unsupported_error())?;
    let valid_origin = parsed.scheme() == "https"
        && parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("auth.openai.com"))
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
        && parsed.port().is_none();
    if !valid_origin || (require_device_path && parsed.path() != "/codex/device") {
        return Err(login_url_unsupported_error());
    }
    Ok(raw.to_string())
}

fn map_model_error(mut error: AppError) -> AppError {
    match error.kind {
        AppErrorKind::NotSignedIn => {
            error.message_key = "errors.notSignedIn".to_string();
            error.recovery = RecoveryAction::SignInWithCli;
        }
        AppErrorKind::ApiKeyNoQuota => {
            error.message_key = "errors.apiKeyNoQuota".to_string();
            error.recovery = RecoveryAction::None;
        }
        _ => {}
    }
    error
}

fn login_url_unsupported_error() -> AppError {
    AppError::new(
        AppErrorKind::UnsupportedCodexVersion,
        "errors.loginUrlUnsupported",
        RecoveryAction::InstallTestedCodex,
        "CODEX_LOGIN_URL_UNSUPPORTED",
    )
}

fn session_protocol_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.appServerProtocolMismatch",
        RecoveryAction::InstallTestedCodex,
        code,
    )
}

fn session_offline_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::OfflineOrTimeout,
        "errors.offlineOrTimeout",
        RecoveryAction::Retry,
        code,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_source_is_evaluated_for_each_resolve_and_errors_never_fall_back() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let source_calls = Arc::clone(&calls);
        let factory = LocalAppServerFactory::with_request_source(move || {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Err(session_protocol_error("FIXTURE_SETTINGS_UNAVAILABLE"))
        });
        for _ in 0..2 {
            assert_eq!(
                factory.resolve().unwrap_err().diagnostic_code,
                "FIXTURE_SETTINGS_UNAVAILABLE"
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn official_login_challenges_require_matching_type_and_device_code() {
        let browser = json!({
            "type": "chatgpt", "loginId": "browser-1",
            "authUrl": "https://auth.openai.com/authorize?fixture=1", "future": true
        });
        assert!(parse_login_challenge(browser.clone(), LoginFlow::Browser).is_ok());
        assert!(parse_login_challenge(browser, LoginFlow::DeviceCode).is_err());
        let mut device = json!({
            "type": "chatgptDeviceCode", "loginId": "device-1",
            "verificationUrl": "https://auth.openai.com/codex/device", "userCode": "ABCD"
        });
        assert!(parse_login_challenge(device.clone(), LoginFlow::DeviceCode).is_ok());
        device.as_object_mut().unwrap().remove("userCode");
        assert!(parse_login_challenge(device, LoginFlow::DeviceCode).is_err());
        assert!(
            parse_login_challenge(
                json!({
                    "loginId": "invented", "authorizationUrl": "https://auth.openai.com/authorize"
                }),
                LoginFlow::Browser
            )
            .is_err()
        );
    }

    #[test]
    fn official_completed_notification_reports_failure_without_leaking_server_error() {
        let event = parse_login_event(AppServerNotification {
            method: "account/login/completed".to_string(),
            params: json!({"loginId": "login-1", "success": false, "error": "fixture-secret"}),
        })
        .unwrap()
        .unwrap();
        let LoginEvent::Failed { login_id, error } = event else {
            panic!("expected failed login")
        };
        assert_eq!(login_id, "login-1");
        assert_eq!(error.diagnostic_code, "APP_SERVER_LOGIN_FAILED");
        assert!(
            !serde_json::to_string(&error)
                .unwrap()
                .contains("fixture-secret")
        );
        let success = parse_login_event(AppServerNotification {
            method: "account/login/completed".to_string(),
            params: json!({"loginId": "login-1", "success": true, "error": null}),
        })
        .unwrap()
        .unwrap();
        assert!(matches!(success, LoginEvent::Completed { .. }));
    }

    #[test]
    fn login_completion_requires_success_and_ignores_uncorrelated_null_ids() {
        for params in [
            json!({"loginId": null, "success": true}),
            json!({"success": false}),
        ] {
            assert!(
                parse_login_event(AppServerNotification {
                    method: "account/login/completed".to_string(),
                    params,
                })
                .unwrap()
                .is_none()
            );
        }
        for params in [
            json!({"loginId": "one"}),
            json!({"loginId": "one", "success": "false"}),
        ] {
            assert!(
                parse_login_event(AppServerNotification {
                    method: "account/login/completed".to_string(),
                    params,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn browser_and_device_login_types_are_fixed() {
        assert_eq!(
            serde_json::json!({
                "browser": "chatgpt",
                "device": "chatgptDeviceCode"
            })["device"],
            "chatgptDeviceCode"
        );
    }

    #[test]
    fn rejects_untrusted_login_origins_and_device_paths() {
        for url in [
            "http://auth.openai.com/codex/device",
            "https://evil.auth.openai.com/codex/device",
            "https://auth.openai.com.evil.test/codex/device",
            "https://user:pass@auth.openai.com/codex/device",
            "https://auth.openai.com/codex/device#fragment",
        ] {
            assert_eq!(
                validate_login_url(url, true).unwrap_err().diagnostic_code,
                "CODEX_LOGIN_URL_UNSUPPORTED"
            );
        }
        assert!(validate_login_url("https://auth.openai.com/codex/device", true).is_ok());
        assert!(validate_login_url("https://auth.openai.com/authorize?x=1", false).is_ok());
    }

    #[test]
    fn non_login_notifications_are_ignored_by_login_event_parser() {
        let notification = AppServerNotification {
            method: "account/updated".to_string(),
            params: serde_json::json!({}),
        };
        assert!(parse_login_event(notification).unwrap().is_none());
    }
}
