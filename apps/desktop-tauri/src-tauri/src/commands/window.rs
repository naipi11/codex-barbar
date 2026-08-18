//! Fixed tray/settings window commands for the V1 desktop shell.
//!
//! Everything here takes only trusted, fixed arguments — no arbitrary URLs,
//! paths, or provider selectors cross the WebView boundary in Phase 0.

use std::sync::Mutex;

use crate::state::AppState;

/// Open (or focus) the detached Settings window on the General tab.
///
/// `WebviewWindowBuilder::build` deadlocks inside synchronous Tauri commands
/// on Windows, so this must stay `async`.
#[tauri::command]
pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::shell::settings_window::open_or_focus(&app, "general")
}

#[tauri::command]
pub fn close_settings_window(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    crate::shell::settings_window::dismiss(&app, &window)
}

#[tauri::command]
pub fn dismiss_tray_panel(app: tauri::AppHandle) -> Result<(), String> {
    crate::shell::flyout_window::hide(&app)
}

#[tauri::command]
pub fn open_tray_panel(app: tauri::AppHandle) -> Result<(), String> {
    crate::shell::flyout_window::open_or_focus(&app, None)
}

/// Persist a user-chosen size for the tray flyout window. Only the size is
/// stored; the flyout is always re-anchored above the tray on open.
#[tauri::command]
pub fn set_flyout_size(width: f64, height: f64) -> Result<(), String> {
    let width = (width.round() as i64).clamp(1, i64::from(u32::MAX)) as u32;
    let height = (height.round() as i64).clamp(1, i64::from(u32::MAX)) as u32;
    crate::shell::flyout_window::save_stored_size(width, height);
    Ok(())
}

/// Serializable view of the current shell surface for the frontend bridge.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceStateDto {
    pub mode: String,
    pub target: crate::surface_target::SurfaceTarget,
}

#[tauri::command]
pub fn get_current_surface_state(
    state: tauri::State<'_, Mutex<AppState>>,
) -> Result<SurfaceStateDto, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(SurfaceStateDto {
        mode: guard.surface_machine.current().as_str().to_string(),
        target: guard.current_target.clone(),
    })
}

/// Quit the application. Phase 0 only stops the current shell process;
/// Phase 2 adds bounded profile sealing and Phase 4 adds final shutdown
/// orchestration.
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle, state: tauri::State<'_, Mutex<AppState>>) {
    crate::commands::request_graceful_quit(app, state);
}
