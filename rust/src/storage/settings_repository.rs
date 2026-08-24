//! Persistent V2 application settings.

use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{
    AppDatabase, MenuLayout, MenuPreferences, MenuPreferencesPatch, StorageError,
    migrate_settings_json, normalize_panel_actions,
};

const SETTINGS_KEY: &str = "app_settings";
const ALLOWED_REFRESH_INTERVALS: [u64; 5] = [0, 60, 300, 900, 1800];
const DEFAULT_SURFACE_TRANSPARENCY_PERCENT: u8 = 25;
const MAX_PERCENT: u8 = 100;
const MAX_NOTIFICATION_PERCENT: u8 = 100;

const fn default_surface_transparency_percent() -> u8 {
    DEFAULT_SURFACE_TRANSPARENCY_PERCENT
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

/// Persisted taskbar controls that remain part of the product surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskbarPresentationPreferences {
    pub show_taskbar_icon: bool,
    pub show_taskbar_account: bool,
    pub show_weekly_label: bool,
    pub show_weekly_percent: bool,
    pub show_reset_date: bool,
    pub density: TaskbarDensity,
    pub hide_status_surfaces_in_fullscreen: bool,
}

impl Default for TaskbarPresentationPreferences {
    fn default() -> Self {
        let legacy = TaskbarTrayPreferences::default();
        Self {
            show_taskbar_icon: legacy.show_taskbar_icon,
            show_taskbar_account: legacy.show_taskbar_account,
            show_weekly_label: legacy.show_weekly_label,
            show_weekly_percent: legacy.show_weekly_percent,
            show_reset_date: legacy.show_reset_date,
            density: legacy.density,
            hide_status_surfaces_in_fullscreen: legacy.hide_status_surfaces_in_fullscreen,
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

/// Visual preferences shared by the taskbar capsule and floating ball.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SurfaceAppearancePreferences {
    #[serde(default = "default_surface_transparency_percent")]
    pub taskbar_transparency_percent: u8,
    #[serde(default = "default_surface_transparency_percent")]
    pub float_ball_transparency_percent: u8,
    #[serde(default = "default_surface_transparency_percent")]
    pub float_ball_glow_percent: u8,
}

impl Default for SurfaceAppearancePreferences {
    fn default() -> Self {
        Self {
            taskbar_transparency_percent: DEFAULT_SURFACE_TRANSPARENCY_PERCENT,
            float_ball_transparency_percent: DEFAULT_SURFACE_TRANSPARENCY_PERCENT,
            float_ball_glow_percent: DEFAULT_SURFACE_TRANSPARENCY_PERCENT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelDensity {
    Compact,
    Standard,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelPreferences {
    pub density: PanelDensity,
    pub show_reset_time: bool,
    pub show_freshness: bool,
    pub show_account_status: bool,
    pub actions: MenuLayout,
}

impl Default for PanelPreferences {
    fn default() -> Self {
        Self {
            density: PanelDensity::Compact,
            show_reset_time: true,
            show_freshness: true,
            show_account_status: true,
            actions: MenuLayout {
                order: super::default_visible_order(&super::PANEL_ACTIONS),
                hidden: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u8,
    pub start_at_login: bool,
    pub refresh_interval_seconds: u64,
    pub display_mode: DisplayMode,
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub codex_executable_override: Option<String>,
    pub taskbar_status_enabled: bool,
    pub float_ball_enabled: bool,
    #[serde(flatten)]
    pub surface_appearance: SurfaceAppearancePreferences,
    #[serde(skip, default)]
    #[doc(hidden)]
    pub taskbar_status_opacity: u8,
    #[serde(skip, default)]
    #[doc(hidden)]
    pub float_ball_opacity: u8,
    #[serde(skip, default)]
    #[doc(hidden)]
    pub float_ball_glow: u8,
    pub notifications: NotificationPreferences,
    pub taskbar_presentation: TaskbarPresentationPreferences,
    #[serde(skip, default)]
    #[doc(hidden)]
    pub taskbar_tray: TaskbarTrayPreferences,
    #[serde(skip, default)]
    #[doc(hidden)]
    pub menu: MenuPreferences,
    pub panel: PanelPreferences,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: super::SETTINGS_SCHEMA_VERSION,
            start_at_login: true,
            refresh_interval_seconds: 300,
            display_mode: DisplayMode::Remaining,
            theme: ThemePreference::System,
            language: LanguagePreference::System,
            codex_executable_override: None,
            taskbar_status_enabled: false,
            float_ball_enabled: true,
            surface_appearance: SurfaceAppearancePreferences::default(),
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
            float_ball_glow: 20,
            notifications: NotificationPreferences::default(),
            taskbar_presentation: TaskbarPresentationPreferences::default(),
            taskbar_tray: TaskbarTrayPreferences::default(),
            menu: MenuPreferences::default(),
            panel: PanelPreferences::default(),
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
    pub taskbar_transparency_percent: Option<u8>,
    pub float_ball_transparency_percent: Option<u8>,
    pub float_ball_glow_percent: Option<u8>,
    #[doc(hidden)]
    pub taskbar_status_opacity: Option<u8>,
    #[doc(hidden)]
    pub float_ball_opacity: Option<u8>,
    #[doc(hidden)]
    pub float_ball_glow: Option<u8>,
    pub notifications: Option<NotificationPreferencesPatch>,
    #[doc(hidden)]
    pub taskbar_tray: Option<TaskbarTrayPreferencesPatch>,
    #[doc(hidden)]
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
            self.taskbar_transparency_percent,
            self.float_ball_transparency_percent,
            self.float_ball_glow_percent,
        ]
        .into_iter()
        .flatten()
        {
            validate_percent(value, "SETTINGS_SURFACE_TRANSPARENCY_INVALID")?;
        }
        for value in [
            self.taskbar_status_opacity,
            self.float_ball_opacity,
            self.float_ball_glow,
        ]
        .into_iter()
        .flatten()
        {
            if value > 80 {
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
    pub fn apply_to(self, preferences: &mut TaskbarTrayPreferences) {
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

    fn apply_to_taskbar_presentation(self, preferences: &mut TaskbarPresentationPreferences) {
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
            self.surface_appearance.taskbar_transparency_percent = scale_legacy_percent(value);
        }
        if let Some(value) = patch.float_ball_opacity {
            self.surface_appearance.float_ball_transparency_percent = scale_legacy_percent(value);
        }
        if let Some(value) = patch.float_ball_glow {
            self.surface_appearance.float_ball_glow_percent = scale_legacy_percent(value);
        }
        if let Some(value) = patch.taskbar_transparency_percent {
            self.surface_appearance.taskbar_transparency_percent = value;
        }
        if let Some(value) = patch.float_ball_transparency_percent {
            self.surface_appearance.float_ball_transparency_percent = value;
        }
        if let Some(value) = patch.float_ball_glow_percent {
            self.surface_appearance.float_ball_glow_percent = value;
        }
        if let Some(notifications) = patch.notifications {
            notifications.apply_to(&mut self.notifications);
        }
        if let Some(taskbar_tray) = patch.taskbar_tray {
            taskbar_tray.apply_to_taskbar_presentation(&mut self.taskbar_presentation);
        }
        if let Some(menu) = patch.menu {
            if let Some(tray_panel) = menu.tray_panel {
                tray_panel.apply_to(&mut self.panel.actions);
            }
        }
    }

    fn normalize(&mut self) {
        self.schema_version = super::SETTINGS_SCHEMA_VERSION;
        self.surface_appearance.taskbar_transparency_percent = self
            .surface_appearance
            .taskbar_transparency_percent
            .min(MAX_PERCENT);
        self.surface_appearance.float_ball_transparency_percent = self
            .surface_appearance
            .float_ball_transparency_percent
            .min(MAX_PERCENT);
        self.surface_appearance.float_ball_glow_percent = self
            .surface_appearance
            .float_ball_glow_percent
            .min(MAX_PERCENT);
        if self.panel.density == PanelDensity::Unknown {
            self.panel.density = PanelDensity::Compact;
        }
        normalize_panel_actions(&mut self.panel.actions);
        self.taskbar_status_opacity =
            scale_v2_percent(self.surface_appearance.taskbar_transparency_percent);
        self.float_ball_opacity =
            scale_v2_percent(self.surface_appearance.float_ball_transparency_percent);
        self.float_ball_glow = scale_v2_percent(self.surface_appearance.float_ball_glow_percent);
        self.taskbar_tray = TaskbarTrayPreferences {
            show_taskbar_icon: self.taskbar_presentation.show_taskbar_icon,
            show_taskbar_account: self.taskbar_presentation.show_taskbar_account,
            show_weekly_label: self.taskbar_presentation.show_weekly_label,
            show_weekly_percent: self.taskbar_presentation.show_weekly_percent,
            show_reset_date: self.taskbar_presentation.show_reset_date,
            density: self.taskbar_presentation.density,
            hide_status_surfaces_in_fullscreen: self
                .taskbar_presentation
                .hide_status_surfaces_in_fullscreen,
            ..TaskbarTrayPreferences::default()
        };
        self.menu = MenuPreferences {
            native_tray: MenuPreferences::default().native_tray,
            tray_panel: self.panel.actions.clone(),
        };
    }

    fn validate(&self) -> Result<(), StorageError> {
        validate_percent(
            self.surface_appearance.taskbar_transparency_percent,
            "SETTINGS_SURFACE_TRANSPARENCY_INVALID",
        )?;
        validate_percent(
            self.surface_appearance.float_ball_transparency_percent,
            "SETTINGS_SURFACE_TRANSPARENCY_INVALID",
        )?;
        validate_percent(
            self.surface_appearance.float_ball_glow_percent,
            "SETTINGS_SURFACE_TRANSPARENCY_INVALID",
        )?;
        if self.notifications.danger_remaining_percent
            >= self.notifications.warning_remaining_percent
            || self.notifications.warning_remaining_percent > MAX_NOTIFICATION_PERCENT
        {
            return Err(notification_thresholds_error());
        }
        Ok(())
    }
}

fn scale_legacy_percent(value: u8) -> u8 {
    (((u16::from(value) * 100) + 40) / 80) as u8
}

fn scale_v2_percent(value: u8) -> u8 {
    (((u16::from(value) * 80) + 50) / 100) as u8
}

fn validate_percent(value: u8, code: &'static str) -> Result<(), StorageError> {
    (value <= MAX_PERCENT).then_some(()).ok_or_else(|| {
        StorageError::new(
            crate::core::AppErrorKind::StorageFailure,
            code,
            "percentage must be between 0 and 100",
        )
    })
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
    let value = serde_json::from_str(&value).map_err(|error| {
        StorageError::new(
            crate::core::AppErrorKind::StorageFailure,
            "SETTINGS_DECODE_FAILED",
            error.to_string(),
        )
    })?;
    let (value, _) = migrate_settings_json(value)?;
    let mut settings = serde_json::from_value::<AppSettings>(value).map_err(|error| {
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
    use crate::storage::{MenuLayoutPatch, SETTINGS_SCHEMA_VERSION, migrate_settings_json};
    use std::sync::Arc;

    #[test]
    fn schema_v1_visual_values_scale_once_to_v2() {
        let input = serde_json::json!({
            "taskbarStatusOpacity": 80,
            "floatBallOpacity": 20,
            "floatBallGlow": 0,
            "taskbarTray": {
                "showTaskbarAccount": false,
                "density": "standard",
                "tooltipAccount": false,
            },
        });

        let (migrated, changed) = migrate_settings_json(input).unwrap();

        assert!(changed);
        assert_eq!(migrated["taskbarTransparencyPercent"], 100);
        assert_eq!(migrated["floatBallTransparencyPercent"], 25);
        assert_eq!(migrated["floatBallGlowPercent"], 0);
        assert_eq!(migrated["taskbarPresentation"]["showTaskbarAccount"], false);
        assert_eq!(migrated["taskbarPresentation"]["density"], "standard");
        assert!(migrated.get("taskbarTray").is_none());
        assert_eq!(migrated["schemaVersion"], SETTINGS_SCHEMA_VERSION);

        let (reloaded, changed_again) = migrate_settings_json(migrated).unwrap();
        assert!(!changed_again);
        assert_eq!(reloaded["taskbarTransparencyPercent"], 100);
    }

    #[test]
    fn persisted_v2_settings_exclude_legacy_visual_tray_and_menu_keys() {
        let (repository, database) = settings_fixture();
        repository
            .update(SettingsPatch {
                taskbar_transparency_percent: Some(50),
                ..SettingsPatch::default()
            })
            .unwrap();

        let persisted = database
            .with_connection(|connection| {
                let value = connection
                    .query_row(
                        "SELECT value_json FROM app_settings WHERE key = ?1",
                        params![SETTINGS_KEY],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(storage_error)?;
                Ok(serde_json::from_str::<serde_json::Value>(&value).unwrap())
            })
            .unwrap();

        assert_eq!(persisted["taskbarTransparencyPercent"], 50);
        assert!(persisted.get("taskbarStatusOpacity").is_none());
        assert!(persisted.get("floatBallOpacity").is_none());
        assert!(persisted.get("floatBallGlow").is_none());
        assert!(persisted.get("taskbarTray").is_none());
        assert!(persisted.get("menu").is_none());
    }

    #[test]
    fn legacy_patch_adapts_visual_taskbar_and_panel_state_into_v2_preferences() {
        let (repository, _) = settings_fixture();
        let updated = repository
            .update(SettingsPatch {
                taskbar_status_opacity: Some(40),
                float_ball_opacity: Some(20),
                float_ball_glow: Some(80),
                taskbar_tray: Some(TaskbarTrayPreferencesPatch {
                    show_taskbar_account: Some(false),
                    density: Some(TaskbarDensity::Standard),
                    hide_status_surfaces_in_fullscreen: Some(false),
                    tooltip_account: Some(false),
                    ..Default::default()
                }),
                menu: Some(MenuPreferencesPatch {
                    native_tray: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into()]),
                        hidden: None,
                    }),
                    tray_panel: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into()]),
                        hidden: Some(vec!["refresh".into()]),
                    }),
                }),
                ..SettingsPatch::default()
            })
            .unwrap();

        assert_eq!(updated.surface_appearance.taskbar_transparency_percent, 50);
        assert_eq!(
            updated.surface_appearance.float_ball_transparency_percent,
            25
        );
        assert_eq!(updated.surface_appearance.float_ball_glow_percent, 100);
        assert!(!updated.taskbar_presentation.show_taskbar_account);
        assert_eq!(
            updated.taskbar_presentation.density,
            TaskbarDensity::Standard
        );
        assert!(
            !updated
                .taskbar_presentation
                .hide_status_surfaces_in_fullscreen
        );
        assert_eq!(
            updated.panel.actions.order.first().map(String::as_str),
            Some("refresh")
        );
        assert!(updated.panel.actions.order.contains(&"quit".to_string()));
    }

    #[test]
    fn tray_only_and_native_menu_patches_do_not_change_or_persist_v2_preferences() {
        let (repository, database) = settings_fixture();
        let before = repository.load().unwrap();
        let updated = repository
            .update(SettingsPatch {
                taskbar_tray: Some(TaskbarTrayPreferencesPatch {
                    tray_icon_mode: Some(TrayIconMode::Monochrome),
                    tooltip_account: Some(false),
                    ..Default::default()
                }),
                menu: Some(MenuPreferencesPatch {
                    native_tray: Some(MenuLayoutPatch {
                        order: Some(vec!["quit".into()]),
                        hidden: None,
                    }),
                    tray_panel: None,
                }),
                ..SettingsPatch::default()
            })
            .unwrap();

        assert_eq!(updated.surface_appearance, before.surface_appearance);
        assert_eq!(updated.taskbar_presentation, before.taskbar_presentation);
        assert_eq!(updated.panel, before.panel);

        let persisted = database
            .with_connection(|connection| {
                let value = connection
                    .query_row(
                        "SELECT value_json FROM app_settings WHERE key = ?1",
                        params![SETTINGS_KEY],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(storage_error)?;
                Ok(serde_json::from_str::<serde_json::Value>(&value).unwrap())
            })
            .unwrap();
        assert!(persisted.get("taskbarTray").is_none());
        assert!(persisted.get("menu").is_none());
    }
    #[test]
    fn expanded_settings_json_round_trips_v2_preferences() {
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
                taskbar_transparency_percent: Some(0),
                float_ball_transparency_percent: Some(100),
                float_ball_glow_percent: Some(50),
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
                ..SettingsPatch::default()
            })
            .unwrap();

        let reloaded = repository.load().unwrap();
        assert_eq!(reloaded, updated);
        assert_eq!(reloaded.theme, ThemePreference::Dark);
        assert_eq!(reloaded.language, LanguagePreference::ZhCn);
        assert_eq!(reloaded.notifications.warning_remaining_percent, 70);
        assert_eq!(reloaded.surface_appearance.float_ball_glow_percent, 50);
        assert_eq!(
            reloaded.panel.actions.order.first().map(String::as_str),
            Some("refresh")
        );
        let serialized = serde_json::to_value(updated).unwrap();
        assert_eq!(serialized["taskbarTransparencyPercent"], 0);
        assert!(serialized.get("taskbarStatusOpacity").is_none());
        assert!(serialized.get("menu").is_none());
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
    fn legacy_panel_layout_normalizes_without_crashing() {
        let (repository, database) = settings_fixture();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)",
                        params![
                            SETTINGS_KEY,
                            r#"{"menu":{"nativeTray":{"order":["quit","unknown"]},"trayPanel":{"order":["dismiss","quit"],"hidden":["refresh"]}}}"#
                        ],
                    )
                    .map_err(storage_error)?;
                Ok(())
            })
            .unwrap();

        let settings = repository.load().unwrap();
        assert_eq!(
            settings.panel.actions.order,
            vec![
                "refresh".to_string(),
                "dismiss".to_string(),
                "quit".to_string(),
                "open_usage".to_string(),
                "settings".to_string(),
            ]
        );
    }

    #[test]
    fn v2_settings_defaults_are_exact() {
        let settings = AppSettings::default();
        assert_eq!(settings.refresh_interval_seconds, 300);
        assert_eq!(settings.display_mode, DisplayMode::Remaining);
        assert_eq!(settings.theme, ThemePreference::System);
        assert_eq!(settings.language, LanguagePreference::System);
        assert!(settings.start_at_login);
        assert!(!settings.taskbar_status_enabled);
        assert!(settings.float_ball_enabled);
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.surface_appearance.taskbar_transparency_percent, 25);
        assert_eq!(
            settings.surface_appearance.float_ball_transparency_percent,
            25
        );
        assert_eq!(settings.surface_appearance.float_ball_glow_percent, 25);
        assert_eq!(settings.panel.density, PanelDensity::Compact);
        assert_eq!(
            settings.panel.actions.order,
            crate::storage::PANEL_ACTIONS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
        assert!(settings.panel.actions.hidden.is_empty());
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
            repository.load().unwrap().schema_version,
            SETTINGS_SCHEMA_VERSION
        );
    }

    #[test]
    fn obsolete_tray_data_is_ignored() {
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

        assert_eq!(
            repository.load().unwrap().panel,
            PanelPreferences::default()
        );
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
    fn old_settings_json_without_menu_layouts_loads_default_panel_actions() {
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
            settings.panel.actions.order,
            crate::storage::PANEL_ACTIONS
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn legacy_native_menu_layout_is_ignored_on_load() {
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
        assert_eq!(settings.panel.actions, PanelPreferences::default().actions);
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
    fn old_settings_json_defaults_visual_percentages_to_twenty_five() {
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
        assert_eq!(settings.surface_appearance.taskbar_transparency_percent, 25);
        assert_eq!(
            settings.surface_appearance.float_ball_transparency_percent,
            25
        );
    }

    #[test]
    fn transparency_percent_bounds_are_inclusive_and_invalid_patch_is_atomic() {
        let (repository, _) = settings_fixture();
        let saved = repository
            .update(SettingsPatch {
                taskbar_transparency_percent: Some(0),
                float_ball_transparency_percent: Some(100),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert_eq!(
            (
                saved.surface_appearance.taskbar_transparency_percent,
                saved.surface_appearance.float_ball_transparency_percent,
            ),
            (0, 100)
        );
        let error = repository
            .update(SettingsPatch {
                taskbar_transparency_percent: Some(101),
                float_ball_transparency_percent: Some(10),
                ..SettingsPatch::default()
            })
            .unwrap_err();
        assert_eq!(error.code(), "SETTINGS_SURFACE_TRANSPARENCY_INVALID");
        let reloaded = repository.load().unwrap();
        assert_eq!(
            (
                reloaded.surface_appearance.taskbar_transparency_percent,
                reloaded.surface_appearance.float_ball_transparency_percent,
            ),
            (0, 100)
        );
    }

    #[test]
    fn partial_transparency_patch_preserves_enable_flags_and_peer_value() {
        let (repository, _) = settings_fixture();
        repository
            .update(SettingsPatch {
                taskbar_status_enabled: Some(true),
                float_ball_enabled: Some(true),
                float_ball_transparency_percent: Some(60),
                ..SettingsPatch::default()
            })
            .unwrap();
        let saved = repository
            .update(SettingsPatch {
                taskbar_transparency_percent: Some(35),
                ..SettingsPatch::default()
            })
            .unwrap();
        assert!(saved.taskbar_status_enabled && saved.float_ball_enabled);
        assert_eq!(saved.surface_appearance.taskbar_transparency_percent, 35);
        assert_eq!(saved.surface_appearance.float_ball_transparency_percent, 60);
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
