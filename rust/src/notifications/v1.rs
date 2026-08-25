use std::{collections::BTreeMap, fs, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    app_paths::AppPaths,
    core::{ProfileId, ProfileUsageState, UsageWindow},
    storage::NotificationPreferences,
};

const WEEKLY_WINDOW_MINUTES: u64 = 10_080;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V1NotificationEvent {
    Warning { remaining_percent: u8 },
    Danger { remaining_percent: u8 },
    WeeklyReset,
    ResetCreditsIncreased { available_count: u64 },
    RefreshFailed,
    RefreshRecovered,
    UpdateAvailable { version: String },
    PricingCatalogChanged { source_count: u8 },
    PricingRefreshFailed,
    PricingRefreshRecovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum QuotaBand {
    Normal,
    Warning,
    Danger,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct NotificationState {
    profiles: BTreeMap<ProfileId, ProfileNotificationState>,
    last_update_version: Option<String>,
    consecutive_pricing_failures: u8,
    pricing_failure_notified: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ProfileNotificationState {
    /// SHA-256 hex of the normalized account email; never raw identity.
    account_marker_hash: Option<String>,
    weekly_reset_at: Option<DateTime<Utc>>,
    armed_band: Option<QuotaBand>,
    known_reset_credits: Option<u64>,
    consecutive_refresh_failures: u8,
    refresh_failure_notified: bool,
}

/// Pure V1 notification decision engine with small, non-secret persisted state.
pub struct V1NotificationEngine {
    state_path: PathBuf,
    state: NotificationState,
}

impl V1NotificationEngine {
    pub fn load(paths: &AppPaths) -> Self {
        let state_path = paths.notification_state.clone();
        let state = match fs::read(&state_path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(state) => state,
                Err(error) => {
                    tracing::warn!(error = %error, "Ignoring unreadable notification runtime state");
                    NotificationState::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                NotificationState::default()
            }
            Err(error) => {
                tracing::warn!(error = %error, "Unable to read notification runtime state");
                NotificationState::default()
            }
        };
        Self { state_path, state }
    }

    #[cfg(test)]
    pub fn observe_usage(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        state: &ProfileUsageState,
        reset_credits: Option<u64>,
    ) -> Vec<V1NotificationEvent> {
        self.observe_usage_for_account(preferences, profile_id, None, state, reset_credits)
    }

    pub fn observe_usage_for_account(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        account_marker: Option<&str>,
        state: &ProfileUsageState,
        reset_credits: Option<u64>,
    ) -> Vec<V1NotificationEvent> {
        self.prepare_profile_for_account(profile_id, account_marker);
        let Some(window) = universal_weekly_window(state) else {
            self.persist();
            return Vec::new();
        };
        let remaining_percent = rounded_percent(window.remaining_percent);
        let current_band = band_for(remaining_percent, preferences);
        let profile = self.state.profiles.entry(profile_id).or_default();
        let first_observation = profile.armed_band.is_none();
        let new_cycle = matches!(
            (profile.weekly_reset_at, window.resets_at),
            (Some(previous), Some(current)) if previous != current
        );
        let previous_band = profile.armed_band;
        let previous_credits = profile.known_reset_credits;

        profile.weekly_reset_at = window.resets_at;
        profile.armed_band = Some(current_band);
        profile.known_reset_credits = reset_credits;

        let mut events = Vec::new();
        if preferences.enabled && !first_observation {
            if new_cycle && preferences.weekly_reset_enabled {
                events.push(V1NotificationEvent::WeeklyReset);
            } else if !new_cycle {
                // A healthier observation re-arms a later descent, as required
                // by the V1 lifecycle (for example Warning -> Normal -> Warning).
                match (previous_band, current_band) {
                    (Some(QuotaBand::Normal), QuotaBand::Warning)
                        if preferences.warning_enabled =>
                    {
                        events.push(V1NotificationEvent::Warning { remaining_percent });
                    }
                    (Some(QuotaBand::Normal | QuotaBand::Warning), QuotaBand::Danger)
                        if preferences.danger_enabled =>
                    {
                        events.push(V1NotificationEvent::Danger { remaining_percent });
                    }
                    _ => {}
                }
            }
            if preferences.reset_credit_increase_enabled
                && previous_credits
                    .is_some_and(|previous| reset_credits.is_some_and(|current| current > previous))
            {
                events.push(V1NotificationEvent::ResetCreditsIncreased {
                    available_count: reset_credits.expect("checked above"),
                });
            }
        }
        self.persist();
        events
    }

    #[cfg(test)]
    pub fn observe_refresh(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        success: bool,
    ) -> Vec<V1NotificationEvent> {
        self.observe_refresh_for_account(preferences, profile_id, None, success)
    }

    pub fn observe_refresh_for_account(
        &mut self,
        preferences: &NotificationPreferences,
        profile_id: ProfileId,
        account_marker: Option<&str>,
        success: bool,
    ) -> Vec<V1NotificationEvent> {
        self.prepare_profile_for_account(profile_id, account_marker);
        let profile = self.state.profiles.entry(profile_id).or_default();
        if !preferences.enabled || !preferences.refresh_failure_enabled {
            profile.consecutive_refresh_failures = 0;
            profile.refresh_failure_notified = false;
            self.persist();
            return Vec::new();
        }
        let mut events = Vec::new();
        if success {
            let had_reported_failure = profile.refresh_failure_notified;
            profile.consecutive_refresh_failures = 0;
            profile.refresh_failure_notified = false;
            if had_reported_failure {
                events.push(V1NotificationEvent::RefreshRecovered);
            }
        } else {
            profile.consecutive_refresh_failures =
                profile.consecutive_refresh_failures.saturating_add(1);
            if profile.consecutive_refresh_failures == 3 {
                profile.refresh_failure_notified = true;
                events.push(V1NotificationEvent::RefreshFailed);
            }
        }
        self.persist();
        events
    }

    fn prepare_profile_for_account(&mut self, profile_id: ProfileId, account_marker: Option<&str>) {
        let profile = self.state.profiles.entry(profile_id).or_default();
        if let Some(account_marker) = account_marker {
            if profile
                .account_marker_hash
                .as_deref()
                .is_some_and(|previous| previous != account_marker)
            {
                *profile = ProfileNotificationState::default();
            }
            profile.account_marker_hash = Some(account_marker.to_string());
        }
    }

    pub fn observe_pricing_refresh(
        &mut self,
        preferences: &NotificationPreferences,
        failed: bool,
        source_count: u8,
        catalog_changed: bool,
    ) -> Vec<V1NotificationEvent> {
        let mut events = Vec::new();
        if !preferences.enabled {
            self.state.consecutive_pricing_failures = 0;
            self.state.pricing_failure_notified = false;
            self.persist();
            return events;
        }
        if failed {
            if preferences.pricing_refresh_failure_enabled {
                self.state.consecutive_pricing_failures =
                    self.state.consecutive_pricing_failures.saturating_add(1);
                if self.state.consecutive_pricing_failures == 3 {
                    self.state.pricing_failure_notified = true;
                    events.push(V1NotificationEvent::PricingRefreshFailed);
                }
            } else {
                self.state.consecutive_pricing_failures = 0;
                self.state.pricing_failure_notified = false;
            }
        } else {
            let had_reported_failure = self.state.pricing_failure_notified;
            self.state.consecutive_pricing_failures = 0;
            self.state.pricing_failure_notified = false;
            if had_reported_failure && preferences.pricing_refresh_failure_enabled {
                events.push(V1NotificationEvent::PricingRefreshRecovered);
            }
            if catalog_changed && preferences.pricing_changed_enabled {
                events.push(V1NotificationEvent::PricingCatalogChanged { source_count });
            }
        }
        self.persist();
        events
    }

    pub fn observe_update_available(
        &mut self,
        preferences: &NotificationPreferences,
        version: &str,
    ) -> Vec<V1NotificationEvent> {
        let is_new = self.state.last_update_version.as_deref() != Some(version);
        self.state.last_update_version = Some(version.to_string());
        let events = if is_new && preferences.enabled && preferences.update_available_enabled {
            vec![V1NotificationEvent::UpdateAvailable {
                version: version.to_string(),
            }]
        } else {
            Vec::new()
        };
        self.persist();
        events
    }

    fn persist(&self) {
        let Some(parent) = self.state_path.parent() else {
            tracing::warn!("Notification runtime state has no parent directory");
            return;
        };
        let result = fs::create_dir_all(parent)
            .and_then(|()| serde_json::to_vec(&self.state).map_err(std::io::Error::other))
            .and_then(|encoded| fs::write(&self.state_path, encoded));
        if let Err(error) = result {
            tracing::warn!(error = %error, "Unable to persist notification runtime state");
        }
    }
}

fn universal_weekly_window(state: &ProfileUsageState) -> Option<&UsageWindow> {
    let snapshot = state.snapshot.as_ref()?;
    snapshot
        .primary
        .iter()
        .chain(snapshot.secondary.iter())
        .find(|window| window.window_duration_minutes == Some(WEEKLY_WINDOW_MINUTES))
}

fn rounded_percent(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

fn band_for(remaining_percent: u8, preferences: &NotificationPreferences) -> QuotaBand {
    if remaining_percent <= preferences.danger_remaining_percent {
        QuotaBand::Danger
    } else if remaining_percent <= preferences.warning_remaining_percent {
        QuotaBand::Warning
    } else {
        QuotaBand::Normal
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::{V1NotificationEngine, V1NotificationEvent};
    use crate::{
        app_paths::AppPaths,
        core::{
            Freshness, ProfileId, ProfileUsageSnapshot, ProfileUsageState, RefreshStatus,
            UsageSource, UsageWindow,
        },
        storage::NotificationPreferences,
    };

    fn enabled() -> NotificationPreferences {
        NotificationPreferences {
            enabled: true,
            ..NotificationPreferences::default()
        }
    }

    fn weekly(used: f64, reset: DateTime<Utc>) -> ProfileUsageState {
        let id = Uuid::new_v4();
        ProfileUsageState {
            profile_id: id,
            snapshot: Some(ProfileUsageSnapshot {
                profile_id: id,
                plan_type: None,
                primary: Some(
                    UsageWindow::normalized("five-hour", None, 99.0, Some(300), None, None).0,
                ),
                secondary: Some(
                    UsageWindow::normalized("weekly", None, used, Some(10_080), Some(reset), None)
                        .0,
                ),
                additional_windows: vec![
                    UsageWindow::normalized("model-weekly", None, 100.0, Some(10_080), None, None)
                        .0,
                ],
                fetched_at: reset,
                source: UsageSource::AppServer,
                protocol_anomaly: false,
                reset_credits: None,
            }),
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Fresh,
            manual_cooldown_until: None,
        }
    }

    fn state_path() -> (tempfile::TempDir, AppPaths) {
        let temp = tempdir().unwrap();
        let paths = AppPaths::from_local_app_data(temp.path());
        (temp, paths)
    }

    #[test]
    fn usage_transitions_are_deduplicated_and_credit_increases_notify_once() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id: ProfileId = Uuid::new_v4();
        let reset = DateTime::from_timestamp(1_752_000_000, 0).unwrap();

        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(20.0, reset), Some(1))
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset), Some(1)),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );
        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(40.0, reset), Some(1))
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(80.0, reset), Some(2)),
            vec![
                V1NotificationEvent::Danger {
                    remaining_percent: 20
                },
                V1NotificationEvent::ResetCreditsIncreased { available_count: 2 },
            ]
        );
    }

    #[test]
    fn disabled_master_establishes_a_baseline_without_replaying_on_enable() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let reset = DateTime::from_timestamp(1_752_000_000, 0).unwrap();
        let disabled = NotificationPreferences::default();

        assert!(
            engine
                .observe_usage(&disabled, id, &weekly(80.0, reset), Some(4))
                .is_empty()
        );
        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(80.0, reset), Some(4))
                .is_empty()
        );
    }

    #[test]
    fn new_weekly_reset_rearms_bands_and_notifies_reset() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let reset_a = DateTime::from_timestamp(1_752_000_000, 0).unwrap();
        let reset_b = DateTime::from_timestamp(1_752_604_800, 0).unwrap();

        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(20.0, reset_a), None)
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset_a), None),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(20.0, reset_b), None),
            vec![V1NotificationEvent::WeeklyReset]
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset_b), None),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );
    }

    #[test]
    fn refresh_failure_and_recovery_are_deduplicated() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();

        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert_eq!(
            engine.observe_refresh(&enabled(), id, false),
            vec![V1NotificationEvent::RefreshFailed]
        );
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert_eq!(
            engine.observe_refresh(&enabled(), id, true),
            vec![V1NotificationEvent::RefreshRecovered]
        );
        assert!(engine.observe_refresh(&enabled(), id, true).is_empty());
    }

    #[test]
    fn two_master_disabled_failures_do_not_count_toward_enabled_threshold() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let disabled = NotificationPreferences::default();

        assert!(engine.observe_refresh(&disabled, id, false).is_empty());
        assert!(engine.observe_refresh(&disabled, id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert_eq!(
            engine.observe_refresh(&enabled(), id, false),
            vec![V1NotificationEvent::RefreshFailed]
        );
        assert_eq!(
            engine.observe_refresh(&enabled(), id, true),
            vec![V1NotificationEvent::RefreshRecovered]
        );
        assert!(engine.observe_refresh(&enabled(), id, true).is_empty());
    }

    #[test]
    fn three_master_disabled_failures_leave_one_enabled_failure_at_count_one() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let disabled = NotificationPreferences::default();

        for _ in 0..3 {
            assert!(engine.observe_refresh(&disabled, id, false).is_empty());
        }
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, true).is_empty());
    }

    #[test]
    fn event_disabled_failures_do_not_count_after_event_is_reenabled() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let event_disabled = NotificationPreferences {
            enabled: true,
            refresh_failure_enabled: false,
            ..NotificationPreferences::default()
        };

        for _ in 0..3 {
            assert!(
                engine
                    .observe_refresh(&event_disabled, id, false)
                    .is_empty()
            );
        }
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert_eq!(
            engine.observe_refresh(&enabled(), id, false),
            vec![V1NotificationEvent::RefreshFailed]
        );
        assert_eq!(
            engine.observe_refresh(&enabled(), id, true),
            vec![V1NotificationEvent::RefreshRecovered]
        );
    }

    #[test]
    fn third_identical_catalog_failure_emits_once() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let prefs = NotificationPreferences {
            enabled: true,
            pricing_refresh_failure_enabled: true,
            ..NotificationPreferences::default()
        };
        assert!(
            engine
                .observe_pricing_refresh(&prefs, true, 0, false)
                .is_empty()
        );
        assert!(
            engine
                .observe_pricing_refresh(&prefs, true, 0, false)
                .is_empty()
        );
        assert_eq!(
            engine.observe_pricing_refresh(&prefs, true, 0, false),
            vec![V1NotificationEvent::PricingRefreshFailed]
        );
        assert!(
            engine
                .observe_pricing_refresh(&prefs, true, 0, false)
                .is_empty()
        );
        assert_eq!(
            engine.observe_pricing_refresh(&prefs, false, 3, false),
            vec![V1NotificationEvent::PricingRefreshRecovered]
        );
    }

    fn reported_refresh_failure_remains_recoverable_after_restart() {
        let (_temp, paths) = state_path();
        let id = Uuid::new_v4();
        let mut engine = V1NotificationEngine::load(&paths);

        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert!(engine.observe_refresh(&enabled(), id, false).is_empty());
        assert_eq!(
            engine.observe_refresh(&enabled(), id, false),
            vec![V1NotificationEvent::RefreshFailed]
        );

        let mut reloaded = V1NotificationEngine::load(&paths);
        assert_eq!(
            reloaded.observe_refresh(&enabled(), id, true),
            vec![V1NotificationEvent::RefreshRecovered]
        );
    }

    #[test]
    fn returning_to_a_healthier_band_rearms_warning_and_danger() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let reset = DateTime::from_timestamp(1_752_000_000, 0).unwrap();

        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(20.0, reset), None)
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset), None),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );
        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(20.0, reset), None)
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset), None),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(80.0, reset), None),
            vec![V1NotificationEvent::Danger {
                remaining_percent: 20
            }]
        );
        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(40.0, reset), None)
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(80.0, reset), None),
            vec![V1NotificationEvent::Danger {
                remaining_percent: 20
            }]
        );
    }

    #[test]
    fn persisted_state_survives_restart_without_duplicate_events() {
        let (_temp, paths) = state_path();
        let id = Uuid::new_v4();
        let reset = DateTime::from_timestamp(1_752_000_000, 0).unwrap();
        let mut engine = V1NotificationEngine::load(&paths);
        assert!(
            engine
                .observe_usage(&enabled(), id, &weekly(20.0, reset), Some(1))
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage(&enabled(), id, &weekly(40.0, reset), Some(1)),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60
            }]
        );

        let mut reloaded = V1NotificationEngine::load(&paths);
        assert!(
            reloaded
                .observe_usage(&enabled(), id, &weekly(40.0, reset), Some(1))
                .is_empty()
        );
        let encoded = std::fs::read_to_string(&paths.notification_state).unwrap();
        assert!(!encoded.contains("email"));
        assert!(!encoded.contains("credential"));
    }

    #[test]
    fn update_available_version_is_deduplicated_after_restart() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);

        assert_eq!(
            engine.observe_update_available(&enabled(), "v9.9.9"),
            vec![V1NotificationEvent::UpdateAvailable {
                version: "v9.9.9".to_string(),
            }]
        );

        let mut reloaded = V1NotificationEngine::load(&paths);
        assert!(
            reloaded
                .observe_update_available(&enabled(), "v9.9.9")
                .is_empty()
        );
    }

    #[test]
    fn known_account_change_clears_all_profile_notification_baselines() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let reset_a = DateTime::from_timestamp(1_752_000_000, 0).unwrap();
        let reset_b = DateTime::from_timestamp(1_752_604_800, 0).unwrap();

        assert!(
            engine
                .observe_usage_for_account(
                    &enabled(),
                    id,
                    Some("account-hash-a"),
                    &weekly(20.0, reset_a),
                    Some(1),
                )
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage_for_account(
                &enabled(),
                id,
                Some("account-hash-a"),
                &weekly(40.0, reset_a),
                Some(1),
            ),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60,
            }]
        );
        for expected in [
            Vec::new(),
            Vec::new(),
            vec![V1NotificationEvent::RefreshFailed],
        ] {
            assert_eq!(
                engine.observe_refresh_for_account(&enabled(), id, Some("account-hash-a"), false,),
                expected
            );
        }

        assert!(
            engine
                .observe_usage_for_account(
                    &enabled(),
                    id,
                    Some("account-hash-b"),
                    &weekly(80.0, reset_b),
                    Some(5),
                )
                .is_empty(),
            "account change must not carry warning, danger, weekly-reset, or credit state"
        );
        assert!(
            engine
                .observe_refresh_for_account(&enabled(), id, Some("account-hash-b"), true)
                .is_empty(),
            "account change must not carry recovery state"
        );
    }

    #[test]
    fn unavailable_account_marker_preserves_current_profile_state() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();
        let reset = DateTime::from_timestamp(1_752_000_000, 0).unwrap();

        assert!(
            engine
                .observe_usage_for_account(
                    &enabled(),
                    id,
                    Some("account-hash-a"),
                    &weekly(20.0, reset),
                    None,
                )
                .is_empty()
        );
        assert_eq!(
            engine.observe_usage_for_account(&enabled(), id, None, &weekly(40.0, reset), None,),
            vec![V1NotificationEvent::Warning {
                remaining_percent: 60,
            }]
        );
        assert!(
            engine
                .observe_refresh_for_account(&enabled(), id, Some("account-hash-a"), false)
                .is_empty()
        );
        assert!(
            engine
                .observe_refresh_for_account(&enabled(), id, None, false)
                .is_empty()
        );
        assert_eq!(
            engine.observe_refresh_for_account(&enabled(), id, None, false),
            vec![V1NotificationEvent::RefreshFailed]
        );
    }

    #[test]
    fn known_account_change_without_usage_window_still_clears_and_persists_baseline() {
        let (_temp, paths) = state_path();
        let mut engine = V1NotificationEngine::load(&paths);
        let id = Uuid::new_v4();

        for expected in [
            Vec::new(),
            Vec::new(),
            vec![V1NotificationEvent::RefreshFailed],
        ] {
            assert_eq!(
                engine.observe_refresh_for_account(&enabled(), id, Some("account-hash-a"), false,),
                expected
            );
        }
        assert!(
            engine
                .observe_usage_for_account(
                    &enabled(),
                    id,
                    Some("account-hash-b"),
                    &ProfileUsageState::missing(id),
                    None,
                )
                .is_empty()
        );

        let mut reloaded = V1NotificationEngine::load(&paths);
        assert!(
            reloaded
                .observe_refresh_for_account(&enabled(), id, None, true)
                .is_empty()
        );
    }
}
