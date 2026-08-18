//! Secret and personal-information redaction.
//!
//! The redactor is applied before secrets enter logs, diagnostics, or
//! frontend-visible errors. It recursively normalizes JSON keys, scans text
//! for JWT/Bearer/GitHub-token/query/API-key forms, and removes high-entropy
//! long tokens while retaining ordinary UUIDs.

use regex_lite::Regex;
use std::sync::OnceLock;

/// Placeholder text for redacted emails
pub const EMAIL_PLACEHOLDER: &str = "Hidden";

fn email_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").expect("Invalid email regex")
    })
}

fn bearer_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)(Bearer\s+)[^\s,;]+").expect("Invalid bearer regex"))
}

fn cookie_header_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)((?:cookie|set-cookie)\s*:\s*)[^\r\n]+")
            .expect("Invalid cookie header regex")
    })
}

fn query_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)([?&](?:token|code|client_secret|api_key|access_token|refresh_token|authorization)=)[^&#\s]+",
        )
        .expect("Invalid query secret regex")
    })
}

fn json_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?i)("?(?:api_key|apiKey|token|access_token|refresh_token|client_secret|authorization|cookie|auth_json)"?\s*[:=]\s*")[^"]+""#,
        )
        .expect("Invalid JSON secret regex")
    })
}

fn api_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:sk|ghp|gho|github_pat|ghu|ghs|zai|nanogpt|openrouter|fk)-[A-Za-z0-9_\-]{8,}\b",
        )
        .expect("Invalid API key regex")
    })
}

fn jwt_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}")
            .expect("Invalid JWT regex")
    })
}

fn high_entropy_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[A-Za-z0-9_-]{32,}").expect("Invalid entropy regex"))
}

fn uuid_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
            .expect("Invalid UUID regex")
    })
}

/// Recursively redact secrets before they enter logs or visible errors.
#[derive(Debug, Clone, Copy, Default)]
pub struct SecretRedactor;

impl SecretRedactor {
    /// Redact secret-bearing text (headers, URLs, JSON fragments, tokens).
    pub fn redact(input: &str) -> String {
        Self.scan_text(input)
    }

    /// Recursively redact a JSON value: secret-named keys first, then text
    /// scanning on every string.
    pub fn redact_value(&self, value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(text) => serde_json::Value::String(self.scan_text(&text)),
            serde_json::Value::Array(values) => serde_json::Value::Array(
                values
                    .into_iter()
                    .map(|value| self.redact_value(value))
                    .collect(),
            ),
            serde_json::Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (key, value) in map {
                    if is_secret_key(&key) {
                        out.insert(key, serde_json::Value::String("[REDACTED]".to_string()));
                    } else {
                        out.insert(key, self.redact_value(value));
                    }
                }
                serde_json::Value::Object(out)
            }
            other => other,
        }
    }

    fn scan_text(&self, input: &str) -> String {
        let mut text = input.to_string();
        text = bearer_regex()
            .replace_all(&text, "${1}[REDACTED]")
            .into_owned();
        text = cookie_header_regex()
            .replace_all(&text, "${1}[REDACTED]")
            .into_owned();
        text = query_secret_regex()
            .replace_all(&text, "${1}[REDACTED]")
            .into_owned();
        text = json_secret_regex()
            .replace_all(&text, "${1}[REDACTED]\"")
            .into_owned();
        text = jwt_regex().replace_all(&text, "[REDACTED]").into_owned();
        text = api_key_regex()
            .replace_all(&text, "[REDACTED]")
            .into_owned();
        text = high_entropy_regex()
            .replace_all(&text, |caps: &regex_lite::Captures<'_>| {
                let candidate = &caps[0];
                if uuid_regex().is_match(candidate) {
                    candidate.to_string()
                } else {
                    "[REDACTED]".to_string()
                }
            })
            .into_owned();
        text
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "token"
            | "accesstoken"
            | "refreshtoken"
            | "authorization"
            | "cookie"
            | "apikey"
            | "authjson"
            | "clientsecret"
            | "pat"
            | "password"
            | "secret"
    )
}

/// Personal information redactor
pub struct PersonalInfoRedactor;

impl PersonalInfoRedactor {
    /// Redact a single email address if privacy mode is enabled
    pub fn redact_email(email: Option<&str>, is_enabled: bool) -> String {
        match email {
            Some(e) if !e.trim().is_empty() => {
                if is_enabled {
                    EMAIL_PLACEHOLDER.to_string()
                } else {
                    e.to_string()
                }
            }
            _ => String::new(),
        }
    }

    /// Redact all email addresses in a text string
    pub fn redact_emails_in_text(text: Option<&str>, is_enabled: bool) -> Option<String> {
        let text = text?;
        if !is_enabled {
            return Some(text.to_string());
        }
        Some(
            email_regex()
                .replace_all(text, EMAIL_PLACEHOLDER)
                .into_owned(),
        )
    }

    /// Partially redact an email, showing first few chars and domain
    pub fn partial_redact_email(email: Option<&str>, is_enabled: bool) -> String {
        match email {
            Some(e) if !e.trim().is_empty() => {
                if !is_enabled {
                    return e.to_string();
                }
                if let Some((local, domain)) = e.split_once('@') {
                    if local.is_empty() {
                        return EMAIL_PLACEHOLDER.to_string();
                    }
                    let first_char: String = local.chars().take(1).collect();
                    format!("{}***@{}", first_char, domain)
                } else {
                    EMAIL_PLACEHOLDER.to_string()
                }
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_email_disabled() {
        let email = "test@example.com";
        assert_eq!(
            PersonalInfoRedactor::redact_email(Some(email), false),
            email
        );
    }

    #[test]
    fn test_redact_email_enabled() {
        let email = "test@example.com";
        assert_eq!(
            PersonalInfoRedactor::redact_email(Some(email), true),
            EMAIL_PLACEHOLDER
        );
    }

    #[test]
    fn test_redact_email_none() {
        assert_eq!(PersonalInfoRedactor::redact_email(None, true), "");
        assert_eq!(PersonalInfoRedactor::redact_email(Some(""), true), "");
        assert_eq!(PersonalInfoRedactor::redact_email(Some("  "), true), "");
    }

    #[test]
    fn test_redact_emails_in_text() {
        let text = "Contact me at user@example.com or admin@test.org for help";
        let result = PersonalInfoRedactor::redact_emails_in_text(Some(text), true);
        assert_eq!(
            result,
            Some("Contact me at Hidden or Hidden for help".to_string())
        );
    }

    #[test]
    fn test_redact_emails_disabled() {
        let text = "Contact me at user@example.com";
        let result = PersonalInfoRedactor::redact_emails_in_text(Some(text), false);
        assert_eq!(result, Some(text.to_string()));
    }

    #[test]
    fn test_partial_redact_email() {
        assert_eq!(
            PersonalInfoRedactor::partial_redact_email(Some("john@example.com"), true),
            "j***@example.com"
        );
        assert_eq!(
            PersonalInfoRedactor::partial_redact_email(Some("test@domain.org"), false),
            "test@domain.org"
        );
    }

    #[test]
    fn redacts_cookie_header_values() {
        let input = "cookie: session=abc123; cf_clearance=secret";
        let redacted = SecretRedactor::redact(input);
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("secret"));
        assert_eq!(redacted, "cookie: [REDACTED]");
    }

    #[test]
    fn redacts_bearer_tokens() {
        let input = "Authorization: Bearer sk-test-secret-token";
        assert_eq!(
            SecretRedactor::redact(input),
            "Authorization: Bearer [REDACTED]"
        );
    }

    #[test]
    fn redacts_url_query_tokens() {
        let input = "https://example.com/callback?token=abc&code=def";
        let redacted = SecretRedactor::redact(input);
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("def"));
        assert!(redacted.contains("token=[REDACTED]"));
        assert!(redacted.contains("code=[REDACTED]"));
    }

    #[test]
    fn redacts_json_secret_fields() {
        let input = r#"{"api_key":"secret-value","client_secret":"other-secret"}"#;
        let redacted = SecretRedactor::redact(input);
        assert!(!redacted.contains("secret-value"));
        assert!(!redacted.contains("other-secret"));
        assert!(redacted.contains(r#""api_key":"[REDACTED]""#));
        assert!(redacted.contains(r#""client_secret":"[REDACTED]""#));
    }

    #[test]
    fn redacts_factory_api_keys() {
        let input = "Factory key fk-test-key-abcdef";
        let redacted = SecretRedactor::redact(input);
        assert!(!redacted.contains("fk-test-key"));
        assert_eq!(redacted, "Factory key [REDACTED]");
    }

    #[test]
    fn recursively_redacts_keys_jwt_bearer_pat_and_query_tokens() {
        const TEST_JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let input = serde_json::json!({
            "AccessToken": TEST_JWT,
            "nested": { "refresh-token": "refresh-secret", "url": "https://x.test/?token=abc" },
            "header": "Bearer abc.def.ghi",
            "pat": "github_pat_EXAMPLE_NOT_A_SECRET"
        });
        let output = SecretRedactor.redact_value(input);
        let text = output.to_string();
        for secret in [
            TEST_JWT,
            "refresh-secret",
            "token=abc",
            "abc.def.ghi",
            "github_pat_",
        ] {
            assert!(!text.contains(secret), "leaked {secret}: {text}");
        }
    }

    #[test]
    fn high_entropy_redaction_retains_ordinary_uuids() {
        let uuid = "123e4567-e89b-12d3-a456-426614174000";
        let input = format!("profile={uuid} secret={}", "a".repeat(48));
        let output = SecretRedactor::redact(&input);
        assert!(output.contains(uuid));
        assert!(!output.contains(&"a".repeat(48)));
    }
}
