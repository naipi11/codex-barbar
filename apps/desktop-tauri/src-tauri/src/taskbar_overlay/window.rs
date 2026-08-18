use tauri::{Manager, WebviewUrl};

use super::positioning::Rect;

pub const TASKBAR_WINDOW_LABEL: &str = "taskbar-status";
pub const TASKBAR_FRONTEND_ROUTE: &str = "index.html?window=taskbar-status";
pub const TASKBAR_MEASUREMENT_WINDOW_LABEL: &str = "taskbar-status-measure";
pub const TASKBAR_MEASUREMENT_FRONTEND_ROUTE: &str = "index.html?window=taskbar-status-measure";
pub const TASKBAR_MEASUREMENT_LOGICAL_WIDTH: u32 = 318;
pub const TASKBAR_MIN_LOGICAL_WIDTH: u32 = 104;
pub const TASKBAR_MAX_LOGICAL_WIDTH: u32 = 318;
pub const TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH: u32 = 318;
pub const TASKBAR_LOGICAL_HEIGHT: u32 = 40;

pub fn get_or_create(
    app: &tauri::AppHandle,
    logical_width: u32,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(TASKBAR_WINDOW_LABEL) {
        crate::shell::dwm::apply_no_activate_tool_window(&window)?;
        return Ok(window);
    }

    let window = tauri::WebviewWindowBuilder::new(
        app,
        TASKBAR_WINDOW_LABEL,
        WebviewUrl::App(TASKBAR_FRONTEND_ROUTE.into()),
    )
    .title("codex-barbar taskbar status")
    .inner_size(f64::from(logical_width), f64::from(TASKBAR_LOGICAL_HEIGHT))
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
    .map_err(|_| "TASKBAR_WINDOW_CREATE_FAILED".to_string())?;

    crate::shell::dwm::apply_no_activate_tool_window(&window)?;
    Ok(window)
}

pub fn get_or_create_measurement(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(TASKBAR_MEASUREMENT_WINDOW_LABEL) {
        return Ok(window);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        TASKBAR_MEASUREMENT_WINDOW_LABEL,
        WebviewUrl::App(TASKBAR_MEASUREMENT_FRONTEND_ROUTE.into()),
    )
    .title("codex-barbar taskbar measurement")
    .inner_size(
        f64::from(TASKBAR_MEASUREMENT_LOGICAL_WIDTH),
        f64::from(TASKBAR_LOGICAL_HEIGHT),
    )
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .skip_taskbar(true)
    .focusable(false)
    .focused(false)
    .theme(Some(tauri::Theme::Dark))
    .visible(false)
    .build()
    .map_err(|_| "TASKBAR_MEASUREMENT_WINDOW_CREATE_FAILED".to_string())
}

pub fn is_measurement_window_label(label: &str) -> bool {
    label == TASKBAR_MEASUREMENT_WINDOW_LABEL
}

pub fn position_and_show(window: &tauri::WebviewWindow, slot: Rect) -> Result<(), String> {
    crate::shell::dwm::set_no_activate_bounds(window, slot.x, slot.y, slot.width, slot.height, true)
}

pub fn hide(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .hide()
        .map_err(|_| "TASKBAR_WINDOW_HIDE_FAILED".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_label_and_frontend_route_are_stable() {
        assert_eq!(TASKBAR_WINDOW_LABEL, "taskbar-status");
        assert_eq!(TASKBAR_FRONTEND_ROUTE, "index.html?window=taskbar-status");
    }

    #[test]
    fn taskbar_dimensions_include_a_functional_safe_fallback() {
        assert_eq!(TASKBAR_MIN_LOGICAL_WIDTH, 104);
        assert_eq!(TASKBAR_MAX_LOGICAL_WIDTH, 318);
        assert_eq!(TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH, 318);
        assert_eq!(TASKBAR_LOGICAL_HEIGHT, 40);
    }

    #[test]
    fn measurement_window_contract_is_fixed_and_hidden() {
        assert_eq!(TASKBAR_MEASUREMENT_WINDOW_LABEL, "taskbar-status-measure");
        assert_eq!(
            TASKBAR_MEASUREMENT_FRONTEND_ROUTE,
            "index.html?window=taskbar-status-measure"
        );
        assert_eq!(TASKBAR_MEASUREMENT_LOGICAL_WIDTH, 318);
        assert_eq!(TASKBAR_LOGICAL_HEIGHT, 40);
        assert!(is_measurement_window_label("taskbar-status-measure"));
        assert!(!is_measurement_window_label("taskbar-status"));
    }
}
