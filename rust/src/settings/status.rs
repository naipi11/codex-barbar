use super::*;

/// Provider status for settings UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

/// Refresh interval options
#[derive(Debug, Clone, Serialize)]
pub struct RefreshIntervalOption {
    pub value: u64,
    pub label: String,
}

/// Get available refresh interval options
pub fn get_refresh_interval_options() -> Vec<RefreshIntervalOption> {
    vec![
        RefreshIntervalOption {
            value: 60,
            label: "1 minute".to_string(),
        },
        RefreshIntervalOption {
            value: 120,
            label: "2 minutes".to_string(),
        },
        RefreshIntervalOption {
            value: 300,
            label: "5 minutes".to_string(),
        },
        RefreshIntervalOption {
            value: 600,
            label: "10 minutes".to_string(),
        },
        RefreshIntervalOption {
            value: 900,
            label: "15 minutes".to_string(),
        },
        RefreshIntervalOption {
            value: 1800,
            label: "30 minutes".to_string(),
        },
    ]
}
