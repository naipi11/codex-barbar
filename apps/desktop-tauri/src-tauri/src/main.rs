#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app_coordinator;
mod auto_refresh;
mod commands;
mod events;
mod float_ball;
mod float_ball_motion;
mod geometry_store;
mod notification_controller;
mod proof_harness;
mod shell;
mod state;
mod status_surfaces;
mod surface;
mod surface_target;
mod taskbar_overlay;
mod tray_bridge;
mod tray_menu;
mod window_positioner;

use std::sync::Arc;
use std::sync::Mutex;

use state::AppState;
#[cfg(test)]
use surface::SurfaceMode;
use tauri::{Emitter, Manager};

fn main() {
    // Fixed internal purge mode used by the NSIS uninstaller. Must run before
    // logging, database, or the single-instance plugin are initialized so the
    // purge process never registers as a running app instance.
    if std::env::args().any(|arg| arg == "--purge-user-data") {
        let result = codexbar::platform::windows::data_cleanup::DataPurger::new()
            .purge_exact_local_app_data_root();
        match result {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("codex-barbar: failed to purge user data: {error}");
                std::process::exit(1);
            }
        }
    }

    codexbar::logging::init(false, false).expect("failed to initialize logging");

    let proof_config = proof_harness::ProofConfig::from_env();
    let is_proof_mode = proof_config.is_some();

    let mut initial_state = AppState::new();
    initial_state.proof_config = proof_config;
    let coordinator = Arc::clone(&initial_state.coordinator);
    coordinator.record(app_coordinator::StartupMilestone::Logging);

    // Phase-2 account bootstrap: open the canonical SQLite database, recover
    // interrupted runtimes, and build the account service when writable.
    if let codexbar::storage::DatabaseBootstrap::Ready(repositories) =
        codexbar::storage::DatabaseBootstrap::open(
            &codexbar::app_paths::AppPaths::discover()
                .map(|paths| paths.database)
                .unwrap_or_else(|_| std::path::PathBuf::from("codex-barbar.db")),
        )
    {
        coordinator.record(app_coordinator::StartupMilestone::Database);
        let app_paths = codexbar::app_paths::AppPaths::discover().ok();
        let vault = Arc::new(codexbar::accounts::vault::CredentialVault::new(
            app_paths
                .as_ref()
                .map(|paths| paths.vault.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("codex-barbar-vault")),
            Arc::new(codexbar::accounts::vault::WindowsDpapiProtector::new()),
        ));
        let runtime_root = app_paths
            .as_ref()
            .map(|paths| paths.runtime.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("codex-barbar-runtime"));
        let runtime_homes =
            codexbar::accounts::runtime_home::RuntimeHomeManager::new(runtime_root.clone());
        let recovery = codexbar::accounts::recovery::AccountRecovery::new(
            codexbar::accounts::runtime_home::RuntimeHomeManager::new(runtime_root),
            Arc::clone(&vault),
        );
        let actor = Arc::new(codexbar::accounts::actor::AccountOperationActor::new(
            Arc::clone(&vault),
        ));
        let identity_cache = Arc::new(codexbar::accounts::identity::AccountIdentityCache::new(
            app_paths
                .as_ref()
                .map(|paths| paths.identity_cache.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("codex-barbar-identity-profiles.json")),
        ));
        let factory: Arc<dyn codexbar::providers::codex::app_server::AppServerFactory> =
            Arc::new(codexbar::providers::codex::app_server::LocalAppServerFactory::default());
        let service = codexbar::accounts::service::AccountProfileService::new(
            repositories,
            Arc::clone(&vault),
            runtime_homes,
            factory,
            recovery,
            actor,
            identity_cache,
        );
        let _ = service.initialize();
        coordinator.record(app_coordinator::StartupMilestone::Recovery);
        coordinator.record(app_coordinator::StartupMilestone::Cache);
        initial_state.account_service = Some(service);
    }

    let notification_paths = codexbar::app_paths::AppPaths::discover().unwrap_or_else(|_| {
        codexbar::app_paths::AppPaths::from_local_app_data(&std::env::temp_dir())
    });
    let notification_controller = notification_controller::NotificationController::new(
        codexbar::notifications::v1::V1NotificationEngine::load(&notification_paths),
        notification_controller::WindowsToastSink,
    );

    tauri::Builder::default()
        .manage(Mutex::new(initial_state))
        .manage(Mutex::new(notification_controller))
        .manage(Mutex::new(status_surfaces::StatusSurfaceState::default()))
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {
            // Second instances only focus/toggle the existing tray flyout and
            // never initialize repositories or open windows.
            let _ = crate::shell::flyout_window::open_or_focus(_app, None);
        }))
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            float_ball_motion::get_float_ball_motion,
            commands::get_settings_snapshot,
            commands::get_notification_capability,
            commands::update_settings,
            commands::apply_menu_preferences,
            commands::send_test_notification,
            commands::set_status_surface_enabled,
            commands::set_float_ball_expanded,
            commands::set_taskbar_status_width,
            commands::get_locale_strings,
            commands::select_profile,
            commands::refresh_selected_profile,
            commands::start_managed_login,
            commands::cancel_managed_login,
            commands::rename_managed_profile,
            commands::remove_managed_profile,
            commands::get_diagnostics_summary,
            commands::export_diagnostics,
            commands::validate_codex_executable,
            commands::check_for_updates,
            commands::open_release_page,
            commands::open_codex_usage_page,
            commands::open_windows_notification_settings,
            commands::open_settings_window,
            commands::close_settings_window,
            commands::dismiss_tray_panel,
            commands::open_tray_panel,
            commands::set_flyout_size,
            commands::set_flyout_interacting,
            commands::get_current_surface_state,
            commands::quit_app,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                shell::dwm::force_dark_caption(&window);
                let _ = window.set_resizable(false);
                window.hide()?;
            }
            // Forward account-service events to the WebView using the fixed
            // V1 event names. Payloads are already redacted DTOs.
            if let Some(service) = app
                .state::<Mutex<AppState>>()
                .lock()
                .ok()
                .and_then(|state| state.account_service.clone())
            {
                let handle = app.handle().clone();
                let settings_repository = service.repositories().settings.clone();
                tauri::async_runtime::spawn(async move {
                    let mut events = service.subscribe();
                    while let Ok(event) = events.recv().await {
                        match event {
                            codexbar::accounts::model::AccountServiceEvent::ProfilesChanged(
                                snapshot,
                            ) => {
                                let identities = service.identity_records().unwrap_or_default();
                                let _ = handle.emit(
                                    crate::events::ACCOUNTS_UPDATED,
                                    commands::AccountsSnapshotDto::from_snapshot(
                                        snapshot,
                                        &identities,
                                    ),
                                );
                            }
                            codexbar::accounts::model::AccountServiceEvent::LoginChanged(
                                status,
                            ) => {
                                let _ = handle.emit(
                                    crate::events::ACCOUNT_LOGIN_UPDATED,
                                    commands::ManagedLoginStateDto::from(&status),
                                );
                            }
                            codexbar::accounts::model::AccountServiceEvent::SelectedProfileChanged {
                                profile_id,
                            } => {
                                let _ = handle.emit(
                                    crate::events::SELECTED_PROFILE_CHANGED,
                                    serde_json::json!({ "profileId": profile_id.to_string() }),
                                );
                            }
                            codexbar::accounts::model::AccountServiceEvent::UsageStateChanged(
                                state,
                            ) => {
                                if !crate::proof_harness::is_proof_mode(&handle)
                                    && state.current_error.is_none()
                                {
                                    let account_marker =
                                        notification_controller::account_marker_for_profile(
                                            &service,
                                            state.profile_id,
                                        );
                                    let controller = handle.state::<Mutex<
                                        notification_controller::NotificationController<
                                            notification_controller::WindowsToastSink,
                                        >,
                                    >>();
                                    if let Ok(mut controller) = controller.lock()
                                        && controller
                                            .observe_usage(
                                                &settings_repository,
                                                state.profile_id,
                                                account_marker.as_deref(),
                                                &state,
                                                None,
                                            )
                                            .is_err()
                                    {
                                        tracing::warn!(
                                            code = "NOTIFICATION_USAGE_DISPATCH_FAILED",
                                            "usage notification was not delivered"
                                        );
                                    }
                                }
                                let _ = handle.emit(
                                    crate::events::PROFILE_USAGE_STATE_CHANGED,
                                    commands::ProfileUsageStateDto::from_state(&state),
                                );
                            }
                            codexbar::accounts::model::AccountServiceEvent::RefreshStateChanged {
                                profile_id,
                                status,
                            } => {
                                let _ = handle.emit(
                                    crate::events::REFRESH_STATE_CHANGED,
                                    serde_json::json!({
                                        "profileId": profile_id.to_string(),
                                        "status": commands::refresh_status_name(status),
                                    }),
                                );
                            }
                            codexbar::accounts::model::AccountServiceEvent::RefreshCompleted {
                                profile_id,
                                success,
                            } => {
                                if !crate::proof_harness::is_proof_mode(&handle) {
                                    let account_marker =
                                        notification_controller::account_marker_for_profile(
                                            &service, profile_id,
                                        );
                                    let controller = handle.state::<Mutex<
                                        notification_controller::NotificationController<
                                            notification_controller::WindowsToastSink,
                                        >,
                                    >>();
                                    if let Ok(mut controller) = controller.lock() {
                                        let event = codexbar::accounts::model::AccountServiceEvent::RefreshCompleted {
                                            profile_id,
                                            success,
                                        };
                                        if controller
                                            .observe_account_service_event(
                                                &settings_repository,
                                                account_marker.as_deref(),
                                                &event,
                                            )
                                            .is_err()
                                        {
                                            tracing::warn!(
                                                code = "NOTIFICATION_REFRESH_DISPATCH_FAILED",
                                                "refresh notification was not delivered"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
            crate::tray_bridge::setup(app)?;
            coordinator.record(app_coordinator::StartupMilestone::Tray);
            let settings_repository =
                app.state::<Mutex<AppState>>()
                    .lock()
                    .ok()
                    .and_then(|state| {
                        state
                            .account_service
                            .as_ref()
                            .map(|service| service.repositories().settings.clone())
                    });
            let settings = settings_repository
                .and_then(|repository| repository.load().ok())
                .unwrap_or_default();
            status_surfaces::apply_status_surface_settings_non_fatal(app.handle(), &settings);
            status_surfaces::start_monitor(app.handle().clone());
            auto_refresh::start(app.handle().clone());
            notification_controller::start_update_check_loop(app.handle().clone());
            tracing::debug!(
                milestones = ?coordinator.trace_names(),
                planned = ?app_coordinator::cached_start_steps()
                    .iter()
                    .map(|milestone| milestone.name())
                    .collect::<Vec<_>>(),
                "cache-first startup order"
            );

            if is_proof_mode {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    proof_harness::activate(&app_handle);
                });
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if taskbar_overlay::window::is_measurement_window_label(window.label()) {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    status_surfaces::handle_taskbar_measurement_window_destroyed(
                        window.app_handle(),
                    );
                }
                return;
            }
            if let Some(surface) = status_surfaces::surface_for_window_label(window.label()) {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        status_surfaces::schedule_set_enabled(
                            window.app_handle().clone(),
                            surface,
                            false,
                        );
                    }
                    tauri::WindowEvent::Destroyed => match surface {
                        status_surfaces::controller::StatusSurfaceKind::TaskbarStatus => {
                            status_surfaces::handle_taskbar_window_destroyed(window.app_handle());
                        }
                        status_surfaces::controller::StatusSurfaceKind::FloatBall => {
                            status_surfaces::handle_float_ball_window_destroyed(
                                window.app_handle(),
                            );
                        }
                    },
                    tauri::WindowEvent::Moved(position)
                        if surface == status_surfaces::controller::StatusSurfaceKind::FloatBall =>
                    {
                        status_surfaces::handle_float_ball_moved(window.app_handle(), *position);
                    }
                    tauri::WindowEvent::ScaleFactorChanged { .. } => match surface {
                        status_surfaces::controller::StatusSurfaceKind::TaskbarStatus => {
                            status_surfaces::schedule_taskbar_reposition(
                                window.app_handle().clone(),
                            );
                        }
                        status_surfaces::controller::StatusSurfaceKind::FloatBall => {
                            status_surfaces::schedule_status_reposition(
                                window.app_handle().clone(),
                            );
                        }
                    },
                    _ => {}
                }
                return;
            }
            // Only the tray flyout window participates in blur-dismiss.
            if window.label() != crate::shell::flyout_window::FLYOUT_LABEL {
                return;
            }
            match event {
                tauri::WindowEvent::Focused(false)
                    if !proof_harness::is_proof_mode(window.app_handle())
                        && crate::shell::flyout_window::should_blur_dismiss() =>
                {
                    let _ = crate::shell::flyout_window::hide(window.app_handle());
                }
                tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    crate::shell::flyout_window::keep_inside_work_area(window.app_handle());
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run codex-barbar desktop shell");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_surface_modes_are_only_hidden_tray_and_settings() {
        assert_eq!(
            SurfaceMode::ALL,
            &[
                SurfaceMode::Hidden,
                SurfaceMode::TrayPanel,
                SurfaceMode::Settings
            ]
        );
    }

    #[test]
    fn tauri_config_has_v1_identity() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["productName"], "codex-barbar");
        assert_eq!(config["identifier"], "com.naipi11.codexbarbar");
        assert_eq!(config["app"]["windows"][0]["title"], "codex-barbar");
        assert_eq!(config["bundle"]["targets"], serde_json::json!(["nsis"]));
        assert_eq!(config["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn nsis_is_current_user_and_only_bundle_target() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["bundle"]["targets"], serde_json::json!(["nsis"]));
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["installMode"],
            "currentUser"
        );
    }

    #[test]
    fn auxiliary_surface_labels_are_allowlisted_for_webview_events() {
        let capabilities: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let windows = capabilities["windows"].as_array().unwrap();
        assert!(windows.iter().any(|value| value == "taskbar-status"));
        assert!(windows.iter().any(|value| value == "float-ball"));
    }
}
