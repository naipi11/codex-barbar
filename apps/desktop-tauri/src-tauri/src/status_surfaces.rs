use std::sync::Mutex;
use std::time::{Duration, Instant};

use codexbar::storage::AppSettings;
use tauri::Manager;

use crate::shell::fullscreen_guard::ForegroundClass;
use crate::shell::surface_lifecycle_trace::{
    SurfaceLifecycleSnapshot, SurfaceSuspensionReason, recent_global, record_global,
};

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
pub(crate) enum ReconcileCause {
    ForegroundChanged,
    ShellChanged,
    PeriodicFallback,
    FullscreenTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReconcileAction {
    KeepVisible,
    Restore,
    Suspend,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SurfacePhase {
    foreground: ForegroundClass,
    hide_in_fullscreen: bool,
    suspension_reason: SurfaceSuspensionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SurfacePhaseOutcome {
    phase: SurfacePhase,
    action: ReconcileAction,
}

fn reduce_surface_phase(current: SurfacePhase, foreground: ForegroundClass) -> SurfacePhaseOutcome {
    let next = SurfacePhase {
        foreground,
        hide_in_fullscreen: current.hide_in_fullscreen,
        suspension_reason: if foreground == ForegroundClass::RealFullscreen
            && current.hide_in_fullscreen
        {
            SurfaceSuspensionReason::Fullscreen
        } else {
            SurfaceSuspensionReason::None
        },
    };
    let action = if next.suspension_reason == SurfaceSuspensionReason::Fullscreen {
        ReconcileAction::Suspend
    } else if foreground == ForegroundClass::ShellTransient {
        // The desktop shell can remain foreground after Start, Explorer,
        // taskbar, or Win+D. Re-run the non-activating restore path so a
        // shell-owned z-order change cannot make enabled surfaces disappear.
        ReconcileAction::Restore
    } else if current.suspension_reason == SurfaceSuspensionReason::Fullscreen
        || current.foreground == ForegroundClass::ShellTransient
            && foreground == ForegroundClass::Normal
        || current.foreground != ForegroundClass::Normal && foreground == ForegroundClass::Normal
    {
        ReconcileAction::Restore
    } else {
        ReconcileAction::KeepVisible
    };
    SurfacePhaseOutcome {
        phase: next,
        action,
    }
}

#[cfg(test)]
fn reconcile_action(hide_in_fullscreen: bool, foreground: ForegroundClass) -> ReconcileAction {
    reduce_surface_phase(
        SurfacePhase {
            hide_in_fullscreen,
            ..SurfacePhase::default()
        },
        foreground,
    )
    .action
}

#[cfg(test)]
fn should_hide_for_fullscreen(settings: &AppSettings, is_fullscreen: bool) -> bool {
    is_fullscreen && settings.taskbar_tray.hide_status_surfaces_in_fullscreen
}

fn record_surface_snapshots(state: &StatusSurfaceState, foreground_class: ForegroundClass) {
    let reason = state.phase.suspension_reason;
    record_global(state.taskbar.observe_lifecycle(foreground_class, reason));
    record_global(state.float_ball.observe_lifecycle(foreground_class, reason));
}

pub fn get_status_surface_diagnostics() -> Vec<SurfaceLifecycleSnapshot> {
    recent_global(64)
}

pub fn schedule_foreground_reconcile(app: tauri::AppHandle, foreground: ForegroundClass) {
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = reconcile_surfaces(&app_for_task, foreground, ReconcileCause::ForegroundChanged);
    });
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
    phase: SurfacePhase,
}

fn restore_enabled_surfaces(
    app: &tauri::AppHandle,
    state: &mut StatusSurfaceState,
) -> Result<(), String> {
    let mut first_error = None;
    if let Err(error) = state.taskbar.restore_after_shell(app) {
        first_error = Some(error);
    }
    if let Err(error) = state.float_ball.restore_after_shell(app)
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    first_error.map_or(Ok(()), Err)
}

fn keep_enabled_surfaces_visible(state: &mut StatusSurfaceState) -> Result<(), String> {
    let mut first_error = None;
    if let Err(error) = state.taskbar.reassert_topmost() {
        first_error = Some(error);
    }
    if let Err(error) = state.float_ball.reassert_topmost()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    first_error.map_or(Ok(()), Err)
}

fn suspend_enabled_surfaces(state: &mut StatusSurfaceState) -> Result<(), String> {
    let mut first_error = None;
    if let Err(error) = state.taskbar.hide_for_fullscreen() {
        first_error = Some(error);
    }
    if let Err(error) = state.float_ball.hide_for_fullscreen()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    first_error.map_or(Ok(()), Err)
}

pub fn reconcile_surfaces(
    app: &tauri::AppHandle,
    foreground: ForegroundClass,
    cause: ReconcileCause,
) -> Result<(), String> {
    let hide_in_fullscreen = app
        .state::<Mutex<crate::state::AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.account_service.clone())
        .and_then(|service| service.repositories().settings.load().ok())
        .map(|settings| settings.taskbar_tray.hide_status_surfaces_in_fullscreen)
        .unwrap_or(true);
    let state = app.state::<Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    state.phase.hide_in_fullscreen = hide_in_fullscreen;
    let outcome = reduce_surface_phase(state.phase, foreground);
    state.phase = outcome.phase;
    let result = match outcome.action {
        ReconcileAction::Suspend => suspend_enabled_surfaces(&mut state),
        ReconcileAction::Restore => restore_enabled_surfaces(app, &mut state),
        ReconcileAction::KeepVisible => {
            if matches!(
                cause,
                ReconcileCause::PeriodicFallback | ReconcileCause::ShellChanged
            ) {
                restore_enabled_surfaces(app, &mut state)
            } else {
                keep_enabled_surfaces_visible(&mut state)
            }
        }
    };
    record_surface_snapshots(&state, foreground);
    result
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
    let foreground = crate::shell::fullscreen_guard::classify_current_foreground();
    state.phase.hide_in_fullscreen = settings.taskbar_tray.hide_status_surfaces_in_fullscreen;
    let outcome = reduce_surface_phase(state.phase, foreground);
    state.phase = outcome.phase;
    if outcome.action == ReconcileAction::Suspend {
        if let Err(error) = state.taskbar.hide_for_fullscreen() {
            first_error.get_or_insert(error);
        }
        if let Err(error) = state.float_ball.hide_for_fullscreen() {
            first_error.get_or_insert(error);
        }
    }
    record_surface_snapshots(&state, foreground);
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
            let foreground = crate::shell::fullscreen_guard::classify_current_foreground();
            let cause = if last_reconcile.elapsed() >= STATUS_SURFACE_RECONCILE_INTERVAL {
                last_reconcile = Instant::now();
                ReconcileCause::PeriodicFallback
            } else {
                ReconcileCause::FullscreenTransition
            };
            if reconcile_surfaces(&app, foreground, cause).is_err() {
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
        let foreground = crate::shell::fullscreen_guard::classify_current_foreground();
        let _ = reconcile_surfaces(&app, foreground, ReconcileCause::ShellChanged);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    #[test]
    fn diagnostics_command_returns_recorded_snapshots() {
        record_global(crate::shell::surface_lifecycle_trace::event(11));
        record_global(crate::shell::surface_lifecycle_trace::event(12));
        let recent = get_status_surface_diagnostics();
        assert!(recent.iter().any(|snapshot| snapshot.observed_at_ms == 12));
        let encoded = serde_json::to_value(&recent).unwrap().to_string();
        assert!(!encoded.contains("title"));
        assert!(!encoded.contains("http"));
    }

    #[test]
    fn shell_transient_preserves_enabled_intent_and_geometry() {
        let current = SurfacePhase {
            foreground: ForegroundClass::Normal,
            hide_in_fullscreen: true,
            suspension_reason: SurfaceSuspensionReason::None,
        };
        let next = reduce_surface_phase(current, ForegroundClass::ShellTransient);
        assert_eq!(next.phase.suspension_reason, SurfaceSuspensionReason::None);
        assert_eq!(next.action, ReconcileAction::Restore);
    }

    #[test]
    fn long_lived_shell_transient_reasserts_surfaces_without_activation() {
        let current = SurfacePhase {
            foreground: ForegroundClass::ShellTransient,
            hide_in_fullscreen: true,
            suspension_reason: SurfaceSuspensionReason::None,
        };
        assert_eq!(
            reduce_surface_phase(current, ForegroundClass::ShellTransient).action,
            ReconcileAction::Restore
        );
    }

    #[test]
    fn normal_after_shell_transient_requests_restore_without_click() {
        let current = SurfacePhase {
            foreground: ForegroundClass::ShellTransient,
            hide_in_fullscreen: true,
            suspension_reason: SurfaceSuspensionReason::None,
        };
        assert_eq!(
            reduce_surface_phase(current, ForegroundClass::Normal).action,
            ReconcileAction::Restore
        );
    }

    #[test]
    fn real_fullscreen_hides_only_when_preference_is_enabled() {
        assert_eq!(
            reconcile_action(true, ForegroundClass::RealFullscreen),
            ReconcileAction::Suspend
        );
        assert_eq!(
            reconcile_action(false, ForegroundClass::RealFullscreen),
            ReconcileAction::KeepVisible
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
    fn fullscreen_hide_respects_the_user_preference() {
        let mut settings = AppSettings::default();
        settings.taskbar_tray.hide_status_surfaces_in_fullscreen = true;
        assert!(should_hide_for_fullscreen(&settings, true));
        assert!(!should_hide_for_fullscreen(&settings, false));

        settings.taskbar_tray.hide_status_surfaces_in_fullscreen = false;
        assert!(!should_hide_for_fullscreen(&settings, true));
        assert!(!should_hide_for_fullscreen(&settings, false));
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
