//! Deterministic pixel-level V1 tray icon renderer.

use super::{TrayLevel, TrayVisualState};

pub const TRAY_ICON_SIZE: u32 = 32;

pub const NORMAL_RGBA: [u8; 4] = [59, 130, 246, 255];
pub const WARNING_RGBA: [u8; 4] = [245, 158, 11, 255];
pub const DANGER_RGBA: [u8; 4] = [239, 68, 68, 255];
pub const STALE_RGBA: [u8; 4] = [156, 163, 175, 255];

const BACKGROUND_RGBA: [u8; 4] = [24, 24, 27, 255];
const TRACK_RGBA: [u8; 4] = [63, 63, 70, 255];

/// Render one V1 visual state to raw RGBA pixels.
pub fn render_tray_icon_rgba(state: TrayVisualState) -> (Vec<u8>, u32, u32) {
    let mut pixels = vec![0; (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize];
    fill_rect(&mut pixels, 2, 2, 30, 30, BACKGROUND_RGBA);

    match state {
        TrayVisualState::Remaining { percent, level } => {
            let color = level_color(level);
            draw_percent(&mut pixels, percent, color);
            draw_status_bar(&mut pixels, percent, color);
        }
        TrayVisualState::Stale { percent } => {
            draw_percent(&mut pixels, percent, STALE_RGBA);
            draw_status_bar(&mut pixels, percent, STALE_RGBA);
        }
        TrayVisualState::Api => {
            draw_text_centered(&mut pixels, "API", 2, NORMAL_RGBA);
            draw_status_bar(&mut pixels, 100, NORMAL_RGBA);
        }
        TrayVisualState::Unavailable => {
            draw_text_centered(&mut pixels, "!", 4, DANGER_RGBA);
            draw_status_bar(&mut pixels, 100, DANGER_RGBA);
        }
    }

    (pixels, TRAY_ICON_SIZE, TRAY_ICON_SIZE)
}

fn level_color(level: TrayLevel) -> [u8; 4] {
    match level {
        TrayLevel::Normal => NORMAL_RGBA,
        TrayLevel::Warning => WARNING_RGBA,
        TrayLevel::Danger => DANGER_RGBA,
    }
}

fn draw_percent(pixels: &mut [u8], percent: u8, color: [u8; 4]) {
    let text = percent.to_string();
    let scale = if text.len() >= 3 { 2 } else { 3 };
    draw_text_centered(pixels, &text, scale, color);
}

fn draw_status_bar(pixels: &mut [u8], percent: u8, color: [u8; 4]) {
    fill_rect(pixels, 5, 26, 27, 29, TRACK_RGBA);
    let width = ((22.0 * f64::from(percent) / 100.0).round() as u32).min(22);
    if width > 0 {
        fill_rect(pixels, 5, 26, 5 + width, 29, color);
    }
}

fn draw_text_centered(pixels: &mut [u8], text: &str, scale: u32, color: [u8; 4]) {
    let glyph_width = 3 * scale;
    let gap = scale.max(1);
    let text_width = text.chars().count() as u32 * glyph_width
        + text.chars().count().saturating_sub(1) as u32 * gap;
    let text_height = 5 * scale;
    let start_x = TRAY_ICON_SIZE.saturating_sub(text_width) / 2;
    let start_y = (TRAY_ICON_SIZE.saturating_sub(text_height) / 2).saturating_sub(2);

    let mut x = start_x;
    for character in text.chars() {
        draw_glyph(pixels, character, x, start_y, scale, color);
        x += glyph_width + gap;
    }
}

fn draw_glyph(pixels: &mut [u8], character: char, x: u32, y: u32, scale: u32, color: [u8; 4]) {
    let Some(rows) = glyph_rows(character) else {
        return;
    };
    for (row_index, row) in rows.into_iter().enumerate() {
        for column in 0..3 {
            let bit = 1 << (2 - column);
            if row & bit == 0 {
                continue;
            }
            for offset_y in 0..scale {
                for offset_x in 0..scale {
                    set_pixel(
                        pixels,
                        x + column * scale + offset_x,
                        y + row_index as u32 * scale + offset_y,
                        color,
                    );
                }
            }
        }
    }
}

fn glyph_rows(character: char) -> Option<[u8; 5]> {
    Some(match character {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        _ => return None,
    })
}

fn fill_rect(pixels: &mut [u8], left: u32, top: u32, right: u32, bottom: u32, color: [u8; 4]) {
    for y in top..bottom.min(TRAY_ICON_SIZE) {
        for x in left..right.min(TRAY_ICON_SIZE) {
            set_pixel(pixels, x, y, color);
        }
    }
}

fn set_pixel(pixels: &mut [u8], x: u32, y: u32, color: [u8; 4]) {
    if x >= TRAY_ICON_SIZE || y >= TRAY_ICON_SIZE {
        return;
    }
    let index = ((y * TRAY_ICON_SIZE + x) * 4) as usize;
    pixels[index..index + 4].copy_from_slice(&color);
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
    fn exact_v1_color_constants_are_used() {
        let (normal, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(51.0, false));
        let (warning, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(50.0, false));
        let (danger, _, _) = render_tray_icon_rgba(TrayVisualState::from_remaining(20.0, false));
        assert!(normal.chunks_exact(4).any(|pixel| pixel == NORMAL_RGBA));
        assert!(warning.chunks_exact(4).any(|pixel| pixel == WARNING_RGBA));
        assert!(danger.chunks_exact(4).any(|pixel| pixel == DANGER_RGBA));
    }

    #[test]
    fn stale_api_and_unavailable_pixels_are_distinct() {
        let stale = render_tray_icon_rgba(TrayVisualState::Stale { percent: 42 }).0;
        let api = render_tray_icon_rgba(TrayVisualState::Api).0;
        let unavailable = render_tray_icon_rgba(TrayVisualState::Unavailable).0;
        assert_ne!(stale, api);
        assert_ne!(api, unavailable);
        assert_ne!(stale, unavailable);
        assert!(stale.chunks_exact(4).any(|pixel| pixel == STALE_RGBA));
    }

    #[test]
    fn compatibility_bar_uses_the_most_exhausted_window() {
        let expected = render_tray_icon_rgba(TrayVisualState::from_remaining(20.0, false)).0;
        let actual = render_bar_icon_rgba(10.0, Some(80.0), false).0;
        assert_eq!(actual, expected);
    }
}
