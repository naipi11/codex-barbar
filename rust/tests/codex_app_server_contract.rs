//! Process-level contracts for the supervised Codex App Server client.

#![cfg(any(windows, target_os = "linux"))]

use std::time::Duration;

use codexbar::providers::codex::app_server::{
    AppServerSpawnSpec, CodexAppServerClient, CurrentCliSession, FakeServerMode, InitializeParams,
    LoginEvent, LoginFlow, ManagedSession,
};
use serde_json::json;

async fn fixture_client(mode: FakeServerMode) -> CodexAppServerClient {
    let spec = AppServerSpawnSpec::test_fixture(mode).expect("fixture spawn spec");
    tokio::time::timeout(Duration::from_secs(5), CodexAppServerClient::connect(spec))
        .await
        .expect("fixture client connect timeout")
        .expect("fixture client connect")
}

async fn fixture_connect_error(mode: FakeServerMode) -> codexbar::core::AppError {
    let spec = AppServerSpawnSpec::test_fixture(mode).expect("fixture spawn spec");
    let result = tokio::time::timeout(Duration::from_secs(15), CodexAppServerClient::connect(spec))
        .await
        .expect("fixture error connect timeout");
    match result {
        Ok(client) => {
            let _ = client.shutdown().await;
            panic!("fixture should fail to connect");
        }
        Err(error) => error,
    }
}

#[tokio::test]
async fn initialize_precedes_initialized_notification_and_requests_correlate() {
    let client = fixture_client(FakeServerMode::Normal).await;
    assert_eq!(client.metrics().initialized_notifications, 1);
    let account = client
        .request("account/read", json!({ "refreshToken": false }))
        .await
        .unwrap();
    assert_eq!(account["account"]["type"], "chatgpt");
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn interleaved_unknown_notification_does_not_steal_response() {
    let client = fixture_client(FakeServerMode::Interleaved).await;
    let mut notifications = client.subscribe_notifications();
    let value = client
        .request("account/rateLimits/read", json!({}))
        .await
        .unwrap();
    assert!(value.get("rateLimitsByLimitId").is_some());
    assert_eq!(client.metrics().unknown_notifications, 1);
    let notification = tokio::time::timeout(Duration::from_millis(200), notifications.recv())
        .await
        .expect("known notification delivery timeout")
        .expect("known notification channel closed");
    assert_eq!(notification.method, "account/updated");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), notifications.recv())
            .await
            .is_err(),
        "unknown notifications must not be dispatched"
    );
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn unknown_notification_mode_only_increments_notification_metric() {
    let client = fixture_client(FakeServerMode::UnknownNotification).await;
    let value = client
        .request("account/read", json!({ "refreshToken": false }))
        .await
        .unwrap();
    assert_eq!(value["account"]["type"], "chatgpt");
    assert_eq!(client.metrics().unknown_notifications, 1);
    assert_eq!(client.metrics().protocol_errors, 0);
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn out_of_order_response_is_not_correlated_to_another_request() {
    let client = fixture_client(FakeServerMode::OutOfOrder).await;
    let (account, limits) = tokio::join!(
        client.request("account/read", json!({ "refreshToken": false })),
        client.request("account/rateLimits/read", json!({})),
    );
    let account = account.unwrap();
    let limits = limits.unwrap();
    assert_eq!(account["account"]["type"], "chatgpt");
    assert!(limits.get("rateLimitsByLimitId").is_some());
    assert_eq!(client.metrics().unknown_responses, 0);
    assert_eq!(client.metrics().protocol_errors, 0);
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_response_id_is_counted_without_stealing_a_future_request() {
    let client = fixture_client(FakeServerMode::DuplicateId).await;
    let first = client
        .request("account/read", json!({ "refreshToken": false }))
        .await
        .unwrap();
    let second = client
        .request("account/rateLimits/read", json!({}))
        .await
        .unwrap();
    assert_eq!(first["account"]["type"], "chatgpt");
    assert!(second.get("rateLimitsByLimitId").is_some());
    assert!(client.metrics().unknown_responses >= 1);
    assert!(client.metrics().protocol_errors >= 1);
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn malformed_frames_fail_pending_requests_with_redacted_protocol_error() {
    for (mode, diagnostic) in [
        (FakeServerMode::InvalidJson, "APP_SERVER_INVALID_JSON"),
        (FakeServerMode::Truncated, "APP_SERVER_TRUNCATED_LINE"),
        (FakeServerMode::Oversized, "APP_SERVER_LINE_TOO_LARGE"),
    ] {
        let error = fixture_connect_error(mode).await;
        assert_eq!(error.kind, codexbar::core::AppErrorKind::ProtocolMismatch);
        assert_eq!(error.diagnostic_code, diagnostic);
        let serialized = serde_json::to_string(&error).unwrap();
        assert!(!serialized.contains("fixture"));
        assert!(!serialized.contains("token"));
    }
}

#[tokio::test]
async fn initialize_timeout_is_bounded_and_redacted() {
    let error = fixture_connect_error(FakeServerMode::InitializeTimeout).await;
    assert_eq!(error.kind, codexbar::core::AppErrorKind::OfflineOrTimeout);
    assert_eq!(error.diagnostic_code, "APP_SERVER_INITIALIZE_TIMEOUT");
}

#[tokio::test]
async fn rpc_timeout_does_not_hang_shutdown() {
    let client = fixture_client(FakeServerMode::RpcTimeout).await;
    let started = std::time::Instant::now();
    let error = client
        .request("account/read", json!({ "refreshToken": false }))
        .await
        .unwrap_err();
    assert_eq!(error.kind, codexbar::core::AppErrorKind::OfflineOrTimeout);
    assert_eq!(error.diagnostic_code, "APP_SERVER_RPC_TIMEOUT");
    client.shutdown().await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[tokio::test]
async fn crash_fails_all_pending_requests_with_the_same_redacted_error() {
    let client = fixture_client(FakeServerMode::Crash).await;
    let (first, second) = tokio::join!(
        client.request("account/read", json!({ "refreshToken": false })),
        client.request("account/rateLimits/read", json!({})),
    );
    let first = first.unwrap_err();
    let second = second.unwrap_err();
    assert_eq!(first, second);
    assert_eq!(first.kind, codexbar::core::AppErrorKind::OfflineOrTimeout);
    assert_eq!(first.diagnostic_code, "APP_SERVER_EOF");
    let serialized = serde_json::to_string(&first).unwrap();
    assert!(!serialized.contains("17"));
    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn refused_exit_is_killed_by_bounded_shutdown() {
    let client = fixture_client(FakeServerMode::RefuseExit).await;
    let started = std::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(6), client.shutdown())
        .await
        .expect("shutdown must be bounded")
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(6));
}

#[test]
fn initialize_params_keep_experimental_api_disabled() {
    let params = InitializeParams::v1();
    assert_eq!(
        serde_json::to_value(params).unwrap()["experimentalApi"],
        false
    );
}

#[test]
fn current_cli_impl_has_no_mutating_or_login_methods() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/providers/codex/app_server/session.rs");
    let source = std::fs::read_to_string(path).expect("session source");
    let start = source
        .find("impl CurrentCliSession")
        .expect("CurrentCli impl");
    let end = source[start..]
        .find("impl ManagedSession")
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    let current_cli_block = &source[start..end];
    for forbidden in [
        "start_login",
        "cancel_login",
        "logout",
        "delete",
        "write_config",
    ] {
        assert!(
            !current_cli_block.contains(forbidden),
            "CurrentCliSession must not expose {forbidden}"
        );
    }
}

#[tokio::test]
async fn current_cli_session_reads_account_and_rate_limits() {
    let client = fixture_client(FakeServerMode::Normal).await;
    let session = CurrentCliSession::from_client(client);
    let account = session.account_read(false).await.unwrap();
    let limits = session.rate_limits_read().await.unwrap();
    assert_eq!(account.auth_mode, codexbar::core::AuthMode::ChatGpt);
    assert!(limits.primary.is_some());
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_browser_login_uses_chatgpt_and_emits_completed_event() {
    let client = fixture_client(FakeServerMode::Normal).await;
    let mut session = ManagedSession::from_client(client);
    let challenge = session.start_login(LoginFlow::Browser).await.unwrap();
    assert_eq!(challenge.login_id, "login-browser");
    assert!(
        challenge
            .authorization_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://auth.openai.com/"))
    );
    let event = session.next_login_event().await.unwrap();
    assert!(matches!(
        event,
        LoginEvent::Completed { login_id } if login_id == "login-browser"
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_device_code_login_validates_exact_device_path_and_cancel() {
    let client = fixture_client(FakeServerMode::Normal).await;
    let mut session = ManagedSession::from_client(client);
    let challenge = session.start_login(LoginFlow::DeviceCode).await.unwrap();
    assert_eq!(challenge.login_id, "login-device");
    assert_eq!(
        challenge.verification_url.as_deref(),
        Some("https://auth.openai.com/codex/device")
    );
    assert_eq!(challenge.user_code.as_deref(), Some("ABCD-EFGH"));
    session.cancel_login(&challenge.login_id).await.unwrap();
    let event = session.next_login_event().await.unwrap();
    assert!(matches!(
        event,
        LoginEvent::Cancelled { login_id } if login_id == "login-device"
    ));
    session.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_login_failure_is_redacted_and_typed() {
    let client = fixture_client(FakeServerMode::LoginFailed).await;
    let mut session = ManagedSession::from_client(client);
    let challenge = session.start_login(LoginFlow::Browser).await.unwrap();
    assert_eq!(challenge.login_id, "login-browser");
    let event = session.next_login_event().await.unwrap();
    match event {
        LoginEvent::Failed { login_id, error } => {
            assert_eq!(login_id, "login-browser");
            assert_eq!(error.kind, codexbar::core::AppErrorKind::AuthExpired);
            assert_eq!(error.diagnostic_code, "APP_SERVER_LOGIN_FAILED");
            assert!(!serde_json::to_string(&error).unwrap().contains("fixture"));
        }
        other => panic!("expected failed login event, got {other:?}"),
    }
    session.shutdown().await.unwrap();
}

#[test]
fn managed_fixture_specs_isolate_two_codex_homes_and_auth_overrides() {
    let home_a = tempfile::TempDir::new().unwrap();
    let home_b = tempfile::TempDir::new().unwrap();
    let runtime = tempfile::TempDir::new().unwrap();
    let first = AppServerSpawnSpec::test_managed_fixture(
        FakeServerMode::Normal,
        home_a.path(),
        runtime.path(),
    )
    .unwrap();
    let second = AppServerSpawnSpec::test_managed_fixture(
        FakeServerMode::Normal,
        home_b.path(),
        runtime.path(),
    )
    .unwrap();
    assert_eq!(
        first.environment().get("CODEX_HOME"),
        Some(home_a.path().as_os_str())
    );
    assert_eq!(
        second.environment().get("CODEX_HOME"),
        Some(home_b.path().as_os_str())
    );
    assert_ne!(
        first.environment().get("CODEX_HOME"),
        second.environment().get("CODEX_HOME")
    );
    for key in ["OPENAI_API_KEY", "CODEX_ACCESS_TOKEN", "OPENAI_BASE_URL"] {
        assert!(first.environment().is_removed(key));
        assert!(second.environment().is_removed(key));
    }
}
