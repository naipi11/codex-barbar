#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
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
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
}

#[cfg(windows)]
pub(crate) fn is_fullscreen_active() -> bool {
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    let foreground = unsafe { GetForegroundWindow() };
    if foreground == 0 {
        return false;
    }

    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(foreground, &mut process_id) };
    if process_id == unsafe { GetCurrentProcessId() } {
        return false;
    }

    let mut window = WinRect::default();
    if unsafe { GetWindowRect(foreground, &mut window) } == 0 {
        return false;
    }

    let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
    if monitor == 0 {
        return false;
    }

    let mut info = MonitorInfo {
        cb_size: std::mem::size_of::<MonitorInfo>() as u32,
        rc_monitor: WinRect::default(),
        rc_work: WinRect::default(),
        dw_flags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return false;
    }

    window_covers_monitor(
        ScreenRect {
            left: window.left,
            top: window.top,
            right: window.right,
            bottom: window.bottom,
        },
        ScreenRect {
            left: info.rc_monitor.left,
            top: info.rc_monitor.top,
            right: info.rc_monitor.right,
            bottom: info.rc_monitor.bottom,
        },
        2,
    )
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
}
