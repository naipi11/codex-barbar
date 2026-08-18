//! Windows user-default locale mapping for the V1 language setting.

/// Map the Windows user-default locale to the only two V1 UI languages:
/// `zh-CN` for Simplified Chinese and `en-US` for everything else.
pub fn default_language() -> &'static str {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::GetUserDefaultLocaleName;

        const LOCALE_NAME_MAX_LENGTH: usize = 85;
        let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
        // SAFETY: `buffer` is a valid mutable slice of the required capacity
        // and remains alive for the call.
        let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };
        if length <= 0 {
            return "en-US";
        }
        let text = String::from_utf16_lossy(&buffer[..(length as usize - 1)]);
        let normalized = text.to_ascii_lowercase();
        if normalized.starts_with("zh-cn") || normalized.starts_with("zh-hans") {
            "zh-CN"
        } else {
            "en-US"
        }
    }
    #[cfg(not(windows))]
    {
        "en-US"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_language_is_always_one_of_the_two_v1_choices() {
        assert!(matches!(default_language(), "zh-CN" | "en-US"));
    }
}
