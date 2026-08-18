use super::positioning::{Rect, TaskbarEdge, TaskbarSnapshot};

pub type WindowHandle = isize;

pub trait Win32TaskbarApi {
    fn find_window(&self, class_name: &str) -> Option<WindowHandle>;
    fn find_descendant(&self, parent: WindowHandle, class_name: &str) -> Option<WindowHandle>;
    fn window_rect(&self, window: WindowHandle) -> Option<Rect>;
    fn monitor_rect(&self, window: WindowHandle) -> Option<Rect>;
    fn dpi_for_window(&self, window: WindowHandle) -> Option<u32>;
    fn is_window_visible(&self, window: WindowHandle) -> bool;
    fn is_auto_hide_enabled(&self, window: WindowHandle) -> bool;
}

pub fn class_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

pub fn edge_for_taskbar(taskbar: Rect, monitor: Rect) -> Option<TaskbarEdge> {
    let taskbar_right = taskbar.x.saturating_add(taskbar.width.max(1));
    let taskbar_bottom = taskbar.y.saturating_add(taskbar.height.max(1));
    let monitor_right = monitor.x.saturating_add(monitor.width.max(1));
    let monitor_bottom = monitor.y.saturating_add(monitor.height.max(1));

    if taskbar.width >= taskbar.height {
        if taskbar.y <= monitor.y {
            Some(TaskbarEdge::Top)
        } else if taskbar_bottom >= monitor_bottom {
            Some(TaskbarEdge::Bottom)
        } else {
            None
        }
    } else if taskbar.x <= monitor.x {
        Some(TaskbarEdge::Left)
    } else if taskbar_right >= monitor_right {
        Some(TaskbarEdge::Right)
    } else {
        None
    }
}

pub fn discover_taskbar<A: Win32TaskbarApi>(_api: &A) -> Option<TaskbarSnapshot> {
    let shell = _api.find_window("Shell_TrayWnd")?;
    let auto_hide = _api.is_auto_hide_enabled(shell);
    if !_api.is_window_visible(shell) && !auto_hide {
        return None;
    }
    let taskbar = _api.window_rect(shell)?;
    let monitor = _api.monitor_rect(shell)?;
    let edge = edge_for_taskbar(taskbar, monitor)?;
    let app_area = ["MSTaskSwWClass", "MSTaskListWClass"]
        .iter()
        .find_map(|class| _api.find_descendant(shell, class))
        .and_then(|window| _api.window_rect(window));
    let notification_area = _api
        .find_descendant(shell, "TrayNotifyWnd")
        .and_then(|window| _api.window_rect(window));

    Some(TaskbarSnapshot {
        taskbar,
        app_area,
        notification_area,
        edge,
        dpi: _api
            .dpi_for_window(shell)
            .filter(|dpi| *dpi > 0)
            .unwrap_or(96),
        auto_hide,
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeWin32TaskbarApi;

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MonitorInfo {
    cb_size: u32,
    rc_monitor: NativeRect,
    rc_work: NativeRect,
    flags: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AppBarData {
    cb_size: u32,
    hwnd: isize,
    callback_message: u32,
    edge: u32,
    rect: NativeRect,
    lparam: isize,
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
    fn FindWindowExW(
        parent: isize,
        child_after: isize,
        class_name: *const u16,
        window_name: *const u16,
    ) -> isize;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut NativeRect) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn GetDpiForWindow(hwnd: isize) -> u32;
    fn IsWindowVisible(hwnd: isize) -> i32;
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> u32;
}

#[cfg(windows)]
const MONITOR_DEFAULTTONEAREST: u32 = 2;
#[cfg(windows)]
const ABM_GETSTATE: u32 = 0x0000_0004;
#[cfg(windows)]
const ABS_AUTOHIDE: u32 = 0x0000_0001;

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn native_rect(rect: NativeRect) -> Rect {
    Rect {
        x: rect.left,
        y: rect.top,
        width: rect.right.saturating_sub(rect.left),
        height: rect.bottom.saturating_sub(rect.top),
    }
}

#[cfg(windows)]
fn class_name(window: WindowHandle) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}

#[cfg(windows)]
fn find_descendant_recursive(
    parent: WindowHandle,
    class_name_expected: &str,
) -> Option<WindowHandle> {
    let mut child = unsafe { FindWindowExW(parent, 0, std::ptr::null(), std::ptr::null()) };
    while child != 0 {
        if class_name(child).is_some_and(|actual| class_matches(&actual, class_name_expected)) {
            return Some(child);
        }
        if let Some(found) = find_descendant_recursive(child, class_name_expected) {
            return Some(found);
        }
        child = unsafe { FindWindowExW(parent, child, std::ptr::null(), std::ptr::null()) };
    }
    None
}

#[cfg(windows)]
impl Win32TaskbarApi for NativeWin32TaskbarApi {
    fn find_window(&self, class_name: &str) -> Option<WindowHandle> {
        let class = wide(class_name);
        let handle = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
        (handle != 0).then_some(handle)
    }

    fn find_descendant(&self, parent: WindowHandle, class_name: &str) -> Option<WindowHandle> {
        find_descendant_recursive(parent, class_name)
    }

    fn window_rect(&self, window: WindowHandle) -> Option<Rect> {
        let mut rect = NativeRect::default();
        let ok = unsafe { GetWindowRect(window, &mut rect) };
        (ok != 0).then(|| native_rect(rect))
    }

    fn monitor_rect(&self, window: WindowHandle) -> Option<Rect> {
        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
        if monitor == 0 {
            return None;
        }
        let mut info = MonitorInfo {
            cb_size: std::mem::size_of::<MonitorInfo>() as u32,
            ..MonitorInfo::default()
        };
        let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
        (ok != 0).then(|| native_rect(info.rc_monitor))
    }

    fn dpi_for_window(&self, window: WindowHandle) -> Option<u32> {
        let dpi = unsafe { GetDpiForWindow(window) };
        (dpi > 0).then_some(dpi)
    }

    fn is_window_visible(&self, window: WindowHandle) -> bool {
        unsafe { IsWindowVisible(window) != 0 }
    }

    fn is_auto_hide_enabled(&self, window: WindowHandle) -> bool {
        let mut data = AppBarData {
            cb_size: std::mem::size_of::<AppBarData>() as u32,
            hwnd: window,
            ..AppBarData::default()
        };
        unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) & ABS_AUTOHIDE != 0 }
    }
}

#[cfg(not(windows))]
impl Win32TaskbarApi for NativeWin32TaskbarApi {
    fn find_window(&self, _class_name: &str) -> Option<WindowHandle> {
        None
    }

    fn find_descendant(&self, _parent: WindowHandle, _class_name: &str) -> Option<WindowHandle> {
        None
    }

    fn window_rect(&self, _window: WindowHandle) -> Option<Rect> {
        None
    }

    fn monitor_rect(&self, _window: WindowHandle) -> Option<Rect> {
        None
    }

    fn dpi_for_window(&self, _window: WindowHandle) -> Option<u32> {
        None
    }

    fn is_window_visible(&self, _window: WindowHandle) -> bool {
        false
    }

    fn is_auto_hide_enabled(&self, _window: WindowHandle) -> bool {
        false
    }
}

pub fn discover_native() -> Option<TaskbarSnapshot> {
    discover_taskbar(&NativeWin32TaskbarApi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct FakeApi {
        windows: BTreeMap<String, WindowHandle>,
        descendants: BTreeMap<(WindowHandle, String), WindowHandle>,
        rects: BTreeMap<WindowHandle, Rect>,
        monitors: BTreeMap<WindowHandle, Rect>,
        dpis: BTreeMap<WindowHandle, u32>,
        visible: BTreeMap<WindowHandle, bool>,
        auto_hide: BTreeMap<WindowHandle, bool>,
    }

    impl Win32TaskbarApi for FakeApi {
        fn find_window(&self, class_name: &str) -> Option<WindowHandle> {
            self.windows
                .iter()
                .find(|(actual, _)| class_matches(actual, class_name))
                .map(|(_, handle)| *handle)
        }

        fn find_descendant(&self, parent: WindowHandle, class_name: &str) -> Option<WindowHandle> {
            self.descendants
                .iter()
                .find(|((candidate_parent, actual), _)| {
                    *candidate_parent == parent && class_matches(actual, class_name)
                })
                .map(|(_, handle)| *handle)
        }

        fn window_rect(&self, window: WindowHandle) -> Option<Rect> {
            self.rects.get(&window).copied()
        }

        fn monitor_rect(&self, window: WindowHandle) -> Option<Rect> {
            self.monitors.get(&window).copied()
        }

        fn dpi_for_window(&self, window: WindowHandle) -> Option<u32> {
            self.dpis.get(&window).copied()
        }

        fn is_window_visible(&self, window: WindowHandle) -> bool {
            self.visible.get(&window).copied().unwrap_or(false)
        }

        fn is_auto_hide_enabled(&self, window: WindowHandle) -> bool {
            self.auto_hide.get(&window).copied().unwrap_or(false)
        }
    }

    #[test]
    fn class_matching_is_case_insensitive() {
        assert!(class_matches("shell_traywnd", "Shell_TrayWnd"));
        assert!(class_matches("TRAYNOTIFYWND", "TrayNotifyWnd"));
        assert!(!class_matches("MSTaskSwWClass", "OtherClass"));
    }

    #[test]
    fn maps_taskbar_rectangle_to_each_edge() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            edge_for_taskbar(
                Rect {
                    x: 0,
                    y: 1032,
                    width: 1920,
                    height: 48,
                },
                monitor
            ),
            Some(TaskbarEdge::Bottom)
        );
        assert_eq!(
            edge_for_taskbar(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 48,
                },
                monitor
            ),
            Some(TaskbarEdge::Top)
        );
        assert_eq!(
            edge_for_taskbar(
                Rect {
                    x: 0,
                    y: 0,
                    width: 48,
                    height: 1080,
                },
                monitor
            ),
            Some(TaskbarEdge::Left)
        );
        assert_eq!(
            edge_for_taskbar(
                Rect {
                    x: 1872,
                    y: 0,
                    width: 48,
                    height: 1080,
                },
                monitor
            ),
            Some(TaskbarEdge::Right)
        );
    }

    #[test]
    fn discovers_shell_children_and_metadata() {
        let mut api = FakeApi::default();
        api.windows.insert("shell_traywnd".into(), 1);
        api.descendants.insert((1, "traynotifywnd".into()), 2);
        api.descendants.insert((1, "mstaskswwclass".into()), 3);
        api.rects.insert(
            1,
            Rect {
                x: 0,
                y: 1032,
                width: 1920,
                height: 48,
            },
        );
        api.rects.insert(
            2,
            Rect {
                x: 1760,
                y: 1032,
                width: 160,
                height: 48,
            },
        );
        api.rects.insert(
            3,
            Rect {
                x: 0,
                y: 1032,
                width: 1200,
                height: 48,
            },
        );
        api.monitors.insert(
            1,
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
        );
        api.dpis.insert(1, 144);
        api.visible.insert(1, true);
        api.auto_hide.insert(1, true);

        let snapshot = discover_taskbar(&api).expect("taskbar should be found");
        assert_eq!(snapshot.edge, TaskbarEdge::Bottom);
        assert_eq!(snapshot.dpi, 144);
        assert!(snapshot.auto_hide);
        assert_eq!(snapshot.notification_area, api.rects.get(&2).copied());
        assert_eq!(snapshot.app_area, api.rects.get(&3).copied());
    }

    #[test]
    fn discovery_failure_returns_none_instead_of_panicking() {
        let api = FakeApi::default();
        assert!(discover_taskbar(&api).is_none());
    }
}
