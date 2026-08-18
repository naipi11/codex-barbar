use serde::{Deserialize, Serialize};

/// Where the shell should navigate within a surface.
///
/// V1 only needs the summary (tray) target and the settings tab selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SurfaceTarget {
    #[default]
    Summary,
    Settings {
        tab: String,
    },
}

impl SurfaceTarget {
    #[allow(dead_code)]
    pub fn mode(&self) -> crate::surface::SurfaceMode {
        match self {
            Self::Summary => crate::surface::SurfaceMode::TrayPanel,
            Self::Settings { .. } => crate::surface::SurfaceMode::Settings,
        }
    }
}

/// Settings tab ids accepted by the proof harness. Keep this list synchronized
/// with the frontend settings tab union and metadata.
pub fn is_supported_settings_tab(tab: &str) -> bool {
    matches!(
        tab,
        "general"
            | "providers"
            | "notifications"
            | "menuBar"
            | "menu"
            | "usageSpend"
            | "advanced"
            | "about"
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_mode_matches_surface_mode() {
        assert_eq!(
            SurfaceTarget::Summary.mode(),
            crate::surface::SurfaceMode::TrayPanel
        );
        assert_eq!(
            SurfaceTarget::Settings {
                tab: "general".into()
            }
            .mode(),
            crate::surface::SurfaceMode::Settings
        );
    }

    #[test]
    fn accepts_all_shipping_settings_tab_ids() {
        for tab in [
            "general",
            "providers",
            "notifications",
            "menuBar",
            "menu",
            "usageSpend",
            "advanced",
            "about",
        ] {
            assert!(
                is_supported_settings_tab(tab),
                "tab should be supported: {tab}"
            );
        }
    }
}
