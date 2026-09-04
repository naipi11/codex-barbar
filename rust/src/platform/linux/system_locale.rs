//! Linux environment locale mapping for the V1 language setting.

use crate::storage::LanguagePreference;

pub fn language() -> LanguagePreference {
    let lc_all = std::env::var("LC_ALL").ok();
    let lc_messages = std::env::var("LC_MESSAGES").ok();
    let lang = std::env::var("LANG").ok();
    language_from_values(lc_all.as_deref(), lc_messages.as_deref(), lang.as_deref())
}

fn language_from_values(
    lc_all: Option<&str>,
    lc_messages: Option<&str>,
    lang: Option<&str>,
) -> LanguagePreference {
    let locale = [lc_all, lc_messages, lang]
        .into_iter()
        .flatten()
        .find(|locale| !locale.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let normalized = locale.replace('-', "_");
    if normalized.starts_with("zh_cn") || normalized.starts_with("zh_hans") {
        LanguagePreference::ZhCn
    } else {
        LanguagePreference::EnUs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_precedence_is_lc_all_then_lc_messages_then_lang() {
        assert_eq!(
            language_from_values(Some("en_US.UTF-8"), Some("zh_CN.UTF-8"), Some("zh_CN")),
            LanguagePreference::EnUs
        );
        assert_eq!(
            language_from_values(None, Some("zh_CN.UTF-8"), Some("en_US.UTF-8")),
            LanguagePreference::ZhCn
        );
    }

    #[test]
    fn simplified_chinese_locale_variants_map_to_zh_cn() {
        for locale in ["zh_CN.UTF-8", "zh-CN", "zh_Hans_CN.UTF-8"] {
            assert_eq!(
                language_from_values(None, None, Some(locale)),
                LanguagePreference::ZhCn
            );
        }
    }

    #[test]
    fn missing_or_other_locales_fall_back_to_english() {
        assert_eq!(
            language_from_values(None, None, None),
            LanguagePreference::EnUs
        );
        assert_eq!(
            language_from_values(Some(""), None, Some("ja_JP.UTF-8")),
            LanguagePreference::EnUs
        );
    }
}
