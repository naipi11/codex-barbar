use crate::window_positioner::Rect;

pub const FLOAT_BALL_COLLAPSED_WIDTH: i32 = 40;
pub const FLOAT_BALL_COLLAPSED_HEIGHT: i32 = 40;
pub const FLOAT_BALL_EXPANDED_WIDTH: i32 = 260;
pub const FLOAT_BALL_EXPANDED_HEIGHT: i32 = 148;
pub const FLOAT_BALL_LOGICAL_SIZE: u32 = FLOAT_BALL_COLLAPSED_WIDTH as u32;
const FLOAT_BALL_LOGICAL_MARGIN: i32 = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FloatBallPresentation {
    #[default]
    Collapsed,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorGeometry {
    pub monitor: Rect,
    pub work_area: Rect,
    pub scale: f64,
    pub primary: bool,
}

fn normalized_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn scaled_i32(value: i32, scale: f64) -> i32 {
    let scaled = f64::from(value) * normalized_scale(scale);
    scaled
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn scaled_presentation_size(presentation: FloatBallPresentation, scale: f64) -> (i32, i32) {
    match presentation {
        FloatBallPresentation::Collapsed => (
            scaled_i32(FLOAT_BALL_COLLAPSED_WIDTH, scale),
            scaled_i32(FLOAT_BALL_COLLAPSED_HEIGHT, scale),
        ),
        FloatBallPresentation::Expanded => (
            scaled_i32(FLOAT_BALL_EXPANDED_WIDTH, scale),
            scaled_i32(FLOAT_BALL_EXPANDED_HEIGHT, scale),
        ),
    }
}

fn clamp_rect(rect: Rect, work_area: Rect, margin: i32) -> Rect {
    let margin = margin.max(0);
    let min_x = work_area.x.saturating_add(margin);
    let min_y = work_area.y.saturating_add(margin);
    let max_x = work_area
        .x
        .saturating_add(work_area.width)
        .saturating_sub(rect.width)
        .saturating_sub(margin)
        .max(min_x);
    let max_y = work_area
        .y
        .saturating_add(work_area.height)
        .saturating_sub(rect.height)
        .saturating_sub(margin)
        .max(min_y);
    Rect {
        x: rect.x.clamp(min_x, max_x),
        y: rect.y.clamp(min_y, max_y),
        width: rect.width,
        height: rect.height,
    }
}

pub fn presentation_rect(
    collapsed: Point,
    work_area: Rect,
    scale: f64,
    presentation: FloatBallPresentation,
) -> Rect {
    let (collapsed_width, collapsed_height) =
        scaled_presentation_size(FloatBallPresentation::Collapsed, scale);
    let (width, height) = scaled_presentation_size(presentation, scale);
    let margin = scaled_i32(FLOAT_BALL_LOGICAL_MARGIN, scale);

    if presentation == FloatBallPresentation::Collapsed {
        return clamp_rect(
            Rect {
                x: collapsed.x,
                y: collapsed.y,
                width,
                height,
            },
            work_area,
            0,
        );
    }

    let distance_left = collapsed.x.saturating_sub(work_area.x);
    let distance_right = work_area
        .x
        .saturating_add(work_area.width)
        .saturating_sub(collapsed.x.saturating_add(collapsed_width));
    let distance_top = collapsed.y.saturating_sub(work_area.y);
    let distance_bottom = work_area
        .y
        .saturating_add(work_area.height)
        .saturating_sub(collapsed.y.saturating_add(collapsed_height));
    let anchor_right = distance_right <= distance_left;
    let anchor_bottom = distance_bottom <= distance_top;
    let x = if anchor_right {
        collapsed
            .x
            .saturating_add(collapsed_width)
            .saturating_sub(width)
    } else {
        collapsed.x
    };
    let y = if anchor_bottom {
        collapsed
            .y
            .saturating_add(collapsed_height)
            .saturating_sub(height)
    } else {
        collapsed.y
    };
    clamp_rect(
        Rect {
            x,
            y,
            width,
            height,
        },
        work_area,
        margin,
    )
}

fn intersection(left: Rect, right: Rect) -> Option<Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = left
        .x
        .saturating_add(left.width.max(0))
        .min(right.x.saturating_add(right.width.max(0)));
    let bottom_edge = left
        .y
        .saturating_add(left.height.max(0))
        .min(right.y.saturating_add(right.height.max(0)));
    (right_edge > x && bottom_edge > y).then_some(Rect {
        x,
        y,
        width: right_edge - x,
        height: bottom_edge - y,
    })
}

fn contains_point(rect: Rect, point: Point) -> bool {
    let right = rect.x.saturating_add(rect.width.max(0));
    let bottom = rect.y.saturating_add(rect.height.max(0));
    point.x >= rect.x && point.x < right && point.y >= rect.y && point.y < bottom
}

pub fn initial_position(monitor: Rect, work_area: Rect, scale: f64) -> Point {
    let size = scaled_i32(FLOAT_BALL_LOGICAL_SIZE as i32, scale);
    let margin = scaled_i32(FLOAT_BALL_LOGICAL_MARGIN, scale);
    let usable = intersection(monitor, work_area).unwrap_or(monitor);
    clamp_position(
        Point {
            x: usable.x.saturating_add(usable.width),
            y: usable.y.saturating_add(usable.height),
        },
        monitor,
        usable,
        size,
        margin,
    )
}

pub fn clamp_position(
    position: Point,
    monitor: Rect,
    work_area: Rect,
    size: i32,
    margin: i32,
) -> Point {
    let usable = intersection(monitor, work_area).unwrap_or(monitor);
    let size = size.max(1);
    let margin = margin.max(0);

    let min_x = usable.x.saturating_add(margin.min(usable.width.max(0)));
    let min_y = usable.y.saturating_add(margin.min(usable.height.max(0)));
    let max_x = usable
        .x
        .saturating_add(usable.width.max(1))
        .saturating_sub(size)
        .saturating_sub(margin)
        .max(min_x);
    let max_y = usable
        .y
        .saturating_add(usable.height.max(1))
        .saturating_sub(size)
        .saturating_sub(margin)
        .max(min_y);

    Point {
        x: position.x.clamp(min_x, max_x),
        y: position.y.clamp(min_y, max_y),
    }
}

pub fn physical_to_logical(position: Point, scale: f64) -> Point {
    let scale = normalized_scale(scale);
    Point {
        x: (f64::from(position.x) / scale)
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        y: (f64::from(position.y) / scale)
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
    }
}

pub fn logical_to_physical(position: Point, scale: f64) -> Point {
    Point {
        x: scaled_i32(position.x, scale),
        y: scaled_i32(position.y, scale),
    }
}

pub fn restore_position(saved_logical: Option<Point>, monitors: &[MonitorGeometry]) -> Point {
    if let Some(saved) = saved_logical {
        for monitor in monitors {
            let physical = logical_to_physical(saved, monitor.scale);
            if contains_point(monitor.monitor, physical) {
                let size = scaled_i32(FLOAT_BALL_LOGICAL_SIZE as i32, monitor.scale);
                return clamp_position(physical, monitor.monitor, monitor.monitor, size, 0);
            }
        }
    }

    monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| monitors.first())
        .map(|monitor| initial_position(monitor.monitor, monitor.work_area, monitor.scale))
        .unwrap_or(Point { x: 0, y: 0 })
}

#[cfg(test)]
mod tests {
    use super::{
        FLOAT_BALL_COLLAPSED_HEIGHT, FLOAT_BALL_COLLAPSED_WIDTH, FLOAT_BALL_LOGICAL_SIZE,
        FloatBallPresentation, MonitorGeometry, Point, clamp_position, initial_position,
        logical_to_physical, physical_to_logical, presentation_rect, restore_position,
    };
    use crate::window_positioner::Rect;

    #[test]
    fn collapsed_size_is_icon_sized_logical_pixels() {
        assert_eq!(FLOAT_BALL_COLLAPSED_WIDTH, 40);
        assert_eq!(FLOAT_BALL_COLLAPSED_HEIGHT, 40);
    }

    #[test]
    fn bottom_right_ball_expands_up_and_left() {
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1032,
        };
        let collapsed = Point { x: 1872, y: 984 };
        assert_eq!(
            presentation_rect(collapsed, work_area, 1.0, FloatBallPresentation::Expanded,),
            Rect {
                x: 1652,
                y: 876,
                width: 260,
                height: 148,
            }
        );
    }

    #[test]
    fn top_left_ball_expands_down_and_right() {
        let work_area = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 1032,
        };
        let collapsed = Point { x: -1912, y: -112 };
        assert_eq!(
            presentation_rect(collapsed, work_area, 1.0, FloatBallPresentation::Expanded,),
            Rect {
                x: -1912,
                y: -112,
                width: 260,
                height: 148,
            }
        );
    }

    #[test]
    fn expanded_rect_is_inside_work_area_at_supported_scales() {
        for scale in [1.0, 1.5, 2.0] {
            let work_area = Rect {
                x: -2560,
                y: -120,
                width: 2560,
                height: 1368,
            };
            let collapsed = initial_position(work_area, work_area, scale);
            let rect =
                presentation_rect(collapsed, work_area, scale, FloatBallPresentation::Expanded);
            assert!(rect.x >= work_area.x);
            assert!(rect.y >= work_area.y);
            assert!(rect.x + rect.width <= work_area.x + work_area.width);
            assert!(rect.y + rect.height <= work_area.y + work_area.height);
            assert!(rect.x < 0);
        }
    }
    #[test]
    fn first_run_anchors_bottom_right_inside_the_work_area() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1032,
        };

        assert_eq!(
            initial_position(monitor, work_area, 1.0),
            Point { x: 1872, y: 984 }
        );
    }

    #[test]
    fn first_run_uses_scaled_size_and_avoids_the_taskbar() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
        };
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 3840,
            height: 2064,
        };

        let position = initial_position(monitor, work_area, 2.0);
        assert_eq!(position, Point { x: 3744, y: 1968 });
        assert!(
            position.y + (FLOAT_BALL_LOGICAL_SIZE as i32 * 2) <= work_area.y + work_area.height
        );
    }

    #[test]
    fn first_run_preserves_negative_monitor_origins() {
        let monitor = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 1080,
        };
        let work_area = Rect {
            x: -1920,
            y: -120,
            width: 1920,
            height: 1032,
        };

        assert_eq!(
            initial_position(monitor, work_area, 1.0),
            Point { x: -48, y: 864 }
        );
    }

    #[test]
    fn clamp_keeps_the_full_ball_inside_the_work_area() {
        let monitor = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let work_area = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1032,
        };

        assert_eq!(
            clamp_position(Point { x: 1910, y: 1020 }, monitor, work_area, 72, 8),
            Point { x: 1840, y: 952 }
        );
    }

    #[test]
    fn dpi_conversion_round_trips_positive_and_negative_coordinates() {
        for point in [Point { x: 300, y: 450 }, Point { x: -300, y: -450 }] {
            let logical = physical_to_logical(point, 1.5);
            assert_eq!(logical_to_physical(logical, 1.5), point);
        }
    }

    #[test]
    fn invalid_scale_falls_back_to_one() {
        let point = Point { x: 100, y: -50 };
        assert_eq!(physical_to_logical(point, 0.0), point);
        assert_eq!(logical_to_physical(point, f64::NAN), point);
    }

    #[test]
    fn detached_saved_position_falls_back_to_the_primary_monitor() {
        let primary = MonitorGeometry {
            monitor: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1032,
            },
            scale: 1.0,
            primary: true,
        };
        let secondary = MonitorGeometry {
            monitor: Rect {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            work_area: Rect {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1392,
            },
            scale: 1.5,
            primary: false,
        };

        assert_eq!(
            restore_position(Some(Point { x: 5000, y: 5000 }), &[secondary, primary]),
            Point { x: 1872, y: 984 }
        );
    }

    #[test]
    fn restore_position_clamps_scaled_ball_inside_work_area() {
        let monitor = MonitorGeometry {
            monitor: Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 2560,
                height: 1368,
            },
            scale: 1.5,
            primary: true,
        };

        assert_eq!(
            restore_position(Some(Point { x: 1630, y: 843 }), &[monitor]),
            Point { x: 2445, y: 1265 }
        );
    }

    #[test]
    fn restored_saved_position_can_overlap_the_taskbar() {
        let monitor = MonitorGeometry {
            monitor: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1032,
            },
            scale: 1.0,
            primary: true,
        };

        let restored = restore_position(Some(Point { x: 900, y: 1040 }), &[monitor]);
        assert_eq!(restored, Point { x: 900, y: 1040 });
        assert_eq!(
            presentation_rect(
                restored,
                monitor.monitor,
                1.0,
                FloatBallPresentation::Collapsed
            ),
            Rect {
                x: 900,
                y: 1040,
                width: FLOAT_BALL_COLLAPSED_WIDTH,
                height: FLOAT_BALL_COLLAPSED_HEIGHT,
            }
        );
        assert_eq!(
            restore_position(Some(Point { x: 900, y: 1060 }), &[monitor]),
            Point { x: 900, y: 1040 }
        );
    }
}
