//! Shared V1 tray visual-state and pixel rendering.

pub mod icon;
pub mod render;

pub use icon::{LoadingPattern, TrayLevel, TrayVisualState, minimum_remaining};
pub use render::{
    DANGER_RGBA, NORMAL_RGBA, STALE_RGBA, TRAY_ICON_SIZE, TrayIconPalette, WARNING_RGBA,
    render_bar_icon_rgba, render_percent_icon_rgba, render_tray_icon_rgba,
    render_tray_icon_rgba_with_palette,
};

pub const CODEX_USAGE_PAGE_URL: &str = "https://chatgpt.com/codex/settings/usage";

/// Open the one fixed Codex usage page. No URL crosses the WebView boundary.
pub fn open_codex_usage_page() -> Result<(), String> {
    open::that(CODEX_USAGE_PAGE_URL).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_usage_page_is_a_fixed_https_action() {
        assert_eq!(
            CODEX_USAGE_PAGE_URL,
            "https://chatgpt.com/codex/settings/usage"
        );
    }
}
