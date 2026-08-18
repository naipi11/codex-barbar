//! User-triggered, manual-only update checking.

use codexbar::update_check::{ManualUpdateChecker, ManualUpdateResult};

#[tauri::command]
pub async fn check_for_updates() -> Result<ManualUpdateResult, String> {
    ManualUpdateChecker::new().check().await
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
