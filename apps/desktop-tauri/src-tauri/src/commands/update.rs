//! User-triggered, manual-only update checking.

use std::sync::Mutex;

use codexbar::update_check::{ManualUpdateChecker, ManualUpdateResult};

use crate::{
    notification_controller::{DesktopNotificationSink, NotificationController},
    state::AppState,
};

#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    controller: tauri::State<'_, Mutex<NotificationController<DesktopNotificationSink>>>,
) -> Result<ManualUpdateResult, String> {
    let result = ManualUpdateChecker::new().check().await?;
    if let ManualUpdateResult::Available { latest_version, .. } = &result
        && !crate::proof_harness::is_proof_mode(&app)
        && let Ok(repository) = super::settings::settings_repository(&state)
        && let Ok(mut controller) = controller.lock()
        && controller
            .observe_update_available(&repository, latest_version)
            .is_err()
    {
        tracing::warn!(
            code = "NOTIFICATION_UPDATE_DISPATCH_FAILED",
            "manual update notification was not delivered"
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_result_shape_is_frozen_and_redacted() {
        let result = ManualUpdateResult::Available {
            current_version: "1.0.0".to_string(),
            latest_version: "v0.1.1".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "available");
        assert_eq!(json["currentVersion"], "1.0.0");
        assert_eq!(json["latestVersion"], "v0.1.1");
        assert!(json.get("downloadUrl").is_none());
        assert!(json.get("installerPath").is_none());
    }
}
