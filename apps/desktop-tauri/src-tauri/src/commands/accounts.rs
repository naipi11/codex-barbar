//! Redacted account lifecycle commands for the V1 desktop shell.
//!
//! The WebView receives only profile summaries, cached usage states, and
//! managed-login status. It never receives tokens, auth.json paths, Vault
//! paths, Codex home paths, environment variables, or raw RPC data.

use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;

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
    use crate::commands::bridge::{ProfileSummaryDto, ProfileUsageStateDto};
    use codexbar::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};

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
}
