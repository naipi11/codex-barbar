use tauri::{Manager, WebviewUrl};

use super::geometry::{FLOAT_BALL_COLLAPSED_HEIGHT, FLOAT_BALL_COLLAPSED_WIDTH};
use crate::window_positioner::Rect;

pub const FLOAT_BALL_WINDOW_LABEL: &str = "float-ball";
pub const FLOAT_BALL_FRONTEND_ROUTE: &str = "index.html?window=float-ball";
pub const FLOAT_BALL_GEOMETRY_KEY: &str = "float-ball";

#[cfg(windows)]
pub type WindowObservation = crate::shell::dwm::OverlayWindowObservation;

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowObservation {
    pub visible: bool,
    pub minimized: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn should_prevent_close(label: &str) -> bool {
    label == FLOAT_BALL_WINDOW_LABEL
}

#[cfg(windows)]
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

#[cfg(not(windows))]
pub fn get_or_create(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(FLOAT_BALL_WINDOW_LABEL) {
        if window.set_always_on_top(true).is_err() {
            tracing::debug!(
                code = "FLOAT_BALL_TOPMOST_UNAVAILABLE",
                "float ball always-on-top preference is unavailable"
            );
        }
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
    .focusable(true)
    .focused(false)
    .theme(Some(tauri::Theme::Dark))
    .visible(false)
    .build()
    .map_err(|_| "FLOAT_BALL_WINDOW_CREATE_FAILED".to_string())?;

    if window.set_always_on_top(true).is_err() {
        tracing::debug!(
            code = "FLOAT_BALL_TOPMOST_UNAVAILABLE",
            "float ball always-on-top preference is unavailable"
        );
    }
    Ok(window)
}

#[cfg(windows)]
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

#[cfg(not(windows))]
pub fn position_and_show(window: &tauri::WebviewWindow, bounds: Rect) -> Result<(), String> {
    window
        .set_position(tauri::PhysicalPosition::new(bounds.x, bounds.y))
        .map_err(|_| "FLOAT_BALL_WINDOW_POSITION_FAILED".to_string())?;
    window
        .set_size(tauri::PhysicalSize::new(
            bounds.width.max(1) as u32,
            bounds.height.max(1) as u32,
        ))
        .map_err(|_| "FLOAT_BALL_WINDOW_SIZE_FAILED".to_string())?;
    window
        .show()
        .map_err(|_| "FLOAT_BALL_WINDOW_SHOW_FAILED".to_string())
}

#[cfg(windows)]
pub fn reassert_topmost(window: &tauri::WebviewWindow) -> Result<(), String> {
    crate::shell::dwm::reassert_topmost(window)
}

#[cfg(not(windows))]
pub fn reassert_topmost(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.set_always_on_top(true).is_err() {
        tracing::debug!(
            code = "FLOAT_BALL_TOPMOST_UNAVAILABLE",
            "float ball always-on-top preference is unavailable"
        );
    }
    Ok(())
}

#[cfg(windows)]
pub fn observe(window: &tauri::WebviewWindow) -> Result<WindowObservation, String> {
    crate::shell::dwm::observe_window(window)
}

#[cfg(not(windows))]
pub fn observe(window: &tauri::WebviewWindow) -> Result<WindowObservation, String> {
    let visible = window.is_visible().unwrap_or(false);
    let position = window
        .outer_position()
        .map_err(|_| "FLOAT_BALL_WINDOW_OBSERVE_FAILED".to_string())?;
    let size = window
        .outer_size()
        .map_err(|_| "FLOAT_BALL_WINDOW_OBSERVE_FAILED".to_string())?;
    Ok(WindowObservation {
        visible,
        minimized: false,
        x: position.x,
        y: position.y,
        width: size.width.min(i32::MAX as u32) as i32,
        height: size.height.min(i32::MAX as u32) as i32,
    })
}

#[cfg(windows)]
pub fn show_noactivate(window: &tauri::WebviewWindow) -> Result<(), String> {
    crate::shell::dwm::show_noactivate(window)
}

#[cfg(not(windows))]
pub fn show_noactivate(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .show()
        .map_err(|_| "FLOAT_BALL_WINDOW_SHOW_FAILED".to_string())
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
