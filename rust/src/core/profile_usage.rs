//! Profile-scoped quota windows and usage snapshots.
//!
//! These are the frozen V1 roadmap contracts that provider internals map into
//! and product surfaces render. All normalization (finite/clamped percents,
//! derived remaining) happens at construction time so downstream code never
//! re-validates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppError, AppErrorKind};

/// Stable identifier for a codex-barbar account profile.
pub type ProfileId = Uuid;

/// How the account behind a profile authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    /// Identity has not been established yet.
    Unknown,
    /// ChatGPT sign-in (OAuth); exposes plan quota.
    ChatGpt,
    /// Raw API key; exposes no ChatGPT plan quota.
    ApiKey,
}

/// Where a usage snapshot's data came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageSource {
    /// The official `codex app-server` stdio JSONL process.
    AppServer,
}

/// One quota window (e.g. the 5-hour session window or the weekly window).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub limit_id: String,
    /// Localization key or official window name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Normalized to the finite range 0..=100.
    pub used_percent: f64,
    /// Always derived as `100 - used_percent` after normalization; never
    /// trusted from a second, duplicate source field.
    pub remaining_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_duration_minutes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reached_type: Option<String>,
}

impl UsageWindow {
    /// Build a normalized window from raw wire values.
    ///
    /// Returns the window plus an `anomaly` flag that is true when the raw
    /// percent was non-finite or outside 0..=100 (the value is clamped for
    /// display but the anomaly is preserved for diagnostics).
    pub fn normalized(
        limit_id: impl Into<String>,
        label: Option<String>,
        raw_used: f64,
        duration: Option<u64>,
        resets_at: Option<DateTime<Utc>>,
        reached_type: Option<String>,
    ) -> (Self, bool) {
        let anomaly = !raw_used.is_finite() || !(0.0..=100.0).contains(&raw_used);
        let used = if raw_used.is_finite() {
            raw_used.clamp(0.0, 100.0)
        } else {
            0.0
        };
        (
            Self {
                limit_id: limit_id.into(),
                label,
                used_percent: used,
                remaining_percent: 100.0 - used,
                window_duration_minutes: duration,
                resets_at,
                reached_type,
            },
            anomaly,
        )
    }

    /// Localization key for a known window duration, if any.
    pub fn duration_label(duration_minutes: Option<u64>) -> Option<String> {
        match duration_minutes {
            Some(300) => Some("usage.window.fiveHours".to_string()),
            Some(10080) => Some("usage.window.weekly".to_string()),
            Some(_) => Some("usage.window.durationMinutes".to_string()),
            None => None,
        }
    }
}

/// A profile-scoped usage snapshot produced by one fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageSnapshot {
    pub profile_id: ProfileId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<UsageWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_windows: Vec<UsageWindow>,
    pub fetched_at: DateTime<Utc>,
    pub source: UsageSource,
    /// True when any wire value had to be clamped/dropped as out of contract.
    pub protocol_anomaly: bool,
    /// Redacted banked reset-credit summary; only the available count crosses
    /// the domain boundary. Opaque identifiers and redemption data never do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_credits: Option<ResetCreditsSummary>,
}

/// Count-only, non-sensitive view of banked reset credits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCreditsSummary {
    pub available_count: u64,
}

/// Refresh lifecycle status of one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefreshStatus {
    Idle,
    Refreshing,
    Cooldown,
    Backoff,
    Blocked,
}

/// Why a profile refresh was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefreshTrigger {
    Startup,
    Timer,
    PanelOpened,
    Manual,
    ProfileSwitched,
}

/// What the scheduler/service did with a refresh request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefreshDisposition {
    Started,
    Joined,
    Cooldown { retry_at: DateTime<Utc> },
    Backoff { retry_at: DateTime<Utc> },
    Blocked { error: AppErrorKind },
}

/// Freshness of the cached snapshot for one profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Freshness {
    Fresh,
    Stale,
    Missing,
}

/// Per-profile usage cache plus the latest error (kept separate so a failed
/// refresh never erases the last successful snapshot).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUsageState {
    pub profile_id: ProfileId,
    pub snapshot: Option<ProfileUsageSnapshot>,
    pub current_error: Option<AppError>,
    pub refresh_status: RefreshStatus,
    pub freshness: Freshness,
    pub manual_cooldown_until: Option<DateTime<Utc>>,
}

impl ProfileUsageState {
    pub fn missing(profile_id: ProfileId) -> Self {
        Self {
            profile_id,
            snapshot: None,
            current_error: None,
            refresh_status: RefreshStatus::Idle,
            freshness: Freshness::Missing,
            manual_cooldown_until: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_without_reset_credits_deserializes_to_none() {
        let json = r#"{"profileId":"00000000-0000-0000-0000-000000000000","fetchedAt":"2026-01-01T00:00:00Z","source":"appServer","protocolAnomaly":false}"#;
        let snapshot: ProfileUsageSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.reset_credits, None);
    }

    #[test]
    fn reset_credit_summary_round_trips_count_only() {
        let snapshot = ProfileUsageSnapshot {
            profile_id: Uuid::nil(),
            plan_type: Some("plus".into()),
            primary: None,
            secondary: None,
            additional_windows: Vec::new(),
            fetched_at: DateTime::from_timestamp(1_750_000_000, 0).unwrap(),
            source: UsageSource::AppServer,
            protocol_anomaly: false,
            reset_credits: Some(ResetCreditsSummary { available_count: 2 }),
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["resetCredits"]["availableCount"], 2);
        let credits_text = value["resetCredits"].to_string().to_ascii_lowercase();
        for forbidden in ["id", "credit", "title", "grantedat", "redeem"] {
            assert!(!credits_text.contains(forbidden), "leaked {forbidden}");
        }
        let back: ProfileUsageSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(back, snapshot);
    }

    #[test]
    fn usage_window_clamps_and_derives_remaining() {
        let (window, anomaly) =
            UsageWindow::normalized("codex", None, 127.5, Some(300), None, None);
        assert_eq!(window.used_percent, 100.0);
        assert_eq!(window.remaining_percent, 0.0);
        assert!(anomaly);
    }

    #[test]
    fn usage_window_normal_value_not_anomaly() {
        let (window, anomaly) = UsageWindow::normalized("codex", None, 42.5, Some(300), None, None);
        assert_eq!(window.used_percent, 42.5);
        assert_eq!(window.remaining_percent, 57.5);
        assert!(!anomaly);
    }

    #[test]
    fn usage_window_non_finite_is_anomaly_and_zeroed() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (window, anomaly) = UsageWindow::normalized("codex", None, bad, None, None, None);
            assert!(anomaly);
            assert_eq!(window.used_percent, 0.0);
            assert_eq!(window.remaining_percent, 100.0);
        }
    }

    #[test]
    fn usage_window_negative_clamps_to_zero_with_anomaly() {
        let (window, anomaly) = UsageWindow::normalized("codex", None, -3.0, None, None, None);
        assert!(anomaly);
        assert_eq!(window.used_percent, 0.0);
        assert_eq!(window.remaining_percent, 100.0);
    }

    #[test]
    fn duration_labels_are_localization_keys() {
        assert_eq!(
            UsageWindow::duration_label(Some(300)).as_deref(),
            Some("usage.window.fiveHours")
        );
        assert_eq!(
            UsageWindow::duration_label(Some(10080)).as_deref(),
            Some("usage.window.weekly")
        );
        assert_eq!(
            UsageWindow::duration_label(Some(60)).as_deref(),
            Some("usage.window.durationMinutes")
        );
        assert_eq!(UsageWindow::duration_label(None), None);
    }

    #[test]
    fn snapshot_serializes_camel_case_and_omits_empty() {
        let snapshot = ProfileUsageSnapshot {
            profile_id: Uuid::nil(),
            plan_type: Some("plus".into()),
            primary: Some(UsageWindow::normalized("codex", None, 10.0, Some(300), None, None).0),
            secondary: None,
            additional_windows: Vec::new(),
            fetched_at: DateTime::from_timestamp(1_750_000_000, 0).unwrap(),
            source: UsageSource::AppServer,
            protocol_anomaly: false,
            reset_credits: None,
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["source"], "appServer");
        assert_eq!(value["planType"], "plus");
        assert_eq!(value["protocolAnomaly"], false);
        assert_eq!(value["primary"]["usedPercent"], 10.0);
        assert_eq!(value["primary"]["remainingPercent"], 90.0);
        assert!(value.get("secondary").is_none());
        assert!(value.get("additionalWindows").is_none());
        // Round-trip.
        let back: ProfileUsageSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(back, snapshot);
    }
}
