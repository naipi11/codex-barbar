use super::window::{TASKBAR_LOGICAL_HEIGHT, TASKBAR_MIN_LOGICAL_WIDTH};
pub use crate::window_positioner::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarEdge {
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskbarSnapshot {
    pub taskbar: Rect,
    pub app_area: Option<Rect>,
    pub notification_area: Option<Rect>,
    pub edge: TaskbarEdge,
    pub dpi: u32,
    pub auto_hide: bool,
}

const MAIN_AXIS_MARGIN: i32 = 8;

fn physical_length(logical: u32, dpi: u32) -> i32 {
    let scaled = (u64::from(logical) * u64::from(dpi.max(1)) + 48) / 96;
    scaled.clamp(1, i64::from(i32::MAX) as u64) as i32
}

fn axis_bounds(rect: Rect, horizontal: bool) -> (i32, i32) {
    if horizontal {
        let end = rect.x.saturating_add(rect.width.max(1));
        (rect.x, end)
    } else {
        let end = rect.y.saturating_add(rect.height.max(1));
        (rect.y, end)
    }
}

fn clamp_axis(value: i32, start: i32, end: i32) -> i32 {
    value.clamp(start.min(end), start.max(end))
}

fn preferred_axis_bounds(snapshot: &TaskbarSnapshot, horizontal: bool, desired: i32) -> (i32, i32) {
    let taskbar = snapshot.taskbar;
    let (taskbar_start, taskbar_end) = axis_bounds(taskbar, horizontal);
    let app_end = snapshot
        .app_area
        .map(|area| axis_bounds(area, horizontal).1)
        .unwrap_or(taskbar_start);
    let notification_start = snapshot
        .notification_area
        .map(|area| axis_bounds(area, horizontal).0)
        .unwrap_or_else(|| {
            let fallback_end = app_end
                .saturating_add(desired)
                .saturating_add(MAIN_AXIS_MARGIN * 2);
            fallback_end.min(taskbar_end)
        });
    let start = clamp_axis(app_end, taskbar_start, taskbar_end);
    let end = clamp_axis(notification_start, taskbar_start, taskbar_end);
    if end >= start {
        (start, end)
    } else {
        (taskbar_start, taskbar_end)
    }
}

fn place_on_axis(start: i32, end: i32, desired: i32, minimum: i32) -> (i32, i32) {
    let start = i64::from(start);
    let end = i64::from(end.max(start as i32));
    let available = (end - start).max(0);
    let fit = (available - i64::from(MAIN_AXIS_MARGIN * 2)).max(1);
    let desired = i64::from(desired.max(1));
    let minimum = i64::from(minimum.max(1));
    let length = if fit >= minimum {
        desired.min(fit)
    } else {
        fit
    };

    let position = if available >= length + i64::from(MAIN_AXIS_MARGIN * 2) {
        end - i64::from(MAIN_AXIS_MARGIN) - length
    } else {
        start + (available - length).max(0) / 2
    };

    (
        position.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        length.clamp(1, i64::from(i32::MAX)) as i32,
    )
}

pub fn compute_slot(snapshot: &TaskbarSnapshot, logical_width: u32) -> Rect {
    let taskbar = snapshot.taskbar;
    let horizontal = matches!(snapshot.edge, TaskbarEdge::Bottom | TaskbarEdge::Top);
    let desired = physical_length(logical_width, snapshot.dpi);
    let minimum = physical_length(TASKBAR_MIN_LOGICAL_WIDTH, snapshot.dpi);
    let cross = physical_length(TASKBAR_LOGICAL_HEIGHT, snapshot.dpi);

    if horizontal {
        let (axis_start, axis_end) = preferred_axis_bounds(snapshot, true, desired);
        let (x, width) = place_on_axis(axis_start, axis_end, desired, minimum);
        let height = cross.min(taskbar.height.max(1));
        let y = taskbar
            .y
            .saturating_add((taskbar.height.max(1) - height) / 2);
        Rect {
            x,
            y,
            width,
            height,
        }
    } else {
        let (axis_start, axis_end) = preferred_axis_bounds(snapshot, false, desired);
        let (y, height) = place_on_axis(axis_start, axis_end, desired, minimum);
        let width = cross.min(taskbar.width.max(1));
        let x = taskbar.x.saturating_add((taskbar.width.max(1) - width) / 2);
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        taskbar: Rect,
        app_area: Option<Rect>,
        notification_area: Option<Rect>,
        edge: TaskbarEdge,
        dpi: u32,
    ) -> TaskbarSnapshot {
        TaskbarSnapshot {
            taskbar,
            app_area,
            notification_area,
            edge,
            dpi,
            auto_hide: false,
        }
    }

    #[test]
    fn bottom_taskbar_places_slot_between_app_area_and_notification_area() {
        let taskbar = Rect {
            x: 0,
            y: 1032,
            width: 1920,
            height: 48,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: 0,
                    y: 1032,
                    width: 1200,
                    height: 48,
                }),
                Some(Rect {
                    x: 1760,
                    y: 1032,
                    width: 160,
                    height: 48,
                }),
                TaskbarEdge::Bottom,
                96,
            ),
            260,
        );

        assert_eq!(slot.width, 260);
        assert_eq!(slot.height, 40);
        assert!(slot.x >= 1200);
        assert!(slot.x + slot.width <= 1760);
        assert_eq!(slot.y, 1036);
    }

    #[test]
    fn top_taskbar_keeps_slot_inside_the_top_taskbar() {
        let taskbar = Rect {
            x: -100,
            y: 0,
            width: 1920,
            height: 48,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: -100,
                    y: 0,
                    width: 900,
                    height: 48,
                }),
                Some(Rect {
                    x: 1700,
                    y: 0,
                    width: 120,
                    height: 48,
                }),
                TaskbarEdge::Top,
                96,
            ),
            260,
        );

        assert!(slot.x >= 800);
        assert!(slot.x + slot.width <= 1700);
        assert_eq!(slot.y, 4);
    }

    #[test]
    fn vertical_taskbar_uses_cross_axis_centering() {
        let taskbar = Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 1080,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: 0,
                    y: 80,
                    width: 64,
                    height: 500,
                }),
                Some(Rect {
                    x: 0,
                    y: 900,
                    width: 64,
                    height: 180,
                }),
                TaskbarEdge::Left,
                96,
            ),
            260,
        );

        assert_eq!(slot.width, 40);
        assert_eq!(slot.x, 12);
        assert!(slot.y >= 580);
        assert!(slot.y + slot.height <= 900);
    }

    #[test]
    fn right_taskbar_uses_the_same_vertical_axis_rules() {
        let taskbar = Rect {
            x: 1856,
            y: -40,
            width: 64,
            height: 1120,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: 1856,
                    y: -40,
                    width: 64,
                    height: 600,
                }),
                Some(Rect {
                    x: 1856,
                    y: 900,
                    width: 64,
                    height: 180,
                }),
                TaskbarEdge::Right,
                96,
            ),
            260,
        );

        assert_eq!(slot.width, 40);
        assert_eq!(slot.x, 1868);
        assert!(slot.y >= 560);
        assert!(slot.y + slot.height <= 900);
    }

    #[test]
    fn narrow_slot_shrinks_to_minimum_and_clamps_inside_taskbar() {
        let taskbar = Rect {
            x: 100,
            y: 1000,
            width: 500,
            height: 48,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: 100,
                    y: 1000,
                    width: 150,
                    height: 48,
                }),
                Some(Rect {
                    x: 426,
                    y: 1000,
                    width: 50,
                    height: 48,
                }),
                TaskbarEdge::Bottom,
                96,
            ),
            260,
        );

        assert_eq!(slot.width, 160);
        assert!(taskbar.contains(&slot));
        assert!(slot.x >= 250);
        assert!(slot.x + slot.width <= 426);
    }

    #[test]
    fn dpi_scales_logical_width_without_overflow() {
        let taskbar = Rect {
            x: 0,
            y: 1032,
            width: 3000,
            height: 60,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: 0,
                    y: 1032,
                    width: 1000,
                    height: 60,
                }),
                Some(Rect {
                    x: 2500,
                    y: 1032,
                    width: 200,
                    height: 60,
                }),
                TaskbarEdge::Bottom,
                120,
            ),
            260,
        );

        assert_eq!(slot.width, 325);
        assert_eq!(slot.height, 50);
        assert!(slot.x >= 1000);
        assert!(slot.x + slot.width <= 2500);
    }

    #[test]
    fn missing_notification_area_falls_back_to_taskbar_end() {
        let taskbar = Rect {
            x: -1920,
            y: 1032,
            width: 1920,
            height: 48,
        };
        let slot = compute_slot(
            &snapshot(
                taskbar,
                Some(Rect {
                    x: -1920,
                    y: 1032,
                    width: 1200,
                    height: 48,
                }),
                None,
                TaskbarEdge::Bottom,
                96,
            ),
            260,
        );

        assert_eq!(slot.x, -712);
        assert_eq!(slot.y, 1036);
        assert!(taskbar.contains(&slot));
    }

    #[test]
    fn auto_hide_rectangles_are_still_clamped_inside_taskbar() {
        let taskbar = Rect {
            x: 0,
            y: 1038,
            width: 1920,
            height: 42,
        };
        let mut value = snapshot(taskbar, None, None, TaskbarEdge::Bottom, 96);
        value.auto_hide = true;
        let slot = compute_slot(&value, 260);

        assert!(taskbar.contains(&slot));
        assert_eq!(slot.height, 40);
    }

    #[test]
    fn every_supported_logical_width_stays_inside_horizontal_and_vertical_taskbars() {
        let horizontal_taskbar = Rect {
            x: 0,
            y: 1032,
            width: 1920,
            height: 48,
        };
        let horizontal = snapshot(
            horizontal_taskbar,
            Some(Rect {
                x: 0,
                y: 1032,
                width: 1200,
                height: 48,
            }),
            Some(Rect {
                x: 1760,
                y: 1032,
                width: 160,
                height: 48,
            }),
            TaskbarEdge::Bottom,
            96,
        );
        let vertical_taskbar = Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 1080,
        };
        let vertical = snapshot(
            vertical_taskbar,
            Some(Rect {
                x: 0,
                y: 80,
                width: 64,
                height: 500,
            }),
            Some(Rect {
                x: 0,
                y: 900,
                width: 64,
                height: 180,
            }),
            TaskbarEdge::Left,
            96,
        );

        for logical_width in [104, 168, 318] {
            assert!(horizontal_taskbar.contains(&compute_slot(&horizontal, logical_width)));
            assert!(vertical_taskbar.contains(&compute_slot(&vertical, logical_width)));
        }
    }
}
