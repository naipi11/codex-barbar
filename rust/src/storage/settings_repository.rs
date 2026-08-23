//! Persistent V1 application settings.

use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{AppDatabase, MenuPreferences, MenuPreferencesPatch, StorageError};

const SETTINGS_KEY: &str = "app_settings";
const ALLOWED_REFRESH_INTERVALS: [u64; 5] = [0, 60, 300, 900, 1800];
const DEFAULT_SURFACE_OPACITY: u8 = 20;
const MAX_SURFACE_OPACITY: u8 = 80;
const MAX_NOTIFICATION_PERCENT: u8 = 100;

const fn default_surface_opacity() -> u8 {
    DEFAULT_SURFACE_OPACITY
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    #[serde(rename = "remaining")]
    Remaining,
    #[serde(rename = "used")]
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreference {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguagePreference {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

impl LanguagePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::ZhCn, Self::EnUs];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub play_sound: bool,
    pub warning_enabled: bool,
    pub danger_enabled: bool,
    pub weekly_reset_enabled: bool,
    pub reset_credit_increase_enabled: bool,
    pub refresh_failure_enabled: bool,
    pub update_available_enabled: bool,
    pub warning_remaining_percent: u8,
    pub danger_remaining_percent: u8,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            play_sound: true,
            warning_enabled: true,
            danger_enabled: true,
            weekly_reset_enabled: true,
            reset_credit_increase_enabled: true,
            refresh_failure_enabled: true,
            update_available_enabled: true,
            warning_remaining_percent: 66,
            danger_remaining_percent: 33,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskbarDensity {
    Compact,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayIconMode {
    Dynamic,
    Monochrome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskbarTrayPreferences {
    pub show_taskbar_icon: bool,
    pub show_taskbar_account: bool,
    pub show_weekly_label: bool,
    pub show_weekly_percent: bool,
    pub show_reset_date: bool,
    pub density: TaskbarDensity,
    pub tray_icon_mode: TrayIconMode,
    pub tooltip_account: bool,
    pub tooltip_weekly: bool,
    pub tooltip_reset_date: bool,
    pub tooltip_updated_at: bool,
    pub hide_status_surfaces_in_fullscreen: bool,
}

impl Default for TaskbarTrayPreferences {
    fn default() -> Self {
        Self {
            show_taskbar_icon: true,
            show_taskbar_account: true,
            show_weekly_label: true,
            show_weekly_percent: true,
            show_reset_date: true,
            density: TaskbarDensity::Compact,
            tray_icon_mode: TrayIconMode::Dynamic,
            tooltip_account: true,
            tooltip_weekly: true,
            tooltip_reset_date: true,
            tooltip_updated_at: true,
            hide_status_surfaces_in_fullscreen: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationPreferencesPatch {
    pub enabled: Option<bool>,
    pub play_sound: Option<bool>,
    pub warning_enabled: Option<bool>,
    pub danger_enabled: Option<bool>,
    pub weekly_reset_enabled: Option<bool>,
    pub reset_credit_increase_enabled: Option<bool>,
    pub refresh_failure_enabled: Option<bool>,
    pub update_available_enabled: Option<bool>,
    pub warning_remaining_percent: Option<u8>,
    pub danger_remaining_percent: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskbarTrayPreferencesPatch {
    pub show_taskbar_icon: Option<bool>,
    pub show_taskbar_account: Option<bool>,
    pub show_weekly_label: Option<bool>,
    pub show_weekly_percent: Option<bool>,
    pub show_reset_date: Option<bool>,
    pub density: Option<TaskbarDensity>,
    pub tray_icon_mode: Option<TrayIconMode>,
    pub tooltip_account: Option<bool>,
    pub tooltip_weekly: Option<bool>,
    pub tooltip_reset_date: Option<bool>,
    pub tooltip_updated_at: Option<bool>,
    pub hide_status_surfaces_in_fullscreen: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct AppSettings {
    pub start_at_login: bool,
    pub refresh_interval_seconds: u64,
    pub display_mode: DisplayMode,
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub codex_executable_override: Option<String>,
    pub taskbar_status_enabled: bool,
    pub float_ball_enabled: bool,
    #[serde(default = "default_surface_opacity")]
    pub taskbar_status_opacity: u8,
    #[serde(default = "default_surface_opacity")]
    pub float_ball_opacity: u8,
    #[serde(default = "default_surface_opacity")]
    pub float_ball_glow: u8,
    pub notifications: NotificationPreferences,
    pub taskbar_tray: TaskbarTrayPreferences,
    pub menu: MenuPreferences,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_at_login: true,
            refresh_interval_seconds: 300,
            display_mode: DisplayMode::Remaining,
            theme: ThemePreference::System,
            language: LanguagePreference::System,
            codex_executable_override: None,
            taskbar_status_enabled: false,
            float_ball_enabled: true,
            taskbar_status_opacity: DEFAULT_SURFACE_OPACITY,
            float_ball_opacity: DEFAULT_SURFACE_OPACITY,
            float_ball_glow: DEFAULT_SURFACE_OPACITY,
            notifications: NotificationPreferences::default(),
            taskbar_tray: TaskbarTrayPreferences::default(),
            menu: MenuPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsPatch {
    pub start_at_login: Option<bool>,
    pub refresh_interval_seconds: Option<u64>,
    pub display_mode: Option<DisplayMode>,
    pub theme: Option<ThemePreference>,
    pub language: Option<LanguagePreference>,
    pub codex_executable_override: Option<Option<String>>,
    pub taskbar_status_enabled: Option<bool>,
    pub float_ball_enabled: Option<bool>,
    pub taskbar_status_opacity: Option<u8>,
    pub float_ball_opacity: Option<u8>,
    pub float_ball_glow: Option<u8>,
    pub notifications: Option<NotificationPreferencesPatch>,
    pub taskbar_tray: Option<TaskbarTrayPreferencesPatch>,
    pub menu: Option<MenuPreferencesPatch>,
}

#[derive(Clone)]
pub struct SettingsRepository {
    db: Arc<AppDatabase>,
}

impl SettingsRepository {
    pub fn new(db: Arc<AppDatabase>) -> Self {
        Self { db }
    }

    pub fn load(&self) -> Result<AppSettings, StorageError> {
        self.db.with_connection(load_from_connection)
    }

    pub fn update(&self, patch: SettingsPatch) -> Result<AppSettings, StorageError> {
        self.db.with_connection_mut(|connection| {
            let transaction = connection.transaction().map_err(storage_error)?;
            let mut settings = load_from_connection(&transaction)?;
            patch.validate()?;
            settings.apply(patch);
            settings.normalize();
            settings.validate()?;
            let encoded = serde_json::to_string(&settings).map_err(|error| {
                StorageError::new(
                    crate::core::AppErrorKind::StorageFailure,
                    "SETTINGS_ENCODE_FAILED",
                    error.to_string(),
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                    params![SETTINGS_KEY, encoded],
                )
                .map_err(storage_error)?;
            transaction.commit().map_err(storage_error)?;
            Ok(settings)
        })
    }

    /// Produce the settings that a patch would save without writing anything.
    ///
    /// Used by transactional native-menu application: the candidate must be
    /// applied to the real tray first, and only persisted after it succeeds.
    pub fn preview_update(&self, patch: SettingsPatch) -> Result<AppSettings, StorageError> {
        let mut settings = self.load()?;
        patch.validate()?;
        settings.apply(patch);
        settings.normalize();
        settings.validate()?;
        Ok(settings)
    }
}

impl SettingsPatch {
    pub fn validate(&self) -> Result<(), StorageError> {
        if let Some(value) = self.refresh_interval_seconds
            && !ALLOWED_REFRESH_INTERVALS.contains(&value)
        {
            return Err(StorageError::new(
                crate::core::AppErrorKind::StorageFailure,
                "SETTINGS_REFRESH_INTERVAL_INVALID",
                "refresh interval is not supported",
            ));
        }
        for value in [
            self.taskbar_status_opacity,
            self.float_ball_opacity,
            self.float_ball_glow,
        ]
        .into_iter()
        .flatten()
        {
            if value > MAX_SURFACE_OPACITY {
                return Err(StorageError::new(
                    crate::core::AppErrorKind::StorageFailure,
                    "SETTINGS_SURFACE_OPACITY_INVALID",
                    "surface opacity must be between 0 and 80",
                ));
            }
        }
        if let Some(notifications) = &self.notifications {
            notifications.validate()?;
        }
        Ok(())
    }
}

impl NotificationPreferencesPatch {
    fn validate(&self) -> Result<(), StorageError> {
        for value in [
            self.warning_remaining_percent,
            self.danger_remaining_percent,
        ]
        .into_iter()
        .flatten()
        {
            if value > MAX_NOTIFICATION_PERCENT {
                return Err(notification_thresholds_error());
            }
        }
        Ok(())
    }

    fn apply_to(self, preferences: &mut NotificationPreferences) {
        if let Some(value) = self.enabled {
            preferences.enabled = value;
        }
        if let Some(value) = self.play_sound {
            preferences.play_sound = value;
        }
        if let Some(value) = self.warning_enabled {
            preferences.warning_enabled = value;
        }
        if let Some(value) = self.danger_enabled {
            preferences.danger_enabled = value;
        }
        if let Some(value) = self.weekly_reset_enabled {
            preferences.weekly_reset_enabled = value;
        }
        if let Some(value) = self.reset_credit_increase_enabled {
            preferences.reset_credit_increase_enabled = value;
        }
        if let Some(value) = self.refresh_failure_enabled {
            preferences.refresh_failure_enabled = value;
        }
        if let Some(value) = self.update_available_enabled {
            preferences.update_available_enabled = value;
        }
        if let Some(value) = self.warning_remaining_percent {
            preferences.warning_remaining_percent = value;
        }
        if let Some(value) = self.danger_remaining_percent {
            preferences.danger_remaining_percent = value;
        }
    }
}

impl TaskbarTrayPreferencesPatch {
    fn apply_to(self, preferences: &mut TaskbarTrayPreferences) {
        if let Some(value) = self.show_taskbar_icon {
            preferences.show_taskbar_icon = value;
        }
        if let Some(value) = self.show_taskbar_account {
            preferences.show_taskbar_account = value;
        }
        if let Some(value) = self.show_weekly_label {
            preferences.show_weekly_label = value;
        }
        if let Some(value) = self.show_weekly_percent {
            preferences.show_weekly_percent = value;
        }
        if let Some(value) = self.show_reset_date {
            preferences.show_reset_date = value;
        }
        if let Some(value) = self.density {
            preferences.density = value;
        }
        if let Some(value) = self.tray_icon_mode {
            preferences.tray_icon_mode = value;
        }
        if let Some(value) = self.tooltip_account {
            preferences.tooltip_account = value;
        }
        if let Some(value) = self.tooltip_weekly {
            preferences.tooltip_weekly = value;
        }
        if let Some(value) = self.tooltip_reset_date {
            preferences.tooltip_reset_date = value;
        }
        if let Some(value) = self.tooltip_updated_at {
            preferences.tooltip_updated_at = value;
        }
        if let Some(value) = self.hide_status_surfaces_in_fullscreen {
            preferences.hide_status_surfaces_in_fullscreen = value;
        }
    }
}

impl AppSettings {
    fn apply(&mut self, patch: SettingsPatch) {
        if let Some(value) = patch.refresh_interval_seconds {
            self.refresh_interval_seconds = value;
        }
        if let Some(value) = patch.start_at_login {
            self.start_at_login = value;
        }
        if let Some(value) = patch.display_mode {
            self.display_mode = value;
        }
        if let Some(value) = patch.theme {
            self.theme = value;
        }
        if let Some(value) = patch.language {
            self.language = value;
        }
        if let Some(value) = patch.codex_executable_override {
            self.codex_executable_override = value;
        }
        if let Some(value) = patch.taskbar_status_enabled {
            self.taskbar_status_enabled = value;
        }
        if let Some(value) = patch.float_ball_enabled {
            self.float_ball_enabled = value;
        }
        if let Some(value) = patch.taskbar_status_opacity {
            self.taskbar_status_opacity = value;
        }
        if let Some(value) = patch.float_ball_opacity {
            self.float_ball_opacity = value;
        }
        if let Some(value) = patch.float_ball_glow {
            self.float_ball_glow = value;
        }
        if let Some(notifications) = patch.notifications {
            notifications.apply_to(&mut self.notifications);
        }
        if let Some(taskbar_tray) = patch.taskbar_tray {
            taskbar_tray.apply_to(&mut self.taskbar_tray);
        }
        if let Some(menu) = patch.menu {
            menu.apply_to(&mut self.menu);
        }
    }

    fn normalize(&mut self) {
        self.menu.normalize();
    }

    fn validate(&self) -> Result<(), StorageError> {
        if self.notifications.danger_remaining_percent
            >= self.notifications.warning_remaining_percent
            || self.notifications.warning_remaining_percent > MAX_NOTIFICATION_PERCENT
        {
            return Err(notification_thresholds_error());
        }
        Ok(())
    }
}

fn notification_thresholds_error() -> StorageError {
    StorageError::new(
        crate::core::AppErrorKind::StorageFailure,
        "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID",
        "notification danger threshold must be lower than warning threshold",
    )
}

fn load_from_connection(connection: &rusqlite::Connection) -> Result<AppSettings, StorageError> {
    let value = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            params![SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    let Some(value) = value else {
        return Ok(AppSettings::default());
    };
    let mut settings = serde_json::from_str::<AppSettings>(&value).map_err(|error| {
        StorageError::new(
            crate::core::AppErrorKind::StorageFailure,
            "SETTINGS_DECODE_FAILED",
            error.to_string(),
        )
    })?;
    settings.normalize();
    Ok(settings)
}

fn storage_error(error: rusqlite::Error) -> StorageError {
    StorageError::new(
        crate::core::AppErrorKind::StorageFailure,
        "SETTINGS_DB_FAILED",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MenuLayoutPatch;
    use std::sync::Arc;
    #[test]
    fn expanded_settings_json_round_trips_every_preference() {
        let (repository, _) = settings_fixture();
        let updated = repository
            .update(SettingsPatch {
                start_at_login: Some(false),
                refresh_interval_seconds: Some(1800),
                display_mode: Some(DisplayMode::Used),
                theme: Some(ThemePreference::Dark),
                language: Some(LanguagePreference::ZhCn),
                taskbar_status_enabled: Some(true),
                float_ball_enabled: Some(false),
                taskbar_status_opacity: Some(0),
                float_ball_opacity: Some(80),
                float_ball_glow: Some(40),
                notifications: Some(NotificationPreferencesPatch {
                    enabled: Some(true),
                    play_sound: Some(false),
                    warning_enabled: Some(false),
                    danger_enabled: Some(true),
                    weekly_reset_enabled: Some(false),
                    reset_credit_increase_enabled: Some(true),
                    refresh_failure_enabled: Some(false),
                    update_available_enabled: Some(true),
                    warning_remaining_percent: Some(70),
                    danger_remaining_percent: Some(30),
                }),
                taskbar_tray: Some(TaskbarTrayPreferencesPatch {
                    show_taskbar_icon: Some(false),
                    show_taskbar_account: Some(true),
                    show_weekly_label: Some(false),
                    show_weekly_percent: Some(true),
                    show_reset_date: Some(false),
                    density: Some(TaskbarDensity::Standard),
                    tray_icon_mode: Some(TrayIconMode::Monochrome),
                    tooltip_account: Some(false),
                    tooltip_weekly: Some(true),
                    tooltip_reset_date: Some(false),
                    tooltip_updated_at: Some(true),
                    hide_status_surfaces_in_fullscreen: Some(false),
                }),
                menu: Some(MenuPreferencesPatch {
                    native_tray: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into(), "settings".into()]),
                        hidden: None,
                    }),
                    tray_panel: None,
                }),
                ..SettingsPatch::default()
            })
            .unwrap();

        let reloaded = repository.load().unwrap();
        assert_eq!(reloaded, updated);
        assert_eq!(reloaded.theme, ThemePreference::Dark);
        assert_eq!(reloaded.language, LanguagePreference::ZhCn);
        assert_eq!(reloaded.notifications.warning_remaining_percent, 70);
        assert_eq!(
            reloaded.taskbar_tray.tray_icon_mode,
            TrayIconMode::Monochrome
        );
        assert!(
            reloaded
                .menu
                .native_tray
                .order
                .contains(&"quit".to_string())
        );
    }

    #[test]
    fn persisted_invalid_threshold_ordering_loads_but_save_is_atomic() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"notifications":{"enabled":true,"warningRemainingPercent":30,"dangerRemainingPercent":70}}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let loaded = repository.load().unwrap();
        assert_eq!(loaded.notifications.warning_remaining_percent, 30);
        assert_eq!(loaded.notifications.danger_remaining_percent, 70);

        let error = repository
            .update(SettingsPatch {
                notifications: Some(NotificationPreferencesPatch {
                    warning_remaining_percent: Some(20),
                    ..Default::default()
                }),
                ..SettingsPatch::default()
            })
            .unwrap_err();
        assert_eq!(error.code(), "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID");
        assert_eq!(
            repository
                .load()
                .unwrap()
                .notifications
                .warning_remaining_percent,
            30
        );
    }

    #[test]
    fn combined_malformed_menu_layout_normalizes_without_crashing() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"menu":{"nativeTray":{"order":["quit","unknown","refresh","refresh","settings"],"hidden":["settings","refresh","unknown"]},"trayPanel":{"order":["dismiss","quit"],"hidden":["refresh"]}}}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let settings = repository.load().unwrap();
        let native = &settings.menu.native_tray;
        assert!(native.order.contains(&"settings".to_string()));
        assert!(native.order.contains(&"quit".to_string()));
        assert!(!native.order.contains(&"unknown".to_string()));
        assert!(!native.order.contains(&"refresh".to_string()));
        assert_eq!(
            settings.menu.tray_panel.order,
            vec![
                "dismiss".to_string(),
                "quit".to_string(),
                "open_usage".to_string(),
                "settings".to_string(),
            ]
        );
    }

    #[test]
    fn v1_settings_defaults_are_exact() {
        let settings = AppSettings::default();
        assert_eq!(settings.refresh_interval_seconds, 300);
        assert_eq!(settings.display_mode, DisplayMode::Remaining);
        assert_eq!(settings.theme, ThemePreference::System);
        assert_eq!(settings.language, LanguagePreference::System);
        assert!(settings.start_at_login);
        assert!(!settings.taskbar_status_enabled);
        assert!(settings.float_ball_enabled);
        assert_eq!(settings.taskbar_status_opacity, 20);
        assert_eq!(settings.float_ball_opacity, 20);
        assert_eq!(settings.float_ball_glow, 20);
        assert!(settings.taskbar_tray.show_taskbar_icon);
        assert!(settings.taskbar_tray.show_taskbar_account);
        assert!(settings.taskbar_tray.show_weekly_label);
        assert!(settings.taskbar_tray.show_weekly_percent);
        assert!(settings.taskbar_tray.show_reset_date);
        assert_eq!(settings.taskbar_tray.density, TaskbarDensity::Compact);
        assert_eq!(settings.taskbar_tray.tray_icon_mode, TrayIconMode::Dynamic);
        assert!(settings.taskbar_tray.tooltip_account);
        assert!(settings.taskbar_tray.tooltip_weekly);
        assert!(settings.taskbar_tray.tooltip_reset_date);
        assert!(settings.taskbar_tray.tooltip_updated_at);
        assert!(settings.taskbar_tray.hide_status_surfaces_in_fullscreen);
        assert_eq!(
            settings.menu.native_tray.order,
            crate::storage::NATIVE_TRAY_ITEMS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            settings.menu.tray_panel.order,
            crate::storage::TRAY_PANEL_ACTIONS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
        assert!(settings.menu.native_tray.hidden.is_empty());
        assert!(settings.menu.tray_panel.hidden.is_empty());
        assert!(!settings.notifications.enabled);
        assert_eq!(settings.notifications.warning_remaining_percent, 66);
        assert_eq!(settings.notifications.danger_remaining_percent, 33);
    }

    #[test]
    fn old_settings_json_without_notifications_loads_disabled_event_defaults() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"startAtLogin":true,"refreshIntervalSeconds":300,"displayMode":"remaining","theme":"system","language":"en-US"}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let notifications = repository.load().unwrap().notifications;
        assert!(!notifications.enabled);
        assert!(notifications.play_sound);
        assert!(notifications.warning_enabled);
        assert!(notifications.danger_enabled);
        assert!(notifications.weekly_reset_enabled);
        assert!(notifications.reset_credit_increase_enabled);
        assert!(notifications.refresh_failure_enabled);
        assert!(notifications.update_available_enabled);
        assert_eq!(notifications.warning_remaining_percent, 66);
        assert_eq!(notifications.danger_remaining_percent, 33);
        assert_eq!(
            repository.load().unwrap().taskbar_tray,
            TaskbarTrayPreferences::default()
        );
    }

    #[test]
    fn invalid_taskbar_tray_enum_uses_safe_settings_recovery_path() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![SETTINGS_KEY, r#"{"taskbarTray":{"density":"spacious"}}"#],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let error = repository.load().unwrap_err();

        assert_eq!(error.code(), "SETTINGS_DECODE_FAILED");
    }

    #[test]
    fn partial_notification_threshold_patch_validates_against_persisted_peer_atomically() {
        let (repository, _) = settings_fixture();
        let before = repository.load().unwrap();

        let error = repository
            .update(SettingsPatch {
                notifications: Some(NotificationPreferencesPatch {
                    warning_remaining_percent: Some(20),
                    danger_remaining_percent: Some(33),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap_err();

        assert_eq!(error.code(), "SETTINGS_NOTIFICATION_THRESHOLDS_INVALID");
        assert_eq!(repository.load().unwrap(), before);
    }

    #[test]
    fn partial_taskbar_tray_patch_preserves_unpatched_preferences() {
        let (repository, _) = settings_fixture();
        repository
            .update(SettingsPatch {
                taskbar_tray: Some(TaskbarTrayPreferencesPatch {
                    density: Some(TaskbarDensity::Standard),
                    tooltip_account: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();

        let updated = repository
            .update(SettingsPatch {
                taskbar_tray: Some(TaskbarTrayPreferencesPatch {
                    show_weekly_percent: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();

        assert!(!updated.taskbar_tray.show_weekly_percent);
        assert_eq!(updated.taskbar_tray.density, TaskbarDensity::Standard);
        assert!(!updated.taskbar_tray.tooltip_account);
        assert!(updated.taskbar_tray.show_taskbar_icon);
    }

    #[test]
    fn new_surface_settings_default_taskbar_disabled_float_enabled() {
        let settings = AppSettings::default();
        assert!(!settings.taskbar_status_enabled);
        assert!(settings.float_ball_enabled);
    }

    #[test]
    fn old_settings_json_without_surface_fields_loads_with_new_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let database =
            Arc::new(crate::storage::AppDatabase::open(&dir.path().join("settings.db")).unwrap());
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        rusqlite::params![
                            SETTINGS_KEY,
                            r#"{"startAtLogin":true,"refreshIntervalSeconds":900,"displayMode":"used","theme":"dark","language":"en-US","codexExecutableOverride":null}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let settings = SettingsRepository::new(database).load().unwrap();
        assert!(settings.start_at_login);
        assert_eq!(settings.refresh_interval_seconds, 900);
        assert!(!settings.taskbar_status_enabled);
        assert!(settings.float_ball_enabled);
    }

    #[test]
    fn old_settings_json_without_menu_layouts_loads_default_registry_orders() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"startAtLogin":true,"refreshIntervalSeconds":300,"displayMode":"remaining","theme":"system","language":"en-US"}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let settings = repository.load().unwrap();
        assert_eq!(
            settings.menu.native_tray.order,
            crate::storage::NATIVE_TRAY_ITEMS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            settings.menu.tray_panel.order,
            crate::storage::TRAY_PANEL_ACTIONS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn malformed_saved_menu_layouts_are_normalized_on_load_without_crashing() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"menu":{"nativeTray":{"order":["quit","unknown","refresh","refresh"],"hidden":["settings","refresh"]}}}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let settings = repository.load().unwrap();
        assert_eq!(
            settings.menu.native_tray.order,
            vec![
                "quit".to_string(),
                "settings".to_string(),
                "open_panel".to_string(),
                "accounts".to_string(),
                "open_usage".to_string(),
                "about".to_string(),
            ]
        );
        assert!(
            settings
                .menu
                .native_tray
                .order
                .contains(&"quit".to_string())
        );
    }

    #[test]
    fn partial_menu_patch_preserves_peer_surface_layout_in_repository() {
        let (repository, _) = settings_fixture();
        let updated = repository
            .update(SettingsPatch {
                menu: Some(MenuPreferencesPatch {
                    native_tray: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into(), "about".into(), "settings".into()]),
                        hidden: None,
                    }),
                    tray_panel: None,
                }),
                ..SettingsPatch::default()
            })
            .unwrap();

        assert_eq!(
            updated.menu.native_tray.order,
            vec![
                "quit".to_string(),
                "about".to_string(),
                "settings".to_string(),
                "open_panel".to_string(),
                "refresh".to_string(),
                "accounts".to_string(),
                "open_usage".to_string(),
            ]
        );
        assert_eq!(
            updated.menu.tray_panel.order,
            crate::storage::TRAY_PANEL_ACTIONS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );

        let reloaded = repository.load().unwrap();
        assert_eq!(reloaded.menu, updated.menu);
    }

    #[test]
    fn menu_preview_update_never_writes_to_storage() {
        let (repository, _) = settings_fixture();
        let old = repository.load().unwrap();
        let candidate = repository
            .preview_update(SettingsPatch {
                menu: Some(MenuPreferencesPatch {
                    native_tray: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into(), "settings".into()]),
                        hidden: None,
                    }),
                    tray_panel: None,
                }),
                ..SettingsPatch::default()
            })
            .unwrap();

        assert_ne!(candidate.menu.native_tray.order, old.menu.native_tray.order);
        assert_eq!(repository.load().unwrap(), old);
    }

    fn settings_fixture() -> (SettingsRepository, Arc<AppDatabase>) {
        let dir = tempfile::tempdir().unwrap();
        let database = Arc::new(AppDatabase::open(&dir.path().join("settings.db")).unwrap());
        // The database retains an open handle; keep its temporary parent until
        // the test process exits.
        std::mem::forget(dir);
        (SettingsRepository::new(Arc::clone(&database)), database)
    }

    #[test]
    fn old_settings_json_defaults_both_opacities_to_twenty() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![SETTINGS_KEY, r#"{"startAtLogin":true,"refreshIntervalSeconds":300,"displayMode":"remaining","theme":"system","language":"zh-CN"}"#],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();
        let settings = repository.load().unwrap();
        assert_eq!(settings.taskbar_status_opacity, 20);
        assert_eq!(settings.float_ball_opacity, 20);
    }

    #[test]
    fn opacity_bounds_are_inclusive_and_invalid_patch_is_atomic() {
        let (repository, _) = settings_fixture();
        let saved = repository
            .update(SettingsPatch {
                taskbar_status_opacity: Some(0),
                float_ball_opacity: Some(80),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert_eq!(
            (saved.taskbar_status_opacity, saved.float_ball_opacity),
            (0, 80)
        );
        let error = repository
            .update(SettingsPatch {
                taskbar_status_opacity: Some(81),
                float_ball_opacity: Some(10),
                ..SettingsPatch::default()
            })
            .unwrap_err();
        assert_eq!(error.code(), "SETTINGS_SURFACE_OPACITY_INVALID");
        let reloaded = repository.load().unwrap();
        assert_eq!(
            (reloaded.taskbar_status_opacity, reloaded.float_ball_opacity),
            (0, 80)
        );
    }

    #[test]
    fn partial_opacity_patch_preserves_enable_flags_and_peer_opacity() {
        let (repository, _) = settings_fixture();
        repository
            .update(SettingsPatch {
                taskbar_status_enabled: Some(true),
                float_ball_enabled: Some(true),
                float_ball_opacity: Some(60),
                ..SettingsPatch::default()
            })
            .unwrap();
        let saved = repository
            .update(SettingsPatch {
                taskbar_status_opacity: Some(35),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert!(saved.taskbar_status_enabled && saved.float_ball_enabled);
        assert_eq!(saved.taskbar_status_opacity, 35);
        assert_eq!(saved.float_ball_opacity, 60);
    }

    #[test]
    fn partial_surface_patch_changes_only_requested_flag() {
        let dir = tempfile::tempdir().unwrap();
        let database =
            Arc::new(crate::storage::AppDatabase::open(&dir.path().join("settings.db")).unwrap());
        let repository = SettingsRepository::new(database);

        let first = repository
            .update(SettingsPatch {
                taskbar_status_enabled: Some(true),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert!(first.taskbar_status_enabled);
        assert!(first.float_ball_enabled);

        let second = repository
            .update(SettingsPatch {
                float_ball_enabled: Some(true),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert!(second.taskbar_status_enabled);
        assert!(second.float_ball_enabled);
    }

    #[test]
    fn invalid_patch_does_not_replace_last_saved_settings() {
        let dir = tempfile::tempdir().unwrap();
        let database =
            Arc::new(crate::storage::AppDatabase::open(&dir.path().join("settings.db")).unwrap());
        let repository = SettingsRepository::new(database);

        let updated = repository
            .update(SettingsPatch {
                refresh_interval_seconds: Some(900),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert_eq!(updated.refresh_interval_seconds, 900);

        let error = repository
            .update(SettingsPatch {
                refresh_interval_seconds: Some(61),
                ..SettingsPatch::default()
            })
            .unwrap_err();
        assert_eq!(error.code(), "SETTINGS_REFRESH_INTERVAL_INVALID");
        assert_eq!(repository.load().unwrap().refresh_interval_seconds, 900);
    }

    #[test]
    fn v1_language_and_theme_choices_are_only_system_simplified_and_english() {
        assert_eq!(
            LanguagePreference::ALL,
            [
                LanguagePreference::System,
                LanguagePreference::ZhCn,
                LanguagePreference::EnUs
            ]
        );
        assert_eq!(
            ThemePreference::ALL,
            [
                ThemePreference::System,
                ThemePreference::Light,
                ThemePreference::Dark
            ]
        );
    }
}
