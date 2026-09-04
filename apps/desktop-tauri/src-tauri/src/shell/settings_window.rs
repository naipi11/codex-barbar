//! Detached Settings window: opens Settings/About in a separate window
//! so the tray panel stays open.

use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl};

pub const SETTINGS_LABEL: &str = "settings";
const SETTINGS_WIDTH: f64 = 680.0;
const SETTINGS_HEIGHT: f64 = 500.0;

#[derive(Debug, Clone, Copy)]
struct SettingsWindowChrome {
    decorations: bool,
    resizable: bool,
    minimizable: bool,
    maximizable: bool,
    closable: bool,
}

const SETTINGS_WINDOW_CHROME: SettingsWindowChrome = SettingsWindowChrome {
    decorations: true,
    resizable: true,
    minimizable: true,
    maximizable: true,
    closable: true,
};

fn settings_window_chrome() -> SettingsWindowChrome {
    SETTINGS_WINDOW_CHROME
}

/// Open the detached Settings window, or focus it if already open.
///
/// When the window already exists, emits `settings-change-tab` so the
/// frontend can switch to the requested tab without a full reload.
pub fn open_or_focus(app: &tauri::AppHandle, tab: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit_to(SETTINGS_LABEL, "settings-change-tab", tab)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let url = WebviewUrl::App(format!("index.html?window=settings&tab={tab}").into());

    let chrome = settings_window_chrome();
    let win = tauri::WebviewWindowBuilder::new(app, SETTINGS_LABEL, url)
        .title("codex-barbar")
        .inner_size(SETTINGS_WIDTH, SETTINGS_HEIGHT)
        .decorations(chrome.decorations)
        .shadow(true)
        .theme(Some(tauri::Theme::Dark))
        .resizable(chrome.resizable)
        .minimizable(chrome.minimizable)
        .maximizable(chrome.maximizable)
        .closable(chrome.closable)
        .build()
        .map_err(|e| e.to_string())?;

    // Keep the native caption dark while leaving its standard controls intact.
    #[cfg(windows)]
    super::dwm::force_native_dark_caption(&win);

    // Manually center: Tauri's .center() is unreliable on Windows when
    // called from async commands. Compute position from the primary monitor.
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = win.scale_factor().unwrap_or(1.0);
        let win_w = (SETTINGS_WIDTH * scale) as i32;
        let win_h = (SETTINGS_HEIGHT * scale) as i32;
        let x = pos.x + (size.width as i32 - win_w) / 2;
        let y = pos.y + (size.height as i32 - win_h) / 2;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    Ok(())
}

/// Dismiss Settings without exiting CodexBar.
///
/// The detached Settings window is hidden instead of closed so Tauri's
/// process/window lifecycle cannot interpret this as an app quit. If Settings
/// is rendered in the main shell surface, hide that surface back to tray.
pub fn dismiss(_app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == SETTINGS_LABEL {
        return window.hide().map_err(|e| e.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SETTINGS_HEIGHT, SETTINGS_WIDTH, settings_window_chrome};

    #[test]
    fn settings_window_uses_compact_default_size() {
        assert_eq!((SETTINGS_WIDTH, SETTINGS_HEIGHT), (680.0, 500.0));
    }

    #[test]
    fn settings_window_exposes_native_minimize_maximize_and_close_controls() {
        let chrome = settings_window_chrome();
        assert!(chrome.decorations);
        assert!(chrome.resizable);
        assert!(chrome.minimizable);
        assert!(chrome.maximizable);
        assert!(chrome.closable);
    }
}
