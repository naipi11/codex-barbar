//! Windows DWM helpers for eliminating the non-client caption area.
//!
//! Even with `decorations(false)`, Windows keeps a thin caption strip
//! that DWM renders. We install a window subclass that intercepts
//! WM_NCCALCSIZE to zero the non-client area and WM_NCPAINT/WM_NCACTIVATE
//! to suppress DWM painting, making the window truly borderless.

#[cfg(windows)]
use raw_window_handle::HasWindowHandle;
#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: isize, attr: u32, data: *const c_void, size: u32) -> i32;
    fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const Margins) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinPoint {
    x: i32,
    y: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// Win32 `MINMAXINFO`. `lparam` of `WM_GETMINMAXINFO` points at one of these.
#[cfg(windows)]
#[repr(C)]
struct MinMaxInfo {
    reserved: WinPoint,
    max_size: WinPoint,
    max_position: WinPoint,
    min_track_size: WinPoint,
    max_track_size: WinPoint,
}

/// Win32 `MONITORINFO` (40 bytes). `cb_size` must be set before the call.
#[cfg(windows)]
#[repr(C)]
struct MonitorInfo {
    cb_size: u32,
    rc_monitor: WinRect,
    rc_work: WinRect,
    dw_flags: u32,
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAncestor(hwnd: isize, flags: u32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, new: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn GetClientRect(hwnd: isize, rect: *mut WinRect) -> i32;
    fn SetWindowRgn(hwnd: isize, rgn: isize, redraw: i32) -> i32;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn GetMonitorInfoW(hmonitor: isize, info: *mut MonitorInfo) -> i32;
}

#[cfg(windows)]
#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(
        hwnd: isize,
        pfn: unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize,
        id: usize,
        data: usize,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn CreateEllipticRgn(left: i32, top: i32, right: i32, bottom: i32) -> isize;
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> isize;
}

#[cfg(windows)]
static DARK_BRUSH: std::sync::OnceLock<isize> = std::sync::OnceLock::new();

#[cfg(windows)]
const WM_NCCALCSIZE: u32 = 0x0083;
#[cfg(windows)]
const WM_NCPAINT: u32 = 0x0085;
#[cfg(windows)]
const WM_NCACTIVATE: u32 = 0x0086;
#[cfg(windows)]
const WM_GETMINMAXINFO: u32 = 0x0024;
#[cfg(windows)]
const WM_ENTERSIZEMOVE: u32 = 0x0231;
#[cfg(windows)]
const WM_EXITSIZEMOVE: u32 = 0x0232;
#[cfg(windows)]
const BORDERLESS_SUBCLASS_ID: usize = 0xC0DE_BA12;

#[cfg(windows)]
unsafe extern "system" fn borderless_subclass_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    match msg {
        WM_NCCALCSIZE => {
            if wparam != 0 {
                // Returning 0 when wparam is TRUE tells Windows the
                // client area == the window area (no non-client area).
                return 0;
            }
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_NCPAINT => {
            // Suppress DWM non-client painting entirely.
            0
        }
        WM_NCACTIVATE => {
            // Return TRUE to accept activation but skip DWM painting.
            1
        }
        WM_ENTERSIZEMOVE => {
            crate::shell::flyout_window::set_interacting(true);
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_EXITSIZEMOVE => {
            crate::shell::flyout_window::set_interacting(false);
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_GETMINMAXINFO => {
            // A borderless window whose non-client area is zeroed maximizes to
            // cover the entire monitor, including the taskbar. Constrain the
            // maximized position/size to the monitor work area instead.
            const MONITOR_DEFAULTTONEAREST: u32 = 2;
            unsafe {
                let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                if hmon != 0 && lparam != 0 {
                    let mut mi = MonitorInfo {
                        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                        rc_monitor: WinRect::default(),
                        rc_work: WinRect::default(),
                        dw_flags: 0,
                    };
                    if GetMonitorInfoW(hmon, &mut mi) != 0 {
                        let mmi = lparam as *mut MinMaxInfo;
                        (*mmi).max_position = WinPoint {
                            x: mi.rc_work.left - mi.rc_monitor.left,
                            y: mi.rc_work.top - mi.rc_monitor.top,
                        };
                        (*mmi).max_size = WinPoint {
                            x: mi.rc_work.right - mi.rc_work.left,
                            y: mi.rc_work.bottom - mi.rc_work.top,
                        };
                        (*mmi).max_track_size = (*mmi).max_size;
                    }
                }
            }
            0
        }
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// Eliminate the DWM caption bar by subclassing the window to zero the
/// non-client area.  Safe to call on multiple windows — each gets its
/// own subclass via `SetWindowSubclass`.
///
/// When `resizable` is true, `WS_THICKFRAME` is preserved so the native
/// resize affordance still works.
#[cfg(windows)]
pub fn force_dark_caption(win: &tauri::WebviewWindow) {
    force_dark_caption_inner(win, false);
}

/// Same as [`force_dark_caption`] but keeps the resize frame.
#[cfg(windows)]
pub fn force_dark_caption_resizable(win: &tauri::WebviewWindow) {
    force_dark_caption_inner(win, true);
}

#[cfg(windows)]
fn force_dark_caption_inner(win: &tauri::WebviewWindow, keep_resize: bool) {
    let Ok(hwnd) = root_hwnd(win) else {
        tracing::warn!("dwm: couldn't resolve root window handle");
        return;
    };
    tracing::info!("dwm: root={hwnd:#x}");

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_CAPTION_COLOR: u32 = 35;
    let dark_mode: u32 = 1;
    let caption_color: u32 = 0x001C1C1E;

    unsafe {
        let r1 = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &raw const dark_mode as *const c_void,
            4,
        );
        let r2 = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &raw const caption_color as *const c_void,
            4,
        );
        tracing::info!("dwm: dark_mode={r1:#x} caption_color={r2:#x}");

        // Extend DWM frame fully into client area
        let margins = Margins {
            left: -1,
            right: -1,
            top: -1,
            bottom: -1,
        };
        let r3 = DwmExtendFrameIntoClientArea(hwnd, &margins);
        tracing::info!("dwm: extend_frame={r3:#x}");

        // Install subclass proc (safe for multiple windows)
        let ok = SetWindowSubclass(hwnd, borderless_subclass_proc, BORDERLESS_SUBCLASS_ID, 0);
        tracing::info!("dwm: subclass installed={ok}");

        // Set background brush to dark (reuse a single GDI brush)
        const GCL_HBRBACKGROUND: i32 = -10;
        let brush = *DARK_BRUSH.get_or_init(|| CreateSolidBrush(0x001C1C1E));
        if brush != 0 {
            SetWindowLongPtrW(hwnd, GCL_HBRBACKGROUND, brush);
        }

        // Remove WS_CAPTION; only strip WS_THICKFRAME for non-resizable windows
        const GWL_STYLE: i32 = -16;
        const WS_CAPTION: isize = 0x00C00000;
        const WS_THICKFRAME: isize = 0x00040000;
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let new_style = if keep_resize {
            style & !WS_CAPTION
        } else {
            style & !WS_CAPTION & !WS_THICKFRAME
        };
        if new_style != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
            if keep_resize {
                tracing::info!("dwm: stripped WS_CAPTION (kept WS_THICKFRAME for resize)");
            } else {
                tracing::info!("dwm: stripped WS_CAPTION/WS_THICKFRAME");
            }
        }

        // Force frame recalculation
        const SWP_FRAMECHANGED: u32 = 0x0020;
        const SWP_NOMOVE: u32 = 0x0002;
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOZORDER: u32 = 0x0004;
        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

#[cfg(windows)]
fn root_hwnd(win: &tauri::WebviewWindow) -> Result<isize, &'static str> {
    let handle = win
        .window_handle()
        .map_err(|_| "WINDOW_HANDLE_UNAVAILABLE")?;
    let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("WINDOW_HANDLE_UNSUPPORTED");
    };
    const GA_ROOT: u32 = 2;
    let inner = handle.hwnd.get();
    let root = unsafe { GetAncestor(inner, GA_ROOT) };
    Ok(if root != 0 { root } else { inner })
}

#[cfg(windows)]
pub fn apply_no_activate_tool_window(win: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const WS_EX_LAYERED: isize = 0x0008_0000;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const REQUIRED: isize = WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE;
    let current = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, current | REQUIRED);
    }
    let observed = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if observed & REQUIRED != REQUIRED {
        return Err("OVERLAY_STYLE_FAILED".to_string());
    }

    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
    };
    if ok == 0 {
        Err("OVERLAY_ZORDER_FAILED".to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn shape_round_window(win: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    let mut rect = WinRect::default();
    let ok = unsafe { GetClientRect(hwnd, &mut rect) };
    if ok == 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    let region = unsafe { CreateEllipticRgn(0, 0, width, height) };
    if region == 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    // SetWindowRgn takes ownership of the region; the system deletes it.
    let applied = unsafe { SetWindowRgn(hwnd, region, 1) };
    if applied == 0 {
        Err("OVERLAY_REGION_FAILED".to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn shape_rounded_rect_window(win: &tauri::WebviewWindow, radius: i32) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    let mut rect = WinRect::default();
    let ok = unsafe { GetClientRect(hwnd, &mut rect) };
    if ok == 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    let radius = radius.max(8);
    let region = unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius) };
    if region == 0 {
        return Err("OVERLAY_REGION_FAILED".to_string());
    }
    let applied = unsafe { SetWindowRgn(hwnd, region, 1) };
    if applied == 0 {
        Err("OVERLAY_REGION_FAILED".to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn shape_rounded_rect_window(_win: &tauri::WebviewWindow, _radius: i32) -> Result<(), String> {
    Ok(())
}
#[cfg(windows)]
pub fn set_no_activate_bounds(
    win: &tauri::WebviewWindow,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    show: bool,
) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    let flags = SWP_NOACTIVATE | if show { SWP_SHOWWINDOW } else { 0 };
    let ok = unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, width.max(1), height.max(1), flags) };
    if ok == 0 {
        Err("OVERLAY_POSITION_FAILED".to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn reassert_topmost(win: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    if ok == 0 {
        Err("OVERLAY_ZORDER_FAILED".to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn hide_window(win: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = root_hwnd(win).map_err(str::to_string)?;
    const SW_HIDE: i32 = 0;
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn force_dark_caption(_win: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn force_dark_caption_resizable(_win: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn apply_no_activate_tool_window(_win: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn shape_round_window(_win: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn set_no_activate_bounds(
    win: &tauri::WebviewWindow,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    show: bool,
) -> Result<(), String> {
    win.set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|_| "OVERLAY_POSITION_FAILED".to_string())?;
    win.set_size(tauri::PhysicalSize::new(
        width.max(1) as u32,
        height.max(1) as u32,
    ))
    .map_err(|_| "OVERLAY_SIZE_FAILED".to_string())?;
    if show {
        win.show().map_err(|_| "OVERLAY_SHOW_FAILED".to_string())?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn reassert_topmost(_win: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn hide_window(win: &tauri::WebviewWindow) -> Result<(), String> {
    win.hide().map_err(|_| "OVERLAY_HIDE_FAILED".to_string())
}
