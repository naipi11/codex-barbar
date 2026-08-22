#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

pub(crate) const WS_CAPTION: usize = 0x00c00000;
const FULLSCREEN_EDGE_TOLERANCE: i32 = 8;

pub(crate) fn is_shell_window_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "ShellHandwritingCanvas"
    ) || class_name.starts_with("ShellHandwritingCanvas ")
}

pub(crate) fn supports_renderer_scan(class_name: &str) -> bool {
    matches!(
        class_name,
        "Chrome_WidgetWin_1" | "Chrome_WidgetWin_0" | "MozillaWindowClass" | "CefBrowserWindow"
    )
}

pub(crate) fn window_covers_monitor(
    window: ScreenRect,
    monitor: ScreenRect,
    tolerance: i32,
) -> bool {
    let tolerance = tolerance.max(0);
    window.right > window.left
        && window.bottom > window.top
        && monitor.right > monitor.left
        && monitor.bottom > monitor.top
        && window.left <= monitor.left.saturating_add(tolerance)
        && window.top <= monitor.top.saturating_add(tolerance)
        && window.right >= monitor.right.saturating_sub(tolerance)
        && window.bottom >= monitor.bottom.saturating_sub(tolerance)
}

pub(crate) fn window_matches_fullscreen(
    window: ScreenRect,
    monitor: ScreenRect,
    work_area: ScreenRect,
    style: usize,
    tolerance: i32,
) -> bool {
    window_covers_monitor(window, monitor, tolerance)
        || (style & WS_CAPTION == 0 && window_covers_monitor(window, work_area, tolerance))
}

pub(crate) fn child_window_matches_fullscreen(
    window: ScreenRect,
    monitor: ScreenRect,
    tolerance: i32,
) -> bool {
    window_covers_monitor(window, monitor, tolerance)
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
    fn GetCurrentProcessId() -> u32;
    fn GetForegroundWindow() -> isize;
    fn GetWindowRect(hwnd: isize, rect: *mut WinRect) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, process_id: *mut u32) -> u32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max_count: i32) -> i32;
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
    fn EnumChildWindows(
        parent: isize,
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
}

#[cfg(windows)]
struct FullscreenScanContext {
    process_id: u32,
    monitor: ScreenRect,
    work_area: ScreenRect,
    found: bool,
}

#[cfg(windows)]
fn monitor_rects(hwnd: isize) -> Option<(ScreenRect, ScreenRect)> {
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor == 0 {
        return None;
    }

    let mut info = MonitorInfo {
        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
        rc_monitor: WinRect::default(),
        rc_work: WinRect::default(),
        dw_flags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }

    Some((
        ScreenRect {
            left: info.rc_monitor.left,
            top: info.rc_monitor.top,
            right: info.rc_monitor.right,
            bottom: info.rc_monitor.bottom,
        },
        ScreenRect {
            left: info.rc_work.left,
            top: info.rc_work.top,
            right: info.rc_work.right,
            bottom: info.rc_work.bottom,
        },
    ))
}

#[cfg(windows)]
fn window_rect(hwnd: isize) -> Option<ScreenRect> {
    let mut rect = WinRect::default();
    (unsafe { GetWindowRect(hwnd, &mut rect) } != 0).then_some(ScreenRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

#[cfg(windows)]
fn window_class(hwnd: isize) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

#[cfg(windows)]
fn window_title(hwnd: isize) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

#[cfg(windows)]
fn is_shell_window(hwnd: isize) -> bool {
    let class_name = window_class(hwnd);
    if is_shell_window_class(&class_name) {
        return true;
    }
    if class_name == "Windows.UI.Core.CoreWindow" {
        let title = window_title(hwnd);
        return matches!(title.trim(), "Start" | "开始" | "Search" | "搜索");
    }
    false
}

#[cfg(windows)]
fn scan_candidate(hwnd: isize, context: &FullscreenScanContext, child: bool) -> bool {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return false;
    }
    let Some(rect) = window_rect(hwnd) else {
        return false;
    };
    if child {
        child_window_matches_fullscreen(rect, context.monitor, FULLSCREEN_EDGE_TOLERANCE)
    } else {
        let style = unsafe { GetWindowLongPtrW(hwnd, -16) as usize };
        window_matches_fullscreen(
            rect,
            context.monitor,
            context.work_area,
            style,
            FULLSCREEN_EDGE_TOLERANCE,
        )
    }
}

#[cfg(windows)]
unsafe extern "system" fn scan_child_window(hwnd: isize, parameter: isize) -> i32 {
    let context = unsafe { &mut *(parameter as *mut FullscreenScanContext) };
    if scan_candidate(hwnd, context, true) {
        context.found = true;
        0
    } else {
        1
    }
}

#[cfg(windows)]
unsafe extern "system" fn scan_top_level_window(hwnd: isize, parameter: isize) -> i32 {
    let context = unsafe { &mut *(parameter as *mut FullscreenScanContext) };
    if context.found {
        return 0;
    }

    let mut process_id = 0;
    if unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) } == 0
        || process_id != context.process_id
    {
        return 1;
    }

    if scan_candidate(hwnd, context, false) {
        context.found = true;
        return 0;
    }

    unsafe { EnumChildWindows(hwnd, Some(scan_child_window), parameter) };
    if context.found { 0 } else { 1 }
}

#[cfg(windows)]
fn process_has_fullscreen_window(
    process_id: u32,
    monitor: ScreenRect,
    work_area: ScreenRect,
) -> bool {
    let mut context = FullscreenScanContext {
        process_id,
        monitor,
        work_area,
        found: false,
    };
    unsafe {
        EnumWindows(
            Some(scan_top_level_window),
            (&mut context as *mut FullscreenScanContext) as isize,
        );
    }
    context.found
}

#[cfg(windows)]
fn detect_fullscreen() -> bool {
    let foreground = unsafe { GetForegroundWindow() };

    if foreground == 0 {
        return false;
    }

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
    if process_id == unsafe { GetCurrentProcessId() } {
        return false;
    }
    if is_shell_window(foreground) {
        return false;
    }
    if unsafe { IsWindowVisible(foreground) } == 0 || unsafe { IsIconic(foreground) } != 0 {
        return false;
    }

    let Some(window) = window_rect(foreground) else {
        return false;
    };
    let Some((monitor, work_area)) = monitor_rects(foreground) else {
        return false;
    };
    let style = unsafe { GetWindowLongPtrW(foreground, -16) as usize };
    if window_matches_fullscreen(window, monitor, work_area, style, FULLSCREEN_EDGE_TOLERANCE) {
        return true;
    }

    if supports_renderer_scan(&window_class(foreground)) {
        process_has_fullscreen_window(process_id, monitor, work_area)
    } else {
        false
    }
}

#[cfg(windows)]
pub(crate) fn is_fullscreen_active() -> bool {
    detect_fullscreen()
}

#[cfg(not(windows))]
pub(crate) fn is_fullscreen_active() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_monitor_window_is_detected_as_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(window_covers_monitor(monitor, monitor, 2));
        assert!(!window_covers_monitor(
            ScreenRect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1032,
            },
            monitor,
            2,
        ));
    }

    #[test]
    fn small_window_edges_are_not_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(!window_covers_monitor(
            ScreenRect {
                left: 100,
                top: 100,
                right: 1820,
                bottom: 980,
            },
            monitor,
            2,
        ));
    }

    #[test]
    fn borderless_window_covering_work_area_is_detected_as_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 823,
        };
        let work_area = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 775,
        };
        let window = ScreenRect {
            left: -7,
            top: -7,
            right: 1470,
            bottom: 782,
        };

        assert!(window_matches_fullscreen(window, monitor, work_area, 0, 8,));
    }

    #[test]
    fn decorated_maximized_window_covering_work_area_is_not_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 823,
        };
        let work_area = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 775,
        };

        assert!(!window_matches_fullscreen(
            work_area, monitor, work_area, WS_CAPTION, 8,
        ));
    }

    #[test]
    fn normal_edge_renderer_work_area_child_is_not_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 823,
        };
        let work_area_child = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 775,
        };

        assert!(!child_window_matches_fullscreen(
            work_area_child,
            monitor,
            8
        ));
    }

    #[test]
    fn edge_video_renderer_covering_monitor_is_fullscreen() {
        let monitor = ScreenRect {
            left: 0,
            top: 0,
            right: 1463,
            bottom: 823,
        };

        assert!(child_window_matches_fullscreen(monitor, monitor, 8));
    }

    #[test]
    fn shell_windows_are_not_browser_renderer_hosts() {
        for class_name in [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "ShellHandwritingCanvas",
        ] {
            assert!(is_shell_window_class(class_name));
            assert!(!supports_renderer_scan(class_name));
        }
    }

    #[test]
    fn browser_window_classes_allow_renderer_scan() {
        for class_name in [
            "Chrome_WidgetWin_1",
            "Chrome_WidgetWin_0",
            "MozillaWindowClass",
            "CefBrowserWindow",
        ] {
            assert!(!is_shell_window_class(class_name));
            assert!(supports_renderer_scan(class_name));
        }
    }
}
