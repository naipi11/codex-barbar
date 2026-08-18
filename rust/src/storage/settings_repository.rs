//! Persistent V1 application settings.

use std::sync::Arc;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{AppDatabase, StorageError};

const SETTINGS_KEY: &str = "app_settings";
const ALLOWED_REFRESH_INTERVALS: [u64; 5] = [0, 60, 300, 900, 1800];
const DEFAULT_SURFACE_OPACITY: u8 = 20;
const MAX_SURFACE_OPACITY: u8 = 80;

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
        for value in [self.taskbar_status_opacity, self.float_ball_opacity]
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
        Ok(())
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
    }
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
    serde_json::from_str(&value).map_err(|error| {
        StorageError::new(
            crate::core::AppErrorKind::StorageFailure,
            "SETTINGS_DECODE_FAILED",
            error.to_string(),
        )
    })
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
    use std::sync::Arc;

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
