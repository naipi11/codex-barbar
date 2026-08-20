//! Deterministic Graphite Knot tray icon renderer.

use super::{TrayLevel, TrayVisualState};

pub const TRAY_ICON_SIZE: u32 = 32;

pub const NORMAL_RGBA: [u8; 4] = [86, 217, 138, 255];
pub const WARNING_RGBA: [u8; 4] = [226, 163, 58, 255];
pub const DANGER_RGBA: [u8; 4] = [226, 75, 85, 255];
pub const STALE_RGBA: [u8; 4] = [154, 163, 178, 255];

const GRAPHITE_RGBA: [u8; 4] = [16, 19, 26, 255];
const KNOT_ALPHA: &[u8; (TRAY_ICON_SIZE * TRAY_ICON_SIZE) as usize] =
    include_bytes!("knot_alpha_32.bin");

/// Render one visual state as a transparent, anti-aliased knot.
pub fn render_tray_icon_rgba(state: TrayVisualState) -> (Vec<u8>, u32, u32) {
    let color = match state {
        TrayVisualState::Remaining { level, .. } => level_color(level),
        TrayVisualState::Stale { .. } | TrayVisualState::Api | TrayVisualState::Unavailable => {
            STALE_RGBA
        }
    };
    let mut pixels = Vec::with_capacity((TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize);
    for y in 0..TRAY_ICON_SIZE {
        for x in 0..TRAY_ICON_SIZE {
            let alpha = KNOT_ALPHA[(y * TRAY_ICON_SIZE + x) as usize];
            if alpha > 0 {
                pixels.extend_from_slice(&[color[0], color[1], color[2], alpha]);
                continue;
            }

            let keyline_alpha = neighboring_alpha(x, y);
            if keyline_alpha > 0 {
                pixels.extend_from_slice(&[
                    GRAPHITE_RGBA[0],
                    GRAPHITE_RGBA[1],
                    GRAPHITE_RGBA[2],
                    keyline_alpha,
                ]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    (pixels, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

fn neighboring_alpha(x: u32, y: u32) -> u8 {
    let left = x.saturating_sub(1);
    let top = y.saturating_sub(1);
    let right = (x + 1).min(TRAY_ICON_SIZE - 1);
    let bottom = (y + 1).min(TRAY_ICON_SIZE - 1);
    (top..=bottom)
        .flat_map(|neighbor_y| {
            (left..=right).map(move |neighbor_x| {
                KNOT_ALPHA[(neighbor_y * TRAY_ICON_SIZE + neighbor_x) as usize]
            })
        })
        .max()
        .unwrap_or(0)
}

fn level_color(level: TrayLevel) -> [u8; 4] {
    match level {
        TrayLevel::Normal => NORMAL_RGBA,
        TrayLevel::Warning => WARNING_RGBA,
        TrayLevel::Danger => DANGER_RGBA,
    }
}

/// Compatibility wrapper for the pre-V1 used-percent icon API.
pub fn render_percent_icon_rgba(percent_used: f64, has_error: bool) -> (Vec<u8>, u32, u32) {
    let remaining = if percent_used.is_finite() {
        100.0 - percent_used.clamp(0.0, 100.0)
    } else {
        f64::NAN
    };
    render_tray_icon_rgba(TrayVisualState::from_remaining(remaining, has_error))
}

/// Compatibility wrapper for the pre-V1 two-bar API.
pub fn render_bar_icon_rgba(
    session_percent_used: f64,
    weekly_percent_used: Option<f64>,
    has_error: bool,
) -> (Vec<u8>, u32, u32) {
    let minimum = std::iter::once(session_percent_used)
        .chain(weekly_percent_used)
        .filter(|value| value.is_finite())
        .map(|used| 100.0 - used.clamp(0.0, 100.0))
        .min_by(f64::total_cmp);
    let state = minimum
        .map(|remaining| TrayVisualState::from_remaining(remaining, has_error))
        .unwrap_or(TrayVisualState::Unavailable);
    render_tray_icon_rgba(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_produces_a_32_pixel_rgba_icon() {
        for state in [
            TrayVisualState::from_remaining(62.0, false),
            TrayVisualState::from_remaining(42.0, false),
            TrayVisualState::from_remaining(12.0, false),
            TrayVisualState::from_remaining(42.0, true),
            TrayVisualState::Api,
            TrayVisualState::Unavailable,
        ] {
            let (rgba, width, height) = render_tray_icon_rgba(state);
            assert_eq!((width, height), (TRAY_ICON_SIZE, TRAY_ICON_SIZE));
            assert_eq!(rgba.len() as u32, width * height * 4);
        }
    }

    #[test]
    fn graphite_knot_has_a_transparent_safe_zone_and_open_center() {
        let (rgba, width, _) = render_tray_icon_rgba(TrayVisualState::Remaining {
            percent: 72,
            level: TrayLevel::Normal,
        });

        let alpha_at = |x: u32, y: u32| rgba[((y * width + x) * 4 + 3) as usize];
        for edge in 0..2 {
            for offset in 0..width {
                assert_eq!(
                    alpha_at(offset, edge),
                    0,
                    "top safe zone at {offset},{edge}"
                );
                assert_eq!(alpha_at(offset, width - 1 - edge), 0, "bottom safe zone");
                assert_eq!(alpha_at(edge, offset), 0, "left safe zone");
                assert_eq!(alpha_at(width - 1 - edge, offset), 0, "right safe zone");
            }
        }
        let center_negative_space = (12..21)
            .flat_map(|y| (12..21).map(move |x| alpha_at(x, y)))
            .filter(|alpha| *alpha < 128)
            .count();
        assert!(
            center_negative_space >= 20,
            "the knot center must retain visible negative space"
        );

        let visible_pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(
            (220..=650).contains(&visible_pixels),
            "the mark must not become an opaque tile: {visible_pixels} pixels"
        );
    }

    #[test]
    fn every_state_tints_one_shared_knot_silhouette() {
        let states = [
            TrayVisualState::Remaining {
                percent: 72,
                level: TrayLevel::Normal,
            },
            TrayVisualState::Remaining {
                percent: 48,
                level: TrayLevel::Warning,
            },
            TrayVisualState::Remaining {
                percent: 12,
                level: TrayLevel::Danger,
            },
            TrayVisualState::Stale { percent: 48 },
            TrayVisualState::Api,
            TrayVisualState::Unavailable,
        ];

        let rendered = states.map(|state| render_tray_icon_rgba(state).0);
        let alpha_mask = |rgba: &[u8]| {
            rgba.as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[3])
                .collect::<Vec<_>>()
        };
        let expected_mask = alpha_mask(&rendered[0]);

        for rgba in &rendered {
            assert_eq!(alpha_mask(rgba), expected_mask);
            let visible_colors = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel[3] > 0)
                .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                visible_colors,
                std::collections::BTreeSet::from([
                    [16, 19, 26],
                    match state_color_for_test(rgba) {
                        Some(color) => color,
                        None => panic!("state must retain its quota-band color"),
                    },
                ]),
                "a graphite keyline and one quota-band color must be the only visible colors"
            );
        }
    }

    fn state_color_for_test(rgba: &[u8]) -> Option<[u8; 3]> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .find(|color| *color != [16, 19, 26])
    }

    #[test]
    fn graphite_knot_uses_the_approved_band_palette() {
        let color_for = |state| {
            render_tray_icon_rgba(state)
                .0
                .as_chunks::<4>()
                .0
                .iter()
                .find(|pixel| pixel[3] > 0 && pixel[..3] != GRAPHITE_RGBA[..3])
                .map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .expect("knot contains visible pixels")
        };

        assert_eq!(
            color_for(TrayVisualState::Remaining {
                percent: 72,
                level: TrayLevel::Normal,
            }),
            [86, 217, 138, 255]
        );
        assert_eq!(
            color_for(TrayVisualState::Remaining {
                percent: 48,
                level: TrayLevel::Warning,
            }),
            [226, 163, 58, 255]
        );
        assert_eq!(
            color_for(TrayVisualState::Remaining {
                percent: 12,
                level: TrayLevel::Danger,
            }),
            [226, 75, 85, 255]
        );
        for state in [
            TrayVisualState::Stale { percent: 48 },
            TrayVisualState::Api,
            TrayVisualState::Unavailable,
        ] {
            assert_eq!(color_for(state), [154, 163, 178, 255]);
        }
    }

    #[test]
    fn exact_v1_color_constants_are_used() {
        let (normal, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(67.0, false));
        let (warning, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(66.0, false));
        let (danger, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(33.0, false));
        assert!(normal.as_chunks::<4>().0.contains(&NORMAL_RGBA));
        assert!(warning.as_chunks::<4>().0.contains(&WARNING_RGBA));
        assert!(danger.as_chunks::<4>().0.contains(&DANGER_RGBA));
    }

    #[test]
    fn stale_api_and_unavailable_share_the_neutral_knot() {
        let stale = render_tray_icon_rgba(TrayVisualState::Stale { percent: 42 }).0;
        let api = render_tray_icon_rgba(TrayVisualState::Api).0;
        let unavailable = render_tray_icon_rgba(TrayVisualState::Unavailable).0;
        assert_eq!(stale, api);
        assert_eq!(api, unavailable);
        assert!(stale.as_chunks::<4>().0.contains(&STALE_RGBA));
    }

    #[test]
    fn compatibility_bar_uses_the_most_exhausted_window() {
        let expected = render_tray_icon_rgba(TrayVisualState::from_remaining(20.0, false)).0;
        let actual = render_bar_icon_rgba(10.0, Some(80.0), false).0;
        assert_eq!(actual, expected);
    }
}
