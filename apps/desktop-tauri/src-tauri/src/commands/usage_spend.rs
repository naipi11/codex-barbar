//! Read-only Usage & Spend bridge command.
//!
//! Combines the selected profile's official weekly allowance with a strict
//! local token/cost report. No reset redemption, purchase, account mutation,
//! or arbitrary file access exists here.

use std::sync::Mutex;

use chrono::{DateTime, Duration, Local, Utc};
use codexbar::core::{Freshness, ProfileUsageSnapshot, ProfileUsageState, UsageWindow};
use codexbar::usage_spend::{
    CodexUsageRange, LocalUsageSpendError, LocalUsageSpendReport, scan_local_codex_usage,
};
use tauri::Manager;

use super::bridge::{
    DailyUsageSpendDto, LocalUsageSpendDto, ModelUsageSpendDto, OfficialUsageDto,
    ResetCreditsStateDto, UsageSpendDto,
};
use crate::state::AppState;

const UNIVERSAL_WEEKLY_MINUTES: u64 = 10_080;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageSpendRangeDto {
    Today,
    Last7Days,
    Last30Days,
    CurrentWeekly,
}

impl UsageSpendRangeDto {
    fn as_str(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Last7Days => "last7Days",
            Self::Last30Days => "last30Days",
            Self::CurrentWeekly => "currentWeekly",
        }
    }
}

fn universal_weekly_window(snapshot: &ProfileUsageSnapshot) -> Option<&UsageWindow> {
    snapshot
        .primary
        .iter()
        .chain(snapshot.secondary.iter())
        .chain(snapshot.additional_windows.iter())
        .find(|window| window.window_duration_minutes == Some(UNIVERSAL_WEEKLY_MINUTES))
}

fn official_dto(state: &ProfileUsageState) -> OfficialUsageDto {
    let snapshot = state.snapshot.as_ref();
    let weekly = snapshot.and_then(universal_weekly_window);
    let freshness = match state.freshness {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "stale",
        Freshness::Missing => "missing",
    };
    let (reset_state, available_count, observed_at) =
        match snapshot.and_then(|snapshot| snapshot.reset_credits.as_ref()) {
            Some(summary) if state.freshness == Freshness::Fresh => (
                "available",
                Some(summary.available_count),
                snapshot.map(|snapshot| snapshot.fetched_at.to_rfc3339()),
            ),
            Some(summary) => (
                "stale",
                Some(summary.available_count),
                snapshot.map(|snapshot| snapshot.fetched_at.to_rfc3339()),
            ),
            None => ("unsupported", None, None),
        };
    OfficialUsageDto {
        remaining_percent: weekly.map(|window| window.remaining_percent.round() as u8),
        resets_at: weekly
            .and_then(|window| window.resets_at)
            .map(|value| value.to_rfc3339()),
        fetched_at: snapshot.map(|snapshot| snapshot.fetched_at.to_rfc3339()),
        freshness,
        reset_credits: ResetCreditsStateDto {
            state: reset_state,
            available_count,
            observed_at,
        },
    }
}

fn resolve_range(
    range: UsageSpendRangeDto,
    weekly_resets_at: Option<DateTime<Utc>>,
) -> Result<CodexUsageRange, ()> {
    let today = Local::now().date_naive();
    match range {
        UsageSpendRangeDto::Today => Ok(CodexUsageRange {
            start: today,
            end: today,
        }),
        UsageSpendRangeDto::Last7Days => Ok(CodexUsageRange {
            start: today - Duration::days(6),
            end: today,
        }),
        UsageSpendRangeDto::Last30Days => Ok(CodexUsageRange {
            start: today - Duration::days(29),
            end: today,
        }),
        UsageSpendRangeDto::CurrentWeekly => {
            let Some(resets_at) = weekly_resets_at else {
                return Err(());
            };
            let end = resets_at.date_naive();
            let start = end - Duration::days(7);
            if start > today || end < today - Duration::days(400) {
                return Err(());
            }
            Ok(CodexUsageRange { start, end })
        }
    }
}

fn local_unavailable(range: UsageSpendRangeDto) -> LocalUsageSpendDto {
    LocalUsageSpendDto {
        attribution: "deviceCombined",
        range: range.as_str(),
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        sessions_count: 0,
        estimated_cost_usd: None,
        unknown_models: Vec::new(),
        daily: Vec::new(),
        models: Vec::new(),
        state: "unavailable",
        malformed_records_skipped: 0,
    }
}

fn local_dto(report: LocalUsageSpendReport, range: UsageSpendRangeDto) -> LocalUsageSpendDto {
    let state = if report.sessions_count == 0 && report.total_tokens == 0 {
        "empty"
    } else {
        "ready"
    };
    LocalUsageSpendDto {
        attribution: "deviceCombined",
        range: range.as_str(),
        input_tokens: report.input_tokens,
        cached_input_tokens: report.cached_input_tokens,
        output_tokens: report.output_tokens,
        total_tokens: report.total_tokens,
        sessions_count: report.sessions_count,
        estimated_cost_usd: report.estimated_cost_usd,
        unknown_models: report.unknown_models,
        daily: report
            .daily
            .into_iter()
            .map(|row| DailyUsageSpendDto {
                date: row.date.format("%Y-%m-%d").to_string(),
                total_tokens: row.total_tokens,
                estimated_cost_usd: row.estimated_cost_usd,
            })
            .collect(),
        models: report
            .models
            .into_iter()
            .map(|row| ModelUsageSpendDto {
                model: row.model,
                input_tokens: row.input_tokens,
                cached_input_tokens: row.cached_input_tokens,
                output_tokens: row.output_tokens,
                total_tokens: row.total_tokens,
                estimated_cost_usd: row.estimated_cost_usd,
            })
            .collect(),
        state,
        malformed_records_skipped: report.malformed_records_skipped,
    }
}

fn cache_root() -> std::path::PathBuf {
    codexbar::app_paths::AppPaths::discover()
        .map(|paths| paths.root)
        .unwrap_or_else(|_| std::env::temp_dir().join("codex-barbar"))
}

#[tauri::command]
pub async fn get_usage_spend(
    app: tauri::AppHandle,
    range: UsageSpendRangeDto,
) -> Result<UsageSpendDto, String> {
    let service = app
        .state::<Mutex<AppState>>()
        .lock()
        .map_err(|_| "USAGE_SPEND_STATE_UNAVAILABLE".to_string())?
        .account_service
        .clone();
    let Some(service) = service else {
        return Ok(UsageSpendDto {
            official: official_dto(&ProfileUsageState::missing(uuid::Uuid::nil())),
            local: local_unavailable(range),
        });
    };
    let Ok(snapshot) = service.snapshot() else {
        return Ok(UsageSpendDto {
            official: official_dto(&ProfileUsageState::missing(uuid::Uuid::nil())),
            local: local_unavailable(range),
        });
    };
    let Ok(usage_state) = service
        .repositories()
        .usage
        .load_state(snapshot.selected_profile_id)
    else {
        return Ok(UsageSpendDto {
            official: official_dto(&ProfileUsageState::missing(snapshot.selected_profile_id)),
            local: local_unavailable(range),
        });
    };

    let official = official_dto(&usage_state);
    let weekly_resets_at = usage_state
        .snapshot
        .as_ref()
        .and_then(universal_weekly_window)
        .and_then(|window| window.resets_at);
    let codex_range = match resolve_range(range, weekly_resets_at) {
        Ok(codex_range) => codex_range,
        Err(()) => {
            return Ok(UsageSpendDto {
                official,
                local: local_unavailable(range),
            });
        }
    };

    let cache_root = cache_root();
    let scanned = tauri::async_runtime::spawn_blocking(move || {
        scan_local_codex_usage(codex_range, &cache_root, None)
    })
    .await
    .map_err(|_| "USAGE_SPEND_SCAN_FAILED".to_string())?;

    let local = match scanned {
        Ok(report) => local_dto(report, range),
        Err(LocalUsageSpendError::Cancelled) => LocalUsageSpendDto {
            state: "cancelled",
            ..local_unavailable(range)
        },
        Err(LocalUsageSpendError::InvalidRange) => local_unavailable(range),
    };
    Ok(UsageSpendDto { official, local })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use codexbar::core::{AppError, AppErrorKind, RecoveryAction, RefreshStatus, UsageSource};
    use uuid::Uuid;

    fn snapshot_with(
        weekly_remaining: f64,
        five_hour_remaining: f64,
        reset_count: Option<u64>,
        resets_at: Option<DateTime<Utc>>,
    ) -> ProfileUsageSnapshot {
        ProfileUsageSnapshot {
            profile_id: Uuid::nil(),
            plan_type: Some("plus".into()),
            primary: Some(
                UsageWindow::normalized(
                    "five-hour",
                    None,
                    100.0 - five_hour_remaining,
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
                    100.0 - weekly_remaining,
                    Some(10_080),
                    resets_at,
                    None,
                )
                .0,
            ),
            additional_windows: Vec::new(),
            fetched_at: Utc.with_ymd_and_hms(2026, 8, 23, 1, 2, 3).unwrap(),
            source: UsageSource::AppServer,
            protocol_anomaly: false,
            reset_credits: reset_count
                .map(|available_count| codexbar::core::ResetCreditsSummary { available_count }),
        }
    }

    fn usage_state(
        snapshot: Option<ProfileUsageSnapshot>,
        freshness: Freshness,
    ) -> ProfileUsageState {
        ProfileUsageState {
            profile_id: Uuid::nil(),
            snapshot,
            current_error: (freshness == Freshness::Stale).then(|| {
                AppError::new(
                    AppErrorKind::OfflineOrTimeout,
                    "errors.offlineOrTimeout",
                    RecoveryAction::Retry,
                    "APP_SERVER_RPC_TIMEOUT",
                )
            }),
            refresh_status: RefreshStatus::Idle,
            freshness,
            manual_cooldown_until: None,
        }
    }

    #[test]
    fn bridge_uses_only_the_universal_weekly_window() {
        let state = usage_state(
            Some(snapshot_with(
                99.0,
                2.0,
                Some(2),
                Some(Utc.with_ymd_and_hms(2026, 8, 30, 0, 0, 0).unwrap()),
            )),
            Freshness::Fresh,
        );
        let dto = official_dto(&state);
        assert_eq!(dto.remaining_percent, Some(99));
        assert_eq!(dto.reset_credits.available_count, Some(2));
        assert_eq!(dto.reset_credits.state, "available");
        assert_ne!(dto.remaining_percent, Some(2));
        assert_eq!(dto.freshness, "fresh");
    }

    #[test]
    fn reset_credits_zero_is_available_and_missing_is_unsupported() {
        let zero = official_dto(&usage_state(
            Some(snapshot_with(50.0, 50.0, Some(0), None)),
            Freshness::Fresh,
        ));
        assert_eq!(zero.reset_credits.state, "available");
        assert_eq!(zero.reset_credits.available_count, Some(0));

        let unsupported = official_dto(&usage_state(
            Some(snapshot_with(50.0, 50.0, None, None)),
            Freshness::Fresh,
        ));
        assert_eq!(unsupported.reset_credits.state, "unsupported");
        assert_eq!(unsupported.reset_credits.available_count, None);
    }

    #[test]
    fn stale_snapshot_with_credits_reports_stale_state() {
        let dto = official_dto(&usage_state(
            Some(snapshot_with(50.0, 50.0, Some(1), None)),
            Freshness::Stale,
        ));
        assert_eq!(dto.reset_credits.state, "stale");
        assert_eq!(dto.reset_credits.available_count, Some(1));
        assert_eq!(dto.freshness, "stale");
    }

    #[test]
    fn missing_snapshot_has_no_official_window() {
        let dto = official_dto(&usage_state(None, Freshness::Missing));
        assert_eq!(dto.remaining_percent, None);
        assert_eq!(dto.freshness, "missing");
        assert_eq!(dto.reset_credits.state, "unsupported");
    }

    #[test]
    fn current_weekly_range_uses_the_trusted_reset_time() {
        let resets_at = Utc.with_ymd_and_hms(2026, 8, 30, 0, 0, 0).unwrap();
        let range = resolve_range(UsageSpendRangeDto::CurrentWeekly, Some(resets_at)).unwrap();
        assert_eq!(range.start.to_string(), "2026-08-23");
        assert_eq!(range.end.to_string(), "2026-08-30");
    }

    #[test]
    fn current_weekly_without_reset_time_or_implausible_time_is_unavailable() {
        assert!(resolve_range(UsageSpendRangeDto::CurrentWeekly, None).is_err());
        let ancient = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        assert!(resolve_range(UsageSpendRangeDto::CurrentWeekly, Some(ancient)).is_err());
    }

    #[test]
    fn invalid_range_string_is_rejected_at_deserialization() {
        let error = serde_json::from_str::<UsageSpendRangeDto>(r#""lastYear""#).unwrap_err();
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn local_dto_marks_empty_reports_and_cancelled_scans_distinctly() {
        let empty = LocalUsageSpendReport {
            range: CodexUsageRange {
                start: Local::now().date_naive(),
                end: Local::now().date_naive(),
            },
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            sessions_count: 0,
            estimated_cost_usd: None,
            unknown_models: Vec::new(),
            daily: Vec::new(),
            models: Vec::new(),
            malformed_records_skipped: 0,
        };
        assert_eq!(local_dto(empty, UsageSpendRangeDto::Today).state, "empty");

        let cancelled = LocalUsageSpendDto {
            state: "cancelled",
            ..local_unavailable(UsageSpendRangeDto::Today)
        };
        assert_eq!(cancelled.state, "cancelled");
    }
}
