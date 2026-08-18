use serde::{Deserialize, Serialize};

/// The surfaces the codex-barbar V1 desktop shell can present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceMode {
    #[default]
    Hidden,
    TrayPanel,
    Settings,
}

impl SurfaceMode {
    /// Surface modes reachable in the codex-barbar V1 desktop shell.
    #[allow(dead_code)]
    pub const ALL: &'static [SurfaceMode] = &[
        SurfaceMode::Hidden,
        SurfaceMode::TrayPanel,
        SurfaceMode::Settings,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::TrayPanel => "trayPanel",
            Self::Settings => "settings",
        }
    }

    #[allow(dead_code)]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "hidden" => Some(Self::Hidden),
            "trayPanel" | "tray" => Some(Self::TrayPanel),
            "settings" => Some(Self::Settings),
            _ => None,
        }
    }
}

/// Tracks the current surface mode and validates transitions.
pub struct SurfaceStateMachine {
    current: SurfaceMode,
}

impl Default for SurfaceStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceStateMachine {
    pub fn new() -> Self {
        Self {
            current: SurfaceMode::Hidden,
        }
    }

    pub fn current(&self) -> SurfaceMode {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        let sm = SurfaceStateMachine::new();
        assert_eq!(sm.current(), SurfaceMode::Hidden);
    }

    #[test]
    fn parse_round_trip() {
        for mode in SurfaceMode::ALL {
            assert_eq!(SurfaceMode::parse(mode.as_str()), Some(*mode));
        }
    }

    #[test]
    fn tray_alias_parses_to_tray_panel() {
        assert_eq!(SurfaceMode::parse("tray"), Some(SurfaceMode::TrayPanel));
    }

    #[test]
    fn pop_out_is_not_a_v1_surface() {
        assert_eq!(SurfaceMode::parse("popOut"), None);
    }
}
