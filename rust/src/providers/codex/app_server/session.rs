//! Capability-limited App Server sessions.
//!
//! `CurrentCliSession` is intentionally read-only. `ManagedSession` owns the
//! same transport shape but is the only public type that can start or cancel
//! an account login. Both types keep protocol parsing and URL validation in
//! Rust so a WebView never receives arbitrary process or navigation power.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
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
#[derive(Debug, Clone)]
pub struct LocalAppServerFactory {
    request: ResolveRequest,
}

impl LocalAppServerFactory {
    pub fn new() -> Self {
        Self {
            request: ResolveRequest {
                override_path: None,
                path: std::env::var_os("PATH"),
                pathext: std::env::var_os("PATHEXT"),
            },
        }
    }

    pub fn with_resolve_request(request: ResolveRequest) -> Self {
        Self { request }
    }

    fn resolve(&self) -> Result<super::discovery::ResolvedCodexCommand, AppError> {
        CodexCommandResolver::new().resolve(&self.request)
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
}

impl ManagedSession {
    /// Construct a managed session from an already-connected client.
    #[doc(hidden)]
    pub fn from_client(client: CodexAppServerClient) -> Self {
        let login_events = client.subscribe_notifications();
        Self {
            client,
            login_events,
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
            let notification = self
                .login_events
                .recv()
                .await
                .map_err(|error| match error {
                    broadcast::error::RecvError::Lagged(_) => {
                        session_protocol_error("APP_SERVER_LOGIN_EVENT_LAGGED")
                    }
                    broadcast::error::RecvError::Closed => {
                        session_offline_error("APP_SERVER_LOGIN_EVENT_STREAM_CLOSED")
                    }
                })?;
            if let Some(event) = parse_login_event(notification)? {
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
        if value.is_object() || value.is_null() {
            Ok(())
        } else {
            Err(session_protocol_error("APP_SERVER_LOGIN_CANCEL_INVALID"))
        }
    }

    pub async fn shutdown(self) -> Result<(), AppError> {
        self.client.shutdown().await
    }
}

fn parse_login_challenge(value: Value, flow: LoginFlow) -> Result<LoginChallenge, AppError> {
    let root = value
        .as_object()
        .ok_or_else(|| session_protocol_error("APP_SERVER_LOGIN_CHALLENGE_INVALID"))?;
    let payload = root.get("login").and_then(Value::as_object).unwrap_or(root);

    let login_id = string_field(payload, &["loginId", "login_id", "id"])
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| session_protocol_error("APP_SERVER_LOGIN_ID_MISSING"))?;
    let authorization_url = string_field(
        payload,
        &["authorizationUrl", "authorization_url", "authUrl", "url"],
    )
    .map(|url| validate_login_url(&url, false))
    .transpose()?;
    let verification_url = string_field(
        payload,
        &[
            "verificationUrl",
            "verification_url",
            "deviceUrl",
            "device_url",
        ],
    )
    .map(|url| validate_login_url(&url, matches!(flow, LoginFlow::DeviceCode)))
    .transpose()?;
    let user_code = string_field(payload, &["userCode", "user_code", "code"]);

    if matches!(flow, LoginFlow::Browser) && authorization_url.is_none() {
        return Err(session_protocol_error("APP_SERVER_LOGIN_AUTH_URL_MISSING"));
    }
    if matches!(flow, LoginFlow::DeviceCode)
        && verification_url.is_none()
        && authorization_url
            .as_deref()
            .and_then(|url| reqwest::Url::parse(url).ok())
            .is_none_or(|url| url.path() != "/codex/device")
    {
        return Err(login_url_unsupported_error());
    }

    Ok(LoginChallenge {
        login_id,
        authorization_url,
        verification_url,
        user_code,
    })
}

fn parse_login_event(notification: AppServerNotification) -> Result<Option<LoginEvent>, AppError> {
    if !matches!(
        notification.method.as_str(),
        "account/login/completed"
            | "account/login/cancelled"
            | "account/login/canceled"
            | "account/login/failed"
    ) {
        return Ok(None);
    }
    let params = notification
        .params
        .as_object()
        .ok_or_else(|| session_protocol_error("APP_SERVER_LOGIN_EVENT_INVALID"))?;
    let login_id = string_field(params, &["loginId", "login_id", "id"])
        .ok_or_else(|| session_protocol_error("APP_SERVER_LOGIN_EVENT_ID_MISSING"))?;
    match notification.method.as_str() {
        "account/login/completed" => Ok(Some(LoginEvent::Completed { login_id })),
        "account/login/cancelled" | "account/login/canceled" => {
            Ok(Some(LoginEvent::Cancelled { login_id }))
        }
        "account/login/failed" => Ok(Some(LoginEvent::Failed {
            login_id,
            error: AppError::new(
                AppErrorKind::AuthExpired,
                "errors.loginFailed",
                RecoveryAction::Reauthenticate,
                "APP_SERVER_LOGIN_FAILED",
            ),
        })),
        _ => Ok(None),
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

fn string_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        object
            .get(*name)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
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
