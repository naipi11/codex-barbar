//! Fixed external browser actions. No URL crosses the WebView boundary.

#[tauri::command]
pub fn open_release_page() -> Result<(), String> {
    codexbar::update_check::open_release_page()
}

#[tauri::command]
pub fn open_codex_usage_page() -> Result<(), String> {
    codexbar::update_check::open_codex_usage_page()
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixed_urls_are_exact_and_https_only() {
        assert_eq!(
            codexbar::update_check::RELEASE_PAGE_URL,
            "https://github.com/naipi11/codex-barbar/releases"
        );
        assert_eq!(
            codexbar::update_check::CODEX_USAGE_PAGE_URL,
            "https://chatgpt.com/codex/settings/usage"
        );
    }
}
