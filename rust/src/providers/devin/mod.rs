use async_trait::async_trait;
use reqwest::{Client, Url};
use serde_json::Value;

use crate::core::{
    CostSnapshot, FetchContext, Provider, ProviderError, ProviderFetchResult, ProviderId,
    ProviderMetadata, RateWindow, SourceMode, UsageSnapshot,
};

const CREDENTIAL_TARGET: &str = "codexbar-devin";
const BASE_URL: &str = "https://api.devin.ai";

pub struct DevinProvider {
    metadata: ProviderMetadata,
    client: Client,
}

impl DevinProvider {
    pub fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                id: ProviderId::Devin,
                display_name: "Devin",
                session_label: "Daily",
                weekly_label: "Weekly",
                supports_opus: false,
                supports_credits: true,
                default_enabled: false,
                is_primary: false,
                dashboard_url: Some("https://app.devin.ai/settings/billing"),
                status_page_url: None,
            },
            client: crate::core::credentialed_http_client_builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }
}

impl Default for DevinProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for DevinProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Devin
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn fetch_usage(&self, ctx: &FetchContext) -> Result<ProviderFetchResult, ProviderError> {
        match ctx.source_mode {
            SourceMode::Auto | SourceMode::OAuth => {
                let token = crate::providers::resolve_api_key(
                    ctx.api_key.as_deref(),
                    CREDENTIAL_TARGET,
                    &["DEVIN_BEARER_TOKEN", "DEVIN_API_KEY"],
                )?;
                let env_org = std::env::var("DEVIN_ORG").ok();
                let org = ctx
                    .workspace_id
                    .as_deref()
                    .or(env_org.as_deref())
                    .ok_or_else(|| {
                        ProviderError::NotInstalled(
                            "Devin organization not found. Set it in provider extras or DEVIN_ORG."
                                .into(),
                        )
                    })?
                    .to_string();
                let response = self
                    .client
                    .get(devin_url(&org)?)
                    .bearer_auth(token)
                    .header("Accept", "application/json")
                    .send()
                    .await?;
                if response.status() == reqwest::StatusCode::UNAUTHORIZED
                    || response.status() == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(ProviderError::AuthRequired);
                }
                if !response.status().is_success() {
                    return Err(ProviderError::Other(format!(
                        "Devin quota returned status {}",
                        response.status()
                    )));
                }
                let value: Value = response.json().await.map_err(|e| {
                    ProviderError::Parse(format!("Failed to parse Devin quota: {e}"))
                })?;
                Ok(fetch_result_from_quota(&value, &org))
            }
            SourceMode::Web | SourceMode::Cli => {
                Err(ProviderError::UnsupportedSource(ctx.source_mode))
            }
        }
    }

    fn available_sources(&self) -> Vec<SourceMode> {
        vec![SourceMode::Auto, SourceMode::OAuth]
    }
}

fn devin_url(org: &str) -> Result<Url, ProviderError> {
    let org = normalized_org(org);
    Url::parse(BASE_URL)
        .and_then(|u| u.join(&format!("{org}/billing/quota/usage")))
        .map_err(|e| ProviderError::Other(format!("Invalid Devin quota URL: {e}")))
}

fn normalized_org(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('/');
    if trimmed.starts_with("org/") || trimmed.starts_with("organizations/") {
        trimmed.to_string()
    } else {
        format!("org/{trimmed}")
    }
}

fn snapshot_from_quota(value: &Value, org: &str) -> UsageSnapshot {
    let daily = percent(value, &["daily_percentage", "dailyPercentage"])
        .unwrap_or_else(|| percent(value, &["used_percent", "usedPercent"]).unwrap_or(0.0));
    let mut snapshot =
        UsageSnapshot::new(RateWindow::new(daily)).with_organization(org.to_string());
    if let Some(weekly) = percent(value, &["weekly_percentage", "weeklyPercentage"]) {
        snapshot = snapshot.with_secondary(RateWindow::new(weekly));
    }
    snapshot
}

fn fetch_result_from_quota(value: &Value, org: &str) -> ProviderFetchResult {
    let mut result = ProviderFetchResult::new(snapshot_from_quota(value, org), "api");
    if let Some(balance) = extra_usage_balance(value) {
        result = result.with_cost(CostSnapshot::new(balance, "USD", "Extra usage balance"));
    }
    result
}

fn percent(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(v) = value.get(*key).and_then(Value::as_f64) {
            return Some(if v < 1.0 { v * 100.0 } else { v });
        }
    }
    let used = ["used", "usage", "used_count", "usedCount", "consumed"]
        .iter()
        .find_map(|k| value.get(*k).and_then(Value::as_f64));
    let limit = ["limit", "quota", "total", "max", "available"]
        .iter()
        .find_map(|k| value.get(*k).and_then(Value::as_f64));
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => Some(used / limit * 100.0),
        _ => None,
    }
}

fn extra_usage_balance(value: &Value) -> Option<f64> {
    let dollars = [
        "overage_balance",
        "overageBalance",
        "extra_usage_balance",
        "extraUsageBalance",
    ]
    .iter()
    .find_map(|key| value.get(*key).and_then(Value::as_f64))
    .filter(|value| value.is_finite() && *value >= 0.0);
    dollars.or_else(|| {
        ["overage_balance_cents", "overageBalanceCents"]
            .iter()
            .find_map(|key| value.get(*key).and_then(Value::as_f64))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|cents| cents / 100.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fraction_percent() {
        let snapshot =
            snapshot_from_quota(&serde_json::json!({"daily_percentage":0.25}), "org/demo");
        assert_eq!(snapshot.primary.used_percent, 25.0);
    }

    #[test]
    fn parses_exact_one_as_one_percent() {
        let snapshot =
            snapshot_from_quota(&serde_json::json!({"daily_percentage":1.0}), "org/demo");
        assert_eq!(snapshot.primary.used_percent, 1.0);
    }

    #[test]
    fn parses_extra_usage_balance() {
        let result = fetch_result_from_quota(
            &serde_json::json!({"daily_percentage": 0.2, "overage_balance": 12.34}),
            "org/demo",
        );

        let cost = result.cost.unwrap();
        assert_eq!(cost.used, 12.34);
        assert_eq!(cost.period, "Extra usage balance");
    }

    #[test]
    fn parses_extra_usage_balance_cents() {
        let result = fetch_result_from_quota(
            &serde_json::json!({"daily_percentage": 0.2, "overage_balance_cents": 7087}),
            "org/demo",
        );

        assert_eq!(result.cost.unwrap().used, 70.87);
    }
}
