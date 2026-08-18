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

/// V1 flyout logical-pixel target.
pub const FLYOUT_LOGICAL_SIZE: (u32, u32) = (400, 520);
/// Physical-pixel inset from the monitor work-area edge.
pub const WORK_AREA_INSET: i32 = 8;

/// Place the flyout inside `work_area`, anchoring bottom-right by default and
/// clamping physical bounds to the work area with `WORK_AREA_INSET`.
pub fn place_flyout(work_area: Rect, scale: f64) -> Rect {
    let desired_width = (f64::from(FLYOUT_LOGICAL_SIZE.0) * scale).round() as i32;
    let desired_height = (f64::from(FLYOUT_LOGICAL_SIZE.1) * scale).round() as i32;

    // Shrink to the work area first: the panel content scrolls instead of
    // ever escaping the monitor's usable bounds.
    let width = desired_width.min(work_area.width.max(1));
    let height = desired_height.min(work_area.height.max(1));

    // Keep the 8 px inset whenever the window actually fits; otherwise pin
    // flush to the work-area edge so the panel never overflows.
    let inset_x = WORK_AREA_INSET.min((work_area.width - width).max(0));
    let inset_y = WORK_AREA_INSET.min((work_area.height - height).max(0));
    let x = work_area.x + work_area.width - width - inset_x;
    let y = work_area.y + work_area.height - height - inset_y;

    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tray_bottom_right_at_200_percent() -> Rect {
        // 1920x1080 logical at 200% => 3840x2160 physical work area with the
        // taskbar on the bottom (24 logical px = 48 physical px).
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
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 1040);
        // Bottom-right anchor with the 8 px inset.
        assert_eq!(rect.x, work_area.width - rect.width - WORK_AREA_INSET);
        assert_eq!(rect.y, work_area.height - rect.height - WORK_AREA_INSET);
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
