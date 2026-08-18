//! Sound effects for CodexBar
//!
//! Plays Windows system sounds for usage threshold alerts

#![allow(dead_code)]

use crate::settings::Settings;

/// Sound types for different alerts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSound {
    /// High usage warning (approaching limit)
    Warning,
    /// Critical usage alert
    Critical,
    /// Usage exhausted / error
    Error,
    /// Status restored / success
    Success,
}

impl AlertSound {
    /// Get the Windows system sound type for this alert
    #[cfg(target_os = "windows")]
    fn system_sound_type(&self) -> u32 {
        // Windows MessageBeep types
        // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messagebeep
        const MB_OK: u32 = 0x00000000;
        const MB_ICONWARNING: u32 = 0x00000030;
        const MB_ICONERROR: u32 = 0x00000010;
        const MB_ICONINFORMATION: u32 = 0x00000040;

        match self {
            AlertSound::Warning => MB_ICONWARNING,
            AlertSound::Critical => MB_ICONERROR,
            AlertSound::Error => MB_ICONERROR,
            AlertSound::Success => MB_ICONINFORMATION,
        }
    }
}

/// Play an alert sound if sound is enabled in settings
pub fn play_alert(sound: AlertSound, settings: &Settings) {
    if !settings.sound_enabled {
        return;
    }

    // Volume is 0-100, but Windows MessageBeep doesn't support volume control
    // For more advanced volume control, we'd need to use a different API
    // For now, we just play at system volume if enabled
    play_system_sound(sound);
}

/// Play a Windows system sound
#[cfg(target_os = "windows")]
fn play_system_sound(sound: AlertSound) {
    use std::ffi::c_uint;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBeep(uType: c_uint) -> i32;
    }

    unsafe {
        MessageBeep(sound.system_sound_type());
    }
}

#[cfg(not(target_os = "windows"))]
fn play_system_sound(_sound: AlertSound) {
    // No-op on non-Windows platforms
    tracing::debug!("Sound playback not supported on this platform");
}

/// Play a warning sound (high usage threshold)
pub fn play_warning(settings: &Settings) {
    play_alert(AlertSound::Warning, settings);
}

/// Play a critical alert sound
pub fn play_critical(settings: &Settings) {
    play_alert(AlertSound::Critical, settings);
}

/// Play an error sound (exhausted/error states)
pub fn play_error(settings: &Settings) {
    play_alert(AlertSound::Error, settings);
}

/// Play a success sound (restored states)
pub fn play_success(settings: &Settings) {
    play_alert(AlertSound::Success, settings);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_sound_types() {
        // Just verify the enum works
        assert_eq!(AlertSound::Warning, AlertSound::Warning);
        assert_ne!(AlertSound::Warning, AlertSound::Critical);
    }
}
