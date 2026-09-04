//! Fixed external browser actions. No URL crosses the WebView boundary.

#[tauri::command]
pub fn open_release_page() -> Result<(), String> {
    codexbar::update_check::open_release_page()
}

#[tauri::command]
pub fn open_codex_usage_page() -> Result<(), String> {
    codexbar::update_check::open_codex_usage_page()
}

#[cfg(any(windows, test))]
fn open_windows_notification_settings_with<F>(launcher: F) -> Result<(), String>
where
    F: FnOnce(&str) -> Result<(), String>,
{
    launcher("ms-settings:notifications")
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub fn open_windows_notification_settings() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    open_windows_notification_settings_with(|target| {
        Command::new("explorer.exe")
            .arg(target)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|_| "WINDOWS_NOTIFICATION_SETTINGS_OPEN_FAILED".to_string())
    })
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub fn open_windows_notification_settings() -> Result<(), String> {
    Err("WINDOWS_NOTIFICATION_SETTINGS_UNAVAILABLE".to_string())
}

#[cfg(test)]
mod tests {
    use super::open_windows_notification_settings_with;

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

    #[test]
    fn notification_settings_action_uses_only_the_fixed_windows_target() {
        let mut launched_target = None;

        open_windows_notification_settings_with(|target| {
            launched_target = Some(target.to_string());
            Ok(())
        })
        .unwrap();

        assert_eq!(
            launched_target.as_deref(),
            Some("ms-settings:notifications")
        );
    }
}
