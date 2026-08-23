//! State-driven V1 tray icon, tooltip, native menu, and click behavior.

use std::sync::Mutex;

use codexbar::accounts::model::{AccountProfile, ProfileLifecycle};
use codexbar::core::{
    AppErrorKind, AuthMode, Freshness, ProfileUsageSnapshot, ProfileUsageState, RefreshStatus,
    RefreshTrigger, UsageWindow,
};
use codexbar::storage::{TaskbarTrayPreferences, TrayIconMode};
use codexbar::tray::{TrayIconPalette, TrayVisualState, render_tray_icon_rgba_with_palette};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Listener, Manager};

use crate::state::AppState;
use crate::tray_menu::{self, TrayMenuAction, TrayProfileMenuItem};

const TRAY_ID: &str = "main";
const UNIVERSAL_WEEKLY_MINUTES: u64 = 10_080;

#[derive(Debug, Clone, PartialEq)]
pub struct TrayPresentation {
    pub visual: TrayVisualState,
    pub icon_palette: TrayIconPalette,
    pub tooltip: String,
    pub profiles: Vec<TrayProfileMenuItem>,
    pub language: String,
}

impl TrayPresentation {
    fn unavailable(language: impl Into<String>) -> Self {
        Self {
            visual: TrayVisualState::Unavailable,
            icon_palette: TrayIconPalette::Dynamic,
            tooltip: "codex-barbar\nState: Unavailable".to_string(),
            profiles: Vec::new(),
            language: language.into(),
        }
    }
}

pub fn presentation_from(
    selected_profile: Option<&AccountProfile>,
    profiles: &[AccountProfile],
    usage: Option<&ProfileUsageState>,
    language: &str,
    prefs: &TaskbarTrayPreferences,
) -> TrayPresentation {
    let selected_profile_id = selected_profile
        .map(|profile| profile.id)
        .unwrap_or_default();
    let profiles = tray_menu::profile_menu_items(
        profiles
            .iter()
            .filter(|profile| profile.lifecycle == ProfileLifecycle::Ready)
            .map(|profile| (profile.id, profile.label.clone())),
        selected_profile_id,
    );
    let visual = visual_state(selected_profile, usage);
    let tooltip = build_tooltip(selected_profile, usage, visual, prefs);
    let icon_palette = if prefs.tray_icon_mode == TrayIconMode::Monochrome {
        TrayIconPalette::Monochrome
    } else {
        TrayIconPalette::Dynamic
    };
    TrayPresentation {
        visual,
        icon_palette,
        tooltip,
        profiles,
        language: language.to_string(),
    }
}

fn visual_state(
    selected_profile: Option<&AccountProfile>,
    usage: Option<&ProfileUsageState>,
) -> TrayVisualState {
    let Some(profile) = selected_profile else {
        return TrayVisualState::Unavailable;
    };
    if profile.auth_mode == AuthMode::ApiKey
        || usage
            .and_then(|state| state.current_error.as_ref())
            .is_some_and(|error| error.kind == AppErrorKind::ApiKeyNoQuota)
    {
        return TrayVisualState::Api;
    }

    let Some(state) = usage else {
        return TrayVisualState::Unavailable;
    };
    let Some(snapshot) = state.snapshot.as_ref() else {
        return TrayVisualState::Unavailable;
    };
    universal_weekly_window(snapshot)
        .map(|window| {
            TrayVisualState::from_remaining(
                window.remaining_percent,
                state.freshness == Freshness::Stale,
            )
        })
        .unwrap_or(TrayVisualState::Unavailable)
}

fn universal_weekly_window(snapshot: &ProfileUsageSnapshot) -> Option<&UsageWindow> {
    snapshot
        .primary
        .iter()
        .chain(snapshot.secondary.iter())
        .find(|window| window.window_duration_minutes == Some(UNIVERSAL_WEEKLY_MINUTES))
}

fn build_tooltip(
    selected_profile: Option<&AccountProfile>,
    usage: Option<&ProfileUsageState>,
    visual: TrayVisualState,
    prefs: &TaskbarTrayPreferences,
) -> String {
    let profile_label = selected_profile
        .map(|profile| sanitize_label(&profile.label))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "No account".to_string());
    let mut lines = vec![if prefs.tooltip_account && !profile_label.is_empty() {
        format!("codex-barbar — {profile_label}")
    } else {
        "codex-barbar".to_string()
    }];

    if prefs.tooltip_weekly {
        let status = compact_tooltip_status(tooltip_status(usage, visual));
        if let Some(percent) = visual.percent() {
            lines.push(format!("Weekly {percent}% {status}"));
        } else {
            lines.push(format!("State: {status}"));
        }
    }

    if let Some(snapshot) = usage.and_then(|state| state.snapshot.as_ref()) {
        if prefs.tooltip_reset_date
            && let Some(window) = universal_weekly_window(snapshot)
            && let Some(resets_at) = window.resets_at
        {
            lines.push(format!(
                "Resets {}",
                resets_at.with_timezone(&chrono::Local).format("%m/%d")
            ));
        }
        if prefs.tooltip_updated_at {
            lines.push(format!(
                "Updated {}",
                snapshot
                    .fetched_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
            ));
        }
    }

    lines.join("\n")
}

fn compact_tooltip_status(status: &'static str) -> &'static str {
    match status {
        "Refreshing" => "Busy",
        status if status.starts_with("Cached") => "Cached",
        "API key (quota unavailable)" => "API",
        status => status,
    }
}

fn tooltip_status(usage: Option<&ProfileUsageState>, visual: TrayVisualState) -> &'static str {
    if usage.is_some_and(|state| state.refresh_status == RefreshStatus::Refreshing) {
        return "Refreshing";
    }
    match visual {
        TrayVisualState::Remaining { .. } => "Fresh",
        TrayVisualState::Stale { .. } => match usage
            .and_then(|state| state.current_error.as_ref())
            .map(|error| error.kind)
        {
            Some(AppErrorKind::OfflineOrTimeout) => "Cached (offline or timeout)",
            Some(AppErrorKind::RateLimited) => "Cached (rate limited)",
            Some(AppErrorKind::AuthExpired | AppErrorKind::NotSignedIn) => {
                "Cached (authentication required)"
            }
            Some(AppErrorKind::ProtocolMismatch) => "Cached (protocol mismatch)",
            Some(AppErrorKind::VaultFailure | AppErrorKind::StorageFailure) => {
                "Cached (local storage error)"
            }
            Some(AppErrorKind::CodexNotFound | AppErrorKind::UnsupportedCodexVersion) => {
                "Cached (Codex unavailable)"
            }
            Some(AppErrorKind::ApiKeyNoQuota) | None => "Cached",
        },
        TrayVisualState::Api => "API key (quota unavailable)",
        TrayVisualState::Unavailable => "Unavailable",
    }
}

fn sanitize_label(label: &str) -> String {
    let normalized = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.eq_ignore_ascii_case("current cli") {
        return "CLI".to_string();
    }

    // Windows reserves a compact 64-character tooltip buffer. Keep the
    // profile label bounded so the weekly value and timestamp stay whole.
    let mut compact = String::new();
    for character in normalized.chars() {
        if compact.len() + character.len_utf8() > 6 {
            break;
        }
        compact.push(character);
    }
    compact
}

fn load_presentation(app: &AppHandle) -> TrayPresentation {
    let proof = app
        .state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.proof_config.clone());
    if let Some(proof) = proof {
        let (profiles, selected, usage) =
            crate::proof_harness::synthetic_account_data(proof.scenario);
        let selected_profile = profiles.iter().find(|p| p.id == selected);
        return presentation_from(
            selected_profile,
            &profiles,
            usage.as_ref(),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );
    }

    let service = app
        .state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.account_service.clone());
    let Some(service) = service else {
        return TrayPresentation::unavailable("en-US");
    };
    let loaded_settings = service.repositories().settings.load().ok();
    let language = loaded_settings
        .as_ref()
        .map(|settings| crate::commands::AppSettingsDto::from_settings(settings).language)
        .unwrap_or("en-US")
        .to_string();
    let language = if language == "system" {
        codexbar::platform::windows::system_locale::default_language().to_string()
    } else {
        language
    };
    let Ok(snapshot) = service.snapshot() else {
        return TrayPresentation::unavailable(language);
    };
    let selected_profile = snapshot
        .profiles
        .iter()
        .find(|profile| profile.id == snapshot.selected_profile_id);
    let usage = service
        .repositories()
        .usage
        .load_state(snapshot.selected_profile_id)
        .ok();
    let prefs = loaded_settings
        .as_ref()
        .map(|settings| settings.taskbar_tray.clone())
        .unwrap_or_default();
    presentation_from(
        selected_profile,
        &snapshot.profiles,
        usage.as_ref(),
        &language,
        &prefs,
    )
}

fn native_tray_order(app: &AppHandle) -> Vec<String> {
    let proof = app
        .state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.proof_config.clone());
    if proof.is_some() {
        return codexbar::storage::MenuPreferences::default()
            .native_tray
            .order;
    }
    app.state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| {
            state
                .account_service
                .as_ref()
                .and_then(|service| service.repositories().settings.load().ok())
        })
        .unwrap_or_default()
        .menu
        .native_tray
        .normalized_order(
            &codexbar::storage::NATIVE_TRAY_ITEMS,
            &codexbar::storage::REQUIRED_NATIVE_TRAY_ITEMS,
        )
}

/// Apply a candidate native menu without touching the persisted settings.
///
/// Used by the transactional menu command: the candidate menu is applied
/// first and only persisted after this call succeeds. On a later persistence
/// failure the prior settings are re-applied through the same function.
pub fn apply_candidate_menu(
    app: &AppHandle,
    settings: &codexbar::storage::AppSettings,
) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let presentation = load_presentation(app);
    let order = settings.menu.native_tray.normalized_order(
        &codexbar::storage::NATIVE_TRAY_ITEMS,
        &codexbar::storage::REQUIRED_NATIVE_TRAY_ITEMS,
    );
    let menu =
        tray_menu::build_native_menu(app, &presentation.profiles, &presentation.language, &order)?;
    tray.set_menu(Some(menu))?;
    Ok(())
}

fn tray_image(visual: TrayVisualState, palette: TrayIconPalette) -> tauri::image::Image<'static> {
    let (rgba, width, height) = render_tray_icon_rgba_with_palette(visual, palette);
    tauri::image::Image::new_owned(rgba, width, height)
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let presentation = load_presentation(app.handle());
    let order = native_tray_order(app.handle());
    let menu = tray_menu::build_native_menu(
        app.handle(),
        &presentation.profiles,
        &presentation.language,
        &order,
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_image(presentation.visual, presentation.icon_palette))
        .tooltip(presentation.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
                && tray_click_action(button, button_state) == TrayClickAction::TogglePanel
            {
                crate::shell::flyout_window::toggle_with_blur_consume(tray.app_handle(), None);
            }
        })
        .build(app)?;

    register_rebuild_listeners(app);
    Ok(())
}

pub fn rebuild(app: &AppHandle) -> tauri::Result<()> {
    let presentation = load_presentation(app);
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let order = native_tray_order(app);
    let menu =
        tray_menu::build_native_menu(app, &presentation.profiles, &presentation.language, &order)?;
    tray.set_icon(Some(tray_image(
        presentation.visual,
        presentation.icon_palette,
    )))?;
    tray.set_tooltip(Some(presentation.tooltip))?;
    tray.set_menu(Some(menu))?;
    Ok(())
}

fn register_rebuild_listeners(app: &mut App) {
    for event_name in rebuild_event_names() {
        let handle = app.handle().clone();
        app.listen(event_name, move |_| {
            if let Err(error) = rebuild(&handle) {
                tracing::warn!(event_name, %error, "tray rebuild failed");
            }
        });
    }
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match tray_menu::menu_action(event.id().as_ref()) {
        TrayMenuAction::OpenPanel => {
            let _ = crate::shell::flyout_window::open_or_focus(app, None);
        }
        TrayMenuAction::Refresh => request_refresh(app),
        TrayMenuAction::OpenUsage => {
            let _ = crate::commands::open_codex_usage_page();
        }
        TrayMenuAction::Settings => {
            let _ = crate::shell::settings_window::open_or_focus(app, "general");
        }
        TrayMenuAction::About => {
            let _ = crate::shell::settings_window::open_or_focus(app, "about");
        }
        TrayMenuAction::Quit => {
            crate::commands::quit_app(app.clone(), app.state());
        }
        TrayMenuAction::SelectProfile(profile_id) => select_profile(app, profile_id),
        TrayMenuAction::None => {}
    }
}

fn request_refresh(app: &AppHandle) {
    let service = app
        .state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.account_service.clone());
    let Some(service) = service else { return };
    let Ok(snapshot) = service.snapshot() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let _ = service
            .request_refresh(snapshot.selected_profile_id, RefreshTrigger::Manual)
            .await;
    });
}

fn select_profile(app: &AppHandle, profile_id: uuid::Uuid) {
    let service = app
        .state::<Mutex<AppState>>()
        .lock()
        .ok()
        .and_then(|state| state.account_service.clone());
    let Some(service) = service else { return };
    if service.select_profile(profile_id).is_err() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = service
            .request_refresh(profile_id, RefreshTrigger::ProfileSwitched)
            .await;
    });
}

pub const fn rebuild_event_names() -> [&'static str; 6] {
    crate::events::TRAY_REBUILD_EVENTS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayClickAction {
    TogglePanel,
    NativeMenu,
    Ignore,
}

pub fn tray_click_action(button: MouseButton, button_state: MouseButtonState) -> TrayClickAction {
    if button_state != MouseButtonState::Up {
        return TrayClickAction::Ignore;
    }
    match button {
        MouseButton::Left => TrayClickAction::TogglePanel,
        MouseButton::Right => TrayClickAction::NativeMenu,
        _ => TrayClickAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use codexbar::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};
    use codexbar::core::{
        AppError, AppErrorKind, AuthMode, Freshness, ProfileUsageSnapshot, ProfileUsageState,
        RecoveryAction, RefreshStatus, UsageSource, UsageWindow,
    };
    use codexbar::storage::{TaskbarTrayPreferences, TrayIconMode};
    use tauri::tray::{MouseButton, MouseButtonState};
    use uuid::Uuid;

    fn profile(auth_mode: AuthMode) -> AccountProfile {
        AccountProfile {
            id: Uuid::from_u128(1),
            kind: ProfileKind::CurrentCli,
            label: "Work".into(),
            auth_mode,
            lifecycle: ProfileLifecycle::Ready,
            email_fingerprint: Some([9; 32]),
            created_at: Utc.with_ymd_and_hms(2026, 8, 6, 0, 0, 0).unwrap(),
            last_selected_at: None,
            last_success_at: None,
        }
    }

    fn usage(primary_used: f64, secondary_used: f64, stale: bool) -> ProfileUsageState {
        ProfileUsageState {
            profile_id: Uuid::from_u128(1),
            snapshot: Some(ProfileUsageSnapshot {
                profile_id: Uuid::from_u128(1),
                plan_type: Some("plus".into()),
                primary: Some(
                    UsageWindow::normalized(
                        "five-hour",
                        Some("ignored-private-label".into()),
                        primary_used,
                        Some(300),
                        None,
                        None,
                    )
                    .0,
                ),
                secondary: Some(
                    UsageWindow::normalized(
                        "weekly",
                        None,
                        secondary_used,
                        Some(10080),
                        None,
                        None,
                    )
                    .0,
                ),
                additional_windows: Vec::new(),
                fetched_at: Utc.with_ymd_and_hms(2026, 8, 6, 1, 2, 3).unwrap(),
                source: UsageSource::AppServer,
                protocol_anomaly: false,
            }),
            current_error: stale.then(|| {
                AppError::new(
                    AppErrorKind::OfflineOrTimeout,
                    "user@example.com token=secret",
                    RecoveryAction::Retry,
                    "diagnostic-secret",
                )
            }),
            refresh_status: RefreshStatus::Idle,
            freshness: if stale {
                Freshness::Stale
            } else {
                Freshness::Fresh
            },
            manual_cooldown_until: None,
        }
    }

    #[test]
    fn selected_profile_uses_the_universal_weekly_window_and_shared_bands() {
        let profile = profile(AuthMode::ChatGpt);
        let mut usage = usage(99.0, 34.0, false);
        usage.snapshot.as_mut().unwrap().additional_windows.push(
            UsageWindow::normalized(
                "codex-spark:weekly",
                Some("GPT-5.3-Codex-Spark".into()),
                0.0,
                Some(10_080),
                None,
                None,
            )
            .0,
        );
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );
        assert_eq!(
            presentation.visual,
            codexbar::tray::TrayVisualState::Remaining {
                percent: 66,
                level: codexbar::tray::TrayLevel::Warning,
            }
        );
    }

    #[test]
    fn api_key_without_quota_uses_api_state() {
        let profile = profile(AuthMode::ApiKey);
        let usage = usage(99.0, 34.0, false);
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );
        assert_eq!(presentation.visual, codexbar::tray::TrayVisualState::Api);
        assert!(!presentation.tooltip.contains("Weekly remaining"));
        assert!(!presentation.tooltip.contains("Weekly:"));
    }

    #[test]
    fn tooltip_uses_the_same_rounded_weekly_percent_as_the_icon() {
        let profile = profile(AuthMode::ChatGpt);
        let usage = usage(99.0, 33.5, false);
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );

        assert_eq!(
            presentation.visual,
            codexbar::tray::TrayVisualState::Remaining {
                percent: 67,
                level: codexbar::tray::TrayLevel::Normal,
            }
        );
        assert!(presentation.tooltip.contains("Weekly 67%"));
        assert!(!presentation.tooltip.contains("Weekly: 66% remaining"));
    }

    #[test]
    fn tooltip_keeps_updated_timestamp_complete_within_windows_limit() {
        let mut profile = profile(AuthMode::ChatGpt);
        profile.label = "Current CLI".into();
        let usage = usage(99.0, 34.0, false);
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );

        let updated = usage.snapshot.as_ref().unwrap().fetched_at;
        assert_eq!(
            presentation.tooltip,
            format!(
                "codex-barbar — CLI\nWeekly 66% Fresh\nUpdated {}",
                updated
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
            )
        );
        assert!(presentation.tooltip.len() <= 64);
    }

    #[test]
    fn tooltip_is_textual_and_never_uses_error_diagnostics() {
        let profile = profile(AuthMode::ChatGpt);
        let usage = usage(58.0, 39.0, true);
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );
        assert!(presentation.tooltip.contains("Work"));
        assert!(presentation.tooltip.contains("Weekly 61%"));
        assert!(!presentation.tooltip.contains("5-hour"));
        assert!(presentation.tooltip.contains("Weekly"));
        assert!(presentation.tooltip.contains("Cached"));
        for forbidden in ["user@example.com", "token", "secret", "diagnostic"] {
            assert!(!presentation.tooltip.contains(forbidden));
        }
    }

    #[test]
    fn rebuild_events_cover_all_tray_inputs() {
        assert_eq!(
            rebuild_event_names(),
            [
                "profile-usage-state-changed",
                "refresh-state-changed",
                "accounts-updated",
                "selected-profile-changed",
                "settings-changed",
                "locale-changed",
            ]
        );
    }

    #[test]
    fn left_click_toggles_panel_and_right_click_is_native_menu_only() {
        assert_eq!(
            tray_click_action(MouseButton::Left, MouseButtonState::Up),
            TrayClickAction::TogglePanel
        );
        assert_eq!(
            tray_click_action(MouseButton::Right, MouseButtonState::Up),
            TrayClickAction::NativeMenu
        );
        assert_eq!(
            tray_click_action(MouseButton::Left, MouseButtonState::Down),
            TrayClickAction::Ignore
        );
    }

    #[test]
    fn tooltip_honors_account_and_updated_preferences() {
        let profile = profile(AuthMode::ChatGpt);
        let usage = usage(99.0, 34.0, false);
        let prefs = TaskbarTrayPreferences {
            tooltip_account: false,
            tooltip_updated_at: false,
            ..TaskbarTrayPreferences::default()
        };
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &prefs,
        );
        assert!(!presentation.tooltip.contains("Work"));
        assert!(!presentation.tooltip.contains("Updated"));
        assert!(presentation.tooltip.contains("Weekly 66%"));
    }

    #[test]
    fn tooltip_includes_reset_date_when_enabled_and_known() {
        let profile = profile(AuthMode::ChatGpt);
        let mut usage = usage(99.0, 34.0, false);
        usage
            .snapshot
            .as_mut()
            .unwrap()
            .secondary
            .as_mut()
            .unwrap()
            .resets_at = Some(Utc.with_ymd_and_hms(2026, 8, 13, 0, 0, 0).unwrap());
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &TaskbarTrayPreferences::default(),
        );
        assert!(presentation.tooltip.contains("Resets"));
    }

    #[test]
    fn monochrome_preference_selects_neutral_icon_palette() {
        let profile = profile(AuthMode::ChatGpt);
        let usage = usage(99.0, 34.0, false);
        let prefs = TaskbarTrayPreferences {
            tray_icon_mode: TrayIconMode::Monochrome,
            ..TaskbarTrayPreferences::default()
        };
        let presentation = presentation_from(
            Some(&profile),
            std::slice::from_ref(&profile),
            Some(&usage),
            "en-US",
            &prefs,
        );
        assert_eq!(
            presentation.icon_palette,
            codexbar::tray::TrayIconPalette::Monochrome
        );
    }
}
