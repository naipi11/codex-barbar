//! V1 tray flyout window: the hidden `main` WebView shown as a resizable,
//! always-on-top, tray-anchored panel that auto-hides on click-outside.

use crate::window_positioner::{Rect, place_flyout};
use tauri::{AppHandle, Manager, PhysicalPosition};

/// The tray panel is the configured `main` WebView. Keeping this label in
/// sync with `App.tsx` is important: unknown labels intentionally render
/// nothing, so a second `flyout` WebView would show only its background.
pub const FLYOUT_LABEL: &str = "main";

/// Whether the tray panel currently exists and is visible.
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(FLYOUT_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// Placeholder size persistence — the panel size is configured in
/// `tauri.conf.json` until the native size bridge is wired.
pub fn save_stored_size(_width: u32, _height: u32) {}

/// Compute the tray-anchored position: bottom-right of the primary monitor's
/// work area with a small margin.
fn anchored_position(app: &AppHandle) -> Option<(i32, i32)> {
    let window = app.get_webview_window(FLYOUT_LABEL)?;
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.available_monitors().ok()?.into_iter().next())?;
    let work_area = monitor.work_area();
    let scale = monitor.scale_factor().max(1.0);
    let work_rect = Rect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width as i32,
        height: work_area.size.height as i32,
    };
    let rect = place_flyout(work_rect, scale);
    Some((rect.x, rect.y))
}

/// Show and focus the configured `main` WebView at the tray-anchored position.
pub fn open_or_focus(app: &AppHandle, position: Option<(i32, i32)>) -> Result<(), String> {
    let window = app
        .get_webview_window(FLYOUT_LABEL)
        .ok_or_else(|| "main tray panel window is unavailable".to_string())?;

    if let Some((x, y)) = position.or_else(|| anchored_position(app)) {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    }
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Toggle the tray panel open/closed for the tray-icon left click.
pub fn toggle_with_blur_consume(app: &AppHandle, position: Option<(i32, i32)>) {
    if is_open(app) {
        let _ = hide(app);
    } else {
        let _ = open_or_focus(app, position);
    }
}

pub fn hide(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(FLYOUT_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::FLYOUT_LABEL;

    #[test]
    fn tray_panel_window_label_matches_frontend_route() {
        assert_eq!(FLYOUT_LABEL, "main");
    }
}
