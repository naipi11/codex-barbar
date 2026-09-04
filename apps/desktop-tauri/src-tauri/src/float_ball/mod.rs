pub mod geometry;
pub mod window;

use crate::geometry_store;
use crate::status_surfaces::window_lifecycle::close_cached_or_labeled;
use geometry::{
    FloatBallPresentation, MonitorGeometry, Point, physical_to_logical, presentation_rect,
    restore_position,
};
use tauri::PhysicalPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityIntent {
    Show,
    Hide,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FloatBallState {
    enabled: bool,
    logical_position: Option<Point>,
    presentation: FloatBallPresentation,
}

impl FloatBallState {
    pub fn apply_enabled(&mut self, enabled: bool) -> VisibilityIntent {
        self.enabled = enabled;
        if enabled {
            VisibilityIntent::Show
        } else {
            VisibilityIntent::Hide
        }
    }

    pub fn remember_logical_position(&mut self, position: Point) {
        self.logical_position = Some(position);
    }

    pub fn presentation(&self) -> FloatBallPresentation {
        self.presentation
    }

    pub fn set_presentation(&mut self, presentation: FloatBallPresentation) {
        self.presentation = presentation;
    }

    pub fn should_persist_moved_event(&self) -> bool {
        self.presentation == FloatBallPresentation::Collapsed
    }

    #[allow(dead_code)]
    pub fn logical_position(&self) -> Option<Point> {
        self.logical_position
    }
}

#[derive(Default)]
pub struct FloatBall {
    window: Option<tauri::WebviewWindow>,
    state: FloatBallState,
}

impl FloatBall {
    pub fn apply_enabled(&mut self, app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
        let previous_enabled = self.state.enabled;
        let intent = self.state.apply_enabled(enabled);
        if intent == VisibilityIntent::Hide {
            match self.cleanup_disabled_window(app) {
                Ok(_) => return Ok(()),
                Err(error) => {
                    self.state.apply_enabled(previous_enabled);
                    return Err(error);
                }
            }
        }

        self.ensure_window(app)?;
        self.reposition(app)
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.state.enabled
    }

    pub(crate) fn force_enabled(&mut self, enabled: bool) {
        self.state.enabled = enabled;
    }

    pub fn reposition(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        if !self.state.enabled {
            return Ok(());
        }
        let window = self.ensure_window(app)?;
        let monitors = collect_monitors(&window)?;
        let saved = self.state.logical_position.or_else(|| {
            geometry_store::load_entry(window::FLOAT_BALL_GEOMETRY_KEY).map(|entry| Point {
                x: entry.x,
                y: entry.y,
            })
        });
        let physical = restore_position(saved, &monitors);
        let monitor = monitor_for_point(physical, &monitors).unwrap_or_else(|| &monitors[0]);
        let rect = presentation_rect(
            physical,
            monitor.monitor,
            monitor.scale,
            self.state.presentation(),
        );
        window::position_and_show(&window, rect)?;
        if self.state.should_persist_moved_event() {
            self.state
                .remember_logical_position(physical_to_logical(physical, monitor.scale));
        }
        Ok(())
    }

    pub fn set_expanded(&mut self, app: &tauri::AppHandle, expanded: bool) -> Result<(), String> {
        if !self.state.enabled {
            return Err("FLOAT_BALL_DISABLED".to_string());
        }
        let previous = self.state.presentation();
        self.state.set_presentation(if expanded {
            FloatBallPresentation::Expanded
        } else {
            FloatBallPresentation::Collapsed
        });
        if let Err(error) = self.reposition(app) {
            self.state.set_presentation(previous);
            return Err(error);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn handle_shell_change(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        match crate::status_surfaces::reconciliation_action(self.is_enabled()) {
            crate::status_surfaces::ReconciliationAction::Reposition => self.reposition(app),
            crate::status_surfaces::ReconciliationAction::Cleanup => {
                self.cleanup_disabled_window(app)
            }
        }
    }

    pub fn hide_for_fullscreen(&self) -> Result<(), String> {
        #[cfg(windows)]
        if let Some(window) = self.window.as_ref() {
            crate::shell::dwm::hide_window(window)
                .map_err(|_| "FLOAT_BALL_FULLSCREEN_HIDE_FAILED".to_string())?;
        }
        Ok(())
    }

    pub fn restore_after_shell(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        if !self.state.enabled {
            return Ok(());
        }
        let window = self.ensure_window(app)?;
        let _ = window::show_noactivate(&window);
        self.reposition(app)?;
        window::reassert_topmost(&window)
    }

    pub fn reassert_topmost(&self) -> Result<(), String> {
        if !self.state.enabled {
            return Ok(());
        }
        if let Some(window) = self.window.as_ref() {
            let _ = window::show_noactivate(window);
            window::reassert_topmost(window)?;
        }
        Ok(())
    }

    pub fn observe_lifecycle(
        &self,
        foreground_class: crate::shell::fullscreen_guard::ForegroundClass,
        suspension_reason: crate::shell::surface_lifecycle_trace::SurfaceSuspensionReason,
    ) -> crate::shell::surface_lifecycle_trace::SurfaceLifecycleSnapshot {
        use crate::shell::surface_lifecycle_trace::{
            SurfaceBounds, SurfaceLabel, SurfaceLifecycleSnapshot, TopmostResult,
        };
        let observation = self
            .window
            .as_ref()
            .and_then(|window| window::observe(window).ok());
        SurfaceLifecycleSnapshot {
            surface: SurfaceLabel::FloatBall,
            desired_visible: self.state.enabled
                && suspension_reason
                    == crate::shell::surface_lifecycle_trace::SurfaceSuspensionReason::None,
            actual_visible: observation.is_some_and(|observed| observed.visible),
            minimized: observation.is_some_and(|observed| observed.minimized),
            bounds: observation.map(|observed| SurfaceBounds {
                x: observed.x,
                y: observed.y,
                width: observed.width,
                height: observed.height,
            }),
            topmost_result: if observation.is_some() {
                TopmostResult::Ok
            } else if self.state.enabled {
                TopmostResult::Failed
            } else {
                TopmostResult::Skipped
            },
            foreground_class,
            suspension_reason,
            observed_at_ms: 0,
        }
        .observed_now()
    }

    pub fn handle_moved(&mut self, window: &tauri::WebviewWindow, position: PhysicalPosition<i32>) {
        if !self.state.enabled || !self.state.should_persist_moved_event() {
            return;
        }
        let scale = window
            .monitor_from_point(f64::from(position.x), f64::from(position.y))
            .ok()
            .flatten()
            .map(|monitor| monitor.scale_factor())
            .or_else(|| window.scale_factor().ok())
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0);
        let logical = physical_to_logical(
            Point {
                x: position.x,
                y: position.y,
            },
            scale,
        );
        self.state.remember_logical_position(logical);
        geometry_store::save_entry(
            window::FLOAT_BALL_GEOMETRY_KEY,
            crate::geometry_store::StoredGeometry {
                x: logical.x,
                y: logical.y,
                width: None,
                height: None,
            },
        );
    }

    pub fn handle_window_destroyed(&mut self) {
        self.window = None;
    }

    fn cleanup_disabled_window(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        close_cached_or_labeled(app, &mut self.window, window::FLOAT_BALL_WINDOW_LABEL)?;
        Ok(())
    }

    fn ensure_window(&mut self, app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
        let window = window::get_or_create(app)?;
        self.window = Some(window.clone());
        Ok(window)
    }
}

fn monitor_rect(monitor: &tauri::Monitor) -> crate::window_positioner::Rect {
    crate::window_positioner::Rect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width.min(i32::MAX as u32) as i32,
        height: monitor.size().height.min(i32::MAX as u32) as i32,
    }
}

fn work_area_rect(monitor: &tauri::Monitor) -> crate::window_positioner::Rect {
    crate::window_positioner::Rect {
        x: monitor.work_area().position.x,
        y: monitor.work_area().position.y,
        width: monitor.work_area().size.width.min(i32::MAX as u32) as i32,
        height: monitor.work_area().size.height.min(i32::MAX as u32) as i32,
    }
}

fn collect_monitors(window: &tauri::WebviewWindow) -> Result<Vec<MonitorGeometry>, String> {
    let available = window
        .available_monitors()
        .map_err(|_| "FLOAT_BALL_MONITORS_UNAVAILABLE".to_string())?;
    let primary_position = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| *monitor.position());
    let mut monitors = available
        .iter()
        .map(|monitor| MonitorGeometry {
            monitor: monitor_rect(monitor),
            work_area: work_area_rect(monitor),
            scale: monitor.scale_factor(),
            primary: primary_position.is_some_and(|position| position == *monitor.position()),
        })
        .collect::<Vec<_>>();
    if monitors.is_empty()
        && let Ok(Some(monitor)) = window.current_monitor()
    {
        monitors.push(MonitorGeometry {
            monitor: monitor_rect(&monitor),
            work_area: work_area_rect(&monitor),
            scale: monitor.scale_factor(),
            primary: primary_position.is_some_and(|position| position == *monitor.position()),
        });
    }
    if monitors.is_empty() {
        return Err("FLOAT_BALL_MONITORS_UNAVAILABLE".to_string());
    }
    Ok(monitors)
}

fn monitor_for_point(point: Point, monitors: &[MonitorGeometry]) -> Option<&MonitorGeometry> {
    monitors.iter().find(|monitor| {
        let right = monitor
            .monitor
            .x
            .saturating_add(monitor.monitor.width.max(1));
        let bottom = monitor
            .monitor
            .y
            .saturating_add(monitor.monitor.height.max(1));
        point.x >= monitor.monitor.x
            && point.y >= monitor.monitor.y
            && point.x < right
            && point.y < bottom
    })
}

#[cfg(test)]
mod tests {
    use super::geometry::{FloatBallPresentation, Point};
    use super::{FloatBallState, VisibilityIntent};

    #[test]
    fn disabled_state_preserves_position_and_requests_hide() {
        let mut state = FloatBallState::default();
        state.remember_logical_position(Point { x: -20, y: 30 });
        assert_eq!(state.apply_enabled(false), VisibilityIntent::Hide);
        assert_eq!(state.logical_position(), Some(Point { x: -20, y: 30 }));
    }

    #[test]
    fn expanded_presentation_never_replaces_saved_collapsed_position() {
        let mut state = FloatBallState::default();
        state.remember_logical_position(Point { x: -240, y: 96 });
        state.set_presentation(FloatBallPresentation::Expanded);
        assert!(!state.should_persist_moved_event());
        assert_eq!(state.logical_position(), Some(Point { x: -240, y: 96 }));
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{FloatBallState, VisibilityIntent};

    #[test]
    fn float_ball_label_and_route_are_stable() {
        assert_eq!(super::window::FLOAT_BALL_WINDOW_LABEL, "float-ball");
        assert_eq!(
            super::window::FLOAT_BALL_FRONTEND_ROUTE,
            "index.html?window=float-ball"
        );
    }

    #[test]
    fn disabling_does_not_discard_collapsed_position() {
        let mut state = FloatBallState::default();
        state.remember_logical_position(super::geometry::Point { x: -240, y: 96 });

        assert_eq!(state.apply_enabled(false), VisibilityIntent::Hide);
        assert_eq!(
            state.logical_position(),
            Some(super::geometry::Point { x: -240, y: 96 })
        );
    }

    #[test]
    fn close_requests_are_consumed_for_the_float_ball_window() {
        assert!(super::window::should_prevent_close(
            super::window::FLOAT_BALL_WINDOW_LABEL
        ));
    }
}
