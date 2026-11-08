//! Tolerant account and rate-limit wire models.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::accounts::identity::AccountStatus;
use crate::core::{
    AppError, AppErrorKind, AuthMode, ProfileId, ProfileUsageSnapshot, RecoveryAction,
    ResetCreditsSummary, UsageSource, UsageWindow,
};

/// Account identity returned by `account/read`.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountIdentity {
    pub auth_mode: AuthMode,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub avatar_candidate: Option<String>,
}

impl std::fmt::Debug for AccountIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountIdentity")
            .field("auth_mode", &self.auth_mode)
            .field("username", &self.username)
            .field("display_name", &self.display_name)
            .field("email", &self.email)
            .field("plan_type", &self.plan_type)
            .field(
                "avatar_candidate",
                &self.avatar_candidate.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl AccountIdentity {
    pub fn status(&self) -> AccountStatus {
        AccountStatus::SignedIn
    }

    pub fn from_value(value: Value) -> Result<Self, AppError> {
        if !value.is_object() {
            return Err(model_error(
                AppErrorKind::ProtocolMismatch,
                "APP_SERVER_INVALID_ACCOUNT",
            ));
        }
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
        if account_value.is_null() {
            return Err(model_error(
                AppErrorKind::NotSignedIn,
                "APP_SERVER_ACCOUNT_MISSING",
            ));
        }
        let account = account_value.as_object().ok_or_else(|| {
            model_error(AppErrorKind::ProtocolMismatch, "APP_SERVER_INVALID_ACCOUNT")
        })?;
        let account_type =
            string_field(account, &["type", "authType", "authMode"]).ok_or_else(|| {
                model_error(
                    AppErrorKind::ProtocolMismatch,
                    "APP_SERVER_ACCOUNT_TYPE_MISSING",
                )
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
            username: string_field(account, &["handle", "username", "userName"]),
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
            avatar_candidate: string_field(
                account,
                &[
                    "avatarUrl",
                    "avatar_url",
                    "picture",
                    "pictureUrl",
                    "imageUrl",
                ],
            ),
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
        let raw_by_id = root
            .get("rateLimitsByLimitId")
            .filter(|value| !value.is_null());
        let by_id = raw_by_id.and_then(Value::as_object);
        let codex = by_id.and_then(|buckets| get_case_insensitive(buckets, "codex"));
        let mut anomaly = credit_anomaly || raw_by_id.is_some() && by_id.is_none();

        // account/rateLimits/read is a full snapshot, not a sparse notification.
        // A valid named Codex bucket wins even when its optional windows are null.
        let mut parsed = if let Some(codex) = codex.filter(|bucket| bucket.is_object()) {
            parse_bucket("codex", codex)?
        } else {
            anomaly |= codex.is_some();
            let fallback = root.get("rateLimits").or_else(|| {
                // Keep the existing direct-bucket compatibility path.
                (root.contains_key("primary")
                    || root.contains_key("secondary")
                    || root.contains_key("usedPercent")
                    || root.contains_key("used_percent"))
                .then_some(&value)
            });
            let fallback = fallback.ok_or_else(|| {
                model_error(
                    AppErrorKind::ProtocolMismatch,
                    "APP_SERVER_RATE_LIMITS_MISSING",
                )
            })?;
            // The official legacy view can select a non-Codex bucket when no
            // Codex bucket exists. Do not rename it or pick an arbitrary map key.
            let limit_id = fallback
                .as_object()
                .and_then(|bucket| string_field(bucket, &["limitId", "limit_id"]))
                .unwrap_or_else(|| "codex".to_string());
            parse_bucket(&limit_id, fallback)?
        };
        parsed.reset_credits = reset_credits;
        parsed.protocol_anomaly |= anomaly;

        if let Some(by_id) = by_id {
            for (limit_id, bucket) in by_id {
                if parsed
                    .selected_limit_id
                    .as_deref()
                    .is_some_and(|selected| limit_id.eq_ignore_ascii_case(selected))
                {
                    continue;
                }
                match parse_bucket_windows(limit_id, bucket) {
                    Ok(extra) => {
                        parsed.additional_windows.extend(extra.windows);
                        parsed.protocol_anomaly |= extra.protocol_anomaly;
                    }
                    // An unrelated malformed bucket must not discard usable quota.
                    Err(_) => parsed.protocol_anomaly = true,
                }
            }
        }
        Ok(parsed)
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
            AppErrorKind::ProtocolMismatch,
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
    let (message_key, recovery) = match kind {
        AppErrorKind::NotSignedIn => ("errors.notSignedIn", RecoveryAction::SignInWithCli),
        AppErrorKind::ApiKeyNoQuota => ("errors.apiKeyNoQuota", RecoveryAction::None),
        _ => (
            "errors.appServerModelMismatch",
            RecoveryAction::InstallTestedCodex,
        ),
    };
    AppError::new(kind, message_key, recovery, diagnostic_code)
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
/// `(None, false)`; a non-negative integral `availableCount` yields a count.
/// Legacy numeric strings are accepted; missing counts, negatives, fractions,
/// and unrelated shapes yield `(None, true)`. Detail rows are never exposed.
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
        return (None, true);
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
        if let Some(window_value) = object.get(name).filter(|value| !value.is_null()) {
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
            "primary"
                | "secondary"
                | "limitId"
                | "limit_id"
                | "credits"
                | "individualLimit"
                | "futureField"
                | "updatedAt"
        ) {
            continue;
        }
        // Future percentage windows are useful, but arbitrary nested metadata
        // is not quota. In particular, a monetary `used` amount is not a percent.
        if !window_value.as_object().is_some_and(|window| {
            first_value(window, &["usedPercent", "used_percent", "percentUsed"]).is_some()
        }) {
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

    // The official reached reason belongs to the snapshot, not each window.
    // Preserve it without inferring recovery from percentages or reset times.
    if let Some(reached_type) = string_field(object, &["rateLimitReachedType"]) {
        for window in &mut windows {
            if window.reached_type.is_none() {
                window.reached_type = Some(reached_type.clone());
            }
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
    )
    .filter(|value| !value.is_null());
    let duration = raw_duration.and_then(parse_u64);
    let mut anomaly = raw_duration.is_some() && duration.is_none();
    let resets_at =
        match first_value(object, &["resetsAt", "resets_at"]).filter(|value| !value.is_null()) {
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
            username: None,
            display_name: None,
            email: Some("user@example.com".into()),
            plan_type: Some("plus".into()),
            avatar_candidate: None,
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
            "0.167.0/account-chatgpt.json" => {
                include_str!("fixtures/0.167.0/account-chatgpt.json")
            }
            "0.167.0/rate-limits-multi-bucket.json" => {
                include_str!("fixtures/0.167.0/rate-limits-multi-bucket.json")
            }
            "0.167.0/rate-limits-nullable.json" => {
                include_str!("fixtures/0.167.0/rate-limits-nullable.json")
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
    fn identity_parser_extracts_username_and_avatar_candidate_without_debug_leak() {
        let account = AccountIdentity::from_value(serde_json::json!({
            "account": {
                "type": "chatgpt",
                "handle": "  stack  ",
                "displayName": "Stack User",
                "avatarUrl": "https://cdn.openai.com/avatar.png",
                "token": "secret-token",
                "cookie": "secret-cookie"
            }
        }))
        .unwrap();

        assert_eq!(account.username.as_deref(), Some("stack"));
        assert_eq!(
            account.avatar_candidate.as_deref(),
            Some("https://cdn.openai.com/avatar.png")
        );
        let debug = format!("{account:?}");
        assert!(!debug.contains("cdn.openai.com"));
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
    fn official_0167_account_allows_null_email_without_inventing_identity() {
        let account = AccountIdentity::from_value(fixture("0.167.0/account-chatgpt.json")).unwrap();
        assert_eq!(account.auth_mode, AuthMode::ChatGpt);
        assert_eq!(account.status(), AccountStatus::SignedIn);
        assert_eq!(account.email, None);
        assert_eq!(account.display_name, None);
        assert_eq!(account.username, None);
        assert_eq!(account.avatar_candidate, None);
        assert_eq!(
            account.plan_type.as_deref(),
            Some("self_serve_business_usage_based")
        );
    }

    #[test]
    fn official_0167_non_chatgpt_accounts_never_become_chatgpt_quota() {
        for account_type in ["amazonBedrock", "gcp", "futureAccount"] {
            let error = AccountIdentity::from_value(serde_json::json!({
                "account": {"type": account_type},
                "requiresOpenaiAuth": false
            }))
            .unwrap_err();
            assert_eq!(error.kind, AppErrorKind::ProtocolMismatch);
            assert_eq!(error.diagnostic_code, "APP_SERVER_UNKNOWN_ACCOUNT_TYPE");
        }
    }

    #[test]
    fn null_or_omitted_account_maps_to_sign_in_recovery() {
        for value in [
            serde_json::json!({"account": null, "requiresOpenaiAuth": true}),
            serde_json::json!({"requiresOpenaiAuth": true}),
        ] {
            let error = AccountIdentity::from_value(value).unwrap_err();
            assert_eq!(error.kind, AppErrorKind::NotSignedIn);
            assert_eq!(error.recovery, RecoveryAction::SignInWithCli);
            assert_eq!(error.message_key, "errors.notSignedIn");
        }
    }

    #[test]
    fn malformed_account_is_not_misreported_as_signed_out() {
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"account": false}),
            serde_json::json!({"account": {}}),
            serde_json::json!({"account": {"type": null}}),
        ] {
            let error = AccountIdentity::from_value(value).unwrap_err();
            assert_eq!(error.kind, AppErrorKind::ProtocolMismatch);
        }
    }

    #[test]
    fn official_0167_metadata_is_not_quota_and_reset_details_are_redacted() {
        let rates =
            ParsedRateLimits::from_value(fixture("0.167.0/rate-limits-multi-bucket.json")).unwrap();
        assert!(!rates.protocol_anomaly);
        assert_eq!(rates.primary.as_ref().unwrap().used_percent, 37.0);
        assert_eq!(rates.secondary.as_ref().unwrap().used_percent, 22.0);
        assert_eq!(rates.additional_windows.len(), 1);
        assert_eq!(rates.additional_windows[0].limit_id, "codex-spark:primary");
        assert_eq!(rates.additional_windows[0].used_percent, 9.0);
        // The detail list can be truncated: never derive the count from its length.
        assert_eq!(rates.reset_credits.as_ref().unwrap().available_count, 4);
        let account = AccountIdentity::from_value(fixture("0.167.0/account-chatgpt.json")).unwrap();
        let snapshot = parse_profile_usage(id(), account, rates, fixed_time()).unwrap();
        let serialized = serde_json::to_string(&snapshot).unwrap();
        for forbidden in [
            "fixture-only-reset-credit",
            "Fixture credit title",
            "Fixture credit description",
            "fixture-only-account",
            "Fixture-only banner",
            "individualLimit",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn official_0167_nullable_windows_and_metadata_are_not_anomalies() {
        let rates =
            ParsedRateLimits::from_value(fixture("0.167.0/rate-limits-nullable.json")).unwrap();
        assert!(!rates.protocol_anomaly);
        assert!(rates.primary.is_none());
        let secondary = rates.secondary.unwrap();
        assert_eq!(secondary.used_percent, 0.0);
        assert_eq!(secondary.remaining_percent, 100.0);
        assert_eq!(secondary.window_duration_minutes, None);
        assert_eq!(secondary.resets_at, None);
        assert!(rates.reset_credits.is_none());
        assert!(rates.additional_windows.is_empty());
    }

    #[test]
    fn malformed_additional_bucket_does_not_discard_main_or_other_quota() {
        for malformed in [
            serde_json::json!(null),
            serde_json::json!(false),
            serde_json::json!("bad bucket"),
            serde_json::json!([]),
            serde_json::json!({"primary": {"usedPercent": "NaN"}}),
        ] {
            let mut value = fixture("0.167.0/rate-limits-multi-bucket.json");
            value["rateLimitsByLimitId"]["broken"] = malformed;
            let rates = ParsedRateLimits::from_value(value).unwrap();
            assert_eq!(rates.primary.unwrap().used_percent, 37.0);
            assert_eq!(rates.secondary.unwrap().used_percent, 22.0);
            assert_eq!(rates.additional_windows.len(), 1);
            assert!(rates.protocol_anomaly);
        }
    }

    #[test]
    fn malformed_named_codex_bucket_falls_back_to_legacy_with_anomaly() {
        for malformed in [serde_json::json!(null), serde_json::json!([])] {
            let mut value = fixture("0.167.0/rate-limits-multi-bucket.json");
            value["rateLimitsByLimitId"]["codex"] = malformed;
            let rates = ParsedRateLimits::from_value(value).unwrap();
            assert_eq!(rates.primary.unwrap().used_percent, 37.0);
            assert_eq!(rates.additional_windows.len(), 1);
            assert!(rates.protocol_anomaly);
        }
    }

    #[test]
    fn unavailable_bucket_map_uses_legacy_without_discarding_shape_anomalies() {
        for (map, anomaly) in [
            (serde_json::json!(null), false),
            (serde_json::json!({}), false),
            (serde_json::json!(true), true),
            (serde_json::json!([]), true),
        ] {
            let mut value = fixture("0.167.0/rate-limits-multi-bucket.json");
            value["rateLimitsByLimitId"] = map;
            let rates = ParsedRateLimits::from_value(value).unwrap();
            assert_eq!(rates.primary.unwrap().used_percent, 37.0);
            assert!(rates.additional_windows.is_empty());
            assert_eq!(rates.protocol_anomaly, anomaly);
        }
    }

    #[test]
    fn legacy_view_retains_real_bucket_identity_and_other_windows() {
        let value = serde_json::json!({
            "rateLimits": {"limitId": "codex-spark", "primary": {"usedPercent": 9}},
            "rateLimitsByLimitId": {
                "codex-spark": {"primary": {"usedPercent": 9}},
                "other": {"primary": {"usedPercent": 11}}
            }
        });
        let rates = ParsedRateLimits::from_value(value).unwrap();
        assert_eq!(rates.selected_limit_id.as_deref(), Some("codex-spark"));
        assert_eq!(rates.primary.unwrap().limit_id, "codex-spark:primary");
        assert_eq!(rates.additional_windows.len(), 1);
        assert_eq!(rates.additional_windows[0].limit_id, "other:primary");
        assert!(!rates.protocol_anomaly);
    }

    #[test]
    fn named_codex_view_is_authoritative_even_when_windows_are_null() {
        let value = serde_json::json!({
            "rateLimits": {"primary": {"usedPercent": 99}},
            "rateLimitsByLimitId": {"CoDeX": {"primary": null, "secondary": null}}
        });
        let rates = ParsedRateLimits::from_value(value).unwrap();
        assert_eq!(rates.selected_limit_id.as_deref(), Some("codex"));
        assert!(rates.primary.is_none());
        assert!(rates.secondary.is_none());
        assert!(rates.additional_windows.is_empty());
        assert!(!rates.protocol_anomaly);
    }

    #[test]
    fn future_percentage_windows_survive_but_metadata_objects_are_ignored() {
        let value = serde_json::json!({
            "rateLimits": {
                "primary": {"usedPercent": 20},
                "futureWindow": {"usedPercent": 10, "windowDurationMins": 60},
                "futureMetadata": {"used": "50.00", "other": "opaque"},
                "otherMetadata": {"nested": true}
            }
        });
        let rates = ParsedRateLimits::from_value(value).unwrap();
        assert_eq!(rates.additional_windows.len(), 1);
        assert_eq!(rates.additional_windows[0].limit_id, "codex:futureWindow");
        assert_eq!(rates.additional_windows[0].used_percent, 10.0);
        assert!(!rates.protocol_anomaly);
    }

    #[test]
    fn official_bucket_reached_type_is_preserved_without_inferring_recovery() {
        let rates = ParsedRateLimits::from_value(serde_json::json!({
            "rateLimits": {
                "rateLimitReachedType": "workspace_member_credits_depleted",
                "primary": {"usedPercent": 0, "resetsAt": 1},
                "secondary": {"usedPercent": 15}
            },
            "ordinaryUsageAllowed": false
        }))
        .unwrap();
        for window in [rates.primary.unwrap(), rates.secondary.unwrap()] {
            assert_eq!(
                window.reached_type.as_deref(),
                Some("workspace_member_credits_depleted")
            );
        }
        assert!(!rates.protocol_anomaly);
    }

    #[test]
    fn present_reset_summary_requires_valid_count_but_not_detail_rows() {
        for summary in [
            serde_json::json!({}),
            serde_json::json!({"credits": []}),
            serde_json::json!({"availableCount": null}),
            serde_json::json!({"availableCount": "not a count"}),
        ] {
            let rates = ParsedRateLimits::from_value(serde_json::json!({
                "rateLimits": {"primary": {"usedPercent": 1}},
                "rateLimitResetCredits": summary
            }))
            .unwrap();
            assert!(rates.reset_credits.is_none());
            assert!(rates.protocol_anomaly);
        }
        for details in [serde_json::json!(null), serde_json::json!([])] {
            let rates = ParsedRateLimits::from_value(serde_json::json!({
                "rateLimits": {"primary": {"usedPercent": 1}},
                "rateLimitResetCredits": {"availableCount": 3, "credits": details}
            }))
            .unwrap();
            assert_eq!(rates.reset_credits.unwrap().available_count, 3);
            assert!(!rates.protocol_anomaly);
        }
    }

    #[test]
    fn api_key_no_quota_does_not_recommend_reinstalling_codex() {
        let account = AccountIdentity::from_value(fixture("account-api-key.json")).unwrap();
        let rates =
            ParsedRateLimits::from_value(fixture("0.167.0/rate-limits-multi-bucket.json")).unwrap();
        let error = parse_profile_usage(id(), account, rates, fixed_time()).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::ApiKeyNoQuota);
        assert_eq!(error.recovery, RecoveryAction::None);
        assert_eq!(error.message_key, "errors.apiKeyNoQuota");
    }

    #[test]
    fn signed_in_without_windows_is_not_reported_as_signed_out() {
        let account = AccountIdentity::from_value(fixture("0.167.0/account-chatgpt.json")).unwrap();
        let rates = ParsedRateLimits::from_value(serde_json::json!({
            "rateLimits": {"primary": null, "secondary": null,
                "credits": {"hasCredits": true, "unlimited": true, "balance": null}},
            "rateLimitResetCredits": {"availableCount": 2}
        }))
        .unwrap();
        assert!(!rates.protocol_anomaly);
        let error = parse_profile_usage(id(), account, rates, fixed_time()).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::ProtocolMismatch);
        assert_eq!(error.diagnostic_code, "APP_SERVER_NO_RATE_LIMITS");
    }

    #[test]
    fn missing_or_invalid_rate_envelopes_never_select_an_arbitrary_bucket() {
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({}),
            serde_json::json!({"rateLimits": null}),
            serde_json::json!({"rateLimitsByLimitId": {"codex": null}}),
            serde_json::json!({"rateLimitsByLimitId": {"other": {"primary": {"usedPercent": 5}}}}),
        ] {
            let error = ParsedRateLimits::from_value(value).unwrap_err();
            assert_eq!(error.kind, AppErrorKind::ProtocolMismatch);
        }
    }

    #[test]
    fn valid_named_bucket_does_not_depend_on_legacy_view() {
        let mut value = fixture("0.167.0/rate-limits-multi-bucket.json");
        value["rateLimits"] = serde_json::json!(null);
        let rates = ParsedRateLimits::from_value(value).unwrap();
        assert_eq!(rates.primary.unwrap().used_percent, 37.0);
        assert_eq!(rates.additional_windows.len(), 1);
        assert!(!rates.protocol_anomaly);
    }

    #[test]
    fn malformed_optional_window_metadata_keeps_percent_and_marks_anomaly() {
        for (duration, reset) in [
            (serde_json::json!(-1), serde_json::json!("bad timestamp")),
            (
                serde_json::json!(1.5),
                serde_json::json!(9223372036854775807_i64),
            ),
            (serde_json::json!(true), serde_json::json!(false)),
        ] {
            let rates = ParsedRateLimits::from_value(serde_json::json!({
                "rateLimits": {"primary": {
                    "usedPercent": 20, "windowDurationMins": duration, "resetsAt": reset
                }}
            }))
            .unwrap();
            let window = rates.primary.unwrap();
            assert_eq!(window.used_percent, 20.0);
            assert!(window.window_duration_minutes.is_none());
            assert!(window.resets_at.is_none());
            assert!(rates.protocol_anomaly);
        }
    }

    #[test]
    fn legacy_direct_bucket_and_numeric_strings_remain_supported() {
        let rates = ParsedRateLimits::from_value(serde_json::json!({
            "limitId": "codex",
            "used_percent": "12.5",
            "windowDurationMins": "300",
            "resetsAt": "1794000000",
            "rateLimitResetCredits": {"availableCount": "3"}
        }))
        .unwrap();
        let primary = rates.primary.unwrap();
        assert_eq!(primary.used_percent, 12.5);
        assert_eq!(primary.window_duration_minutes, Some(300));
        assert_eq!(primary.resets_at.unwrap().timestamp(), 1_794_000_000);
        assert_eq!(rates.reset_credits.unwrap().available_count, 3);
        assert!(!rates.protocol_anomaly);
    }

    fn fixed_time() -> DateTime<Utc> {
        DateTime::from_timestamp(1_793_900_000, 0).unwrap()
    }

    #[test]
    fn profile_snapshot_maps_source_and_plan() {
        let account = AccountIdentity {
            auth_mode: AuthMode::ChatGpt,
            username: None,
            display_name: None,
            email: Some("user@example.com".into()),
            plan_type: Some("plus".into()),
            avatar_candidate: None,
        };
        let rates = ParsedRateLimits::from_value(fixture("rate-limits-legacy.json")).unwrap();
        let snapshot = parse_profile_usage(id(), account, rates, Utc::now()).unwrap();
        assert_eq!(snapshot.source, UsageSource::AppServer);
        assert_eq!(snapshot.plan_type.as_deref(), Some("plus"));
    }
}
