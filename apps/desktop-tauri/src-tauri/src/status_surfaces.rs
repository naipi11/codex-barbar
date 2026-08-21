use std::sync::Mutex;
use std::time::{Duration, Instant};

use codexbar::storage::AppSettings;
use tauri::Manager;

pub(crate) mod controller;
pub(crate) mod window_lifecycle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReconciliationAction {
    Reposition,
    Cleanup,
}

const STATUS_SURFACE_REASSERT_INTERVAL_MS: u64 = 250;
const STATUS_SURFACE_RECONCILE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FullscreenTransition {
    Suspend,
    Resume,
    Stable,
}

fn fullscreen_transition(was_suspended: bool, is_fullscreen: bool) -> FullscreenTransition {
    match (was_suspended, is_fullscreen) {
        (false, true) => FullscreenTransition::Suspend,
        (true, false) => FullscreenTransition::Resume,
        _ => FullscreenTransition::Stable,
    }
}

pub(crate) fn reconciliation_action(enabled: bool) -> ReconciliationAction {
    if enabled {
        ReconciliationAction::Reposition
    } else {
        ReconciliationAction::Cleanup
    }
}

pub fn surface_for_window_label(label: &str) -> Option<controller::StatusSurfaceKind> {
    if crate::float_ball::window::should_prevent_close(label) {
        return Some(controller::StatusSurfaceKind::FloatBall);
    }
    match label {
        crate::taskbar_overlay::window::TASKBAR_WINDOW_LABEL => {
            Some(controller::StatusSurfaceKind::TaskbarStatus)
        }
        _ => None,
    }
}

pub fn schedule_set_enabled(
    app: tauri::AppHandle,
    surface: controller::StatusSurfaceKind,
    enabled: bool,
) {
    tauri::async_runtime::spawn(async move {
        if controller::set_enabled_and_emit(&app, surface, enabled).is_err() {
            tracing::warn!(
                code = "STATUS_SURFACE_TRANSITION_FAILED",
                "status surface transition did not complete"
            );
        }
    });
}

#[derive(Default)]
pub struct StatusSurfaceState {
    pub taskbar: crate::taskbar_overlay::TaskbarOverlay,
    pub float_ball: crate::float_ball::FloatBall,
    pub feedback: controller::StatusSurfaceFeedbackState,
    fullscreen_suspended: bool,
}

pub fn run_non_fatal<F>(operation: F)
where
    F: FnOnce() -> Result<(), String>,
{
    if operation().is_err() {
        tracing::warn!(
            code = "STATUS_SURFACE_APPLY_FAILED",
            "status surface update skipped"
        );
    }
}

pub fn apply_status_surface_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
) -> Result<(), String> {
    let state = app.state::<Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    let mut first_error = None;
    if let Err(error) = state
        .taskbar
        .apply_enabled(app, settings.taskbar_status_enabled)
    {
        first_error = Some(error);
    }
    if let Err(error) = state
        .float_ball
        .apply_enabled(app, settings.float_ball_enabled)
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if crate::shell::fullscreen_guard::is_fullscreen_active() {
        state.fullscreen_suspended = true;
        if let Err(error) = state.taskbar.hide_for_fullscreen() {
            first_error.get_or_insert(error);
        }
        if let Err(error) = state.float_ball.hide_for_fullscreen() {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub fn apply_status_surface_settings_non_fatal(app: &tauri::AppHandle, settings: &AppSettings) {
    run_non_fatal(|| apply_status_surface_settings(app, settings));
}

fn try_with_state<T>(state: &Mutex<T>, operation: impl FnOnce(&mut T)) -> bool {
    let Ok(mut state) = state.try_lock() else {
        return false;
    };
    operation(&mut state);
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateMutationDispatch {
    Immediate,
    Deferred,
}

fn try_with_state_or_defer<T, O, D>(
    state: &Mutex<T>,
    operation: O,
    defer: D,
) -> StateMutationDispatch
where
    O: FnOnce(&mut T),
    D: FnOnce(),
{
    match state.try_lock() {
        Ok(mut state) => {
            operation(&mut state);
            StateMutationDispatch::Immediate
        }
        Err(_) => {
            defer();
            StateMutationDispatch::Deferred
        }
    }
}

fn reconcile_deferred_measurement_state<T, W>(
    state: &Mutex<T>,
    lookup_live_measurement: impl FnOnce() -> Option<W>,
    reconcile: impl FnOnce(&mut T, Option<W>),
) -> bool {
    let Ok(mut state) = state.lock() else {
        return false;
    };
    let live_measurement = lookup_live_measurement();
    reconcile(&mut state, live_measurement);
    true
}

pub fn start_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_millis(STATUS_SURFACE_REASSERT_INTERVAL_MS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_reconcile = Instant::now() - STATUS_SURFACE_RECONCILE_INTERVAL;
        loop {
            interval.tick().await;
            let fullscreen = crate::shell::fullscreen_guard::is_fullscreen_active();
            let result = app
                .state::<Mutex<StatusSurfaceState>>()
                .lock()
                .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())
                .and_then(|mut state| {
                    let transition = fullscreen_transition(state.fullscreen_suspended, fullscreen);
                    state.fullscreen_suspended = fullscreen;
                    if fullscreen {
                        let mut first_error = None;
                        if let Err(error) = state.taskbar.hide_for_fullscreen() {
                            first_error = Some(error);
                        }
                        if let Err(error) = state.float_ball.hide_for_fullscreen()
                            && first_error.is_none()
                        {
                            first_error = Some(error);
                        }
                        return first_error.map_or(Ok(()), Err);
                    }

                    let mut first_error = None;
                    if let Err(error) = state.taskbar.reassert_topmost() {
                        first_error = Some(error);
                    }
                    if let Err(error) = state.float_ball.reassert_topmost()
                        && first_error.is_none()
                    {
                        first_error = Some(error);
                    }
                    if transition == FullscreenTransition::Resume
                        || last_reconcile.elapsed() >= STATUS_SURFACE_RECONCILE_INTERVAL
                    {
                        if let Err(error) = state.taskbar.handle_shell_change(&app) {
                            first_error = Some(error);
                        }
                        if let Err(error) = state.float_ball.handle_shell_change(&app)
                            && first_error.is_none()
                        {
                            first_error = Some(error);
                        }
                        last_reconcile = Instant::now();
                    }
                    first_error.map_or(Ok(()), Err)
                });
            if result.is_err() {
                tracing::debug!(
                    code = "TASKBAR_REPOSITION_DEFERRED",
                    "taskbar overlay retry deferred"
                );
            }
        }
    });
}

pub fn handle_taskbar_window_destroyed(app: &tauri::AppHandle) {
    try_with_state(&app.state::<Mutex<StatusSurfaceState>>(), |state| {
        state.taskbar.handle_window_destroyed();
    });
}

pub fn handle_taskbar_measurement_window_destroyed(app: &tauri::AppHandle) {
    let deferred_app = app.clone();
    let dispatch = try_with_state_or_defer(
        &app.state::<Mutex<StatusSurfaceState>>(),
        |state| state.taskbar.handle_measurement_window_destroyed(),
        move || {
            tauri::async_runtime::spawn(async move {
                let state = deferred_app.state::<Mutex<StatusSurfaceState>>();
                if !reconcile_deferred_measurement_state(
                    &state,
                    || {
                        deferred_app.get_webview_window(
                            crate::taskbar_overlay::window::TASKBAR_MEASUREMENT_WINDOW_LABEL,
                        )
                    },
                    |state, live_measurement| {
                        state
                            .taskbar
                            .reconcile_measurement_window_after_destroy(live_measurement);
                    },
                ) {
                    tracing::debug!(
                        code = "TASKBAR_MEASUREMENT_DESTROY_DEFERRED_FAILED",
                        "taskbar measurement cache cleanup retry failed"
                    );
                }
            });
        },
    );
    if dispatch == StateMutationDispatch::Deferred {
        tracing::debug!(
            code = "TASKBAR_MEASUREMENT_DESTROY_DEFERRED",
            "taskbar measurement cache cleanup deferred"
        );
    }
}

pub fn handle_float_ball_window_destroyed(app: &tauri::AppHandle) {
    try_with_state(&app.state::<Mutex<StatusSurfaceState>>(), |state| {
        state.float_ball.handle_window_destroyed();
    });
}

pub fn handle_float_ball_moved(app: &tauri::AppHandle, position: tauri::PhysicalPosition<i32>) {
    let Some(window) = app.get_webview_window(crate::float_ball::window::FLOAT_BALL_WINDOW_LABEL)
    else {
        return;
    };
    try_with_state(&app.state::<Mutex<StatusSurfaceState>>(), |state| {
        state.float_ball.handle_moved(&window, position);
    });
}

pub fn set_float_ball_expanded(app: &tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let state = app.state::<Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    state.float_ball.set_expanded(app, expanded)
}

fn authorize_taskbar_width(caller_label: &str, taskbar_enabled: bool) -> Result<(), &'static str> {
    if !crate::taskbar_overlay::window::is_measurement_window_label(caller_label) {
        return Err("TASKBAR_MEASUREMENT_UNAUTHORIZED");
    }
    if !taskbar_enabled {
        return Err("TASKBAR_STATUS_DISABLED");
    }
    Ok(())
}

fn authorize_and_apply_taskbar_width(
    caller_label: &str,
    taskbar_enabled: bool,
    apply: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    authorize_taskbar_width(caller_label, taskbar_enabled).map_err(str::to_string)?;
    apply()
}

pub fn set_taskbar_status_width(
    app: &tauri::AppHandle,
    caller_label: &str,
    width: f64,
) -> Result<(), String> {
    let state = app.state::<Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    let taskbar_enabled = state.taskbar.is_enabled();
    authorize_and_apply_taskbar_width(caller_label, taskbar_enabled, || {
        state
            .taskbar
            .set_content_width(app, crate::taskbar_overlay::clamp_logical_width(width))
    })
}

pub fn schedule_taskbar_reposition(app: tauri::AppHandle) {
    schedule_status_reposition(app);
}

pub fn schedule_status_reposition(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let result = app
            .state::<Mutex<StatusSurfaceState>>()
            .lock()
            .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())
            .and_then(|mut state| {
                if state.fullscreen_suspended
                    || crate::shell::fullscreen_guard::is_fullscreen_active()
                {
                    return Ok(());
                }
                let mut first_error = None;
                if let Err(error) = state.taskbar.handle_shell_change(&app) {
                    first_error = Some(error);
                }
                if let Err(error) = state.float_ball.handle_shell_change(&app)
                    && first_error.is_none()
                {
                    first_error = Some(error);
                }
                first_error.map_or(Ok(()), Err)
            });
        if result.is_err() {
            tracing::debug!(
                code = "TASKBAR_REPOSITION_DEFERRED",
                "taskbar overlay reposition deferred"
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn fullscreen_transition_only_changes_on_edges() {
        assert_eq!(
            fullscreen_transition(false, true),
            FullscreenTransition::Suspend
        );
        assert_eq!(
            fullscreen_transition(true, false),
            FullscreenTransition::Resume
        );
        assert_eq!(
            fullscreen_transition(true, true),
            FullscreenTransition::Stable
        );
        assert_eq!(
            fullscreen_transition(false, false),
            FullscreenTransition::Stable
        );
    }

    #[test]
    fn overlay_reassertion_runs_faster_than_shell_blink_window() {
        const { assert!(STATUS_SURFACE_REASSERT_INTERVAL_MS <= 500) };
    }

    #[test]
    fn auxiliary_labels_map_to_permanent_disable_intents() {
        assert_eq!(
            surface_for_window_label("taskbar-status"),
            Some(controller::StatusSurfaceKind::TaskbarStatus)
        );
        assert_eq!(
            surface_for_window_label("float-ball"),
            Some(controller::StatusSurfaceKind::FloatBall)
        );
        assert_eq!(surface_for_window_label("taskbar-status-measure"), None);
        assert_eq!(surface_for_window_label("settings"), None);
    }

    #[test]
    fn disabled_surface_reconciliation_selects_cleanup() {
        assert_eq!(reconciliation_action(false), ReconciliationAction::Cleanup);
        assert_eq!(
            reconciliation_action(true),
            ReconciliationAction::Reposition
        );
    }

    #[test]
    fn taskbar_width_requires_the_enabled_measurement_window() {
        assert_eq!(
            authorize_taskbar_width("taskbar-status-measure", true),
            Ok(())
        );
        assert_eq!(
            authorize_taskbar_width("taskbar-status", true),
            Err("TASKBAR_MEASUREMENT_UNAUTHORIZED")
        );
        assert_eq!(
            authorize_taskbar_width("taskbar-status-measure", false),
            Err("TASKBAR_STATUS_DISABLED")
        );
    }

    #[test]
    fn rejected_taskbar_width_requests_do_not_run_mutation() {
        for (label, enabled, expected_error) in [
            ("taskbar-status", true, "TASKBAR_MEASUREMENT_UNAUTHORIZED"),
            ("taskbar-status-measure", false, "TASKBAR_STATUS_DISABLED"),
        ] {
            let mutation_ran = std::cell::Cell::new(false);
            assert_eq!(
                authorize_and_apply_taskbar_width(label, enabled, || {
                    mutation_ran.set(true);
                    Ok(())
                }),
                Err(expected_error.to_string())
            );
            assert!(!mutation_ran.get());
        }
    }

    #[test]
    fn overlay_failure_is_non_fatal_to_startup() {
        let mut attempted = false;
        run_non_fatal(|| {
            attempted = true;
            Err("TASKBAR_UNAVAILABLE".to_string())
        });
        assert!(attempted);
    }

    #[test]
    fn deferred_old_destroy_preserves_replacement_cache_from_late_registry_lookup() {
        #[derive(Debug)]
        struct FakeMeasurementState {
            enabled: bool,
            cache: Option<&'static str>,
            reconciliations: usize,
        }

        let state = Arc::new(Mutex::new(FakeMeasurementState {
            enabled: true,
            cache: Some("old helper"),
            reconciliations: 0,
        }));
        let registry = Arc::new(Mutex::new(Some("old helper")));
        let mut guard = state.lock().expect("surface manager lock");
        let deferred_state = Arc::clone(&state);
        let deferred_registry = Arc::clone(&registry);
        let (started_tx, started_rx) = mpsc::channel();
        let (lookup_tx, lookup_rx) = mpsc::channel();
        let (lookup_gate_tx, lookup_gate_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();

        let dispatch = try_with_state_or_defer(
            state.as_ref(),
            |state| state.cache = None,
            move || {
                std::thread::spawn(move || {
                    started_tx.send(()).expect("report deferred start");
                    let reconciled = reconcile_deferred_measurement_state(
                        deferred_state.as_ref(),
                        || {
                            lookup_tx.send(()).expect("report registry lookup");
                            lookup_gate_rx.recv().expect("allow registry lookup");
                            *deferred_registry.lock().expect("registry lock")
                        },
                        |state, live| {
                            state.cache = live;
                            state.reconciliations += 1;
                        },
                    );
                    completed_tx
                        .send(reconciled)
                        .expect("report deferred reconciliation");
                });
            },
        );

        assert_eq!(dispatch, StateMutationDispatch::Deferred);
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("deferred worker start");
        let lookup_was_early = lookup_rx.recv_timeout(Duration::from_millis(100)).is_ok();

        guard.cache = Some("new helper");
        *registry.lock().expect("registry lock") = Some("new helper");
        assert!(guard.enabled);
        drop(guard);
        lookup_gate_tx.send(()).expect("release registry lookup");
        assert!(
            completed_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("deferred reconciliation completion")
        );

        let state = state.lock().expect("final state lock");
        assert!(!lookup_was_early);
        assert_eq!(state.cache, Some("new helper"));
        assert_eq!(state.reconciliations, 1);
        assert!(state.enabled);
    }

    #[test]
    fn deferred_destroy_clears_cache_when_registry_has_no_live_helper() {
        #[derive(Debug)]
        struct FakeMeasurementState {
            enabled: bool,
            cache: Option<&'static str>,
        }

        let state = Arc::new(Mutex::new(FakeMeasurementState {
            enabled: true,
            cache: Some("stale helper"),
        }));
        let guard = state.lock().expect("surface manager lock");
        let deferred_state = Arc::clone(&state);
        let (completed_tx, completed_rx) = mpsc::channel();

        let dispatch = try_with_state_or_defer(
            state.as_ref(),
            |state| state.cache = None,
            move || {
                std::thread::spawn(move || {
                    let reconciled = reconcile_deferred_measurement_state(
                        deferred_state.as_ref(),
                        || None,
                        |state, live| state.cache = live,
                    );
                    completed_tx
                        .send(reconciled)
                        .expect("report deferred reconciliation");
                });
            },
        );

        assert_eq!(dispatch, StateMutationDispatch::Deferred);
        assert_eq!(guard.cache, Some("stale helper"));
        drop(guard);
        assert!(
            completed_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("deferred reconciliation completion")
        );

        let state = state.lock().expect("final state lock");
        assert_eq!(state.cache, None);
        assert!(state.enabled);
    }
}
