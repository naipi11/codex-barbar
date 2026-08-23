//! Tolerant account and rate-limit wire models.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::accounts::identity::AccountStatus;
use crate::core::{
    AppError, AppErrorKind, AuthMode, ProfileId, ProfileUsageSnapshot, RecoveryAction,
    ResetCreditsSummary, UsageSource, UsageWindow,
};

/// Account identity returned by `account/read`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountIdentity {
    pub auth_mode: AuthMode,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
}

impl AccountIdentity {
    pub fn status(&self) -> AccountStatus {
        AccountStatus::SignedIn
    }

    pub fn from_value(value: Value) -> Result<Self, AppError> {
        let account_value = if value.get("account").is_some() {
            value.get("account").expect("checked above")
        } else if value.get("type").is_some() {
            &value
        } else {
            return Err(model_error(
                AppErrorKind::NotSignedIn,
                "APP_SERVER_ACCOUNT_MISSING",
            ));
        };
        let account = account_value
            .as_object()
            .ok_or_else(|| model_error(AppErrorKind::NotSignedIn, "APP_SERVER_ACCOUNT_MISSING"))?;
        let account_type =
            string_field(account, &["type", "authType", "authMode"]).ok_or_else(|| {
                model_error(AppErrorKind::NotSignedIn, "APP_SERVER_ACCOUNT_TYPE_MISSING")
            })?;
        let normalized = account_type
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        let auth_mode = match normalized.as_str() {
            "chatgpt" | "oauth" | "oauth2" | "chatgptoauth" => AuthMode::ChatGpt,
            "apikey" | "api" | "token" => AuthMode::ApiKey,
            _ => {
                return Err(model_error(
                    AppErrorKind::ProtocolMismatch,
                    "APP_SERVER_UNKNOWN_ACCOUNT_TYPE",
                ));
            }
        };
        Ok(Self {
            auth_mode,
            display_name: string_field(
                account,
                &[
                    "displayName",
                    "display_name",
                    "name",
                    "fullName",
                    "full_name",
                ],
            ),
            email: string_field(account, &["email", "emailAddress"]),
            plan_type: string_field(account, &["planType", "plan_type", "plan"]),
        })
    }
}

/// Parsed quota windows for the selected Codex bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRateLimits {
    pub selected_limit_id: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub additional_windows: Vec<UsageWindow>,
    pub reset_credits: Option<ResetCreditsSummary>,
    pub protocol_anomaly: bool,
}

impl ParsedRateLimits {
    pub fn empty() -> Self {
        Self {
            selected_limit_id: None,
            primary: None,
            secondary: None,
            additional_windows: Vec::new(),
            reset_credits: None,
            protocol_anomaly: false,
        }
    }

    pub fn from_value(value: Value) -> Result<Self, AppError> {
        let root = value.as_object().ok_or_else(|| {
            model_error(
                AppErrorKind::ProtocolMismatch,
                "APP_SERVER_INVALID_RATE_LIMITS",
            )
        })?;
        let (reset_credits, credit_anomaly) = parse_reset_credits(root);

        if let Some(by_id) = root.get("rateLimitsByLimitId").and_then(Value::as_object)
            && let Some(codex) = get_case_insensitive(by_id, "codex")
        {
            let mut parsed = parse_bucket("codex", codex)?;
            parsed.reset_credits = reset_credits;
            parsed.protocol_anomaly |= credit_anomaly;
            for (limit_id, bucket) in by_id {
                if !limit_id.eq_ignore_ascii_case("codex") {
                    let extra = parse_bucket_windows(limit_id, bucket)?;
                    parsed.additional_windows.extend(extra.windows);
                    parsed.protocol_anomaly |= extra.protocol_anomaly;
                }
            }
            parsed.selected_limit_id = Some("codex".to_string());
            return Ok(parsed);
        }

        if let Some(legacy) = root.get("rateLimits") {
            let mut parsed = parse_bucket("codex", legacy)?;
            parsed.reset_credits = reset_credits;
            parsed.protocol_anomaly |= credit_anomaly;
            parsed.selected_limit_id = Some("codex".to_string());
            return Ok(parsed);
        }

        // Some compatibility builds return the selected bucket directly.
        if root.contains_key("primary")
            || root.contains_key("secondary")
            || root.contains_key("usedPercent")
            || root.contains_key("used_percent")
        {
            let mut parsed = parse_bucket("codex", &value)?;
            parsed.reset_credits = reset_credits;
            parsed.protocol_anomaly |= credit_anomaly;
            parsed.selected_limit_id = Some("codex".to_string());
            return Ok(parsed);
        }

        Err(model_error(
            AppErrorKind::ProtocolMismatch,
            "APP_SERVER_RATE_LIMITS_MISSING",
        ))
    }
}

/// Map account identity plus parsed windows into the public product snapshot.
pub fn parse_profile_usage(
    profile_id: ProfileId,
    account: AccountIdentity,
    rates: ParsedRateLimits,
    fetched_at: DateTime<Utc>,
) -> Result<ProfileUsageSnapshot, AppError> {
    if account.auth_mode == AuthMode::ApiKey {
        return Err(model_error(
            AppErrorKind::ApiKeyNoQuota,
            "APP_SERVER_API_KEY_NO_QUOTA",
        ));
    }
    if rates.primary.is_none() && rates.secondary.is_none() && rates.additional_windows.is_empty() {
        return Err(model_error(
            AppErrorKind::NotSignedIn,
            "APP_SERVER_NO_RATE_LIMITS",
        ));
    }
    Ok(ProfileUsageSnapshot {
        profile_id,
        plan_type: account.plan_type,
        primary: rates.primary,
        secondary: rates.secondary,
        additional_windows: rates.additional_windows,
        fetched_at,
        source: UsageSource::AppServer,
        protocol_anomaly: rates.protocol_anomaly,
        reset_credits: rates.reset_credits,
    })
}

fn model_error(kind: AppErrorKind, diagnostic_code: &'static str) -> AppError {
    AppError::new(
        kind,
        "errors.appServerModelMismatch",
        RecoveryAction::InstallTestedCodex,
        diagnostic_code,
    )
}

fn string_field(object: &Map<String, Value>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        object
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn get_case_insensitive<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    object
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

fn parse_bucket(limit_id: &str, value: &Value) -> Result<ParsedRateLimits, AppError> {
    let parsed = parse_bucket_windows(limit_id, value)?;
    let mut primary = None;
    let mut secondary = None;
    let mut additional = Vec::new();

    for window in parsed.windows {
        if window.limit_id == format!("{limit_id}:primary") {
            primary = Some(window);
        } else if window.limit_id == format!("{limit_id}:secondary") {
            secondary = Some(window);
        } else if primary.is_none() && window.window_duration_minutes == Some(300) {
            primary = Some(window);
        } else if secondary.is_none() && window.window_duration_minutes == Some(10_080) {
            secondary = Some(window);
        } else {
            additional.push(window);
        }
    }
    Ok(ParsedRateLimits {
        selected_limit_id: Some(limit_id.to_string()),
        primary,
        secondary,
        additional_windows: additional,
        reset_credits: None,
        protocol_anomaly: parsed.protocol_anomaly,
    })
}

/// Parse only the redacted reset-credit count from the root object.
///
/// Returns `(summary, anomaly)`. A missing or explicit `null` field yields
/// `(None, false)`; a non-negative integral `availableCount` yields a count;
/// strings, negatives, fractions, or unrelated shapes yield `(None, true)`.
fn parse_reset_credits(root: &Map<String, Value>) -> (Option<ResetCreditsSummary>, bool) {
    let Some(credits) = root.get("rateLimitResetCredits") else {
        return (None, false);
    };
    if credits.is_null() {
        return (None, false);
    }
    let Some(credits) = credits.as_object() else {
        return (None, true);
    };
    let Some(available) = credits.get("availableCount") else {
        return (None, false);
    };
    match available {
        Value::Number(number) => {
            if let Some(count) = number.as_u64() {
                return (
                    Some(ResetCreditsSummary {
                        available_count: count,
                    }),
                    false,
                );
            }
            (None, true)
        }
        Value::String(text) => match text.trim().parse::<u64>() {
            Ok(count) => (
                Some(ResetCreditsSummary {
                    available_count: count,
                }),
                false,
            ),
            Err(_) => (None, true),
        },
        _ => (None, true),
    }
}

struct BucketWindows {
    windows: Vec<UsageWindow>,
    protocol_anomaly: bool,
}

fn parse_bucket_windows(limit_id: &str, value: &Value) -> Result<BucketWindows, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| model_error(AppErrorKind::ProtocolMismatch, "APP_SERVER_INVALID_BUCKET"))?;
    let mut windows = Vec::new();
    let mut protocol_anomaly = false;

    for name in ["primary", "secondary"] {
        if let Some(window_value) = object.get(name) {
            match parse_window(&format!("{limit_id}:{name}"), None, window_value) {
                Ok((Some(window), anomaly)) => {
                    windows.push(window);
                    protocol_anomaly |= anomaly;
                }
                Ok((None, anomaly)) => protocol_anomaly |= anomaly,
                Err(_) => protocol_anomaly = true,
            }
        }
    }

    for (name, window_value) in object {
        if matches!(
            name.as_str(),
            "primary" | "secondary" | "limitId" | "limit_id" | "futureField" | "updatedAt"
        ) {
            continue;
        }
        if !window_value.is_object() {
            continue;
        }
        match parse_window(
            &format!("{limit_id}:{name}"),
            Some(name.as_str()),
            window_value,
        ) {
            Ok((Some(window), anomaly)) => {
                windows.push(window);
                protocol_anomaly |= anomaly;
            }
            Ok((None, anomaly)) => protocol_anomaly |= anomaly,
            Err(_) => protocol_anomaly = true,
        }
    }

    // A bucket can itself be a single window rather than an object containing
    // named primary/secondary keys.
    if windows.is_empty()
        && (object.contains_key("usedPercent") || object.contains_key("used_percent"))
    {
        match parse_window(limit_id, None, value) {
            Ok((Some(window), anomaly)) => {
                windows.push(window);
                protocol_anomaly |= anomaly;
            }
            Ok((None, anomaly)) => protocol_anomaly |= anomaly,
            Err(_) => protocol_anomaly = true,
        }
    }

    Ok(BucketWindows {
        windows,
        protocol_anomaly,
    })
}

fn parse_window(
    limit_id: &str,
    explicit_label: Option<&str>,
    value: &Value,
) -> Result<(Option<UsageWindow>, bool), AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| model_error(AppErrorKind::ProtocolMismatch, "APP_SERVER_INVALID_WINDOW"))?;
    let used_value = first_value(
        object,
        &["usedPercent", "used_percent", "percentUsed", "used"],
    );
    let Some(raw_used) = used_value.and_then(parse_finite_f64) else {
        return Ok((None, true));
    };
    let raw_duration = first_value(
        object,
        &[
            "windowDurationMins",
            "windowDurationMinutes",
            "window_duration_minutes",
            "durationMinutes",
        ],
    );
    let duration = raw_duration.and_then(parse_u64);
    let mut anomaly = used_value.is_none() || raw_duration.is_some() && duration.is_none();
    let resets_at = match first_value(object, &["resetsAt", "resets_at"]) {
        Some(value) => match parse_unix_timestamp(value) {
            Some(timestamp) => Some(timestamp),
            None => {
                anomaly = true;
                None
            }
        },
        None => None,
    };
    let explicit_wire_label = string_field(object, &["label", "windowLabel"]);
    let label = explicit_wire_label.or_else(|| {
        explicit_label
            .and_then(|label| (!label.is_empty()).then(|| label.to_string()))
            .or_else(|| UsageWindow::duration_label(duration))
    });
    let reached_type = string_field(object, &["reachedType", "reached_type"]);
    let (window, percent_anomaly) = UsageWindow::normalized(
        limit_id.to_string(),
        label,
        raw_used,
        duration,
        resets_at,
        reached_type,
    );
    anomaly |= percent_anomaly;
    Ok((Some(window), anomaly))
}

fn first_value<'a>(object: &'a Map<String, Value>, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| object.get(*name))
}

fn parse_finite_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64().filter(|value| value.is_finite()),
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite()),
        _ => None,
    }
}

fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_unix_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let seconds = match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }?;
    DateTime::from_timestamp(seconds, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::identity::AccountStatus;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn reset_credit_known_count_is_parsed_and_redacted() {
        let parsed =
            ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-known.json")).unwrap();
        assert_eq!(parsed.reset_credits.as_ref().unwrap().available_count, 2);
        let credits_text = serde_json::to_value(&parsed.reset_credits)
            .unwrap()
            .to_string()
            .to_ascii_lowercase();
        for forbidden in ["id", "title", "grantedat", "redeem"] {
            assert!(!credits_text.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn reset_credit_zero_count_is_distinct_from_missing() {
        let parsed =
            ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-zero.json")).unwrap();
        assert_eq!(parsed.reset_credits.unwrap().available_count, 0);
        assert!(!parsed.protocol_anomaly);
    }

    #[test]
    fn reset_credit_malformed_shape_sets_anomaly_and_exposes_no_summary() {
        let parsed =
            ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-malformed.json"))
                .unwrap();
        assert!(parsed.reset_credits.is_none());
        assert!(parsed.protocol_anomaly);
    }

    #[test]
    fn missing_or_null_reset_credits_are_not_anomalous() {
        for value in [
            serde_json::json!({"rateLimits": {"primary": {"usedPercent": 1}}}),
            serde_json::json!({
                "rateLimits": {"primary": {"usedPercent": 1}},
                "rateLimitResetCredits": null
            }),
        ] {
            let parsed = ParsedRateLimits::from_value(value).unwrap();
            assert!(parsed.reset_credits.is_none());
            assert!(!parsed.protocol_anomaly);
        }
    }

    #[test]
    fn negative_and_fractional_reset_counts_are_anomalous() {
        for raw in ["-1", "1.5", "true", "{}"] {
            let value = serde_json::json!({
                "rateLimits": {"primary": {"usedPercent": 1}},
                "rateLimitResetCredits": {"availableCount": serde_json::from_str::<serde_json::Value>(raw).unwrap()}
            });
            let parsed = ParsedRateLimits::from_value(value).unwrap();
            assert!(parsed.reset_credits.is_none());
            assert!(parsed.protocol_anomaly);
        }
    }

    #[test]
    fn parse_profile_usage_carries_the_reset_credit_summary() {
        let account = AccountIdentity {
            auth_mode: AuthMode::ChatGpt,
            display_name: None,
            email: Some("user@example.com".into()),
            plan_type: Some("plus".into()),
        };
        let rates =
            ParsedRateLimits::from_value(fixture("rate-limits-reset-credits-known.json")).unwrap();
        let snapshot = parse_profile_usage(id(), account, rates, Utc::now()).unwrap();
        assert_eq!(snapshot.reset_credits.as_ref().unwrap().available_count, 2);
    }

    fn fixture(name: &str) -> serde_json::Value {
        let raw = match name {
            "account-chatgpt.json" => include_str!("fixtures/account-chatgpt.json"),
            "account-api-key.json" => include_str!("fixtures/account-api-key.json"),
            "rate-limits-by-id.json" => include_str!("fixtures/rate-limits-by-id.json"),
            "rate-limits-legacy.json" => include_str!("fixtures/rate-limits-legacy.json"),
            "rate-limits-anomaly.json" => include_str!("fixtures/rate-limits-anomaly.json"),
            "rate-limits-reset-credits-known.json" => {
                include_str!("fixtures/rate-limits-reset-credits-known.json")
            }
            "rate-limits-reset-credits-zero.json" => {
                include_str!("fixtures/rate-limits-reset-credits-zero.json")
            }
            "rate-limits-reset-credits-malformed.json" => {
                include_str!("fixtures/rate-limits-reset-credits-malformed.json")
            }
            _ => panic!("unknown fixture {name}"),
        };
        serde_json::from_str(raw).unwrap()
    }

    fn id() -> ProfileId {
        Uuid::nil()
    }

    #[test]
    fn selects_named_codex_bucket_without_object_order_dependency() {
        let value = fixture("rate-limits-by-id.json");
        let parsed = ParsedRateLimits::from_value(value).unwrap();
        assert_eq!(parsed.selected_limit_id.as_deref(), Some("codex"));
        assert_eq!(
            parsed.primary.as_ref().unwrap().window_duration_minutes,
            Some(300)
        );
        assert_eq!(
            parsed.secondary.as_ref().unwrap().window_duration_minutes,
            Some(10_080)
        );
    }

    #[test]
    fn clamps_abnormal_percent_and_marks_protocol_anomaly() {
        let parsed = ParsedRateLimits::from_value(fixture("rate-limits-anomaly.json")).unwrap();
        assert_eq!(parsed.primary.as_ref().unwrap().used_percent, 100.0);
        assert!(parsed.protocol_anomaly);
    }

    #[test]
    fn api_key_identity_maps_to_no_quota() {
        let account = AccountIdentity::from_value(fixture("account-api-key.json")).unwrap();
        let error =
            parse_profile_usage(id(), account, ParsedRateLimits::empty(), Utc::now()).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::ApiKeyNoQuota);
    }

    #[test]
    fn chatgpt_account_extracts_identity() {
        let account = AccountIdentity::from_value(fixture("account-chatgpt.json")).unwrap();
        assert_eq!(account.auth_mode, AuthMode::ChatGpt);
        assert_eq!(account.display_name, None);
        assert_eq!(account.email.as_deref(), Some("user@example.com"));
        assert_eq!(account.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn chatgpt_account_with_email_only_is_signed_in() {
        let account = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "email": "user@example.com",
                "planType": "plus"
            }
        }))
        .unwrap();

        assert_eq!(account.status(), AccountStatus::SignedIn);
    }

    #[test]
    fn display_name_precedes_email_and_accepts_camel_case() {
        let account = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "displayName": "  Ming Zhao  ",
                "email": "user@example.com"
            }
        }))
        .unwrap();

        assert_eq!(account.display_name.as_deref(), Some("Ming Zhao"));
        assert_eq!(account.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn snake_case_name_and_full_name_are_supported() {
        let name = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "display_name": "",
                "name": "Named User",
                "fullName": "Full User"
            }
        }))
        .unwrap();
        assert_eq!(name.display_name.as_deref(), Some("Named User"));

        let full_name = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "full_name": "Full User"
            }
        }))
        .unwrap();
        assert_eq!(full_name.display_name.as_deref(), Some("Full User"));
    }

    #[test]
    fn empty_name_falls_back_to_email() {
        let account = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "displayName": " ",
                "emailAddress": "fallback@example.com"
            }
        }))
        .unwrap();

        assert_eq!(account.display_name, None);
        assert_eq!(account.email.as_deref(), Some("fallback@example.com"));
    }

    #[test]
    fn identity_parser_ignores_token_and_cookie_fields() {
        let account = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "displayName": "Safe Name",
                "email": "safe@example.com",
                "token": "secret-token",
                "cookie": "secret-cookie"
            }
        }))
        .unwrap();
        let debug = format!("{account:?}");

        assert!(debug.contains("Safe Name"));
        assert!(debug.contains("safe@example.com"));
        assert!(!debug.contains("secret-token"));
        assert!(!debug.contains("secret-cookie"));
    }

    #[test]
    fn legacy_rate_limits_field_is_supported() {
        let parsed = ParsedRateLimits::from_value(fixture("rate-limits-legacy.json")).unwrap();
        assert_eq!(parsed.selected_limit_id.as_deref(), Some("codex"));
        assert_eq!(parsed.primary.as_ref().unwrap().used_percent, 12.5);
    }

    #[test]
    fn known_durations_receive_localization_labels_and_unexpired_reset() {
        let parsed = ParsedRateLimits::from_value(serde_json::json!({
            "rateLimits": {
                "primary": {
                    "usedPercent": 1,
                    "windowDurationMins": 300,
                    "resetsAt": 1750000000
                },
                "secondary": {
                    "usedPercent": 2,
                    "windowDurationMins": 10080,
                    "resetsAt": 1
                }
            }
        }))
        .unwrap();
        assert_eq!(
            parsed.primary.as_ref().unwrap().label.as_deref(),
            Some("usage.window.fiveHours")
        );
        assert_eq!(
            parsed.secondary.as_ref().unwrap().label.as_deref(),
            Some("usage.window.weekly")
        );
        assert_eq!(
            parsed.secondary.as_ref().unwrap().resets_at,
            chrono::DateTime::from_timestamp(1, 0)
        );
    }

    #[test]
    fn invalid_numeric_values_omit_only_the_bad_window_and_mark_anomaly() {
        let parsed = ParsedRateLimits::from_value(serde_json::json!({
            "rateLimits": {
                "primary": {"usedPercent": "oops", "windowDurationMins": 300},
                "secondary": {"usedPercent": "45.5", "windowDurationMins": "10080"}
            }
        }))
        .unwrap();
        assert!(parsed.primary.is_none());
        assert_eq!(parsed.secondary.as_ref().unwrap().used_percent, 45.5);
        assert!(parsed.protocol_anomaly);
    }

    #[test]
    fn missing_account_is_not_signed_in() {
        let error = AccountIdentity::from_value(serde_json::json!({})).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::NotSignedIn);
        assert_eq!(error.diagnostic_code, "APP_SERVER_ACCOUNT_MISSING");
    }

    #[test]
    fn additional_buckets_and_windows_are_preserved() {
        let parsed = ParsedRateLimits::from_value(fixture("rate-limits-by-id.json")).unwrap();
        assert!(
            parsed
                .additional_windows
                .iter()
                .any(|window| window.limit_id.starts_with("other:"))
        );
    }

    #[test]
    fn out_of_range_percent_is_clamped_and_anomalous() {
        let parsed = ParsedRateLimits::from_value(serde_json::json!({
            "rateLimits": {
                "primary": {"usedPercent": -10, "windowDurationMins": 60}
            }
        }))
        .unwrap();
        let primary = parsed.primary.unwrap();
        assert_eq!(primary.used_percent, 0.0);
        assert_eq!(primary.remaining_percent, 100.0);
        assert!(parsed.protocol_anomaly);
    }

    #[test]
    fn profile_snapshot_maps_source_and_plan() {
        let account = AccountIdentity {
            auth_mode: AuthMode::ChatGpt,
            display_name: None,
            email: Some("user@example.com".into()),
            plan_type: Some("plus".into()),
        };
        let rates = ParsedRateLimits::from_value(fixture("rate-limits-legacy.json")).unwrap();
        let snapshot = parse_profile_usage(id(), account, rates, Utc::now()).unwrap();
        assert_eq!(snapshot.source, UsageSource::AppServer);
        assert_eq!(snapshot.plan_type.as_deref(), Some("plus"));
    }
}
