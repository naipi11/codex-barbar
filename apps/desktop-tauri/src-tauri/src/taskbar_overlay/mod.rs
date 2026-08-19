pub mod positioning;
pub mod win32;
pub mod window;

use crate::status_surfaces::window_lifecycle::{CloseOutcome, close_cached_or_labeled};
use positioning::{Rect, compute_slot};
use tauri::LogicalSize;

const TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED: &str = "TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeasurementAvailability {
    Ready,
    Deferred,
}

fn enable_windows_with(
    ensure_visible: impl FnOnce() -> Result<(), String>,
    ensure_measurement: impl FnOnce() -> Result<(), String>,
) -> Result<MeasurementAvailability, String> {
    ensure_visible()?;
    Ok(if ensure_measurement().is_ok() {
        MeasurementAvailability::Ready
    } else {
        MeasurementAvailability::Deferred
    })
}

fn disable_windows_with(
    close_measurement: impl FnOnce() -> Result<(), String>,
    close_visible: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    close_measurement().map_err(|_| TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED.to_string())?;
    close_visible()
}

fn apply_disabled_with(
    enabled: &mut bool,
    close_measurement: impl FnOnce() -> Result<(), String>,
    close_visible: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let previous_enabled = *enabled;
    *enabled = false;
    if let Err(error) = disable_windows_with(close_measurement, close_visible) {
        *enabled = previous_enabled;
        return Err(error);
    }
    Ok(())
}

fn require_measurement_destroyed(outcome: CloseOutcome) -> Result<(), &'static str> {
    match outcome {
        CloseOutcome::Destroyed => Ok(()),
        CloseOutcome::HiddenPendingDestroy => Err(TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED),
    }
}

fn enable_logical_width(previously_enabled: bool, current_width: u32) -> u32 {
    if previously_enabled {
        current_width
    } else {
        window::TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH
    }
}

fn replace_measurement_cache_preserving_enabled<T>(
    enabled: &mut bool,
    cache: &mut Option<T>,
    live_measurement: Option<T>,
) {
    let previous_enabled = *enabled;
    *cache = live_measurement;
    *enabled = previous_enabled;
}

trait EnabledWindowOperations {
    fn ensure_measurement(&mut self) -> Result<(), String>;
    fn reposition_visible(&mut self) -> Result<(), String>;
}

fn reconcile_enabled_with(
    operations: &mut impl EnabledWindowOperations,
    on_measurement_deferred: impl FnOnce(),
) -> Result<MeasurementAvailability, String> {
    let availability = if operations.ensure_measurement().is_ok() {
        MeasurementAvailability::Ready
    } else {
        MeasurementAvailability::Deferred
    };
    if availability == MeasurementAvailability::Deferred {
        on_measurement_deferred();
    }
    operations.reposition_visible()?;
    Ok(availability)
}

pub fn clamp_logical_width(width: f64) -> u32 {
    if !width.is_finite() || width <= 0.0 {
        return window::TASKBAR_MIN_LOGICAL_WIDTH;
    }
    // Measurements are CSS logical pixels, so round to the nearest integer
    // before applying the supported inclusive range.
    (width.round() as i64).clamp(
        i64::from(window::TASKBAR_MIN_LOGICAL_WIDTH),
        i64::from(window::TASKBAR_MAX_LOGICAL_WIDTH),
    ) as u32
}

trait TaskbarWidthOperations {
    fn logical_width(&self) -> u32;
    fn set_logical_width(&mut self, width: u32);
    fn resize(&mut self, width: u32) -> Result<(), String>;
    fn reposition(&mut self) -> Result<(), String>;
    fn invalidate_slot(&mut self);
}

fn apply_content_width_transaction(
    operations: &mut impl TaskbarWidthOperations,
    width: u32,
) -> Result<(), String> {
    let previous_width = operations.logical_width();
    if width == previous_width {
        return Ok(());
    }

    operations
        .resize(width)
        .map_err(|_| "TASKBAR_STATUS_RESIZE_FAILED".to_string())?;
    operations.set_logical_width(width);
    if operations.reposition().is_ok() {
        return Ok(());
    }

    // The native window is already the requested width, so return it to the
    // previous size before reporting failure. If that compensating resize
    // fails, keep requested width in state because it is the only known
    // truthful native size; the slot is unknown in either failure branch.
    operations.set_logical_width(previous_width);
    if operations.resize(previous_width).is_err() {
        operations.set_logical_width(width);
        operations.invalidate_slot();
        return Err("TASKBAR_STATUS_RESIZE_FAILED".to_string());
    }
    if operations.reposition().is_err() {
        operations.invalidate_slot();
    }
    Err("TASKBAR_STATUS_RESIZE_FAILED".to_string())
}

pub struct TaskbarOverlay {
    window: Option<tauri::WebviewWindow>,
    measurement_window: Option<tauri::WebviewWindow>,
    enabled: bool,
    logical_width: u32,
    last_slot: Option<Rect>,
}

impl Default for TaskbarOverlay {
    fn default() -> Self {
        Self {
            window: None,
            measurement_window: None,
            enabled: false,
            logical_width: window::TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH,
            last_slot: None,
        }
    }
}

impl TaskbarOverlay {
    pub fn apply_enabled(&mut self, app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
        let previous_enabled = self.enabled;
        if !enabled {
            return self.cleanup_disabled_window(app);
        }
        self.enabled = true;

        self.logical_width = enable_logical_width(previous_enabled, self.logical_width);

        let logical_width = self.logical_width;
        let mut visible_window = None;
        let mut measurement_window = None;
        let availability = enable_windows_with(
            || {
                visible_window = Some(window::get_or_create(app, logical_width)?);
                Ok(())
            },
            || {
                measurement_window = Some(window::get_or_create_measurement(app)?);
                Ok(())
            },
        )?;
        self.window = visible_window;
        if let Some(measurement_window) = measurement_window {
            self.measurement_window = Some(measurement_window);
        }
        if availability == MeasurementAvailability::Deferred {
            tracing::debug!(
                code = "TASKBAR_MEASUREMENT_CREATE_DEFERRED",
                proof_mode = crate::proof_harness::is_taskbar_status_proof(app),
                "taskbar measurement helper retry deferred"
            );
        }
        self.reposition(app)
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn force_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_content_width(&mut self, app: &tauri::AppHandle, width: u32) -> Result<(), String> {
        let width = clamp_logical_width(f64::from(width));
        let mut operations = OverlayWidthOperations { overlay: self, app };
        apply_content_width_transaction(&mut operations, width)
    }

    pub fn reposition(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        let window = self.ensure_window(app)?;
        let Some(snapshot) = win32::discover_native() else {
            // Keep the last known slot visible. Hiding here made the bar blink
            // whenever Explorer briefly failed discovery after a desktop click.
            if let Some(slot) = self.last_slot {
                let _ = window::position_and_show(&window, slot);
                return Ok(());
            }
            return Err("TASKBAR_DISCOVERY_UNAVAILABLE".to_string());
        };
        let slot = compute_slot(&snapshot, self.logical_width);
        window::position_and_show(&window, slot)?;
        self.last_slot = Some(slot);
        Ok(())
    }

    pub fn handle_shell_change(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        match crate::status_surfaces::reconciliation_action(self.is_enabled()) {
            crate::status_surfaces::ReconciliationAction::Reposition => {
                let mut operations = OverlayEnabledWindowOperations { overlay: self, app };
                reconcile_enabled_with(&mut operations, || {
                    tracing::debug!(
                        code = "TASKBAR_MEASUREMENT_CREATE_DEFERRED",
                        proof_mode = crate::proof_harness::is_taskbar_status_proof(app),
                        "taskbar measurement helper retry deferred"
                    );
                })?;
                Ok(())
            }
            crate::status_surfaces::ReconciliationAction::Cleanup => {
                self.cleanup_disabled_window(app)
            }
        }
    }

    pub fn handle_window_destroyed(&mut self) {
        self.window = None;
        self.last_slot = None;
    }

    pub fn handle_measurement_window_destroyed(&mut self) {
        self.reconcile_measurement_window_after_destroy(None);
    }

    pub fn reconcile_measurement_window_after_destroy(
        &mut self,
        live_measurement: Option<tauri::WebviewWindow>,
    ) {
        replace_measurement_cache_preserving_enabled(
            &mut self.enabled,
            &mut self.measurement_window,
            live_measurement,
        );
    }

    fn cleanup_disabled_window(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        let measurement_window = &mut self.measurement_window;
        let visible_window = &mut self.window;
        apply_disabled_with(
            &mut self.enabled,
            || {
                let outcome = close_cached_or_labeled(
                    app,
                    measurement_window,
                    window::TASKBAR_MEASUREMENT_WINDOW_LABEL,
                )?;
                require_measurement_destroyed(outcome).map_err(str::to_string)
            },
            || {
                close_cached_or_labeled(app, visible_window, window::TASKBAR_WINDOW_LABEL)
                    .map(|_| ())
            },
        )?;
        self.last_slot = None;
        Ok(())
    }

    fn ensure_measurement_window(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        let measurement = window::get_or_create_measurement(app)?;
        self.measurement_window = Some(measurement);
        Ok(())
    }

    fn ensure_window(&mut self, app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
        let window = window::get_or_create(app, self.logical_width)?;
        self.window = Some(window.clone());
        Ok(window)
    }
}

struct OverlayEnabledWindowOperations<'a> {
    overlay: &'a mut TaskbarOverlay,
    app: &'a tauri::AppHandle,
}

impl EnabledWindowOperations for OverlayEnabledWindowOperations<'_> {
    fn ensure_measurement(&mut self) -> Result<(), String> {
        self.overlay.ensure_measurement_window(self.app)
    }

    fn reposition_visible(&mut self) -> Result<(), String> {
        self.overlay.reposition(self.app)
    }
}

struct OverlayWidthOperations<'a> {
    overlay: &'a mut TaskbarOverlay,
    app: &'a tauri::AppHandle,
}

impl TaskbarWidthOperations for OverlayWidthOperations<'_> {
    fn logical_width(&self) -> u32 {
        self.overlay.logical_width
    }

    fn set_logical_width(&mut self, width: u32) {
        self.overlay.logical_width = width;
    }

    fn resize(&mut self, width: u32) -> Result<(), String> {
        let Some(window) = self.overlay.window.as_ref() else {
            return Ok(());
        };
        window
            .set_size(LogicalSize::new(
                f64::from(width),
                f64::from(window::TASKBAR_LOGICAL_HEIGHT),
            ))
            .map_err(|_| "TASKBAR_STATUS_RESIZE_FAILED".to_string())
    }

    fn reposition(&mut self) -> Result<(), String> {
        self.overlay.reposition(self.app)
    }

    fn invalidate_slot(&mut self) {
        self.overlay.last_slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn overlay_starts_at_the_safe_fallback_width() {
        let overlay = TaskbarOverlay::default();
        assert_eq!(
            overlay.logical_width,
            window::TASKBAR_SAFE_FALLBACK_LOGICAL_WIDTH
        );
        assert_eq!(overlay.last_slot, None);
    }

    #[test]
    fn a_new_enable_uses_fallback_while_reconciliation_keeps_confirmed_width() {
        assert_eq!(enable_logical_width(false, 167), 318);
        assert_eq!(enable_logical_width(true, 167), 167);
    }

    #[test]
    fn measurement_creation_failure_keeps_visible_enabled_at_fallback() {
        let result = enable_windows_with(|| Ok(()), || Err("CREATE".into())).unwrap();
        assert_eq!(result, MeasurementAvailability::Deferred);
    }

    #[test]
    fn disable_closes_measurement_before_visible() {
        let calls = std::cell::RefCell::new(Vec::new());
        disable_windows_with(
            || {
                calls.borrow_mut().push("measurement");
                Ok(())
            },
            || {
                calls.borrow_mut().push("visible");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*calls.borrow(), ["measurement", "visible"]);
    }

    #[test]
    fn measurement_close_failure_does_not_close_visible() {
        let visible_closed = std::cell::Cell::new(false);
        assert_eq!(
            disable_windows_with(
                || Err("CLOSE".into()),
                || {
                    visible_closed.set(true);
                    Ok(())
                },
            ),
            Err("TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED".to_string())
        );
        assert!(!visible_closed.get());
    }

    #[test]
    fn failed_measurement_close_restores_enabled_without_closing_visible() {
        let mut enabled = true;
        let visible_closed = std::cell::Cell::new(false);

        assert_eq!(
            apply_disabled_with(
                &mut enabled,
                || Err("native close failed".to_string()),
                || {
                    visible_closed.set(true);
                    Ok(())
                },
            ),
            Err("TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED".to_string())
        );
        assert!(enabled);
        assert!(!visible_closed.get());
    }

    #[test]
    fn measurement_destroyed_clears_cache_without_changing_enabled() {
        let mut enabled = true;
        let mut measurement_cache = Some("stale helper");

        replace_measurement_cache_preserving_enabled(&mut enabled, &mut measurement_cache, None);

        assert!(enabled);
        assert_eq!(measurement_cache, None);
    }

    #[test]
    fn deferred_measurement_reconciliation_refreshes_cache_without_changing_enabled() {
        let mut enabled = true;
        let mut measurement_cache = Some("old helper");

        replace_measurement_cache_preserving_enabled(
            &mut enabled,
            &mut measurement_cache,
            Some("new helper"),
        );

        assert!(enabled);
        assert_eq!(measurement_cache, Some("new helper"));
    }

    #[test]
    fn visible_close_failure_restores_enabled_then_reconciliation_recreates_helper_first() {
        let mut enabled = true;
        let calls = std::cell::RefCell::new(Vec::new());

        assert_eq!(
            apply_disabled_with(
                &mut enabled,
                || {
                    calls.borrow_mut().push("close measurement");
                    Ok(())
                },
                || {
                    calls.borrow_mut().push("close visible");
                    Err("visible close failed".to_string())
                },
            ),
            Err("visible close failed".to_string())
        );
        assert!(enabled);

        struct FakeEnabledWindowOperations<'a> {
            calls: &'a std::cell::RefCell<Vec<&'static str>>,
        }

        impl EnabledWindowOperations for FakeEnabledWindowOperations<'_> {
            fn ensure_measurement(&mut self) -> Result<(), String> {
                self.calls.borrow_mut().push("recreate measurement");
                Ok(())
            }

            fn reposition_visible(&mut self) -> Result<(), String> {
                self.calls.borrow_mut().push("reposition visible");
                Ok(())
            }
        }

        let mut operations = FakeEnabledWindowOperations { calls: &calls };
        assert_eq!(
            reconcile_enabled_with(&mut operations, || {}),
            Ok(MeasurementAvailability::Ready)
        );
        assert_eq!(
            *calls.borrow(),
            [
                "close measurement",
                "close visible",
                "recreate measurement",
                "reposition visible"
            ]
        );
    }

    #[test]
    fn deferred_diagnostic_runs_before_failed_visible_reposition() {
        struct FailingEnabledWindowOperations<'a> {
            calls: &'a std::cell::RefCell<Vec<&'static str>>,
        }

        impl EnabledWindowOperations for FailingEnabledWindowOperations<'_> {
            fn ensure_measurement(&mut self) -> Result<(), String> {
                self.calls.borrow_mut().push("ensure measurement");
                Err("helper create failed".to_string())
            }

            fn reposition_visible(&mut self) -> Result<(), String> {
                self.calls.borrow_mut().push("reposition visible");
                Err("visible reposition failed".to_string())
            }
        }

        let calls = std::cell::RefCell::new(Vec::new());
        let mut operations = FailingEnabledWindowOperations { calls: &calls };

        assert_eq!(
            reconcile_enabled_with(&mut operations, || {
                calls.borrow_mut().push("measurement deferred diagnostic");
            }),
            Err("visible reposition failed".to_string())
        );
        assert_eq!(
            *calls.borrow(),
            [
                "ensure measurement",
                "measurement deferred diagnostic",
                "reposition visible"
            ]
        );
    }

    #[test]
    fn hidden_pending_destroy_is_not_a_completed_measurement_close() {
        assert_eq!(
            require_measurement_destroyed(CloseOutcome::HiddenPendingDestroy),
            Err("TASKBAR_MEASUREMENT_WINDOW_CLOSE_FAILED")
        );
    }

    #[test]
    fn clamp_logical_width_rounds_then_keeps_the_supported_range() {
        assert_eq!(clamp_logical_width(f64::NAN), 104);
        assert_eq!(clamp_logical_width(-1.0), 104);
        assert_eq!(clamp_logical_width(103.0), 104);
        assert_eq!(clamp_logical_width(168.4), 168);
        assert_eq!(clamp_logical_width(318.0), 318);
        assert_eq!(clamp_logical_width(900.0), 318);
    }

    #[derive(Default)]
    struct FakeWidthOperations {
        logical_width: u32,
        resize_results: VecDeque<Result<(), String>>,
        reposition_results: VecDeque<Result<(), String>>,
        calls: Vec<String>,
        slot_invalidated: bool,
    }

    impl TaskbarWidthOperations for FakeWidthOperations {
        fn logical_width(&self) -> u32 {
            self.logical_width
        }

        fn set_logical_width(&mut self, width: u32) {
            self.logical_width = width;
        }

        fn resize(&mut self, width: u32) -> Result<(), String> {
            self.calls.push(format!("resize:{width}"));
            self.resize_results.pop_front().unwrap_or(Ok(()))
        }

        fn reposition(&mut self) -> Result<(), String> {
            self.calls
                .push(format!("reposition:{}", self.logical_width));
            self.reposition_results.pop_front().unwrap_or(Ok(()))
        }

        fn invalidate_slot(&mut self) {
            self.slot_invalidated = true;
        }
    }

    #[test]
    fn width_transaction_commits_after_resize_and_reposition_succeed() {
        let mut operations = FakeWidthOperations {
            logical_width: 166,
            ..Default::default()
        };

        apply_content_width_transaction(&mut operations, 185).unwrap();

        assert_eq!(operations.logical_width, 185);
        assert_eq!(operations.calls, ["resize:185", "reposition:185"]);
        assert!(!operations.slot_invalidated);
    }

    #[test]
    fn width_transaction_keeps_previous_state_when_initial_resize_fails() {
        let mut operations = FakeWidthOperations {
            logical_width: 166,
            resize_results: VecDeque::from([Err("native failure".to_string())]),
            ..Default::default()
        };

        assert_eq!(
            apply_content_width_transaction(&mut operations, 185),
            Err("TASKBAR_STATUS_RESIZE_FAILED".to_string())
        );

        assert_eq!(operations.logical_width, 166);
        assert_eq!(operations.calls, ["resize:185"]);
        assert!(!operations.slot_invalidated);
    }

    #[test]
    fn width_transaction_compensates_size_and_position_after_reposition_failure() {
        let mut operations = FakeWidthOperations {
            logical_width: 166,
            reposition_results: VecDeque::from([Err("position failure".to_string()), Ok(())]),
            ..Default::default()
        };

        assert_eq!(
            apply_content_width_transaction(&mut operations, 185),
            Err("TASKBAR_STATUS_RESIZE_FAILED".to_string())
        );

        assert_eq!(operations.logical_width, 166);
        assert_eq!(
            operations.calls,
            [
                "resize:185",
                "reposition:185",
                "resize:166",
                "reposition:166"
            ]
        );
        assert!(!operations.slot_invalidated);
    }

    #[test]
    fn width_transaction_keeps_requested_state_when_compensation_resize_fails() {
        let mut operations = FakeWidthOperations {
            logical_width: 166,
            resize_results: VecDeque::from([Ok(()), Err("compensation failure".to_string())]),
            reposition_results: VecDeque::from([Err("position failure".to_string())]),
            ..Default::default()
        };

        assert_eq!(
            apply_content_width_transaction(&mut operations, 185),
            Err("TASKBAR_STATUS_RESIZE_FAILED".to_string())
        );

        assert_eq!(operations.logical_width, 185);
        assert_eq!(
            operations.calls,
            ["resize:185", "reposition:185", "resize:166"]
        );
        assert!(operations.slot_invalidated);
    }
}
