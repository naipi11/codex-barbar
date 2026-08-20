//! V1 tray visual-state contracts.
//!
//! The selected Codex profile owns one visual state. Percentage thresholds
//! are computed from the same rounded integer displayed by the status
//! surfaces so every surface keeps one color band.

/// Color threshold for a fresh remaining-quota icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayLevel {
    Normal,
    Warning,
    Danger,
}

/// Complete V1 tray visual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayVisualState {
    Remaining { percent: u8, level: TrayLevel },
    Stale { percent: u8 },
    Api,
    Unavailable,
}

impl TrayVisualState {
    /// Construct a remaining-quota state.
    ///
    /// Thresholds use the rounded remaining value shared with the frontend:
    /// - 67 or greater: normal
    /// - 34 through 66: warning
    /// - 33 or lower: danger
    pub fn from_remaining(remaining: f64, stale: bool) -> Self {
        if !remaining.is_finite() {
            return Self::Unavailable;
        }

        let normalized = remaining.clamp(0.0, 100.0);
        let percent = normalized.round() as u8;
        if stale {
            return Self::Stale { percent };
        }

        let level = if percent >= 67 {
            TrayLevel::Normal
        } else if percent >= 34 {
            TrayLevel::Warning
        } else {
            TrayLevel::Danger
        };
        Self::Remaining { percent, level }
    }

    /// Construct a state from every selected-profile quota window.
    pub fn from_remaining_values(
        remaining_values: impl IntoIterator<Item = f64>,
        stale: bool,
    ) -> Self {
        minimum_remaining(remaining_values)
            .map(|remaining| Self::from_remaining(remaining, stale))
            .unwrap_or(Self::Unavailable)
    }

    pub fn level(self) -> Option<TrayLevel> {
        match self {
            Self::Remaining { level, .. } => Some(level),
            Self::Stale { .. } | Self::Api | Self::Unavailable => None,
        }
    }

    pub fn percent(self) -> Option<u8> {
        match self {
            Self::Remaining { percent, .. } | Self::Stale { percent } => Some(percent),
            Self::Api | Self::Unavailable => None,
        }
    }
}

/// Lowest finite normalized remaining value across quota windows.
pub fn minimum_remaining(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    values
        .into_iter()
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 100.0))
        .min_by(f64::total_cmp)
}

/// Legacy loading animation contract retained for compatibility with the
/// existing public crate surface. V1 tray status itself is not animated.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LoadingPattern {
    #[default]
    KnightRider,
    Cylon,
    OutsideIn,
    Race,
    Pulse,
    Unbraid,
}

impl LoadingPattern {
    pub fn value(&self, phase: f64) -> f64 {
        let phase = phase.fract();
        match self {
            Self::KnightRider => {
                let t = (phase * 2.0).min(2.0 - phase * 2.0);
                t * 100.0
            }
            Self::Cylon => phase * 100.0,
            Self::OutsideIn => ((phase * std::f64::consts::PI * 2.0).cos() * 0.5 + 0.5) * 100.0,
            Self::Race => phase * phase * 100.0,
            Self::Pulse => {
                let t = (phase * std::f64::consts::PI * 2.0).sin() * 0.5 + 0.5;
                40.0 + t * 60.0
            }
            Self::Unbraid => {
                if phase < 0.5 {
                    let expand = phase * 2.0;
                    expand * expand * (3.0 - 2.0 * expand) * 80.0
                } else {
                    let settle = (phase - 0.5) * 2.0;
                    let ease = settle * settle * (3.0 - 2.0 * settle);
                    80.0 + ease * 20.0 * (settle * std::f64::consts::PI * 4.0).sin().abs()
                }
            }
        }
    }

    pub fn secondary_offset(&self) -> f64 {
        match self {
            Self::KnightRider => 0.25,
            Self::Cylon => 0.15,
            Self::OutsideIn => 0.5,
            Self::Race => 0.2,
            Self::Pulse => 0.3,
            Self::Unbraid => 0.1,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::KnightRider,
            Self::Cylon,
            Self::OutsideIn,
            Self::Race,
            Self::Pulse,
            Self::Unbraid,
        ]
    }

    pub fn random() -> Self {
        let patterns = Self::all();
        let index = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize)
            % patterns.len();
        patterns[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_thresholds_use_exact_v1_levels() {
        assert_eq!(
            TrayVisualState::from_remaining(67.0, false).level(),
            Some(TrayLevel::Normal)
        );
        assert_eq!(
            TrayVisualState::from_remaining(66.0, false).level(),
            Some(TrayLevel::Warning)
        );
        assert_eq!(
            TrayVisualState::from_remaining(34.0, false).level(),
            Some(TrayLevel::Warning)
        );
        assert_eq!(
            TrayVisualState::from_remaining(33.0, false).level(),
            Some(TrayLevel::Danger)
        );
    }

    #[test]
    fn remaining_display_is_rounded_before_level_selection() {
        assert_eq!(
            TrayVisualState::from_remaining(66.6, false),
            TrayVisualState::Remaining {
                percent: 67,
                level: TrayLevel::Normal,
            }
        );
        assert_eq!(
            TrayVisualState::from_remaining(66.4, false),
            TrayVisualState::Remaining {
                percent: 66,
                level: TrayLevel::Warning,
            }
        );
    }

    #[test]
    fn minimum_window_ignores_non_finite_values_and_clamps() {
        assert_eq!(minimum_remaining([f64::NAN, 62.0, 18.5, 140.0]), Some(18.5));
        assert_eq!(minimum_remaining([f64::NAN, f64::INFINITY]), None);
    }

    #[test]
    fn stale_state_keeps_percentage_without_a_color_level() {
        let state = TrayVisualState::from_remaining(42.2, true);
        assert_eq!(state, TrayVisualState::Stale { percent: 42 });
        assert_eq!(state.level(), None);
    }
}
