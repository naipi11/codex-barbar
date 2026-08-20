//! Pure monitor/work-area geometry helpers for the V1 shell.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    #[allow(dead_code)]
    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }
}

/// Default flyout logical-pixel size used on first open.
pub const FLYOUT_LOGICAL_SIZE: (u32, u32) = (380, 470);
/// Physical-pixel inset from the monitor work-area edge.
pub const WORK_AREA_INSET: i32 = 20;

/// Subtract a bottom/top/side taskbar from a monitor work area.
pub fn subtract_taskbar(work_area: Rect, taskbar: Rect) -> Rect {
    let work_right = work_area.x + work_area.width;
    let work_bottom = work_area.y + work_area.height;
    let bar_right = taskbar.x + taskbar.width;
    let bar_bottom = taskbar.y + taskbar.height;
    if taskbar.width >= taskbar.height {
        if bar_bottom >= work_bottom && taskbar.y < work_bottom {
            return Rect {
                height: (taskbar.y - work_area.y).max(1),
                ..work_area
            };
        }
        if taskbar.y <= work_area.y && bar_bottom > work_area.y {
            let y = bar_bottom;
            return Rect {
                y,
                height: (work_bottom - y).max(1),
                ..work_area
            };
        }
    } else if bar_right >= work_right && taskbar.x < work_right {
        return Rect {
            width: (taskbar.x - work_area.x).max(1),
            ..work_area
        };
    } else if taskbar.x <= work_area.x && bar_right > work_area.x {
        let x = bar_right;
        return Rect {
            x,
            width: (work_right - x).max(1),
            ..work_area
        };
    }
    work_area
}

/// Place a flyout of `width` x `height` physical pixels inside `work_area`,
/// anchoring bottom-right so the panel never covers the taskbar.
pub fn place_flyout_sized(work_area: Rect, width: i32, height: i32) -> Rect {
    let width = width.max(1).min(work_area.width.max(1));
    let height = height.max(1).min(work_area.height.max(1));
    let inset_x = WORK_AREA_INSET.min((work_area.width - width).max(0));
    let inset_y = WORK_AREA_INSET.min((work_area.height - height).max(0));
    Rect {
        x: work_area.x + work_area.width - width - inset_x,
        y: work_area.y + work_area.height - height - inset_y,
        width,
        height,
    }
}

/// Keep a window on-screen, but allow it to overlap the taskbar so the user
/// can drag anywhere on the monitor.
pub fn clamp_to_monitor(monitor: Rect, x: i32, y: i32, width: i32, height: i32) -> Rect {
    let width = width.max(1).min(monitor.width.max(1));
    let height = height.max(1).min(monitor.height.max(1));
    let min_x = monitor.x;
    let min_y = monitor.y;
    let max_x = monitor.x + monitor.width - width;
    let max_y = monitor.y + monitor.height - height;
    Rect {
        x: x.clamp(min_x.min(max_x), max_x.max(min_x)),
        y: y.clamp(min_y.min(max_y), max_y.max(min_y)),
        width,
        height,
    }
}
/// Keep an existing flyout fully inside the work area after a move or resize.
#[cfg(test)]
pub fn clamp_flyout(work_area: Rect, x: i32, y: i32, width: i32, height: i32) -> Rect {
    let width = width.max(1).min(work_area.width.max(1));
    let height = height.max(1).min(work_area.height.max(1));
    let min_x = work_area.x;
    let min_y = work_area.y;
    let max_x = work_area.x + work_area.width - width;
    let max_y = work_area.y + work_area.height - height;
    Rect {
        x: x.clamp(min_x.min(max_x), max_x.max(min_x)),
        y: y.clamp(min_y.min(max_y), max_y.max(min_y)),
        width,
        height,
    }
}

/// Place the default-sized flyout inside `work_area`.
#[cfg(test)]
pub fn place_flyout(work_area: Rect, scale: f64) -> Rect {
    let width = (f64::from(FLYOUT_LOGICAL_SIZE.0) * scale).round() as i32;
    let height = (f64::from(FLYOUT_LOGICAL_SIZE.1) * scale).round() as i32;
    place_flyout_sized(work_area, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tray_bottom_right_at_200_percent() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 3840,
            height: 2064,
        }
    }

    #[test]
    fn flyout_rect_is_clamped_inside_scaled_work_area() {
        let work_area = tray_bottom_right_at_200_percent();
        let rect = place_flyout(work_area, 2.0);
        assert!(work_area.contains(&rect));
        assert_eq!(rect.width, 760);
        assert_eq!(rect.height, 940);
        assert_eq!(rect.x, work_area.width - rect.width - WORK_AREA_INSET);
        assert_eq!(rect.y, work_area.height - rect.height - WORK_AREA_INSET);
    }

    #[test]
    fn taller_window_still_stays_above_the_taskbar() {
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let rect = place_flyout_sized(work_area, 400, 520);
        assert!(work_area.contains(&rect));
        assert_eq!(rect.y + rect.height, work_area.height - WORK_AREA_INSET);
    }

    #[test]
    fn clamp_pushes_an_overflowing_panel_back_into_the_work_area() {
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        let rect = clamp_flyout(work_area, 1800, 1000, 400, 400);
        assert!(work_area.contains(&rect));
        assert!(rect.y + rect.height <= work_area.height);
    }

    #[test]
    fn subtract_taskbar_lifts_a_bottom_bar_out_of_the_work_area() {
        let work = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let bar = Rect {
            x: 0,
            y: 1032,
            width: 1920,
            height: 48,
        };
        let usable = subtract_taskbar(work, bar);
        assert_eq!(usable.height, 1032);
        let rect = place_flyout_sized(usable, 400, 520);
        assert!(rect.y + rect.height <= 1032 - WORK_AREA_INSET);
    }

    #[test]
    fn flyout_never_escapes_small_work_area() {
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 300,
            height: 200,
        };
        let rect = place_flyout(work_area, 1.0);
        assert!(work_area.contains(&rect));
    }
}
