//! Proof/debug harness for the V1 Tauri desktop shell.
//!
//! Activated by `CODEXBAR_PROOF_MODE`. Supported values are the fixed
//! [`ProofScenario`] names below. Proof mode suppresses blur-dismiss so
//! windows stay visible for automated screenshot capture, and serves only
//! synthetic, credential-free UI data. It does not resolve Codex for proof
//! payloads, but normal local repository and recovery startup still occurs.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::surface::SurfaceMode;
use crate::surface_target::is_supported_settings_tab;

use codexbar::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};
use codexbar::core::{
    AppError, AppErrorKind, AuthMode, Freshness, ProfileUsageSnapshot, ProfileUsageState,
    RecoveryAction, RefreshStatus, UsageSource, UsageWindow,
};
use uuid::Uuid;

/// Deterministic visual state for a taskbar-status or float-ball proof
/// scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusProofState {
    Ready,
    Warning,
    Critical,
    Refreshing,
    Stale,
    Missing,
    Weekly,
}

impl StatusProofState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Refreshing => "refreshing",
            Self::Stale => "stale",
            Self::Missing => "missing",
            Self::Weekly => "weekly",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ready" => Some(Self::Ready),
            "warning" => Some(Self::Warning),
            "critical" => Some(Self::Critical),
            "refreshing" => Some(Self::Refreshing),
            "stale" => Some(Self::Stale),
            "missing" => Some(Self::Missing),
            "weekly" => Some(Self::Weekly),
            _ => None,
        }
    }
}

/// Fixed, credential-free proof scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProofScenario {
    TrayPanelReady,
    TrayPanelStale,
    TrayPanelError,
    TrayPanelApi,
    TrayPanelProfiles,
    SettingsGeneral,
    SettingsProviders,
    SettingsAdvanced,
    SettingsAbout,
    TaskbarStatus(StatusProofState),
    FloatBall(StatusProofState),
}

impl ProofScenario {
    #[allow(dead_code)]
    pub const ALL_NAMES: [&'static str; 23] = [
        "trayPanel:ready",
        "trayPanel:stale",
        "trayPanel:error",
        "trayPanel:api",
        "trayPanel:profiles",
        "settings:general",
        "settings:providers",
        "settings:advanced",
        "settings:about",
        "taskbar-status:ready",
        "taskbar-status:warning",
        "taskbar-status:critical",
        "taskbar-status:refreshing",
        "taskbar-status:stale",
        "taskbar-status:missing",
        "taskbar-status:weekly",
        "float-ball:ready",
        "float-ball:warning",
        "float-ball:critical",
        "float-ball:refreshing",
        "float-ball:stale",
        "float-ball:missing",
        "float-ball:weekly",
    ];

    #[allow(dead_code)]
    pub fn name(self) -> String {
        match self {
            Self::TrayPanelReady => "trayPanel:ready".to_string(),
            Self::TrayPanelStale => "trayPanel:stale".to_string(),
            Self::TrayPanelError => "trayPanel:error".to_string(),
            Self::TrayPanelApi => "trayPanel:api".to_string(),
            Self::TrayPanelProfiles => "trayPanel:profiles".to_string(),
            Self::SettingsGeneral => "settings:general".to_string(),
            Self::SettingsProviders => "settings:providers".to_string(),
            Self::SettingsAdvanced => "settings:advanced".to_string(),
            Self::SettingsAbout => "settings:about".to_string(),
            Self::TaskbarStatus(state) => format!("taskbar-status:{}", state.as_str()),
            Self::FloatBall(state) => format!("float-ball:{}", state.as_str()),
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "trayPanel:ready" => Some(Self::TrayPanelReady),
            "trayPanel:stale" => Some(Self::TrayPanelStale),
            "trayPanel:error" => Some(Self::TrayPanelError),
            "trayPanel:api" => Some(Self::TrayPanelApi),
            "trayPanel:profiles" => Some(Self::TrayPanelProfiles),
            "settings:general" => Some(Self::SettingsGeneral),
            "settings:providers" => Some(Self::SettingsProviders),
            "settings:advanced" => Some(Self::SettingsAdvanced),
            "settings:about" => Some(Self::SettingsAbout),
            "taskbar-status" => Some(Self::TaskbarStatus(StatusProofState::Ready)),
            "float-ball" => Some(Self::FloatBall(StatusProofState::Ready)),
            _ => {
                let (surface, state) = raw.split_once(':')?;
                match surface {
                    "taskbar-status" => Some(Self::TaskbarStatus(StatusProofState::parse(state)?)),
                    "float-ball" => Some(Self::FloatBall(StatusProofState::parse(state)?)),
                    _ => None,
                }
            }
        }
    }

    pub fn surface(self) -> SurfaceMode {
        match self {
            Self::TrayPanelReady
            | Self::TrayPanelStale
            | Self::TrayPanelError
            | Self::TrayPanelApi
            | Self::TrayPanelProfiles => SurfaceMode::TrayPanel,
            Self::SettingsGeneral
            | Self::SettingsProviders
            | Self::SettingsAdvanced
            | Self::SettingsAbout => SurfaceMode::Settings,
            Self::TaskbarStatus(_) | Self::FloatBall(_) => SurfaceMode::Hidden,
        }
    }

    pub fn settings_tab(self) -> Option<&'static str> {
        match self {
            Self::SettingsGeneral => Some("general"),
            Self::SettingsProviders => Some("providers"),
            Self::SettingsAdvanced => Some("advanced"),
            Self::SettingsAbout => Some("about"),
            Self::TaskbarStatus(_) | Self::FloatBall(_) => None,
            _ => None,
        }
    }
}

/// Proof configuration parsed from `CODEXBAR_PROOF_MODE`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofConfig {
    /// The surface to show on startup (camelCase id).
    pub target_surface: String,
    /// Optional settings tab id.
    pub settings_tab: Option<String>,
    /// The fixed proof scenario.
    pub scenario: ProofScenario,
}

impl ProofConfig {
    /// Read proof configuration from the environment.
    ///
    /// Returns `None` when `CODEXBAR_PROOF_MODE` is unset or empty.
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var("CODEXBAR_PROOF_MODE").ok()?;
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        let scenario = match ProofScenario::parse(raw) {
            Some(scenario) => scenario,
            None => match raw {
                "tray" | "trayPanel" => ProofScenario::TrayPanelReady,
                "settings" => ProofScenario::SettingsGeneral,
                "settings:general" if is_supported_settings_tab("general") => {
                    ProofScenario::SettingsGeneral
                }
                _ => {
                    tracing::warn!("CODEXBAR_PROOF_MODE: unsupported target '{raw}', ignoring");
                    return None;
                }
            },
        };

        let target_surface = match scenario {
            ProofScenario::TaskbarStatus(_) => "taskbar-status",
            ProofScenario::FloatBall(_) => "float-ball",
            _ => scenario.surface().as_str(),
        };

        Some(ProofConfig {
            target_surface: target_surface.to_string(),
            settings_tab: scenario.settings_tab().map(str::to_string),
            scenario,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusProofProjection {
    taskbar_status_enabled: bool,
    float_ball_enabled: bool,
    taskbar_status_opacity: u8,
    float_ball_opacity: u8,
}

fn status_proof_projection(scenario: ProofScenario) -> Option<StatusProofProjection> {
    match scenario {
        ProofScenario::TaskbarStatus(_) => Some(StatusProofProjection {
            taskbar_status_enabled: true,
            float_ball_enabled: false,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
        }),
        ProofScenario::FloatBall(_) => Some(StatusProofProjection {
            taskbar_status_enabled: false,
            float_ball_enabled: true,
            taskbar_status_opacity: 20,
            float_ball_opacity: 20,
        }),
        _ => None,
    }
}

fn status_proof_settings(scenario: ProofScenario) -> Option<codexbar::storage::AppSettings> {
    let projection = status_proof_projection(scenario)?;
    Some(codexbar::storage::AppSettings {
        taskbar_status_enabled: projection.taskbar_status_enabled,
        float_ball_enabled: projection.float_ball_enabled,
        taskbar_status_opacity: projection.taskbar_status_opacity,
        float_ball_opacity: projection.float_ball_opacity,
        ..codexbar::storage::AppSettings::default()
    })
}

fn apply_status_proof_with(
    scenario: ProofScenario,
    apply_runtime: impl FnOnce(&codexbar::storage::AppSettings) -> Result<(), String>,
) -> Result<(), String> {
    let settings = status_proof_settings(scenario)
        .ok_or_else(|| "PROOF_STATUS_SCENARIO_UNAVAILABLE".to_string())?;
    apply_runtime(&settings)
}

const PROOF_TRAY_ACTIVATION_FAILED: &str = "PROOF_TRAY_ACTIVATION_FAILED";
const PROOF_SETTINGS_ACTIVATION_FAILED: &str = "PROOF_SETTINGS_ACTIVATION_FAILED";
const PROOF_STATUS_ACTIVATION_FAILED: &str = "PROOF_STATUS_ACTIVATION_FAILED";

fn activate_scenario_with(
    scenario: ProofScenario,
    settings_tab: Option<&str>,
    open_tray: impl FnOnce() -> Result<(), String>,
    open_settings: impl FnOnce(&str) -> Result<(), String>,
    apply_status_runtime: impl FnOnce(&codexbar::storage::AppSettings) -> Result<(), String>,
) -> Result<(), &'static str> {
    match scenario.surface() {
        SurfaceMode::TrayPanel => open_tray().map_err(|_| PROOF_TRAY_ACTIVATION_FAILED),
        SurfaceMode::Settings => open_settings(settings_tab.unwrap_or("general"))
            .map_err(|_| PROOF_SETTINGS_ACTIVATION_FAILED),
        SurfaceMode::Hidden => apply_status_proof_with(scenario, apply_status_runtime)
            .map_err(|_| PROOF_STATUS_ACTIVATION_FAILED),
    }
}

/// Activate the proof-mode target surface.
///
/// Called from the Tauri `setup` closure when proof mode is active.
pub fn activate(app: &AppHandle) {
    let config = {
        let st = app.state::<Mutex<AppState>>();
        st.lock().unwrap().proof_config.clone()
    };

    let Some(config) = config else { return };
    tracing::info!(
        "proof-harness: activating surface={} tab={:?}",
        config.target_surface,
        config.settings_tab,
    );

    let result = activate_scenario_with(
        config.scenario,
        config.settings_tab.as_deref(),
        || crate::shell::flyout_window::open_or_focus(app, None),
        |settings_tab| crate::shell::settings_window::open_or_focus(app, settings_tab),
        |settings| crate::status_surfaces::apply_status_surface_settings(app, settings),
    );

    match result {
        Ok(()) => tracing::info!(
            code = "PROOF_ACTIVATION_SUCCEEDED",
            "proof activation completed"
        ),
        Err(code) => tracing::error!(code, "proof activation failed"),
    }
}

/// Returns `true` when proof mode is active in the shared state.
pub fn is_proof_mode(app: &AppHandle) -> bool {
    app.try_state::<Mutex<AppState>>()
        .map(|st| st.lock().unwrap().proof_config.is_some())
        .unwrap_or(false)
}

pub fn is_taskbar_status_scenario(scenario: ProofScenario) -> bool {
    matches!(scenario, ProofScenario::TaskbarStatus(_))
}

pub fn is_taskbar_status_proof(app: &AppHandle) -> bool {
    let state = app.state::<Mutex<AppState>>();
    state
        .lock()
        .ok()
        .and_then(|state| state.proof_config.as_ref().map(|cfg| cfg.scenario))
        .is_some_and(is_taskbar_status_scenario)
}

fn profile(id: Uuid, kind: ProfileKind, label: &str, auth_mode: AuthMode) -> AccountProfile {
    AccountProfile {
        id,
        kind,
        label: label.to_string(),
        auth_mode,
        lifecycle: ProfileLifecycle::Ready,
        email_fingerprint: None,
        created_at: chrono::DateTime::from_timestamp(1_752_000_000, 0).unwrap(),
        last_selected_at: None,
        last_success_at: Some(chrono::DateTime::from_timestamp(1_752_000_000, 0).unwrap()),
    }
}

fn snapshot(profile_id: Uuid, primary_used: f64, secondary_used: f64) -> ProfileUsageSnapshot {
    ProfileUsageSnapshot {
        profile_id,
        plan_type: Some("plus".to_string()),
        primary: Some(
            UsageWindow::normalized("five-hour", None, primary_used, Some(300), None, None).0,
        ),
        secondary: Some(
            UsageWindow::normalized("weekly", None, secondary_used, Some(10_080), None, None).0,
        ),
        additional_windows: Vec::new(),
        fetched_at: chrono::DateTime::from_timestamp(1_752_000_000, 0).unwrap(),
        source: UsageSource::AppServer,
        protocol_anomaly: false,
    }
}

fn weekly_snapshot(profile_id: Uuid) -> ProfileUsageSnapshot {
    ProfileUsageSnapshot {
        profile_id,
        plan_type: Some("plus".to_string()),
        primary: Some(
            UsageWindow::normalized(
                "weekly",
                None,
                2.0,
                Some(10_080),
                Some(chrono::DateTime::from_timestamp(4_090_867_200, 0).unwrap()),
                None,
            )
            .0,
        ),
        secondary: None,
        additional_windows: Vec::new(),
        fetched_at: chrono::DateTime::from_timestamp(1_786_665_600, 0).unwrap(),
        source: UsageSource::AppServer,
        protocol_anomaly: false,
    }
}

fn error(kind: AppErrorKind) -> AppError {
    AppError::new(
        kind,
        "errors.proofSynthetic",
        RecoveryAction::Retry,
        "PROOF_SYNTHETIC",
    )
}

fn status_surface_state(
    current: Uuid,
    state: StatusProofState,
) -> (Vec<AccountProfile>, Option<ProfileUsageState>) {
    let profiles = vec![profile(
        current,
        ProfileKind::CurrentCli,
        "Current CLI",
        AuthMode::ChatGpt,
    )];
    let usage = match state {
        StatusProofState::Ready => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(snapshot(current, 58.0, 39.0)),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }),
        StatusProofState::Warning => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(snapshot(current, 78.0, 39.0)),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }),
        StatusProofState::Critical => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(snapshot(current, 94.0, 39.0)),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }),
        StatusProofState::Refreshing => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(snapshot(current, 58.0, 39.0)),
            current_error: None,
            refresh_status: RefreshStatus::Refreshing,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }),
        StatusProofState::Stale => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(snapshot(current, 58.0, 39.0)),
            current_error: Some(error(AppErrorKind::OfflineOrTimeout)),
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Stale,
            manual_cooldown_until: None,
        }),
        StatusProofState::Missing => Some(ProfileUsageState::missing(current)),
        StatusProofState::Weekly => Some(ProfileUsageState {
            profile_id: current,
            snapshot: Some(weekly_snapshot(current)),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }),
    };
    (profiles, usage)
}

/// Synthetic, credential-free account profiles and usage for proof scenarios.
pub fn synthetic_account_data(
    scenario: ProofScenario,
) -> (Vec<AccountProfile>, Uuid, Option<ProfileUsageState>) {
    let current = Uuid::from_u128(1);
    let work = Uuid::from_u128(2);
    let personal = Uuid::from_u128(3);

    let (profiles, usage) = match scenario {
        ProofScenario::TrayPanelApi => (
            vec![profile(
                current,
                ProfileKind::CurrentCli,
                "Current CLI",
                AuthMode::ApiKey,
            )],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: None,
                current_error: Some(error(AppErrorKind::ApiKeyNoQuota)),
                refresh_status: RefreshStatus::Idle,
                freshness: Freshness::Missing,
                manual_cooldown_until: None,
            }),
        ),
        ProofScenario::TrayPanelProfiles => (
            vec![
                profile(
                    current,
                    ProfileKind::CurrentCli,
                    "Current CLI",
                    AuthMode::ChatGpt,
                ),
                profile(work, ProfileKind::Managed, "Work", AuthMode::ChatGpt),
                profile(
                    personal,
                    ProfileKind::Managed,
                    "Personal",
                    AuthMode::ChatGpt,
                ),
            ],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: Some(snapshot(current, 58.0, 39.0)),
                current_error: None,
                refresh_status: RefreshStatus::Idle,
                freshness: Freshness::Fresh,
                manual_cooldown_until: None,
            }),
        ),
        ProofScenario::TrayPanelReady => (
            vec![profile(
                current,
                ProfileKind::CurrentCli,
                "Current CLI",
                AuthMode::ChatGpt,
            )],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: Some(snapshot(current, 58.0, 39.0)),
                current_error: None,
                refresh_status: RefreshStatus::Idle,
                freshness: Freshness::Fresh,
                manual_cooldown_until: None,
            }),
        ),
        ProofScenario::TrayPanelStale => (
            vec![profile(
                current,
                ProfileKind::CurrentCli,
                "Current CLI",
                AuthMode::ChatGpt,
            )],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: Some(snapshot(current, 58.0, 39.0)),
                current_error: Some(error(AppErrorKind::OfflineOrTimeout)),
                refresh_status: RefreshStatus::Idle,
                freshness: Freshness::Stale,
                manual_cooldown_until: None,
            }),
        ),
        ProofScenario::TrayPanelError => (
            vec![profile(
                current,
                ProfileKind::CurrentCli,
                "Current CLI",
                AuthMode::ChatGpt,
            )],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: None,
                current_error: Some(error(AppErrorKind::RateLimited)),
                refresh_status: RefreshStatus::Backoff,
                freshness: Freshness::Missing,
                manual_cooldown_until: None,
            }),
        ),
        ProofScenario::TaskbarStatus(state) | ProofScenario::FloatBall(state) => {
            status_surface_state(current, state)
        }
        _ => (
            vec![profile(
                current,
                ProfileKind::CurrentCli,
                "Current CLI",
                AuthMode::ChatGpt,
            )],
            Some(ProfileUsageState {
                profile_id: current,
                snapshot: Some(snapshot(current, 58.0, 39.0)),
                current_error: None,
                refresh_status: RefreshStatus::Idle,
                freshness: Freshness::Fresh,
                manual_cooldown_until: None,
            }),
        ),
    };

    (profiles, current, usage)
}

/// Synthetic bootstrap payload for proof scenarios. Never touches real
/// accounts, vaults, or Codex.
pub fn synthetic_bootstrap(scenario: ProofScenario) -> crate::commands::BootstrapDto {
    let (profiles, selected, usage) = synthetic_account_data(scenario);
    let settings = status_proof_settings(scenario)
        .map(|settings| crate::commands::AppSettingsDto::from_settings(&settings))
        .unwrap_or_default();
    let identities = profiles
        .iter()
        .map(|profile| {
            let weekly_status_proof = matches!(
                scenario,
                ProofScenario::TaskbarStatus(StatusProofState::Weekly)
                    | ProofScenario::FloatBall(StatusProofState::Weekly)
            );
            (
                profile.id,
                codexbar::accounts::identity::AccountIdentityRecord {
                    display_name: (profile.kind == ProfileKind::CurrentCli).then(|| {
                        if weekly_status_proof {
                            "ProofUser".to_string()
                        } else {
                            "Ming Zhao".to_string()
                        }
                    }),
                    email: (profile.kind == ProfileKind::CurrentCli).then(|| {
                        if weekly_status_proof {
                            "proof@example.com".to_string()
                        } else {
                            "ming.zhao@example.com".to_string()
                        }
                    }),
                    plan_type: Some("plus".to_string()),
                    status: codexbar::accounts::identity::AccountStatus::SignedIn,
                    updated_at: chrono::Utc::now(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let profiles_dto: Vec<_> = profiles
        .iter()
        .map(|profile| {
            crate::commands::ProfileSummaryDto::from_profile(profile, identities.get(&profile.id))
        })
        .collect();
    let mut usage_by_profile = std::collections::BTreeMap::new();
    if let Some(state) = usage {
        usage_by_profile.insert(
            selected.to_string(),
            crate::commands::ProfileUsageStateDto::from_state(&state),
        );
    }
    crate::commands::BootstrapDto {
        product_name: "codex-barbar",
        version: env!("CARGO_PKG_VERSION").to_string(),
        settings,
        status_surface_feedback: crate::commands::StatusSurfaceFeedbackDto::default(),
        profiles: profiles_dto,
        selected_profile_id: selected.to_string(),
        usage_by_profile,
        codex: crate::commands::CodexCompatibilityDto::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn with_proof_mode_env(value: Option<&str>, test: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("CODEXBAR_PROOF_MODE").ok();

        match value {
            Some(value) => unsafe { std::env::set_var("CODEXBAR_PROOF_MODE", value) },
            None => unsafe { std::env::remove_var("CODEXBAR_PROOF_MODE") },
        }

        test();

        match prev {
            Some(prev) => unsafe { std::env::set_var("CODEXBAR_PROOF_MODE", prev) },
            None => unsafe { std::env::remove_var("CODEXBAR_PROOF_MODE") },
        }
    }

    #[test]
    fn parse_tray_surface() {
        with_proof_mode_env(Some("trayPanel"), || {
            let cfg = ProofConfig::from_env().unwrap();
            assert_eq!(cfg.target_surface, "trayPanel");
            assert!(cfg.settings_tab.is_none());
        });
    }

    #[test]
    fn parse_tray_alias() {
        with_proof_mode_env(Some("tray"), || {
            let cfg = ProofConfig::from_env().unwrap();
            assert_eq!(cfg.target_surface, "trayPanel");
        });
    }

    #[test]
    fn parse_settings_with_tab() {
        with_proof_mode_env(Some("settings:general"), || {
            let cfg = ProofConfig::from_env().unwrap();
            assert_eq!(cfg.target_surface, "settings");
            assert_eq!(cfg.settings_tab.as_deref(), Some("general"));
            assert_eq!(cfg.scenario, ProofScenario::SettingsGeneral);
        });
    }

    #[test]
    fn empty_env_returns_none() {
        with_proof_mode_env(Some(""), || {
            assert!(ProofConfig::from_env().is_none());
        });
    }

    #[test]
    fn unset_env_returns_none() {
        with_proof_mode_env(None, || {
            assert!(ProofConfig::from_env().is_none());
        });
    }

    #[test]
    fn invalid_surface_returns_none() {
        with_proof_mode_env(Some("bogus"), || {
            assert!(ProofConfig::from_env().is_none());
        });
    }

    #[test]
    fn pop_out_is_rejected() {
        with_proof_mode_env(Some("popOut"), || {
            assert!(ProofConfig::from_env().is_none());
        });
    }

    #[test]
    fn invalid_settings_tab_returns_none() {
        with_proof_mode_env(Some("settings:security"), || {
            assert!(ProofConfig::from_env().is_none());
        });
    }

    #[test]
    fn every_canonical_proof_name_parses_and_round_trips() {
        for name in ProofScenario::ALL_NAMES {
            let scenario = ProofScenario::parse(name).unwrap();
            assert_eq!(scenario.name(), name);
            with_proof_mode_env(Some(name), || {
                let cfg = ProofConfig::from_env().unwrap();
                assert_eq!(cfg.scenario, scenario);
            });
        }
    }

    #[test]
    fn proof_modes_are_fixed_and_secret_free() {
        assert_eq!(
            ProofScenario::ALL_NAMES,
            [
                "trayPanel:ready",
                "trayPanel:stale",
                "trayPanel:error",
                "trayPanel:api",
                "trayPanel:profiles",
                "settings:general",
                "settings:providers",
                "settings:advanced",
                "settings:about",
                "taskbar-status:ready",
                "taskbar-status:warning",
                "taskbar-status:critical",
                "taskbar-status:refreshing",
                "taskbar-status:stale",
                "taskbar-status:missing",
                "taskbar-status:weekly",
                "float-ball:ready",
                "float-ball:warning",
                "float-ball:critical",
                "float-ball:refreshing",
                "float-ball:stale",
                "float-ball:missing",
                "float-ball:weekly",
            ]
        );
    }

    #[test]
    fn auxiliary_status_proof_modes_parse() {
        for (name, scenario) in [
            (
                "taskbar-status",
                ProofScenario::TaskbarStatus(StatusProofState::Ready),
            ),
            (
                "float-ball",
                ProofScenario::FloatBall(StatusProofState::Ready),
            ),
        ] {
            with_proof_mode_env(Some(name), || {
                let config = ProofConfig::from_env().expect("auxiliary proof mode");
                assert_eq!(config.scenario, scenario);
                assert_eq!(config.target_surface, name);
            });
        }
    }

    #[test]
    fn only_taskbar_proof_scenarios_enable_the_probe() {
        assert!(is_taskbar_status_scenario(ProofScenario::TaskbarStatus(
            StatusProofState::Weekly
        )));
        assert!(!is_taskbar_status_scenario(ProofScenario::FloatBall(
            StatusProofState::Weekly
        )));
    }

    #[test]
    fn status_proof_projection_is_mutually_exclusive_and_deterministic() {
        let taskbar =
            status_proof_projection(ProofScenario::TaskbarStatus(StatusProofState::Weekly))
                .unwrap();
        assert_eq!(
            taskbar,
            StatusProofProjection {
                taskbar_status_enabled: true,
                float_ball_enabled: false,
                taskbar_status_opacity: 20,
                float_ball_opacity: 20,
            }
        );

        let float =
            status_proof_projection(ProofScenario::FloatBall(StatusProofState::Weekly)).unwrap();
        assert!(!float.taskbar_status_enabled);
        assert!(float.float_ball_enabled);
        assert_eq!(
            (float.taskbar_status_opacity, float.float_ball_opacity),
            (20, 20)
        );
        assert!(status_proof_projection(ProofScenario::SettingsGeneral).is_none());
    }

    #[test]
    fn activation_routing_uses_runtime_only_once_for_status_proofs() {
        let tray_calls = std::cell::Cell::new(0);
        let settings_calls = std::cell::Cell::new(0);
        let applied = std::cell::RefCell::new(Vec::new());
        activate_scenario_with(
            ProofScenario::FloatBall(StatusProofState::Weekly),
            None,
            || {
                tray_calls.set(tray_calls.get() + 1);
                Ok(())
            },
            |_| {
                settings_calls.set(settings_calls.get() + 1);
                Ok(())
            },
            |settings| {
                applied
                    .borrow_mut()
                    .push((settings.taskbar_status_enabled, settings.float_ball_enabled));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*applied.borrow(), [(false, true)]);
        assert_eq!(tray_calls.get(), 0);
        assert_eq!(settings_calls.get(), 0);
    }

    #[test]
    fn activation_routing_never_applies_status_runtime_for_other_surfaces() {
        for scenario in [
            ProofScenario::TrayPanelReady,
            ProofScenario::SettingsGeneral,
        ] {
            let runtime_calls = std::cell::Cell::new(0);
            activate_scenario_with(
                scenario,
                scenario.settings_tab(),
                || Ok(()),
                |_| Ok(()),
                |_| {
                    runtime_calls.set(runtime_calls.get() + 1);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(runtime_calls.get(), 0);
        }
    }

    #[test]
    fn activation_routing_returns_fixed_codes_instead_of_raw_surface_failures() {
        let raw_tray_failure = "tauri WebView failed at C:\\secret\\storage";
        let tray = activate_scenario_with(
            ProofScenario::TrayPanelReady,
            None,
            || Err(raw_tray_failure.to_string()),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .unwrap_err();
        assert_eq!(tray, "PROOF_TRAY_ACTIVATION_FAILED");
        assert_ne!(tray, raw_tray_failure);

        let raw_settings_failure = "WebView storage error: tauri://settings";
        let settings = activate_scenario_with(
            ProofScenario::SettingsAdvanced,
            Some("advanced"),
            || unreachable!(),
            |_| Err(raw_settings_failure.to_string()),
            |_| unreachable!(),
        )
        .unwrap_err();
        assert_eq!(settings, "PROOF_SETTINGS_ACTIVATION_FAILED");
        assert_ne!(settings, raw_settings_failure);
    }

    #[test]
    fn synthetic_current_profile_never_uses_current_cli_label() {
        let bootstrap = synthetic_bootstrap(ProofScenario::TaskbarStatus(StatusProofState::Ready));
        let current = bootstrap.profiles.first().expect("synthetic profile");
        assert_ne!(current.label, "Current CLI");
        assert_eq!(current.account_display_name.as_deref(), Some("Ming Zhao"));
        assert!(
            !serde_json::to_string(&bootstrap)
                .unwrap()
                .contains("Current CLI")
        );
    }

    #[test]
    fn synthetic_bootstrap_defaults_status_surface_feedback_to_clear() {
        let bootstrap = synthetic_bootstrap(ProofScenario::TaskbarStatus(StatusProofState::Ready));

        assert!(
            !bootstrap
                .status_surface_feedback
                .taskbar_status_close_failed
        );
        assert!(!bootstrap.status_surface_feedback.float_ball_close_failed);
    }

    #[test]
    fn every_status_surface_proof_state_parses() {
        for state in [
            "ready",
            "warning",
            "critical",
            "refreshing",
            "stale",
            "missing",
            "weekly",
        ] {
            assert!(ProofScenario::parse(&format!("taskbar-status:{state}")).is_some());
            assert!(ProofScenario::parse(&format!("float-ball:{state}")).is_some());
        }
    }

    #[test]
    fn weekly_status_surface_proofs_are_trusted_weekly_only_and_compact() {
        for scenario in [
            ProofScenario::TaskbarStatus(StatusProofState::Weekly),
            ProofScenario::FloatBall(StatusProofState::Weekly),
        ] {
            let bootstrap = synthetic_bootstrap(scenario);
            let usage = bootstrap.usage_by_profile.values().next().unwrap();
            let windows = [&usage.primary, &usage.secondary]
                .into_iter()
                .flatten()
                .chain(usage.additional_windows.iter())
                .collect::<Vec<_>>();

            assert_eq!(windows.len(), 1);
            let weekly = windows[0];
            assert_eq!(weekly.window_duration_minutes, Some(10_080));
            assert_eq!(weekly.remaining_percent, 98.0);
            assert_eq!(
                weekly.resets_at.as_deref(),
                Some("2099-08-20T00:00:00+00:00")
            );
            assert!(
                !windows
                    .iter()
                    .any(|window| window.window_duration_minutes == Some(300))
            );
            assert_eq!(usage.freshness, "fresh");
            assert_eq!(usage.refresh_status, "idle");
            assert!(usage.current_error.is_none());
            assert_eq!(bootstrap.settings.taskbar_status_opacity, 20);
            assert_eq!(bootstrap.settings.float_ball_opacity, 20);
            match scenario {
                ProofScenario::TaskbarStatus(_) => {
                    assert!(bootstrap.settings.taskbar_status_enabled);
                    assert!(!bootstrap.settings.float_ball_enabled);
                }
                ProofScenario::FloatBall(_) => {
                    assert!(!bootstrap.settings.taskbar_status_enabled);
                    assert!(bootstrap.settings.float_ball_enabled);
                }
                _ => unreachable!(),
            }

            let profile = bootstrap.profiles.first().unwrap();
            assert_eq!(profile.account_display_name.as_deref(), Some("ProofUser"));
            assert_eq!(profile.label, "ProofUser");
            assert_eq!(profile.label.chars().take(6).count(), 6);
            assert_eq!(profile.label.chars().take(6).collect::<String>(), "ProofU");
            assert_eq!(profile.account_email.as_deref(), Some("proof@example.com"));
        }
    }

    #[test]
    fn status_proof_payloads_cover_visual_states_without_credentials() {
        let warning = synthetic_bootstrap(ProofScenario::TaskbarStatus(StatusProofState::Warning));
        let warning_usage = warning.usage_by_profile.values().next().unwrap();
        assert_eq!(warning_usage.primary.as_ref().unwrap().used_percent, 78.0);

        let missing = synthetic_bootstrap(ProofScenario::FloatBall(StatusProofState::Missing));
        let missing_usage = missing.usage_by_profile.values().next().unwrap();
        assert!(missing_usage.primary.is_none());
        assert!(missing_usage.secondary.is_none());

        let encoded = serde_json::to_string(&warning).unwrap();
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("cookie"));
    }
}
