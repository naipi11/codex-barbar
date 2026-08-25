use tauri::{Manager, WebviewUrl};

use super::geometry::{FLOAT_BALL_COLLAPSED_HEIGHT, FLOAT_BALL_COLLAPSED_WIDTH};
use crate::window_positioner::Rect;

pub const FLOAT_BALL_WINDOW_LABEL: &str = "float-ball";
pub const FLOAT_BALL_FRONTEND_ROUTE: &str = "index.html?window=float-ball";
pub const FLOAT_BALL_GEOMETRY_KEY: &str = "float-ball";

pub fn should_prevent_close(label: &str) -> bool {
    label == FLOAT_BALL_WINDOW_LABEL
}

pub fn get_or_create(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(FLOAT_BALL_WINDOW_LABEL) {
        crate::shell::dwm::apply_no_activate_tool_window(&window)?;
        return Ok(window);
    }

    let window = tauri::WebviewWindowBuilder::new(
        app,
        FLOAT_BALL_WINDOW_LABEL,
        WebviewUrl::App(FLOAT_BALL_FRONTEND_ROUTE.into()),
    )
    .title("codex-barbar float ball")
    .inner_size(
        f64::from(FLOAT_BALL_COLLAPSED_WIDTH),
        f64::from(FLOAT_BALL_COLLAPSED_HEIGHT),
    )
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(true)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .skip_taskbar(true)
    .always_on_top(true)
    .focusable(false)
    .focused(false)
    .theme(Some(tauri::Theme::Dark))
    .visible(false)
    .build()
    .map_err(|_| "FLOAT_BALL_WINDOW_CREATE_FAILED".to_string())?;

    crate::shell::dwm::apply_no_activate_tool_window(&window)?;
    if let Err(_error) = crate::shell::dwm::shape_round_window(&window) {
        tracing::warn!(
            code = "FLOAT_BALL_REGION_FAILED",
            "float ball window region shape failed"
        );
    }
    Ok(window)
}

pub fn position_and_show(window: &tauri::WebviewWindow, bounds: Rect) -> Result<(), String> {
    crate::shell::dwm::set_no_activate_bounds(
        window,
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        true,
    )?;
    // Keep the window clipped to a circle even if DWM/SetWindowPos dropped
    // the region during the move or resize.
    if let Err(_error) = crate::shell::dwm::shape_round_window(window) {
        tracing::warn!(
            code = "FLOAT_BALL_REGION_REAPPLY_FAILED",
            "float ball window region reapply failed"
        );
    }
    Ok(())
}

pub fn reassert_topmost(window: &tauri::WebviewWindow) -> Result<(), String> {
    crate::shell::dwm::reassert_topmost(window)
}

pub fn observe(
    window: &tauri::WebviewWindow,
) -> Result<crate::shell::dwm::OverlayWindowObservation, String> {
    crate::shell::dwm::observe_window(window)
}

pub fn show_noactivate(window: &tauri::WebviewWindow) -> Result<(), String> {
    crate::shell::dwm::show_noactivate(window)
}

#[allow(dead_code)]
pub fn hide(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .hide()
        .map_err(|_| "FLOAT_BALL_WINDOW_HIDE_FAILED".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_ball_window_contract_is_stable() {
        assert_eq!(FLOAT_BALL_WINDOW_LABEL, "float-ball");
        assert_eq!(FLOAT_BALL_FRONTEND_ROUTE, "index.html?window=float-ball");
        assert_eq!(FLOAT_BALL_GEOMETRY_KEY, "float-ball");
    }

    #[test]
    fn close_request_is_consumed_only_for_float_ball() {
        assert!(should_prevent_close(FLOAT_BALL_WINDOW_LABEL));
        assert!(!should_prevent_close("settings"));
    }
}
