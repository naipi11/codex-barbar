use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use codexbar::storage::{AppSettings, SettingsPatch, SettingsRepository};

use super::StatusSurfaceState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusSurfaceKind {
    TaskbarStatus,
    FloatBall,
}

pub struct StatusSurfaceController;

impl StatusSurfaceController {
    pub const fn supports(surface: StatusSurfaceKind) -> bool {
        match surface {
            StatusSurfaceKind::TaskbarStatus => cfg!(windows),
            StatusSurfaceKind::FloatBall => cfg!(any(windows, target_os = "linux")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusSurfaceFeedback {
    #[default]
    None,
    CloseFailed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusSurfaceFeedbackState {
    taskbar_status: StatusSurfaceFeedback,
    float_ball: StatusSurfaceFeedback,
}

impl StatusSurfaceFeedbackState {
    pub fn close_failed(self, surface: StatusSurfaceKind) -> bool {
        match surface {
            StatusSurfaceKind::TaskbarStatus => {
                self.taskbar_status == StatusSurfaceFeedback::CloseFailed
            }
            StatusSurfaceKind::FloatBall => self.float_ball == StatusSurfaceFeedback::CloseFailed,
        }
    }

    pub fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool) {
        let value = if close_failed {
            StatusSurfaceFeedback::CloseFailed
        } else {
            StatusSurfaceFeedback::None
        };
        match surface {
            StatusSurfaceKind::TaskbarStatus => self.taskbar_status = value,
            StatusSurfaceKind::FloatBall => self.float_ball = value,
        }
    }
}

pub(crate) fn feedback_snapshot(
    state: &StatusSurfaceState,
) -> crate::commands::StatusSurfaceFeedbackDto {
    crate::commands::StatusSurfaceFeedbackDto {
        taskbar_status_close_failed: state
            .feedback
            .close_failed(StatusSurfaceKind::TaskbarStatus),
        float_ball_close_failed: state.feedback.close_failed(StatusSurfaceKind::FloatBall),
    }
}

fn emit_feedback_with(
    payload: &crate::commands::StatusSurfaceFeedbackChangedDto,
    emit: impl FnOnce(&crate::commands::StatusSurfaceFeedbackChangedDto) -> Result<(), ()>,
) {
    if emit(payload).is_err() {
        tracing::warn!(
            code = "STATUS_SURFACE_FEEDBACK_EVENT_FAILED",
            "status surface feedback event was not delivered"
        );
    }
}

fn complete_transition_with_feedback<T, U>(
    state: &std::sync::Mutex<StatusSurfaceState>,
    surface: StatusSurfaceKind,
    enabled: bool,
    result: Result<T, String>,
    emit: impl FnOnce(&crate::commands::StatusSurfaceFeedbackChangedDto) -> Result<(), ()>,
    complete_success: impl FnOnce(T) -> U,
) -> Result<U, String> {
    let fallback_close_failed = !enabled && result.is_err();
    match state.lock() {
        Ok(state) => {
            let payload = crate::commands::StatusSurfaceFeedbackChangedDto {
                surface,
                close_failed: state.feedback.close_failed(surface),
            };
            emit_feedback_with(&payload, emit);
            drop(state);
        }
        Err(_) => {
            tracing::warn!(
                code = "STATUS_SURFACE_FEEDBACK_SNAPSHOT_UNAVAILABLE",
                "status surface feedback snapshot was unavailable"
            );
            let payload = crate::commands::StatusSurfaceFeedbackChangedDto {
                surface,
                close_failed: fallback_close_failed,
            };
            emit_feedback_with(&payload, emit);
        }
    }
    result.map(complete_success)
}

fn lookup_repository_with_feedback<T>(
    surface: StatusSurfaceKind,
    enabled: bool,
    mut set_feedback: impl FnMut(StatusSurfaceKind, bool) -> Result<(), String>,
    lookup: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    set_feedback(surface, false)?;
    match lookup() {
        Ok(repository) => Ok(repository),
        Err(error) => {
            if !enabled {
                let _ = set_feedback(surface, true);
            }
            Err(error)
        }
    }
}

pub trait SurfaceRuntime {
    fn apply(&mut self, surface: StatusSurfaceKind, enabled: bool) -> Result<(), String>;
    fn force_enabled(&mut self, surface: StatusSurfaceKind, enabled: bool);
    fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool);
}

pub trait SurfaceSettingsStore {
    fn load(&self) -> Result<AppSettings, String>;
    fn write_enabled(
        &self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<AppSettings, String>;
}

impl StatusSurfaceKind {
    fn enabled_in(self, settings: &AppSettings) -> bool {
        match self {
            Self::TaskbarStatus => settings.taskbar_status_enabled,
            Self::FloatBall => settings.float_ball_enabled,
        }
    }

    fn patch(self, enabled: bool) -> SettingsPatch {
        match self {
            Self::TaskbarStatus => SettingsPatch {
                taskbar_status_enabled: Some(enabled),
                ..SettingsPatch::default()
            },
            Self::FloatBall => SettingsPatch {
                float_ball_enabled: Some(enabled),
                ..SettingsPatch::default()
            },
        }
    }
}

impl SurfaceSettingsStore for SettingsRepository {
    fn load(&self) -> Result<AppSettings, String> {
        SettingsRepository::load(self)
            .map_err(|_| "STATUS_SURFACE_SETTINGS_LOAD_FAILED".to_string())
    }

    fn write_enabled(
        &self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<AppSettings, String> {
        self.update(surface.patch(enabled))
            .map_err(|_| "STATUS_SURFACE_SETTINGS_SAVE_FAILED".to_string())
    }
}

pub fn transition<R, S>(
    runtime: &mut R,
    store: &S,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettings, String>
where
    R: SurfaceRuntime,
    S: SurfaceSettingsStore,
{
    runtime.set_close_failed(surface, false);
    let previous = store.load()?;
    let previous_enabled = surface.enabled_in(&previous);
    if let Err(error) = runtime.apply(surface, enabled) {
        if surface == StatusSurfaceKind::TaskbarStatus
            && error == crate::taskbar_overlay::TASKBAR_STATUS_UNSUPPORTED_PLATFORM
        {
            return Err(error);
        }
        if !enabled {
            runtime.set_close_failed(surface, true);
        }
        if previous_enabled == enabled || !enabled {
            return Err(error);
        }
        if runtime.apply(surface, previous_enabled).is_err() {
            runtime.force_enabled(surface, previous_enabled);
            return Err("STATUS_SURFACE_ROLLBACK_FAILED".to_string());
        }
        return Err(error);
    }

    if previous_enabled == enabled {
        return Ok(previous);
    }

    match store.write_enabled(surface, enabled) {
        Ok(settings) => Ok(settings),
        Err(error) => {
            if !enabled {
                runtime.set_close_failed(surface, true);
            }
            if runtime.apply(surface, previous_enabled).is_err() {
                runtime.force_enabled(surface, previous_enabled);
                return Err("STATUS_SURFACE_ROLLBACK_FAILED".to_string());
            }
            Err(error)
        }
    }
}

struct TauriSurfaceRuntime<'a> {
    app: &'a tauri::AppHandle,
    state: &'a mut StatusSurfaceState,
}

impl SurfaceRuntime for TauriSurfaceRuntime<'_> {
    fn apply(&mut self, surface: StatusSurfaceKind, enabled: bool) -> Result<(), String> {
        match surface {
            StatusSurfaceKind::TaskbarStatus => self.state.taskbar.apply_enabled(self.app, enabled),
            StatusSurfaceKind::FloatBall => self.state.float_ball.apply_enabled(self.app, enabled),
        }
    }

    fn force_enabled(&mut self, surface: StatusSurfaceKind, enabled: bool) {
        match surface {
            StatusSurfaceKind::TaskbarStatus => self.state.taskbar.force_enabled(enabled),
            StatusSurfaceKind::FloatBall => self.state.float_ball.force_enabled(enabled),
        }
    }

    fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool) {
        self.state.feedback.set_close_failed(surface, close_failed);
    }
}

fn settings_repository(app: &tauri::AppHandle) -> Result<SettingsRepository, String> {
    app.state::<std::sync::Mutex<crate::state::AppState>>()
        .lock()
        .map_err(|_| "STATUS_SURFACE_SETTINGS_UNAVAILABLE".to_string())?
        .account_service
        .as_ref()
        .map(|service| service.repositories().settings.clone())
        .ok_or_else(|| "STATUS_SURFACE_SETTINGS_UNAVAILABLE".to_string())
}

fn set_feedback(
    app: &tauri::AppHandle,
    surface: StatusSurfaceKind,
    close_failed: bool,
) -> Result<(), String> {
    let state = app.state::<std::sync::Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    state.feedback.set_close_failed(surface, close_failed);
    Ok(())
}

pub fn set_enabled_with_repository(
    app: &tauri::AppHandle,
    repository: &SettingsRepository,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettings, String> {
    let state = app.state::<std::sync::Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    let mut runtime = TauriSurfaceRuntime {
        app,
        state: &mut state,
    };
    transition(&mut runtime, repository, surface, enabled)
}

pub fn set_enabled_and_emit(
    app: &tauri::AppHandle,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<crate::commands::AppSettingsDto, String> {
    let repository = lookup_repository_with_feedback(
        surface,
        enabled,
        |surface, close_failed| set_feedback(app, surface, close_failed),
        || settings_repository(app),
    );
    let transition_result = repository
        .and_then(|repository| set_enabled_with_repository(app, &repository, surface, enabled));
    let state = app.state::<std::sync::Mutex<StatusSurfaceState>>();
    complete_transition_with_feedback(
        state.inner(),
        surface,
        enabled,
        transition_result,
        |payload| {
            app.emit(crate::events::STATUS_SURFACE_FEEDBACK_CHANGED, payload)
                .map_err(|_| ())
        },
        |settings| {
            let dto = crate::commands::AppSettingsDto::from_settings(&settings);
            if app.emit(crate::events::SETTINGS_CHANGED, &dto).is_err() {
                tracing::warn!(
                    code = "STATUS_SURFACE_SETTINGS_EVENT_FAILED",
                    "status surface settings event was not delivered"
                );
            }
            dto
        },
    )
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use codexbar::storage::AppSettings;

    use super::{
        StatusSurfaceFeedbackState, StatusSurfaceKind, SurfaceRuntime, SurfaceSettingsStore,
        complete_transition_with_feedback, feedback_snapshot, lookup_repository_with_feedback,
        transition,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RuntimeAction {
        Feedback(StatusSurfaceKind, bool),
        Apply(StatusSurfaceKind, bool),
        Force(StatusSurfaceKind, bool),
    }

    #[derive(Default)]
    struct FakeRuntime {
        actions: Vec<RuntimeAction>,
        feedback: StatusSurfaceFeedbackState,
        fail_on: Vec<bool>,
        failure_error: Option<&'static str>,
    }

    impl FakeRuntime {
        fn enabled() -> Self {
            Self::default()
        }

        fn failing_on(mut self, enabled: bool) -> Self {
            self.fail_on.push(enabled);
            self
        }

        fn failing_on_both(self) -> Self {
            self.failing_on(true).failing_on(false)
        }

        fn failing_with(mut self, enabled: bool, error: &'static str) -> Self {
            self.fail_on.push(enabled);
            self.failure_error = Some(error);
            self
        }

        fn actions(&self) -> &[RuntimeAction] {
            &self.actions
        }
    }

    impl SurfaceRuntime for FakeRuntime {
        fn apply(&mut self, surface: StatusSurfaceKind, enabled: bool) -> Result<(), String> {
            self.actions.push(RuntimeAction::Apply(surface, enabled));
            if self.fail_on.contains(&enabled) {
                Err(self
                    .failure_error
                    .unwrap_or("STATUS_SURFACE_WINDOW_CLOSE_FAILED")
                    .to_string())
            } else {
                Ok(())
            }
        }

        fn force_enabled(&mut self, surface: StatusSurfaceKind, enabled: bool) {
            self.actions.push(RuntimeAction::Force(surface, enabled));
        }

        fn set_close_failed(&mut self, surface: StatusSurfaceKind, close_failed: bool) {
            self.feedback.set_close_failed(surface, close_failed);
            self.actions
                .push(RuntimeAction::Feedback(surface, close_failed));
        }
    }

    struct FakeStore {
        saved: RefCell<AppSettings>,
        fail_save: bool,
        save_count: Cell<usize>,
    }

    impl FakeStore {
        fn with_settings(saved: AppSettings) -> Self {
            Self {
                saved: RefCell::new(saved),
                fail_save: false,
                save_count: Cell::new(0),
            }
        }

        fn failing_save(mut self) -> Self {
            self.fail_save = true;
            self
        }

        fn saved(&self) -> AppSettings {
            self.saved.borrow().clone()
        }

        fn save_count(&self) -> usize {
            self.save_count.get()
        }
    }

    impl SurfaceSettingsStore for FakeStore {
        fn load(&self) -> Result<AppSettings, String> {
            Ok(self.saved())
        }

        fn write_enabled(
            &self,
            surface: StatusSurfaceKind,
            enabled: bool,
        ) -> Result<AppSettings, String> {
            if self.fail_save {
                return Err("STATUS_SURFACE_SETTINGS_SAVE_FAILED".to_string());
            }
            self.save_count.set(self.save_count.get() + 1);
            let mut next = self.saved();
            match surface {
                StatusSurfaceKind::TaskbarStatus => next.taskbar_status_enabled = enabled,
                StatusSurfaceKind::FloatBall => next.float_ball_enabled = enabled,
            }
            *self.saved.borrow_mut() = next.clone();
            Ok(next)
        }
    }

    fn settings(taskbar: bool, float_ball: bool) -> AppSettings {
        AppSettings {
            taskbar_status_enabled: taskbar,
            float_ball_enabled: float_ball,
            ..AppSettings::default()
        }
    }

    fn poisoned_status_state() -> Mutex<super::StatusSurfaceState> {
        let state = Mutex::new(super::StatusSurfaceState::default());
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.lock().unwrap();
            panic!("poison status surface state");
        }));
        assert!(state.is_poisoned());
        state
    }

    #[test]
    fn feedback_is_process_local_and_surface_isolated() {
        let mut feedback = StatusSurfaceFeedbackState::default();
        assert!(!feedback.close_failed(StatusSurfaceKind::TaskbarStatus));
        assert!(!feedback.close_failed(StatusSurfaceKind::FloatBall));

        feedback.set_close_failed(StatusSurfaceKind::TaskbarStatus, true);
        assert!(feedback.close_failed(StatusSurfaceKind::TaskbarStatus));
        assert!(!feedback.close_failed(StatusSurfaceKind::FloatBall));
    }

    #[test]
    fn feedback_snapshot_maps_native_surfaces_independently() {
        let mut state = super::StatusSurfaceState::default();
        state
            .feedback
            .set_close_failed(StatusSurfaceKind::TaskbarStatus, true);

        let snapshot = feedback_snapshot(&state);

        assert!(snapshot.taskbar_status_close_failed);
        assert!(!snapshot.float_ball_close_failed);

        state
            .feedback
            .set_close_failed(StatusSurfaceKind::TaskbarStatus, false);
        state
            .feedback
            .set_close_failed(StatusSurfaceKind::FloatBall, true);
        let snapshot = feedback_snapshot(&state);
        assert!(!snapshot.taskbar_status_close_failed);
        assert!(snapshot.float_ball_close_failed);
    }

    #[test]
    fn disable_repository_lookup_failure_clears_then_latches_feedback() {
        let actions = RefCell::new(Vec::new());
        let result: Result<(), String> = lookup_repository_with_feedback(
            StatusSurfaceKind::TaskbarStatus,
            false,
            |surface, close_failed| {
                actions
                    .borrow_mut()
                    .push(("feedback", surface, close_failed));
                Ok(())
            },
            || {
                actions
                    .borrow_mut()
                    .push(("lookup", StatusSurfaceKind::TaskbarStatus, false));
                Err("STATUS_SURFACE_SETTINGS_UNAVAILABLE".to_string())
            },
        );

        assert_eq!(result.unwrap_err(), "STATUS_SURFACE_SETTINGS_UNAVAILABLE");
        assert_eq!(
            actions.into_inner(),
            [
                ("feedback", StatusSurfaceKind::TaskbarStatus, false),
                ("lookup", StatusSurfaceKind::TaskbarStatus, false),
                ("feedback", StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
    }

    #[test]
    fn feedback_event_failure_does_not_replace_transition_result() {
        let state = Mutex::new(super::StatusSurfaceState::default());
        state
            .lock()
            .unwrap()
            .feedback
            .set_close_failed(StatusSurfaceKind::TaskbarStatus, true);
        let original = "STATUS_SURFACE_WINDOW_CLOSE_FAILED".to_string();
        let result: Result<(), String> = Err(original.clone());

        let result = complete_transition_with_feedback(
            &state,
            StatusSurfaceKind::TaskbarStatus,
            false,
            result,
            |_| Err(()),
            |value| value,
        );

        assert_eq!(result.unwrap_err(), original);
    }

    #[test]
    fn unavailable_snapshot_and_failed_emitter_preserve_success_and_settings_completion() {
        let state = poisoned_status_state();
        let emitted = Cell::new(false);
        let completed = Cell::new(false);
        let close_failed = Cell::new(true);
        let original = settings(false, true);

        let returned = complete_transition_with_feedback(
            &state,
            StatusSurfaceKind::TaskbarStatus,
            false,
            Ok(original.clone()),
            |payload| {
                emitted.set(true);
                close_failed.set(payload.close_failed);
                Err(())
            },
            |settings| {
                completed.set(true);
                settings
            },
        )
        .unwrap();

        assert!(emitted.get());
        assert!(completed.get());
        assert!(!close_failed.get());
        assert_eq!(
            (returned.taskbar_status_enabled, returned.float_ball_enabled,),
            (original.taskbar_status_enabled, original.float_ball_enabled,)
        );
    }

    #[test]
    fn unavailable_snapshot_preserves_error_and_emits_disable_failure_fallback() {
        let state = poisoned_status_state();
        let emitted = Cell::new(None);
        let original = "STATUS_SURFACE_WINDOW_CLOSE_FAILED".to_string();
        let result: Result<(), String> = Err(original.clone());

        let result = complete_transition_with_feedback(
            &state,
            StatusSurfaceKind::FloatBall,
            false,
            result,
            |payload| {
                emitted.set(Some((payload.surface, payload.close_failed)));
                Ok(())
            },
            |_| panic!("failed transitions must not complete settings"),
        );

        assert_eq!(result.unwrap_err(), original);
        assert_eq!(emitted.get(), Some((StatusSurfaceKind::FloatBall, true)));
    }

    #[test]
    fn feedback_emission_holds_state_lock_until_payload_delivery_finishes() {
        let state = Arc::new(Mutex::new(super::StatusSurfaceState::default()));
        state
            .lock()
            .unwrap()
            .feedback
            .set_close_failed(StatusSurfaceKind::TaskbarStatus, true);
        let completion_state = Arc::clone(&state);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (payload_tx, payload_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let completion = std::thread::spawn(move || {
            let result: Result<(), String> = Err("STATUS_SURFACE_WINDOW_CLOSE_FAILED".to_string());
            complete_transition_with_feedback(
                completion_state.as_ref(),
                StatusSurfaceKind::TaskbarStatus,
                false,
                result,
                |payload| {
                    payload_tx.send(payload.close_failed).unwrap();
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                },
                |value| value,
            )
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("feedback emitter start");

        let mutation_state = Arc::clone(&state);
        let (mutated_tx, mutated_rx) = mpsc::channel();
        let mutation = std::thread::spawn(move || {
            mutation_state
                .lock()
                .unwrap()
                .feedback
                .set_close_failed(StatusSurfaceKind::TaskbarStatus, false);
            mutated_tx.send(()).unwrap();
        });

        assert!(mutated_rx.recv_timeout(Duration::from_millis(100)).is_err());
        release_tx.send(()).unwrap();
        assert_eq!(
            completion.join().unwrap().unwrap_err(),
            "STATUS_SURFACE_WINDOW_CLOSE_FAILED"
        );
        mutated_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("mutation after feedback emission");
        mutation.join().unwrap();

        assert!(payload_rx.recv().unwrap());
        assert!(
            !state
                .lock()
                .unwrap()
                .feedback
                .close_failed(StatusSurfaceKind::TaskbarStatus)
        );
    }

    #[test]
    fn completed_transition_payloads_have_only_frozen_feedback_fields() {
        for (result, close_failed) in [(Ok(()), false), (Err("stable".to_string()), true)] {
            let state = Mutex::new(super::StatusSurfaceState::default());
            state
                .lock()
                .unwrap()
                .feedback
                .set_close_failed(StatusSurfaceKind::FloatBall, close_failed);
            let encoded = RefCell::new(None);

            let _ = complete_transition_with_feedback(
                &state,
                StatusSurfaceKind::FloatBall,
                !close_failed,
                result,
                |payload| {
                    encoded.replace(Some(serde_json::to_value(payload).unwrap()));
                    Ok(())
                },
                |value| value,
            );

            let encoded = encoded.into_inner().unwrap();
            assert_eq!(
                encoded
                    .as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
                ["closeFailed", "surface"]
            );
            assert_eq!(encoded["surface"], "floatBall");
            assert_eq!(encoded["closeFailed"], close_failed);
        }
    }

    #[test]
    fn runtime_failure_does_not_persist_false() {
        let mut runtime = FakeRuntime::enabled().failing_on(false);
        let store = FakeStore::with_settings(settings(true, false));
        let error = transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap_err();
        assert_eq!(error, "STATUS_SURFACE_WINDOW_CLOSE_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
        assert!(store.saved().taskbar_status_enabled);
    }

    #[test]
    fn runtime_enable_failure_rolls_back_to_previous_state_without_persisting() {
        let mut runtime = FakeRuntime::enabled().failing_on(true);
        let store = FakeStore::with_settings(settings(false, false));
        let error =
            transition(&mut runtime, &store, StatusSurfaceKind::TaskbarStatus, true).unwrap_err();
        assert_eq!(error, "STATUS_SURFACE_WINDOW_CLOSE_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
        assert!(!store.saved().taskbar_status_enabled);
    }

    #[test]
    fn unsupported_taskbar_transition_propagates_without_rollback_or_persistence() {
        let mut runtime = FakeRuntime::enabled()
            .failing_with(
                true,
                crate::taskbar_overlay::TASKBAR_STATUS_UNSUPPORTED_PLATFORM,
            )
            .failing_on(false);
        let store = FakeStore::with_settings(settings(false, true));

        let error =
            transition(&mut runtime, &store, StatusSurfaceKind::TaskbarStatus, true).unwrap_err();

        assert_eq!(error, "TASKBAR_STATUS_UNSUPPORTED_PLATFORM");
        assert_eq!(store.save_count(), 0);
        assert!(!store.saved().taskbar_status_enabled);
        assert!(store.saved().float_ball_enabled);
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
    }

    #[test]
    fn failed_enable_rollback_failure_forces_disabled_state() {
        let mut runtime = FakeRuntime::enabled().failing_on_both();
        let store = FakeStore::with_settings(settings(false, false));
        let error =
            transition(&mut runtime, &store, StatusSurfaceKind::TaskbarStatus, true).unwrap_err();
        assert_eq!(error, "STATUS_SURFACE_ROLLBACK_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Force(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
        assert!(!store.saved().taskbar_status_enabled);
    }

    #[test]
    fn successful_disable_clears_feedback_and_does_not_relatch() {
        let mut runtime = FakeRuntime::enabled();
        let store = FakeStore::with_settings(settings(true, false));
        let saved = transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap();

        assert!(!saved.taskbar_status_enabled);
        assert_eq!(store.save_count(), 1);
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
    }

    #[test]
    fn persistence_failure_latches_close_feedback_before_runtime_rollback() {
        let mut runtime = FakeRuntime::enabled();
        let store = FakeStore::with_settings(settings(true, false)).failing_save();
        let error = transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap_err();

        assert_eq!(error, "STATUS_SURFACE_SETTINGS_SAVE_FAILED");
        assert!(store.saved().taskbar_status_enabled);
        assert_eq!(store.save_count(), 0);
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
    }

    #[test]
    fn persistence_rollback_failure_forces_previous_state() {
        let mut runtime = FakeRuntime::enabled().failing_on(true);
        let store = FakeStore::with_settings(settings(true, false)).failing_save();
        let error = transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap_err();
        assert_eq!(error, "STATUS_SURFACE_ROLLBACK_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Force(StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
        assert!(store.saved().taskbar_status_enabled);
    }

    #[test]
    fn enable_persistence_rollback_failure_forces_previous_disabled_state() {
        let mut runtime = FakeRuntime::enabled().failing_on(false);
        let store = FakeStore::with_settings(settings(false, false)).failing_save();
        let error =
            transition(&mut runtime, &store, StatusSurfaceKind::TaskbarStatus, true).unwrap_err();

        assert_eq!(error, "STATUS_SURFACE_ROLLBACK_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, true),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Force(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
        assert!(!store.saved().taskbar_status_enabled);
    }

    #[test]
    fn already_persisted_false_reconciles_runtime_without_rewriting() {
        let mut runtime = FakeRuntime::enabled();
        let store = FakeStore::with_settings(settings(false, false));
        transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap();
        assert_eq!(store.save_count(), 0);
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
    }

    #[test]
    fn same_state_runtime_failure_returns_original_without_duplicate_transition() {
        let mut runtime = FakeRuntime::enabled().failing_on(false);
        let store = FakeStore::with_settings(settings(false, false));
        let error = transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap_err();
        assert_eq!(error, "STATUS_SURFACE_WINDOW_CLOSE_FAILED");
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, true),
            ]
        );
        assert!(!store.saved().taskbar_status_enabled);
    }

    #[test]
    fn taskbar_transition_does_not_change_float_ball_feedback() {
        let mut runtime = FakeRuntime::enabled();
        runtime
            .feedback
            .set_close_failed(StatusSurfaceKind::FloatBall, true);
        let store = FakeStore::with_settings(settings(true, false));

        transition(
            &mut runtime,
            &store,
            StatusSurfaceKind::TaskbarStatus,
            false,
        )
        .unwrap();

        assert!(
            !runtime
                .feedback
                .close_failed(StatusSurfaceKind::TaskbarStatus)
        );
        assert!(runtime.feedback.close_failed(StatusSurfaceKind::FloatBall));
        assert_eq!(
            runtime.actions(),
            &[
                RuntimeAction::Feedback(StatusSurfaceKind::TaskbarStatus, false),
                RuntimeAction::Apply(StatusSurfaceKind::TaskbarStatus, false),
            ]
        );
    }
}
