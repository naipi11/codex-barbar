use super::bridge::AppSettingsDto;
use crate::status_surfaces::controller::StatusSurfaceKind;

#[tauri::command]
pub async fn set_status_surface_enabled(
    app: tauri::AppHandle,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettingsDto, String> {
    crate::status_surfaces::controller::set_enabled_and_emit(&app, surface, enabled)
}

#[tauri::command]
pub async fn set_float_ball_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    crate::status_surfaces::set_float_ball_expanded(&app, expanded)
}

#[tauri::command]
pub async fn set_taskbar_status_width(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    width: f64,
) -> Result<(), String> {
    crate::status_surfaces::set_taskbar_status_width(&app, window.label(), width)
}

#[cfg(test)]
mod tests {
    use super::StatusSurfaceKind;

    #[test]
    fn set_status_surface_enabled_uses_frozen_wire_names() {
        assert_eq!(
            serde_json::from_str::<StatusSurfaceKind>(r#""taskbarStatus""#).unwrap(),
            StatusSurfaceKind::TaskbarStatus
        );
        assert_eq!(
            serde_json::from_str::<StatusSurfaceKind>(r#""floatBall""#).unwrap(),
            StatusSurfaceKind::FloatBall
        );
    }
}
