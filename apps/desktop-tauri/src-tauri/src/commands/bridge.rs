//! Frozen, redacted DTOs and the non-account bridge commands.
//!
//! This module is deliberately the narrow boundary between the Rust
//! application state and React.  It contains no token, cookie, raw auth file,
//! arbitrary process, or arbitrary URL/path payload.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use codexbar::accounts::identity::{AccountIdentityRecord, AccountStatus};
use codexbar::accounts::model::{
    AccountProfile, AccountProfilesSnapshot, ManagedLoginStage, ManagedLoginStatus, ProfileKind,
};
use codexbar::core::{
    AppError, AppErrorKind, AuthMode, Freshness, ProfileUsageState, RefreshStatus, UsageWindow,
};
use codexbar::storage::{
    AppSettings, DisplayMode, LanguagePreference, MenuPreferences, MenuPreferencesPatch,
    NotificationPreferences, NotificationPreferencesPatch, SettingsPatch, TaskbarDensity,
    TaskbarTrayPreferences, TaskbarTrayPreferencesPatch, ThemePreference, TrayIconMode,
};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub kind: &'static str,
    pub user_message_key: String,
    pub action: &'static str,
    pub retry_after: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub autostart_enabled: bool,
    pub refresh_interval_seconds: u64,
    pub display_mode: &'static str,
    pub theme: &'static str,
    pub language: &'static str,
    pub codex_executable_override: Option<String>,
    pub taskbar_status_enabled: bool,
    pub float_ball_enabled: bool,
    pub taskbar_status_opacity: u8,
    pub float_ball_opacity: u8,
    pub float_ball_glow: u8,
    pub notifications: NotificationPreferencesDto,
    pub taskbar_tray: TaskbarTrayPreferencesDto,
    pub menu: MenuPreferencesDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuLayoutDto {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPreferencesDto {
    pub native_tray: MenuLayoutDto,
    pub tray_panel: MenuLayoutDto,
}

impl MenuPreferencesDto {
    fn from_preferences(preferences: &MenuPreferences) -> Self {
        Self {
            native_tray: MenuLayoutDto {
                order: preferences.native_tray.order.clone(),
                hidden: preferences.native_tray.hidden.clone(),
            },
            tray_panel: MenuLayoutDto {
                order: preferences.tray_panel.order.clone(),
                hidden: preferences.tray_panel.hidden.clone(),
            },
        }
    }
}

impl Default for MenuPreferencesDto {
    fn default() -> Self {
        Self::from_preferences(&MenuPreferences::default())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPreferencesDto {
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

impl NotificationPreferencesDto {
    fn from_preferences(preferences: &NotificationPreferences) -> Self {
        Self {
            enabled: preferences.enabled,
            play_sound: preferences.play_sound,
            warning_enabled: preferences.warning_enabled,
            danger_enabled: preferences.danger_enabled,
            weekly_reset_enabled: preferences.weekly_reset_enabled,
            reset_credit_increase_enabled: preferences.reset_credit_increase_enabled,
            refresh_failure_enabled: preferences.refresh_failure_enabled,
            update_available_enabled: preferences.update_available_enabled,
            warning_remaining_percent: preferences.warning_remaining_percent,
            danger_remaining_percent: preferences.danger_remaining_percent,
        }
    }
}

impl Default for NotificationPreferencesDto {
    fn default() -> Self {
        Self::from_preferences(&NotificationPreferences::default())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarTrayPreferencesDto {
    pub show_taskbar_icon: bool,
    pub show_taskbar_account: bool,
    pub show_weekly_label: bool,
    pub show_weekly_percent: bool,
    pub show_reset_date: bool,
    pub density: &'static str,
    pub tray_icon_mode: &'static str,
    pub tooltip_account: bool,
    pub tooltip_weekly: bool,
    pub tooltip_reset_date: bool,
    pub tooltip_updated_at: bool,
    pub hide_status_surfaces_in_fullscreen: bool,
}

impl TaskbarTrayPreferencesDto {
    fn from_preferences(preferences: &TaskbarTrayPreferences) -> Self {
        Self {
            show_taskbar_icon: preferences.show_taskbar_icon,
            show_taskbar_account: preferences.show_taskbar_account,
            show_weekly_label: preferences.show_weekly_label,
            show_weekly_percent: preferences.show_weekly_percent,
            show_reset_date: preferences.show_reset_date,
            density: match preferences.density {
                TaskbarDensity::Compact => "compact",
                TaskbarDensity::Standard => "standard",
            },
            tray_icon_mode: match preferences.tray_icon_mode {
                TrayIconMode::Dynamic => "dynamic",
                TrayIconMode::Monochrome => "monochrome",
            },
            tooltip_account: preferences.tooltip_account,
            tooltip_weekly: preferences.tooltip_weekly,
            tooltip_reset_date: preferences.tooltip_reset_date,
            tooltip_updated_at: preferences.tooltip_updated_at,
            hide_status_surfaces_in_fullscreen: preferences.hide_status_surfaces_in_fullscreen,
        }
    }
}

impl Default for TaskbarTrayPreferencesDto {
    fn default() -> Self {
        Self::from_preferences(&TaskbarTrayPreferences::default())
    }
}

impl Default for AppSettingsDto {
    fn default() -> Self {
        Self {
            autostart_enabled: false,
            refresh_interval_seconds: 300,
            display_mode: "remaining",
            theme: "system",
            language: "system",
            codex_executable_override: None,
            taskbar_status_enabled: false,
            float_ball_enabled: false,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
            float_ball_glow: 20,
            notifications: NotificationPreferencesDto::default(),
            taskbar_tray: TaskbarTrayPreferencesDto::default(),
            menu: MenuPreferencesDto::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatchDto {
    pub autostart_enabled: Option<bool>,
    pub refresh_interval_seconds: Option<u64>,
    pub display_mode: Option<String>,
    pub theme: Option<String>,
    pub language: Option<String>,
    pub codex_executable_override: Option<Option<String>>,
    pub taskbar_status_enabled: Option<bool>,
    pub float_ball_enabled: Option<bool>,
    pub taskbar_status_opacity: Option<u8>,
    pub float_ball_opacity: Option<u8>,
    pub float_ball_glow: Option<u8>,
    pub notifications: Option<NotificationPreferencesPatchDto>,
    pub taskbar_tray: Option<TaskbarTrayPreferencesPatchDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuLayoutPatchDto {
    pub order: Option<Vec<String>>,
    pub hidden: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPreferencesPatchDto {
    pub native_tray: Option<MenuLayoutPatchDto>,
    pub tray_panel: Option<MenuLayoutPatchDto>,
}

impl MenuPreferencesPatchDto {
    pub(crate) fn into_patch(self) -> MenuPreferencesPatch {
        MenuPreferencesPatch {
            native_tray: self
                .native_tray
                .map(|patch| codexbar::storage::MenuLayoutPatch {
                    order: patch.order,
                    hidden: patch.hidden,
                }),
            tray_panel: self
                .tray_panel
                .map(|patch| codexbar::storage::MenuLayoutPatch {
                    order: patch.order,
                    hidden: patch.hidden,
                }),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPreferencesPatchDto {
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskbarTrayPreferencesPatchDto {
    pub show_taskbar_icon: Option<bool>,
    pub show_taskbar_account: Option<bool>,
    pub show_weekly_label: Option<bool>,
    pub show_weekly_percent: Option<bool>,
    pub show_reset_date: Option<bool>,
    pub density: Option<String>,
    pub tray_icon_mode: Option<String>,
    pub tooltip_account: Option<bool>,
    pub tooltip_weekly: Option<bool>,
    pub tooltip_reset_date: Option<bool>,
    pub tooltip_updated_at: Option<bool>,
    pub hide_status_surfaces_in_fullscreen: Option<bool>,
}

impl AppSettingsDto {
    pub(crate) fn from_settings(settings: &AppSettings) -> Self {
        Self {
            autostart_enabled: settings.start_at_login,
            refresh_interval_seconds: settings.refresh_interval_seconds,
            display_mode: match settings.display_mode {
                DisplayMode::Remaining => "remaining",
                DisplayMode::Used => "used",
            },
            theme: match settings.theme {
                ThemePreference::System => "system",
                ThemePreference::Light => "light",
                ThemePreference::Dark => "dark",
            },
            language: match settings.language {
                LanguagePreference::System => "system",
                LanguagePreference::ZhCn => "zh-CN",
                LanguagePreference::EnUs => "en-US",
            },
            codex_executable_override: settings.codex_executable_override.clone(),
            taskbar_status_enabled: settings.taskbar_status_enabled,
            float_ball_enabled: settings.float_ball_enabled,
            taskbar_status_opacity: settings.taskbar_status_opacity,
            float_ball_opacity: settings.float_ball_opacity,
            float_ball_glow: settings.float_ball_glow,
            notifications: NotificationPreferencesDto::from_preferences(&settings.notifications),
            taskbar_tray: TaskbarTrayPreferencesDto::from_preferences(&settings.taskbar_tray),
            menu: MenuPreferencesDto::from_preferences(&settings.menu),
        }
    }
}

impl SettingsPatchDto {
    pub(crate) fn into_patch(self) -> Result<SettingsPatch, String> {
        let display_mode = self
            .display_mode
            .map(|value| match value.as_str() {
                "remaining" => Ok(DisplayMode::Remaining),
                "used" => Ok(DisplayMode::Used),
                _ => Err("unsupported display mode".to_string()),
            })
            .transpose()?;
        let theme = self
            .theme
            .map(|value| match value.as_str() {
                "system" => Ok(ThemePreference::System),
                "light" => Ok(ThemePreference::Light),
                "dark" => Ok(ThemePreference::Dark),
                _ => Err("unsupported theme".to_string()),
            })
            .transpose()?;
        let language = self
            .language
            .map(|value| match value.as_str() {
                "system" => Ok(LanguagePreference::System),
                "zh-CN" => Ok(LanguagePreference::ZhCn),
                "en-US" => Ok(LanguagePreference::EnUs),
                _ => Err("unsupported language".to_string()),
            })
            .transpose()?;
        let notifications = self
            .notifications
            .map(NotificationPreferencesPatchDto::into_patch)
            .transpose()?;
        let taskbar_tray = self
            .taskbar_tray
            .map(TaskbarTrayPreferencesPatchDto::into_patch)
            .transpose()?;
        Ok(SettingsPatch {
            start_at_login: self.autostart_enabled,
            refresh_interval_seconds: self.refresh_interval_seconds,
            display_mode,
            theme,
            language,
            codex_executable_override: self.codex_executable_override,
            taskbar_status_enabled: self.taskbar_status_enabled,
            float_ball_enabled: self.float_ball_enabled,
            taskbar_status_opacity: self.taskbar_status_opacity,
            float_ball_opacity: self.float_ball_opacity,
            float_ball_glow: self.float_ball_glow,
            notifications,
            taskbar_tray,
            menu: None,
        })
    }
}

impl NotificationPreferencesPatchDto {
    fn into_patch(self) -> Result<NotificationPreferencesPatch, String> {
        for value in [
            self.warning_remaining_percent,
            self.danger_remaining_percent,
        ]
        .into_iter()
        .flatten()
        {
            if value > 100 {
                return Err("notification threshold must be between 0 and 100".to_string());
            }
        }
        Ok(NotificationPreferencesPatch {
            enabled: self.enabled,
            play_sound: self.play_sound,
            warning_enabled: self.warning_enabled,
            danger_enabled: self.danger_enabled,
            weekly_reset_enabled: self.weekly_reset_enabled,
            reset_credit_increase_enabled: self.reset_credit_increase_enabled,
            refresh_failure_enabled: self.refresh_failure_enabled,
            update_available_enabled: self.update_available_enabled,
            warning_remaining_percent: self.warning_remaining_percent,
            danger_remaining_percent: self.danger_remaining_percent,
        })
    }
}

impl TaskbarTrayPreferencesPatchDto {
    fn into_patch(self) -> Result<TaskbarTrayPreferencesPatch, String> {
        let density = self
            .density
            .map(|value| match value.as_str() {
                "compact" => Ok(TaskbarDensity::Compact),
                "standard" => Ok(TaskbarDensity::Standard),
                _ => Err("unsupported taskbar density".to_string()),
            })
            .transpose()?;
        let tray_icon_mode = self
            .tray_icon_mode
            .map(|value| match value.as_str() {
                "dynamic" => Ok(TrayIconMode::Dynamic),
                "monochrome" => Ok(TrayIconMode::Monochrome),
                _ => Err("unsupported tray icon mode".to_string()),
            })
            .transpose()?;
        Ok(TaskbarTrayPreferencesPatch {
            show_taskbar_icon: self.show_taskbar_icon,
            show_taskbar_account: self.show_taskbar_account,
            show_weekly_label: self.show_weekly_label,
            show_weekly_percent: self.show_weekly_percent,
            show_reset_date: self.show_reset_date,
            density,
            tray_icon_mode,
            tooltip_account: self.tooltip_account,
            tooltip_weekly: self.tooltip_weekly,
            tooltip_reset_date: self.tooltip_reset_date,
            tooltip_updated_at: self.tooltip_updated_at,
            hide_status_surfaces_in_fullscreen: self.hide_status_surfaces_in_fullscreen,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummaryDto {
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub email: Option<String>,
    pub account_display_name: Option<String>,
    pub account_email: Option<String>,
    pub account_status: &'static str,
    pub account_updated_at: Option<String>,
    pub plan_type: Option<String>,
    pub auth_mode: &'static str,
    pub removable: bool,
    pub last_success_at: Option<String>,
}

impl ProfileSummaryDto {
    pub(crate) fn from_profile(
        profile: &AccountProfile,
        identity: Option<&AccountIdentityRecord>,
    ) -> Self {
        let account_display_name = identity.and_then(|value| value.display_name.clone());
        let account_email = identity.and_then(|value| value.email.clone());
        let account_status = identity
            .map(|value| account_status_name(value.status))
            .unwrap_or("unavailable");
        let account_updated_at = identity.map(|value| value.updated_at.to_rfc3339());
        let label = if profile.kind == ProfileKind::CurrentCli {
            account_display_name
                .clone()
                .or_else(|| account_email.clone())
                .unwrap_or_else(|| account_label_fallback(account_status).to_string())
        } else {
            profile.label.clone()
        };
        Self {
            id: profile.id.to_string(),
            kind: profile_kind_name(profile.kind),
            label,
            // The account repository stores only a one-way fingerprint.  The
            // clear-text email is intentionally unavailable at this boundary.
            email: None,
            account_display_name,
            account_email,
            account_status,
            account_updated_at,
            plan_type: identity.and_then(|value| value.plan_type.clone()),
            auth_mode: auth_mode_name(profile.auth_mode),
            removable: profile.kind == ProfileKind::Managed,
            last_success_at: profile.last_success_at.map(|value| value.to_rfc3339()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsSnapshotDto {
    pub profiles: Vec<ProfileSummaryDto>,
    pub selected_profile_id: String,
}

impl AccountsSnapshotDto {
    pub(crate) fn from_snapshot(
        snapshot: AccountProfilesSnapshot,
        identities: &BTreeMap<uuid::Uuid, AccountIdentityRecord>,
    ) -> Self {
        Self {
            profiles: snapshot
                .profiles
                .iter()
                .map(|profile| {
                    ProfileSummaryDto::from_profile(profile, identities.get(&profile.id))
                })
                .collect(),
            selected_profile_id: snapshot.selected_profile_id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindowDto {
    pub limit_id: String,
    pub label: Option<String>,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_duration_minutes: Option<u64>,
    pub resets_at: Option<String>,
    pub reached_type: Option<String>,
}

impl From<&UsageWindow> for UsageWindowDto {
    fn from(window: &UsageWindow) -> Self {
        Self {
            limit_id: window.limit_id.clone(),
            label: window.label.clone(),
            used_percent: window.used_percent,
            remaining_percent: window.remaining_percent,
            window_duration_minutes: window.window_duration_minutes,
            resets_at: window.resets_at.map(|value| value.to_rfc3339()),
            reached_type: window.reached_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageStateDto {
    pub profile_id: String,
    pub primary: Option<UsageWindowDto>,
    pub secondary: Option<UsageWindowDto>,
    pub additional_windows: Vec<UsageWindowDto>,
    pub fetched_at: Option<String>,
    pub current_error: Option<AppErrorDto>,
    pub freshness: &'static str,
    pub refresh_status: &'static str,
    pub manual_cooldown_until: Option<String>,
    pub protocol_anomaly: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCreditsStateDto {
    pub state: &'static str,
    pub available_count: Option<u64>,
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialUsageDto {
    pub remaining_percent: Option<u8>,
    pub resets_at: Option<String>,
    pub fetched_at: Option<String>,
    pub freshness: &'static str,
    pub reset_credits: ResetCreditsStateDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsageSpendDto {
    pub date: String,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageSpendDto {
    pub model: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalUsageSpendDto {
    pub attribution: &'static str,
    pub range: &'static str,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub sessions_count: u32,
    pub estimated_cost_usd: Option<f64>,
    pub unknown_models: Vec<String>,
    pub daily: Vec<DailyUsageSpendDto>,
    pub models: Vec<ModelUsageSpendDto>,
    pub state: &'static str,
    pub malformed_records_skipped: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSpendDto {
    pub official: OfficialUsageDto,
    pub local: LocalUsageSpendDto,
}

impl ProfileUsageStateDto {
    pub(crate) fn from_state(state: &ProfileUsageState) -> Self {
        let snapshot = state.snapshot.as_ref();
        Self {
            profile_id: state.profile_id.to_string(),
            primary: snapshot
                .and_then(|value| value.primary.as_ref())
                .map(UsageWindowDto::from),
            secondary: snapshot
                .and_then(|value| value.secondary.as_ref())
                .map(UsageWindowDto::from),
            additional_windows: snapshot
                .map(|value| {
                    value
                        .additional_windows
                        .iter()
                        .map(UsageWindowDto::from)
                        .collect()
                })
                .unwrap_or_default(),
            fetched_at: snapshot.map(|value| value.fetched_at.to_rfc3339()),
            current_error: state.current_error.as_ref().map(AppErrorDto::from_error),
            freshness: freshness_name(state.freshness),
            refresh_status: refresh_status_name(state.refresh_status),
            manual_cooldown_until: state.manual_cooldown_until.map(|value| value.to_rfc3339()),
            protocol_anomaly: snapshot.is_some_and(|value| value.protocol_anomaly),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCompatibilityDto {
    pub status: &'static str,
    pub installation: Option<&'static str>,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub capabilities: CodexCapabilitiesDto,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCapabilitiesDto {
    pub account_read: bool,
    pub rate_limits_read: bool,
    pub managed_login: bool,
}

impl Default for CodexCompatibilityDto {
    fn default() -> Self {
        Self {
            status: "notChecked",
            installation: None,
            executable_path: None,
            version: None,
            capabilities: CodexCapabilitiesDto {
                account_read: false,
                rate_limits_read: false,
                managed_login: false,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedLoginStateDto {
    pub operation_id: String,
    pub profile_id: String,
    pub stage: &'static str,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
    pub error_kind: Option<&'static str>,
}

impl From<&ManagedLoginStatus> for ManagedLoginStateDto {
    fn from(status: &ManagedLoginStatus) -> Self {
        Self {
            operation_id: status.operation_id.to_string(),
            profile_id: status.profile_id.to_string(),
            stage: login_stage_name(status.stage),
            verification_url: status.verification_url.clone(),
            user_code: status.user_code.clone(),
            error_kind: status.error_kind.map(error_kind_name),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDto {
    pub product_name: &'static str,
    pub version: String,
    pub settings: AppSettingsDto,
    pub status_surface_feedback: StatusSurfaceFeedbackDto,
    pub profiles: Vec<ProfileSummaryDto>,
    pub selected_profile_id: String,
    pub usage_by_profile: BTreeMap<String, ProfileUsageStateDto>,
    pub codex: CodexCompatibilityDto,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSurfaceFeedbackDto {
    pub taskbar_status_close_failed: bool,
    pub float_ball_close_failed: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSurfaceFeedbackChangedDto {
    pub surface: crate::status_surfaces::controller::StatusSurfaceKind,
    pub close_failed: bool,
}

/// Build the cache-first bootstrap payload.  A read-only/unavailable account
/// service yields an empty profile list rather than exposing storage details.
pub(crate) fn bootstrap_from_state(
    state: &AppState,
    status_surface_feedback: StatusSurfaceFeedbackDto,
) -> Result<BootstrapDto, String> {
    if let Some(proof) = state.proof_config.as_ref() {
        let mut bootstrap = crate::proof_harness::synthetic_bootstrap(proof.scenario);
        bootstrap.status_surface_feedback = status_surface_feedback;
        return Ok(bootstrap);
    }

    let mut bootstrap = BootstrapDto {
        product_name: crate::commands::app::PRODUCT_NAME,
        version: env!("CARGO_PKG_VERSION").to_string(),
        settings: AppSettingsDto::default(),
        status_surface_feedback,
        profiles: Vec::new(),
        selected_profile_id: String::new(),
        usage_by_profile: BTreeMap::new(),
        codex: CodexCompatibilityDto::default(),
    };

    let Some(service) = state.account_service.as_ref() else {
        return Ok(bootstrap);
    };

    bootstrap.settings = service
        .repositories()
        .settings
        .load()
        .map_err(|error| error.to_string())
        .map(|settings| AppSettingsDto::from_settings(&settings))?;
    let snapshot = service.snapshot().map_err(|error| error.to_string())?;
    let identities = service.identity_records().unwrap_or_default();
    bootstrap.profiles = snapshot
        .profiles
        .iter()
        .map(|profile| ProfileSummaryDto::from_profile(profile, identities.get(&profile.id)))
        .collect();
    bootstrap.selected_profile_id = snapshot.selected_profile_id.to_string();

    for usage in service
        .repositories()
        .usage
        .load_all_states()
        .map_err(|error| error.to_string())?
    {
        bootstrap.usage_by_profile.insert(
            usage.profile_id.to_string(),
            ProfileUsageStateDto::from_state(&usage),
        );
    }

    Ok(bootstrap)
}

#[tauri::command]
pub async fn get_bootstrap_state(
    state: tauri::State<'_, Mutex<AppState>>,
    status_surfaces: tauri::State<'_, Mutex<crate::status_surfaces::StatusSurfaceState>>,
) -> Result<BootstrapDto, String> {
    let mut bootstrap = {
        let guard = state
            .lock()
            .map_err(|_| "BOOTSTRAP_STATE_UNAVAILABLE".to_string())?;
        bootstrap_from_state(&guard, StatusSurfaceFeedbackDto::default())?
    };
    let feedback = {
        let guard = status_surfaces
            .lock()
            .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
        crate::status_surfaces::controller::feedback_snapshot(&guard)
    };
    bootstrap.status_surface_feedback = feedback;
    Ok(bootstrap)
}

#[tauri::command]
pub fn get_locale_strings(_language: Option<String>) -> BTreeMap<String, String> {
    BTreeMap::from([("app.name".to_string(), "codex-barbar".to_string())])
}

impl AppErrorDto {
    fn from_error(error: &AppError) -> Self {
        Self {
            kind: error_kind_name(error.kind),
            user_message_key: error.message_key.clone(),
            action: recovery_action_name(error.recovery),
            retry_after: None,
        }
    }
}

fn profile_kind_name(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::CurrentCli => "currentCli",
        ProfileKind::Managed => "managed",
    }
}

fn auth_mode_name(mode: AuthMode) -> &'static str {
    match mode {
        AuthMode::Unknown => "unknown",
        AuthMode::ChatGpt => "chatGpt",
        AuthMode::ApiKey => "apiKey",
    }
}

fn freshness_name(value: Freshness) -> &'static str {
    match value {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "stale",
        Freshness::Missing => "missing",
    }
}

pub(crate) fn refresh_status_name(value: RefreshStatus) -> &'static str {
    match value {
        RefreshStatus::Idle => "idle",
        RefreshStatus::Refreshing => "refreshing",
        RefreshStatus::Cooldown => "cooldown",
        RefreshStatus::Backoff => "backoff",
        RefreshStatus::Blocked => "blocked",
    }
}

fn login_stage_name(value: ManagedLoginStage) -> &'static str {
    match value {
        ManagedLoginStage::Starting => "starting",
        ManagedLoginStage::AwaitingUser => "awaitingUser",
        ManagedLoginStage::Succeeded => "succeeded",
        ManagedLoginStage::Failed => "failed",
        ManagedLoginStage::Cancelled => "cancelled",
    }
}

pub(crate) fn error_kind_name(kind: AppErrorKind) -> &'static str {
    match kind {
        AppErrorKind::CodexNotFound => "codexNotFound",
        AppErrorKind::UnsupportedCodexVersion => "unsupportedCodexVersion",
        AppErrorKind::NotSignedIn => "notSignedIn",
        AppErrorKind::ApiKeyNoQuota => "apiKeyNoQuota",
        AppErrorKind::AuthExpired => "authExpired",
        AppErrorKind::OfflineOrTimeout => "offlineOrTimeout",
        AppErrorKind::RateLimited => "rateLimited",
        AppErrorKind::ProtocolMismatch => "protocolMismatch",
        AppErrorKind::VaultFailure => "vaultFailure",
        AppErrorKind::StorageFailure => "storageFailure",
    }
}

fn recovery_action_name(action: codexbar::core::RecoveryAction) -> &'static str {
    match action {
        codexbar::core::RecoveryAction::None => "retry",
        codexbar::core::RecoveryAction::InstallTestedCodex => "installTestedCodex",
        codexbar::core::RecoveryAction::SignInWithCli => "signIn",
        codexbar::core::RecoveryAction::Reauthenticate => "reloginManagedProfile",
        codexbar::core::RecoveryAction::Retry => "retry",
        codexbar::core::RecoveryAction::WaitForReset => "waitAndRetry",
        codexbar::core::RecoveryAction::ExportDiagnostics => "exportDiagnostics",
    }
}

fn account_status_name(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::SignedIn => "signedIn",
        AccountStatus::SignedOut => "signedOut",
        AccountStatus::Unavailable => "unavailable",
    }
}

fn account_label_fallback(status: &str) -> &'static str {
    match status {
        "signedIn" => "已登录（名称不可用）",
        "signedOut" => "未登录",
        _ => "账号信息不可用",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use codexbar::accounts::identity::AccountIdentityRecord;
    use codexbar::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};
    use codexbar::core::AuthMode;
    use uuid::Uuid;

    #[test]
    fn bridge_profile_usage_preserves_snapshot_and_error() {
        let (primary, _anomaly) = UsageWindow::normalized(
            "five-hour",
            Some("usage.fiveHours".into()),
            58.0,
            Some(300),
            None,
            None,
        );
        let state = ProfileUsageState {
            profile_id: Uuid::nil(),
            snapshot: Some(codexbar::core::ProfileUsageSnapshot {
                profile_id: Uuid::nil(),
                plan_type: Some("plus".into()),
                primary: Some(primary),
                secondary: None,
                additional_windows: Vec::new(),
                fetched_at: DateTime::from_timestamp(1_754_000_000, 0).unwrap(),
                source: codexbar::core::UsageSource::AppServer,
                protocol_anomaly: true,
                reset_credits: None,
            }),
            current_error: Some(AppError::bare(
                AppErrorKind::OfflineOrTimeout,
                "errors.offlineOrTimeout",
                codexbar::core::RecoveryAction::Retry,
            )),
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Stale,
            manual_cooldown_until: None,
        };

        let dto = ProfileUsageStateDto::from_state(&state);
        assert_eq!(dto.primary.as_ref().unwrap().remaining_percent, 42.0);
        assert_eq!(dto.current_error.as_ref().unwrap().kind, "offlineOrTimeout");
        assert_eq!(dto.freshness, "stale");
        assert!(dto.protocol_anomaly);
    }

    #[test]
    fn feedback_serialization_uses_frozen_safe_wire_names() {
        let value = serde_json::to_value(StatusSurfaceFeedbackChangedDto {
            surface: crate::status_surfaces::controller::StatusSurfaceKind::TaskbarStatus,
            close_failed: true,
        })
        .unwrap();
        assert_eq!(value["surface"], "taskbarStatus");
        assert_eq!(value["closeFailed"], true);
        let text = value.to_string().to_ascii_lowercase();
        for forbidden in ["sqlite", "webview", "path", "token", "error"] {
            assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
        }
    }

    #[test]
    fn taskbar_tray_patch_maps_only_supported_wire_values() {
        let patch: SettingsPatchDto = serde_json::from_str(
            r#"{"taskbarTray":{"density":"standard","trayIconMode":"monochrome","showWeeklyPercent":false}}"#,
        )
        .unwrap();
        let mapped = patch.into_patch().unwrap().taskbar_tray.unwrap();

        assert_eq!(mapped.density, Some(TaskbarDensity::Standard));
        assert_eq!(mapped.tray_icon_mode, Some(TrayIconMode::Monochrome));
        assert_eq!(mapped.show_weekly_percent, Some(false));

        let invalid_density: SettingsPatchDto =
            serde_json::from_str(r#"{"taskbarTray":{"density":"wide"}}"#).unwrap();
        assert_eq!(
            invalid_density.into_patch().unwrap_err(),
            "unsupported taskbar density"
        );
        let invalid_icon_mode: SettingsPatchDto =
            serde_json::from_str(r#"{"taskbarTray":{"trayIconMode":"colorful"}}"#).unwrap();
        assert_eq!(
            invalid_icon_mode.into_patch().unwrap_err(),
            "unsupported tray icon mode"
        );
    }

    #[test]
    fn bootstrap_serialization_has_only_frozen_top_level_fields() {
        let value = serde_json::to_value(BootstrapDto {
            product_name: "codex-barbar",
            version: "1.0.0".into(),
            settings: AppSettingsDto::default(),
            status_surface_feedback: StatusSurfaceFeedbackDto {
                taskbar_status_close_failed: true,
                float_ball_close_failed: false,
            },
            profiles: Vec::new(),
            selected_profile_id: String::new(),
            usage_by_profile: BTreeMap::new(),
            codex: CodexCompatibilityDto::default(),
        })
        .unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object.keys().cloned().collect::<Vec<_>>(),
            vec![
                "codex",
                "productName",
                "profiles",
                "selectedProfileId",
                "settings",
                "statusSurfaceFeedback",
                "usageByProfile",
                "version"
            ]
        );
        let text = value.to_string().to_ascii_lowercase();
        for forbidden in ["token", "authjson", "vaultpath", "codexhome"] {
            assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
        }
    }

    #[test]
    fn bootstrap_snapshot_preserves_status_surface_feedback() {
        let feedback = StatusSurfaceFeedbackDto {
            taskbar_status_close_failed: true,
            float_ball_close_failed: false,
        };

        let bootstrap = bootstrap_from_state(&AppState::new(), feedback).unwrap();

        assert!(
            bootstrap
                .status_surface_feedback
                .taskbar_status_close_failed
        );
        assert!(!bootstrap.status_surface_feedback.float_ball_close_failed);
    }

    fn profile(kind: ProfileKind, label: &str) -> AccountProfile {
        AccountProfile {
            id: Uuid::nil(),
            kind,
            label: label.to_string(),
            auth_mode: AuthMode::ChatGpt,
            lifecycle: ProfileLifecycle::Ready,
            email_fingerprint: None,
            created_at: DateTime::from_timestamp(1_754_000_000, 0).unwrap(),
            last_selected_at: None,
            last_success_at: None,
        }
    }

    fn identity(name: Option<&str>, email: Option<&str>) -> AccountIdentityRecord {
        AccountIdentityRecord {
            display_name: name.map(str::to_string),
            email: email.map(str::to_string),
            plan_type: Some("plus".into()),
            status: codexbar::accounts::identity::AccountStatus::SignedIn,
            updated_at: DateTime::from_timestamp(1_754_000_000, 0).unwrap(),
        }
    }

    #[test]
    fn current_cli_profile_uses_account_identity_instead_of_current_cli_label() {
        let dto = ProfileSummaryDto::from_profile(
            &profile(ProfileKind::CurrentCli, "Current CLI"),
            Some(&identity(Some("Ming Zhao"), Some("user@example.com"))),
        );

        assert_eq!(dto.label, "Ming Zhao");
        assert_eq!(dto.account_display_name.as_deref(), Some("Ming Zhao"));
        assert_eq!(dto.account_email.as_deref(), Some("user@example.com"));
        assert_ne!(dto.label, "Current CLI");
    }

    #[test]
    fn managed_profile_keeps_custom_label_and_exposes_identity_separately() {
        let dto = ProfileSummaryDto::from_profile(
            &profile(ProfileKind::Managed, "Work"),
            Some(&identity(None, Some("work@example.com"))),
        );

        assert_eq!(dto.label, "Work");
        assert_eq!(dto.account_display_name, None);
        assert_eq!(dto.account_email.as_deref(), Some("work@example.com"));
    }

    #[test]
    fn signed_in_identity_without_name_uses_explicit_status_label() {
        let dto = ProfileSummaryDto::from_profile(
            &profile(ProfileKind::CurrentCli, "Current CLI"),
            Some(&identity(None, None)),
        );

        assert_eq!(dto.account_status, "signedIn");
        assert_eq!(dto.label, "已登录（名称不可用）");
        assert!(dto.account_updated_at.is_some());
    }

    #[test]
    fn profile_dto_never_serializes_credentials_or_paths() {
        let dto = ProfileSummaryDto::from_profile(
            &profile(ProfileKind::CurrentCli, "Current CLI"),
            Some(&identity(Some("Safe"), Some("safe@example.com"))),
        );
        let text = serde_json::to_string(&dto).unwrap().to_ascii_lowercase();
        for forbidden in ["token", "cookie", "vault", "codexhome", "c:\\"] {
            assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
        }
    }
}
