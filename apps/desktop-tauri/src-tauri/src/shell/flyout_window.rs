//! Tray flyout window: the hidden `main` WebView shown as a movable,
//! fixed-size, always-on-top panel that stays above the taskbar.

use crate::window_positioner::{Rect, clamp_to_monitor, place_flyout_sized, subtract_taskbar};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// The tray panel is the configured `main` WebView. Keeping this label in
/// sync with `App.tsx` is important: unknown labels intentionally render
/// nothing, so a second `flyout` WebView would show only its background.
pub const FLYOUT_LABEL: &str = "main";
const FLYOUT_SIZE_KEY: &str = "flyout";
const MIN_LOGICAL_WIDTH: u32 = 320;
const MIN_LOGICAL_HEIGHT: u32 = 320;
static FLYOUT_INTERACTING: AtomicBool = AtomicBool::new(false);

/// Whether the tray panel currently exists and is visible.
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(FLYOUT_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// Persist a user-chosen logical size so the next open keeps it.
pub fn save_stored_size(width: u32, height: u32) {
    crate::geometry_store::save_size(
        FLYOUT_SIZE_KEY,
        crate::geometry_store::StoredSize { width, height },
    );
}

fn stored_logical_size() -> (u32, u32) {
    crate::window_positioner::FLYOUT_LOGICAL_SIZE
}

/// Resize the visible flyout to hug content, then re-anchor it above the taskbar.
pub fn apply_content_size(app: &AppHandle, width: u32, height: u32) -> Result<(), String> {
    let width = width.clamp(MIN_LOGICAL_WIDTH, 720);
    let height = height.clamp(MIN_LOGICAL_HEIGHT, 900);
    save_stored_size(width, height);
    let Some(window) = app.get_webview_window(FLYOUT_LABEL) else {
        return Ok(());
    };
    if !window.is_visible().unwrap_or(false) {
        return Ok(());
    }
    let Some(monitor) = current_monitor(&window) else {
        return Ok(());
    };
    let (physical_width, physical_height) = physical_size_for(&monitor, (width, height));
    let position = window.outer_position().ok();
    let rect = if let Some(position) = position {
        clamp_to_monitor(
            monitor_rect(&monitor),
            position.x,
            position.y,
            physical_width,
            physical_height,
        )
    } else {
        place_flyout_sized(usable_work_rect(&monitor), physical_width, physical_height)
    };
    apply_rect(&window, rect)?;
    let _ = crate::shell::dwm::shape_rounded_rect_window(&window, 28);
    Ok(())
}
pub fn set_interacting(active: bool) {
    FLYOUT_INTERACTING.store(active, Ordering::SeqCst);
}

pub fn should_blur_dismiss() -> bool {
    !FLYOUT_INTERACTING.load(Ordering::SeqCst)
}

fn usable_work_rect(monitor: &tauri::Monitor) -> Rect {
    let work = work_rect(monitor);
    crate::taskbar_overlay::win32::discover_native()
        .map(|snapshot| subtract_taskbar(work, snapshot.taskbar))
        .unwrap_or(work)
}
fn monitor_rect(monitor: &tauri::Monitor) -> Rect {
    let size = monitor.size();
    let pos = monitor.position();
    Rect {
        x: pos.x,
        y: pos.y,
        width: size.width.min(i32::MAX as u32) as i32,
        height: size.height.min(i32::MAX as u32) as i32,
    }
}
fn work_rect(monitor: &tauri::Monitor) -> Rect {
    let work_area = monitor.work_area();
    Rect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width.min(i32::MAX as u32) as i32,
        height: work_area.size.height.min(i32::MAX as u32) as i32,
    }
}

fn current_monitor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| window.available_monitors().ok()?.into_iter().next())
}

fn physical_size_for(monitor: &tauri::Monitor, logical: (u32, u32)) -> (i32, i32) {
    let scale = monitor.scale_factor().max(1.0);
    (
        (f64::from(logical.0) * scale).round() as i32,
        (f64::from(logical.1) * scale).round() as i32,
    )
}

fn apply_rect(window: &tauri::WebviewWindow, rect: Rect) -> Result<(), String> {
    window
        .set_size(PhysicalSize::new(
            rect.width.max(1) as u32,
            rect.height.max(1) as u32,
        ))
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(rect.x, rect.y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Compute the tray-anchored rectangle from the current or stored size.
fn anchored_rect(app: &AppHandle) -> Option<Rect> {
    let window = app.get_webview_window(FLYOUT_LABEL)?;
    let monitor = current_monitor(&window)?;
    let work = usable_work_rect(&monitor);
    let logical = stored_logical_size();
    let (width, height) = physical_size_for(&monitor, logical);
    Some(place_flyout_sized(work, width, height))
}

/// Keep the visible flyout fully above the taskbar after a user resize/move.
pub fn keep_inside_work_area(app: &AppHandle) {
    let Some(window) = app.get_webview_window(FLYOUT_LABEL) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) || !should_blur_dismiss() {
        return;
    }
    let Some(monitor) = current_monitor(&window) else {
        return;
    };
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let next = clamp_to_monitor(
        monitor_rect(&monitor),
        position.x,
        position.y,
        size.width.max(1) as i32,
        size.height.max(1) as i32,
    );
    if next.x != position.x
        || next.y != position.y
        || next.width != size.width as i32
        || next.height != size.height as i32
    {
        let _ = apply_rect(&window, next);
        let _ = crate::shell::dwm::shape_rounded_rect_window(&window, 28);
    } else {
        let _ = crate::shell::dwm::shape_rounded_rect_window(&window, 28);
    }
}

/// Show and focus the configured `main` WebView above the taskbar.
pub fn open_or_focus(app: &AppHandle, position: Option<(i32, i32)>) -> Result<(), String> {
    let window = app
        .get_webview_window(FLYOUT_LABEL)
        .ok_or_else(|| "main tray panel window is unavailable".to_string())?;

    crate::shell::dwm::force_dark_caption(&window);
    let _ = window.set_resizable(false);
    let _ = window.set_shadow(true);

    let rect = if let Some((x, y)) = position {
        let monitor = current_monitor(&window);
        let size = window.outer_size().ok();
        if let (Some(monitor), Some(size)) = (monitor.as_ref(), size) {
            clamp_to_monitor(
                monitor_rect(monitor),
                x,
                y,
                size.width.max(1) as i32,
                size.height.max(1) as i32,
            )
        } else {
            anchored_rect(app).unwrap_or(Rect {
                x,
                y,
                width: 400,
                height: 400,
            })
        }
    } else {
        anchored_rect(app).ok_or_else(|| "unable to resolve tray panel position".to_string())?
    };
    apply_rect(&window, rect)?;
    let _ = crate::shell::dwm::shape_rounded_rect_window(&window, 28);
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
    use crate::window_positioner::{Rect, WORK_AREA_INSET, place_flyout_sized};

    #[test]
    fn tray_panel_window_label_matches_frontend_route() {
        assert_eq!(FLYOUT_LABEL, "main");
    }

    #[test]
    fn default_open_leaves_a_gap_above_the_taskbar() {
        let work = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let rect = place_flyout_sized(work, 400, 520);
        assert_eq!(rect.y + rect.height + WORK_AREA_INSET, work.height);
    }
}
