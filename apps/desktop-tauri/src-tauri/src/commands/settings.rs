//! V1 settings commands: snapshot, patch, and safe Codex path validation.

use std::path::PathBuf;
use std::sync::Mutex;

use codexbar::providers::codex::app_server::{CodexCommandResolver, CodexInstallation};
use codexbar::storage::SettingsPatch;
use tauri::Emitter;

use super::bridge::{AppSettingsDto, CodexCompatibilityDto, SettingsPatchDto};
use crate::{
    notification_controller::{
        NotificationCapabilityDto, NotificationCapabilityStatus, NotificationController,
        WindowsToastSink, notification_capability,
    },
    state::AppState,
};

pub(crate) fn settings_repository(
    state: &tauri::State<'_, Mutex<AppState>>,
) -> Result<codexbar::storage::SettingsRepository, String> {
    let guard = state
        .lock()
        .map_err(|_| "SETTINGS_REPOSITORY_UNAVAILABLE".to_string())?;
    guard
        .account_service
        .as_ref()
        .map(|service| service.repositories().settings.clone())
        .ok_or_else(|| "SETTINGS_REPOSITORY_UNAVAILABLE".to_string())
}

fn split_surface_patch(
    mut patch: SettingsPatch,
) -> (
    SettingsPatch,
    Vec<(crate::status_surfaces::controller::StatusSurfaceKind, bool)>,
) {
    let mut requested = Vec::new();
    if let Some(enabled) = patch.taskbar_status_enabled.take() {
        requested.push((
            crate::status_surfaces::controller::StatusSurfaceKind::TaskbarStatus,
            enabled,
        ));
    }
    if let Some(enabled) = patch.float_ball_enabled.take() {
        requested.push((
            crate::status_surfaces::controller::StatusSurfaceKind::FloatBall,
            enabled,
        ));
    }
    (patch, requested)
}

fn storage_error_code(error: codexbar::storage::StorageError) -> String {
    match error.code() {
        "SETTINGS_SURFACE_OPACITY_INVALID" | "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID" => {
            error.code().to_string()
        }
        _ => "SETTINGS_SAVE_FAILED".to_string(),
    }
}

fn prepare_settings_update(
    patch: SettingsPatch,
) -> Result<
    (
        SettingsPatch,
        Vec<(crate::status_surfaces::controller::StatusSurfaceKind, bool)>,
    ),
    String,
> {
    patch.validate().map_err(storage_error_code)?;
    Ok(split_surface_patch(patch))
}

#[tauri::command]
pub fn get_settings_snapshot(state: tauri::State<'_, Mutex<AppState>>) -> AppSettingsDto {
    match settings_repository(&state).and_then(|repository| {
        repository
            .load()
            .map_err(|_| "SETTINGS_LOAD_FAILED".to_string())
    }) {
        Ok(settings) => AppSettingsDto::from_settings(&settings),
        Err(_) => AppSettingsDto::default(),
    }
}

#[tauri::command]
pub fn get_notification_capability() -> NotificationCapabilityDto {
    notification_capability()
}

#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    patch: SettingsPatchDto,
) -> Result<AppSettingsDto, String> {
    let patch = patch.into_patch()?;
    let (patch, requested_surfaces) = prepare_settings_update(patch)?;
    if let Some(enabled) = patch.start_at_login {
        codexbar::platform::windows::autostart::set_enabled(enabled)
            .map_err(|_| "AUTOSTART_UPDATE_FAILED".to_string())?;
    }
    let repository = settings_repository(&state)?;
    let mut settings = if patch == SettingsPatch::default() {
        repository
            .load()
            .map_err(|_| "SETTINGS_LOAD_FAILED".to_string())?
    } else {
        repository.update(patch).map_err(storage_error_code)?
    };
    for (surface, enabled) in requested_surfaces {
        settings = crate::status_surfaces::controller::set_enabled_with_repository(
            &app,
            &repository,
            surface,
            enabled,
        )?;
    }
    let dto = AppSettingsDto::from_settings(&settings);
    if app.emit(crate::events::SETTINGS_CHANGED, &dto).is_err() {
        tracing::warn!(
            code = "SETTINGS_EVENT_FAILED",
            "settings event was not delivered"
        );
    }
    Ok(dto)
}

#[tauri::command]
pub fn send_test_notification(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    controller: tauri::State<'_, Mutex<NotificationController<WindowsToastSink>>>,
) -> Result<(), String> {
    let capability = notification_capability();
    let proof_mode = crate::proof_harness::is_proof_mode(&app);
    send_test_notification_with(proof_mode, capability, || {
        let repository = settings_repository(&state)?;
        controller
            .lock()
            .map_err(|_| "NOTIFICATION_TEST_FAILED".to_string())?
            .send_test(&repository)
            .map_err(map_notification_test_error)
    })
}

fn send_test_notification_with<F>(
    proof_mode: bool,
    capability: NotificationCapabilityDto,
    send: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    match capability.status {
        NotificationCapabilityStatus::AppDisabled
        | NotificationCapabilityStatus::GlobalDisabled => {
            Err("NOTIFICATION_PERMISSION_DISABLED".to_string())
        }
        NotificationCapabilityStatus::Unsupported => Err("NOTIFICATION_TEST_FAILED".to_string()),
        NotificationCapabilityStatus::Available if proof_mode => Ok(()),
        NotificationCapabilityStatus::Available => send(),
    }
}

fn map_notification_test_error(error: String) -> String {
    if error == "NOTIFICATION_PERMISSION_DISABLED" {
        error
    } else {
        "NOTIFICATION_TEST_FAILED".to_string()
    }
}

#[tauri::command]
pub fn validate_codex_executable(
    app: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AppState>>,
    path: String,
) -> Result<CodexCompatibilityDto, String> {
    let command = match CodexCommandResolver::new().resolve_override(&PathBuf::from(path)) {
        Ok(command) => command,
        Err(error) => {
            return Ok(CodexCompatibilityDto {
                status: if error.kind == codexbar::core::AppErrorKind::CodexNotFound {
                    "notFound"
                } else {
                    "unsupported"
                },
                installation: None,
                executable_path: None,
                version: None,
                capabilities: Default::default(),
            });
        }
    };

    let Some(version) = codexbar::providers::codex::app_server::discovery::probe_version(&command)
    else {
        return Ok(CodexCompatibilityDto {
            status: "unsupported",
            installation: None,
            executable_path: None,
            version: None,
            capabilities: Default::default(),
        });
    };

    let normalized = command.launch_program().to_string_lossy().into_owned();
    let repository = settings_repository(&state)?;
    let settings = repository
        .update(SettingsPatch {
            codex_executable_override: Some(Some(normalized.clone())),
            ..SettingsPatch::default()
        })
        .map_err(|_| "SETTINGS_SAVE_FAILED".to_string())?;
    let dto = AppSettingsDto::from_settings(&settings);
    let _ = app.emit(crate::events::SETTINGS_CHANGED, &dto);

    Ok(CodexCompatibilityDto {
        status: "compatible",
        installation: match command.installation() {
            CodexInstallation::VerifiedNpmLayout => Some("verifiedNpmLayout"),
            CodexInstallation::NativeExe | CodexInstallation::StoreAlias => Some("nativeExe"),
        },
        executable_path: Some(normalized),
        version: Some(version),
        capabilities: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::bridge::NotificationPreferencesPatchDto;
    use codexbar::storage::{
        DisplayMode, LanguagePreference, NotificationPreferencesPatch, ThemePreference,
    };

    struct AppDisabledProbe;

    impl crate::notification_controller::NotificationSettingProbe for AppDisabledProbe {
        fn notification_setting(&self) -> Result<u32, ()> {
            Ok(1)
        }
    }

    #[test]
    fn surface_fields_are_removed_before_generic_repository_update() {
        let patch = SettingsPatch {
            theme: Some(ThemePreference::Dark),
            taskbar_status_enabled: Some(false),
            float_ball_enabled: Some(true),
            ..SettingsPatch::default()
        };
        let (base, requested) = split_surface_patch(patch);
        assert_eq!(base.theme, Some(ThemePreference::Dark));
        assert_eq!(base.taskbar_status_enabled, None);
        assert_eq!(base.float_ball_enabled, None);
        assert_eq!(
            requested,
            vec![
                (
                    crate::status_surfaces::controller::StatusSurfaceKind::TaskbarStatus,
                    false,
                ),
                (
                    crate::status_surfaces::controller::StatusSurfaceKind::FloatBall,
                    true,
                ),
            ]
        );
    }

    #[test]
    fn invalid_mixed_patch_preflight_blocks_all_side_effects_and_preserves_opacity_code() {
        let patch = SettingsPatch {
            start_at_login: Some(true),
            taskbar_status_enabled: Some(true),
            taskbar_status_opacity: Some(81),
            ..SettingsPatch::default()
        };
        let mut autostart_calls = 0;
        let mut persistence_calls = 0;
        let mut surface_calls = 0;

        let result = prepare_settings_update(patch);
        if let Ok((generic_patch, requested_surfaces)) = result.as_ref() {
            if generic_patch.start_at_login.is_some() {
                autostart_calls += 1;
            }
            if generic_patch != &SettingsPatch::default() {
                persistence_calls += 1;
            }
            surface_calls += requested_surfaces.len();
        }

        assert_eq!(
            result.unwrap_err(),
            "SETTINGS_SURFACE_OPACITY_INVALID".to_string()
        );
        assert_eq!(autostart_calls, 0);
        assert_eq!(persistence_calls, 0);
        assert_eq!(surface_calls, 0);
    }

    #[test]
    fn patch_maps_all_frozen_fields() {
        let patch = SettingsPatchDto {
            autostart_enabled: Some(true),
            refresh_interval_seconds: Some(900),
            display_mode: Some("used".to_string()),
            theme: Some("dark".to_string()),
            language: Some("zh-CN".to_string()),
            codex_executable_override: Some(Some(r"C:\Codex\codex.exe".to_string())),
            taskbar_status_enabled: Some(true),
            float_ball_enabled: Some(false),
            taskbar_status_opacity: Some(0),
            float_ball_opacity: Some(80),
            float_ball_glow: Some(40),
            notifications: Some(NotificationPreferencesPatchDto {
                enabled: Some(true),
                play_sound: Some(false),
                warning_enabled: Some(false),
                danger_enabled: Some(true),
                weekly_reset_enabled: Some(false),
                reset_credit_increase_enabled: Some(true),
                refresh_failure_enabled: Some(false),
                update_available_enabled: Some(true),
                warning_remaining_percent: Some(66),
                danger_remaining_percent: Some(33),
            }),
        };
        let settings = patch.into_patch().unwrap();
        assert_eq!(settings.start_at_login, Some(true));
        assert_eq!(settings.refresh_interval_seconds, Some(900));
        assert_eq!(settings.display_mode, Some(DisplayMode::Used));
        assert_eq!(settings.theme, Some(ThemePreference::Dark));
        assert_eq!(settings.language, Some(LanguagePreference::ZhCn));
        assert_eq!(
            settings.codex_executable_override,
            Some(Some(r"C:\Codex\codex.exe".to_string()))
        );
        assert_eq!(settings.taskbar_status_enabled, Some(true));
        assert_eq!(settings.float_ball_enabled, Some(false));
        assert_eq!(settings.taskbar_status_opacity, Some(0));
        assert_eq!(settings.float_ball_opacity, Some(80));
        assert_eq!(settings.float_ball_glow, Some(40));
        assert_eq!(
            settings.notifications,
            Some(NotificationPreferencesPatch {
                enabled: Some(true),
                play_sound: Some(false),
                warning_enabled: Some(false),
                danger_enabled: Some(true),
                weekly_reset_enabled: Some(false),
                reset_credit_increase_enabled: Some(true),
                refresh_failure_enabled: Some(false),
                update_available_enabled: Some(true),
                warning_remaining_percent: Some(66),
                danger_remaining_percent: Some(33),
            })
        );
    }

    #[test]
    fn patch_rejects_unknown_enum_values() {
        let patch = SettingsPatchDto {
            display_mode: Some("percentage".to_string()),
            ..SettingsPatchDto::default()
        };
        assert!(patch.into_patch().is_err());
    }

    #[test]
    fn dto_round_trips_default_settings() {
        let settings = codexbar::storage::AppSettings::default();
        let dto = AppSettingsDto::from_settings(&settings);
        assert!(dto.autostart_enabled);
        assert_eq!(dto.refresh_interval_seconds, 300);
        assert_eq!(dto.display_mode, "remaining");
        assert_eq!(dto.theme, "system");
        assert_eq!(dto.language, "system");
        assert_eq!(dto.codex_executable_override, None);
        assert!(!dto.taskbar_status_enabled);
        assert!(dto.float_ball_enabled);
        assert_eq!(dto.taskbar_status_opacity, 20);
        assert_eq!(dto.float_ball_opacity, 20);
        assert_eq!(dto.float_ball_glow, 20);
        assert!(!dto.notifications.enabled);
        assert!(dto.notifications.play_sound);
        assert_eq!(dto.notifications.warning_remaining_percent, 66);
        assert_eq!(dto.notifications.danger_remaining_percent, 33);
    }

    #[test]
    fn notification_patch_rejects_out_of_range_threshold_before_storage() {
        let patch = SettingsPatchDto {
            notifications: Some(NotificationPreferencesPatchDto {
                warning_remaining_percent: Some(101),
                ..Default::default()
            }),
            ..SettingsPatchDto::default()
        };

        assert_eq!(
            patch.into_patch().unwrap_err(),
            "notification threshold must be between 0 and 100"
        );
    }

    #[test]
    fn notification_threshold_error_code_is_preserved_for_inline_feedback() {
        let patch = SettingsPatch {
            notifications: Some(NotificationPreferencesPatch {
                warning_remaining_percent: Some(101),
                ..NotificationPreferencesPatch::default()
            }),
            ..SettingsPatch::default()
        };

        assert_eq!(
            prepare_settings_update(patch).unwrap_err(),
            "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID"
        );
    }

    #[test]
    fn notification_permission_disabled_code_is_preserved_for_recovery_ui() {
        assert_eq!(
            map_notification_test_error("NOTIFICATION_PERMISSION_DISABLED".to_string()),
            "NOTIFICATION_PERMISSION_DISABLED"
        );
        assert_eq!(
            map_notification_test_error("raw transport detail".to_string()),
            "NOTIFICATION_TEST_FAILED"
        );
    }

    #[test]
    fn proof_mode_app_enabled_zero_returns_disabled_before_transport_noop() {
        let capability =
            crate::notification_controller::detect_notification_capability(&AppDisabledProbe, true);
        let mut transport_started = false;

        let result = send_test_notification_with(true, capability, || {
            transport_started = true;
            Ok(())
        });

        assert_eq!(result.unwrap_err(), "NOTIFICATION_PERMISSION_DISABLED");
        assert!(!transport_started);
    }

    #[test]
    fn proof_mode_skips_transport_only_for_known_available_capability() {
        let mut transport_started = false;
        let available = crate::notification_controller::NotificationCapabilityDto {
            status: crate::notification_controller::NotificationCapabilityStatus::Available,
            can_open_settings: true,
        };
        let unsupported = crate::notification_controller::NotificationCapabilityDto {
            status: crate::notification_controller::NotificationCapabilityStatus::Unsupported,
            can_open_settings: false,
        };

        assert_eq!(
            send_test_notification_with(true, available, || {
                transport_started = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(!transport_started);
        assert_eq!(
            send_test_notification_with(true, unsupported, || {
                transport_started = true;
                Ok(())
            })
            .unwrap_err(),
            "NOTIFICATION_TEST_FAILED"
        );
        assert!(!transport_started);
    }
}
