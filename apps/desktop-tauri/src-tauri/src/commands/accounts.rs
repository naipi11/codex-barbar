//! Redacted account lifecycle commands for the V1 desktop shell.
//!
//! The WebView receives only profile summaries, cached usage states, and
//! managed-login status. It never receives tokens, auth.json paths, Vault
//! paths, Codex home paths, environment variables, or raw RPC data.

use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;

use codexbar::accounts::avatar::{AvatarError, AvatarStore, decode_png_data_url};
use codexbar::accounts::model::{ManagedLoginMethod, StartManagedLogin};
use codexbar::accounts::service::AccountProfileService;
use codexbar::core::RefreshTrigger;

use super::bridge::{AccountsSnapshotDto, ManagedLoginStateDto};
use crate::state::AppState;

fn service(
    state: &tauri::State<'_, Mutex<AppState>>,
) -> Result<Arc<AccountProfileService>, String> {
    let guard = state.lock().map_err(|error| error.to_string())?;
    guard
        .account_service
        .clone()
        .ok_or_else(|| "account service unavailable (read-only database)".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectProfileArgs {
    pub profile_id: String,
}

#[tauri::command]
pub fn select_profile(
    state: tauri::State<'_, Mutex<AppState>>,
    args: SelectProfileArgs,
) -> Result<AccountsSnapshotDto, String> {
    let id = uuid::Uuid::parse_str(&args.profile_id).map_err(|error| error.to_string())?;
    let service = service(&state)?;
    let snapshot = service
        .select_profile(id)
        .map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    Ok(AccountsSnapshotDto::from_snapshot(snapshot, &identities))
}

#[tauri::command]
pub async fn refresh_selected_profile(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let selected = {
        let guard = state.lock().map_err(|error| error.to_string())?;
        guard
            .account_service
            .as_ref()
            .ok_or_else(|| "account service unavailable".to_string())?
            .snapshot()
            .map_err(|error| error.to_string())?
            .selected_profile_id
    };
    let _ = service(&state)?
        .request_refresh(selected, RefreshTrigger::Manual)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartManagedLoginArgs {
    pub label: String,
    pub method: String,
    pub replace_profile_id: Option<String>,
}

#[tauri::command]
pub async fn start_managed_login(
    state: tauri::State<'_, Mutex<AppState>>,
    args: StartManagedLoginArgs,
) -> Result<ManagedLoginStateDto, String> {
    let method = match args.method.as_str() {
        "deviceCode" => ManagedLoginMethod::DeviceCode,
        "browser" => ManagedLoginMethod::Browser,
        _ => return Err("unsupported managed login method".to_string()),
    };
    let replace_profile_id = args
        .replace_profile_id
        .as_deref()
        .map(uuid::Uuid::parse_str)
        .transpose()
        .map_err(|error| error.to_string())?;
    let status = service(&state)?
        .start_managed_login(StartManagedLogin {
            label: args.label,
            method,
            replace_profile_id,
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(ManagedLoginStateDto::from(&status))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelManagedLoginArgs {
    pub operation_id: String,
}

#[tauri::command]
pub async fn cancel_managed_login(
    state: tauri::State<'_, Mutex<AppState>>,
    args: CancelManagedLoginArgs,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&args.operation_id).map_err(|error| error.to_string())?;
    service(&state)?
        .cancel_managed_login(id)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameManagedProfileArgs {
    pub profile_id: String,
    pub label: String,
}

#[tauri::command]
pub fn rename_managed_profile(
    state: tauri::State<'_, Mutex<AppState>>,
    args: RenameManagedProfileArgs,
) -> Result<AccountsSnapshotDto, String> {
    let id = uuid::Uuid::parse_str(&args.profile_id).map_err(|error| error.to_string())?;
    let service = service(&state)?;
    let snapshot = service
        .rename_managed_profile(id, args.label)
        .map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    Ok(AccountsSnapshotDto::from_snapshot(snapshot, &identities))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveManagedProfileArgs {
    pub profile_id: String,
}

#[tauri::command]
pub async fn remove_managed_profile(
    state: tauri::State<'_, Mutex<AppState>>,
    args: RemoveManagedProfileArgs,
) -> Result<AccountsSnapshotDto, String> {
    let id = uuid::Uuid::parse_str(&args.profile_id).map_err(|error| error.to_string())?;
    let service = service(&state)?;
    let snapshot = service
        .remove_managed_profile(id)
        .await
        .map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    Ok(AccountsSnapshotDto::from_snapshot(snapshot, &identities))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileAvatarArgs {
    pub profile_id: String,
    pub png_data_url: String,
}

#[tauri::command]
pub fn save_profile_avatar(
    state: tauri::State<'_, Mutex<AppState>>,
    args: SaveProfileAvatarArgs,
) -> Result<AccountsSnapshotDto, String> {
    let profile_id =
        uuid::Uuid::parse_str(&args.profile_id).map_err(|_| "PROFILE_AVATAR_INVALID")?;
    let bytes =
        decode_profile_avatar_payload(&args.png_data_url).map_err(|error| error.to_string())?;
    let service = service(&state)?;
    service
        .save_profile_avatar(profile_id, &bytes)
        .map_err(|error| error.to_string())?;
    let snapshot = service.snapshot().map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    Ok(AccountsSnapshotDto::from_snapshot(snapshot, &identities))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearProfileAvatarArgs {
    pub profile_id: String,
}

#[tauri::command]
pub fn clear_profile_avatar(
    state: tauri::State<'_, Mutex<AppState>>,
    args: ClearProfileAvatarArgs,
) -> Result<AccountsSnapshotDto, String> {
    let profile_id =
        uuid::Uuid::parse_str(&args.profile_id).map_err(|_| "PROFILE_AVATAR_INVALID")?;
    let service = service(&state)?;
    service
        .clear_profile_avatar(profile_id)
        .map_err(|error| error.to_string())?;
    let snapshot = service.snapshot().map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    Ok(AccountsSnapshotDto::from_snapshot(snapshot, &identities))
}

fn decode_profile_avatar_payload(value: &str) -> Result<Vec<u8>, AvatarError> {
    decode_png_data_url(value)
}

pub(crate) fn account_avatar_protocol_response(
    store: &AvatarStore,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Method, StatusCode};

    if request.method() != Method::GET {
        return empty_avatar_response(StatusCode::METHOD_NOT_ALLOWED);
    }
    if !request.body().is_empty() {
        return empty_avatar_response(StatusCode::BAD_REQUEST);
    }
    let Some((profile_id, revision)) = parse_avatar_request_uri(request.uri()) else {
        return empty_avatar_response(StatusCode::BAD_REQUEST);
    };
    match store.read_asset(profile_id, revision) {
        Ok(Some(bytes)) => tauri::http::Response::builder()
            .status(StatusCode::OK)
            .header(tauri::http::header::CONTENT_TYPE, "image/png")
            .header(
                tauri::http::header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            )
            .header("x-content-type-options", "nosniff")
            .body(bytes)
            .unwrap_or_else(|_| empty_avatar_response(StatusCode::INTERNAL_SERVER_ERROR)),
        Ok(None) | Err(_) => empty_avatar_response(StatusCode::NOT_FOUND),
    }
}

fn parse_avatar_request_uri(uri: &tauri::http::Uri) -> Option<(uuid::Uuid, &str)> {
    let authority = uri.authority()?.as_str();
    let path = uri.path();
    let profile_id = match (uri.scheme_str()?, authority) {
        ("account-avatar", "profile") => path.strip_prefix('/')?,
        ("http" | "https", "account-avatar.localhost") => path.strip_prefix("/profile/")?,
        _ => return None,
    };
    if profile_id.is_empty() || profile_id.contains('/') {
        return None;
    }
    let profile_id = uuid::Uuid::parse_str(profile_id).ok()?;
    let revision = uri.query()?.strip_prefix("rev=")?;
    if revision.len() != 64
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some((profile_id, revision))
}

fn empty_avatar_response(status: tauri::http::StatusCode) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header("x-content-type-options", "nosniff")
        .body(Vec::new())
        .expect("static avatar response")
}

/// Internal graceful shutdown helper.  It is intentionally not registered as
/// a second Tauri command; `quit_app` is the sole public quit entry point.
pub fn request_graceful_quit(app: tauri::AppHandle, state: tauri::State<'_, Mutex<AppState>>) {
    let coordinator = {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        guard.coordinator.clone()
    };
    let service = match state.lock() {
        Ok(guard) => guard.account_service.clone(),
        Err(_) => return,
    };
    tracing::debug!(
        state = ?coordinator.quit_state(),
        "graceful quit requested"
    );
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        coordinator
            .request_quit(|| async {
                let Some(service) = service else {
                    app_handle.exit(0);
                    return;
                };
                let _ = tokio::time::timeout(
                    crate::app_coordinator::StartupBudget::CACHED_TRAY_READY,
                    service.shutdown(std::time::Duration::from_secs(3)),
                )
                .await;
                app_handle.exit(0);
            })
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::{account_avatar_protocol_response, decode_profile_avatar_payload};
    use crate::commands::bridge::{ProfileSummaryDto, ProfileUsageStateDto};
    use codexbar::accounts::avatar::AvatarStore;
    use codexbar::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};
    use codexbar::accounts::presentation::avatar_asset_uri;
    use tauri::http::{Method, Request, StatusCode};

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("codexbar-avatar-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn valid_png_bytes() -> Vec<u8> {
        vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137,
        ]
    }

    #[test]
    fn profile_dto_never_carries_secret_fields() {
        let profile = AccountProfile {
            id: uuid::Uuid::nil(),
            kind: ProfileKind::Managed,
            label: "Managed".to_string(),
            auth_mode: codexbar::core::AuthMode::ChatGpt,
            lifecycle: ProfileLifecycle::Ready,
            email_fingerprint: Some([7; 32]),
            created_at: chrono::Utc::now(),
            last_selected_at: None,
            last_success_at: None,
        };
        let dto = ProfileSummaryDto::from_profile(&profile, None);
        let json = serde_json::to_value(&dto).unwrap();
        let text = json.to_string().to_ascii_lowercase();
        assert_eq!(json["removable"], true);
        for forbidden in ["token", "authjson", "codexhome", "vaultpath", "fingerprint"] {
            assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
        }
    }

    #[test]
    fn usage_state_dto_keeps_success_and_error_fields() {
        let state = codexbar::core::ProfileUsageState::missing(uuid::Uuid::nil());
        let dto = ProfileUsageStateDto::from_state(&state);
        assert!(dto.primary.is_none());
        assert!(dto.current_error.is_none());
        assert_eq!(dto.freshness, "missing");
    }

    #[test]
    fn avatar_protocol_serves_only_matching_profile_revision_as_png() {
        let dir = TestDirectory::new();
        let store = AvatarStore::new(dir.path().to_path_buf());
        let profile_id = uuid::Uuid::from_u128(9);
        let asset = store.write_manual(profile_id, &valid_png_bytes()).unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(avatar_asset_uri(profile_id, &asset.revision))
            .body(Vec::new())
            .unwrap();

        let response = account_avatar_protocol_response(&store, request);

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/png");
        assert_eq!(response.body(), &valid_png_bytes());
    }

    #[test]
    fn avatar_protocol_rejects_invalid_paths_queries_and_methods() {
        let dir = TestDirectory::new();
        let store = AvatarStore::new(dir.path().to_path_buf());
        let profile_id = uuid::Uuid::from_u128(9);
        let asset = store.write_manual(profile_id, &valid_png_bytes()).unwrap();
        let invalid = [
            format!(
                "account-avatar://profile/{profile_id}/extra?rev={}",
                asset.revision
            ),
            format!("account-avatar://profile/not-a-uuid?rev={}", asset.revision),
            format!(
                "account-avatar://profile/{profile_id}?rev={}&extra=1",
                asset.revision
            ),
            format!("account-avatar://profile/{profile_id}?rev=not-opaque"),
        ];
        for uri in invalid {
            let request = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Vec::new())
                .unwrap();
            assert_ne!(
                account_avatar_protocol_response(&store, request).status(),
                StatusCode::OK
            );
        }
        let request = Request::builder()
            .method(Method::POST)
            .uri(avatar_asset_uri(profile_id, &asset.revision))
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            account_avatar_protocol_response(&store, request).status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn manual_avatar_payload_rejects_non_png_data_urls() {
        assert!(decode_profile_avatar_payload("data:text/plain;base64,AA==").is_err());
    }
}
