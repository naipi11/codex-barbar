//! OpenAI Web Friendly Errors
//!
//! Convert raw HTML/text responses from OpenAI web dashboard
//! into human-readable error messages with actionable guidance.

/// Indicators that suggest the page is a public landing page
const LANDING_PAGE_INDICATORS: &[&str] =
    &["skip to content", "about openai", "learn about chatgpt"];

/// Indicators that suggest the user is logged out
const LOGGED_OUT_INDICATORS: &[&str] = &[
    "sign in",
    "log in",
    "create account",
    "continue with google",
    "continue with apple",
    "continue with microsoft",
    "continue with email",
    "create free account",
    "get started",
];

/// Cloudflare challenge indicators
const CLOUDFLARE_INDICATORS: &[&str] = &[
    "just a moment",
    "checking your browser",
    "verify you are human",
    "cloudflare",
    "please wait",
    "ray id",
];

/// Rate limit indicators
const RATE_LIMIT_INDICATORS: &[&str] = &["rate limit", "too many requests", "429", "slow down"];

/// Server error indicators
const SERVER_ERROR_INDICATORS: &[&str] = &[
    "500 internal server error",
    "502 bad gateway",
    "503 service unavailable",
    "504 gateway timeout",
    "something went wrong",
    "we're having trouble",
    "please try again later",
];

/// Error type detected from HTML response
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAIWebErrorKind {
    /// User is not logged in
    NotLoggedIn,
    /// Page appears to be a public landing page
    PublicLanding,
    /// Cloudflare challenge/interstitial
    CloudflareChallenge,
    /// Rate limited
    RateLimited,
    /// Server error
    ServerError,
    /// Empty response
    EmptyResponse,
    /// Cookie mismatch with expected account
    CookieMismatch {
        expected: String,
        got: Option<String>,
    },
    /// Unknown error (page doesn't match known patterns)
    Unknown,
}

impl OpenAIWebErrorKind {
    /// Detect the error kind from HTML body content
    pub fn detect(body: &str) -> Option<Self> {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Some(OpenAIWebErrorKind::EmptyResponse);
        }

        let lower = trimmed.to_lowercase();

        // Check for Cloudflare challenge first (takes priority)
        for indicator in CLOUDFLARE_INDICATORS {
            if lower.contains(indicator) {
                return Some(OpenAIWebErrorKind::CloudflareChallenge);
            }
        }

        // Check for rate limiting
        for indicator in RATE_LIMIT_INDICATORS {
            if lower.contains(indicator) {
                return Some(OpenAIWebErrorKind::RateLimited);
            }
        }

        // Check for server errors
        for indicator in SERVER_ERROR_INDICATORS {
            if lower.contains(indicator) {
                return Some(OpenAIWebErrorKind::ServerError);
            }
        }

        // Check if it looks like a public landing page
        let looks_like_landing = LANDING_PAGE_INDICATORS
            .iter()
            .any(|ind| lower.contains(ind))
            && (lower.contains("about") || lower.contains("openai") || lower.contains("chatgpt"));

        // Check if it looks logged out
        let looks_logged_out = LOGGED_OUT_INDICATORS.iter().any(|ind| lower.contains(ind));

        if looks_logged_out {
            return Some(OpenAIWebErrorKind::NotLoggedIn);
        }

        if looks_like_landing {
            return Some(OpenAIWebErrorKind::PublicLanding);
        }

        None
    }

    /// Get a short label for the error kind
    pub fn label(&self) -> &'static str {
        match self {
            OpenAIWebErrorKind::NotLoggedIn => "Not Logged In",
            OpenAIWebErrorKind::PublicLanding => "Public Page",
            OpenAIWebErrorKind::CloudflareChallenge => "Cloudflare Challenge",
            OpenAIWebErrorKind::RateLimited => "Rate Limited",
            OpenAIWebErrorKind::ServerError => "Server Error",
            OpenAIWebErrorKind::EmptyResponse => "Empty Response",
            OpenAIWebErrorKind::CookieMismatch { .. } => "Cookie Mismatch",
            OpenAIWebErrorKind::Unknown => "Unknown Error",
        }
    }
}

/// Generate a friendly error message from OpenAI web dashboard response
pub fn friendly_error(
    body: &str,
    target_email: Option<&str>,
    cookie_import_status: Option<&str>,
) -> Option<String> {
    let trimmed = body.trim();
    let status = cookie_import_status.map(|s| s.trim());

    // Empty page
    if trimmed.is_empty() {
        return Some(
            "OpenAI web dashboard returned an empty page. \
             Sign in to chatgpt.com and update OpenAI cookies in Providers → Codex."
                .to_string(),
        );
    }

    // Detect error kind
    let error_kind = OpenAIWebErrorKind::detect(trimmed)?;

    // Build target label
    let target_label = target_email
        .filter(|e| !e.trim().is_empty())
        .map(|e| e.trim())
        .unwrap_or("your OpenAI account");

    // Handle specific error kinds
    match error_kind {
        OpenAIWebErrorKind::EmptyResponse => Some(
            "OpenAI web dashboard returned an empty page. \
             Sign in to chatgpt.com and update OpenAI cookies in Providers → Codex."
                .to_string(),
        ),

        OpenAIWebErrorKind::CloudflareChallenge => Some(
            "OpenAI is showing a Cloudflare challenge. \
             Please visit chatgpt.com in your browser to complete the challenge, \
             then update cookies in Providers → Codex."
                .to_string(),
        ),

        OpenAIWebErrorKind::RateLimited => {
            Some("OpenAI rate limit reached. Please wait a few minutes and try again.".to_string())
        }

        OpenAIWebErrorKind::ServerError => {
            Some("OpenAI is experiencing server issues. Please try again later.".to_string())
        }

        OpenAIWebErrorKind::NotLoggedIn | OpenAIWebErrorKind::PublicLanding => {
            // Check for cookie import status
            if let Some(status) = status
                && (status.contains("cookies do not match Codex account")
                    || status.to_lowercase().contains("cookie import failed"))
            {
                return Some(format!(
                    "{} Sign in to chatgpt.com as {}, then update OpenAI cookies in Providers → Codex.",
                    status, target_label
                ));
            }

            Some(format!(
                "OpenAI web dashboard returned a public page (not signed in). \
                 Sign in to chatgpt.com as {}, then update OpenAI cookies in Providers → Codex.",
                target_label
            ))
        }

        OpenAIWebErrorKind::CookieMismatch {
            ref expected,
            ref got,
        } => {
            let got_label = got.as_deref().unwrap_or("a different account");
            Some(format!(
                "Cookie mismatch: expected {} but got {}. \
                 Sign in to chatgpt.com as {} and update cookies.",
                expected, got_label, expected
            ))
        }

        OpenAIWebErrorKind::Unknown => None,
    }
}

/// Extract signed-in email from client-bootstrap JSON in HTML
pub fn extract_signed_in_email(html: &str) -> Option<String> {
    // Look for client-bootstrap script tag
    let script_start = html.find("id=\"client-bootstrap\"")?;
    let script_content_start = html[script_start..].find('>')?;
    let content_start = script_start + script_content_start + 1;
    let content_end = html[content_start..].find("</script>")?;
    let script_content = &html[content_start..content_start + content_end];

    // Parse as JSON and extract email
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(script_content) {
        // Try various paths where email might be stored
        let email = json
            .get("user")
            .and_then(|u| u.get("email"))
            .and_then(|e| e.as_str())
            .or_else(|| {
                json.get("session")
                    .and_then(|s| s.get("user"))
                    .and_then(|u| u.get("email"))
                    .and_then(|e| e.as_str())
            });

        return email.map(|s| s.to_string());
    }

    None
}

/// Extract auth status from client-bootstrap JSON
pub fn extract_auth_status(html: &str) -> Option<String> {
    // Look for client-bootstrap script tag
    let script_start = html.find("id=\"client-bootstrap\"")?;
    let script_content_start = html[script_start..].find('>')?;
    let content_start = script_start + script_content_start + 1;
    let content_end = html[content_start..].find("</script>")?;
    let script_content = &html[content_start..content_start + content_end];

    // Parse as JSON and extract authStatus
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(script_content) {
        return json
            .get("authStatus")
            .and_then(|a| a.as_str())
            .map(|s| s.to_string());
    }

    None
}

/// Check if the page appears to be logged out based on auth status
pub fn is_logged_out(html: &str) -> bool {
    if let Some(status) = extract_auth_status(html) {
        return status == "logged_out";
    }

    // Fall back to content detection
    OpenAIWebErrorKind::detect(html)
        .map(|k| {
            matches!(
                k,
                OpenAIWebErrorKind::NotLoggedIn | OpenAIWebErrorKind::PublicLanding
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_empty_response() {
        assert_eq!(
            OpenAIWebErrorKind::detect(""),
            Some(OpenAIWebErrorKind::EmptyResponse)
        );
        assert_eq!(
            OpenAIWebErrorKind::detect("   "),
            Some(OpenAIWebErrorKind::EmptyResponse)
        );
    }

    #[test]
    fn test_detect_cloudflare() {
        let html = "<html><title>Just a moment...</title></html>";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::CloudflareChallenge)
        );

        let html = "Please wait while we're checking your browser";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::CloudflareChallenge)
        );
    }

    #[test]
    fn test_detect_logged_out() {
        let html = "<html><button>Sign in</button></html>";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::NotLoggedIn)
        );

        let html = "Continue with Google | Continue with Apple | Create account";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::NotLoggedIn)
        );
    }

    #[test]
    fn test_blocking_states_win_over_public_route_detection() {
        let logged_out_landing =
            "Skip to content About OpenAI Learn about ChatGPT Sign in Continue with Google";
        assert_eq!(
            OpenAIWebErrorKind::detect(logged_out_landing),
            Some(OpenAIWebErrorKind::NotLoggedIn)
        );

        let cloudflare_landing = "Skip to content About OpenAI Just a moment... Ray ID";
        assert_eq!(
            OpenAIWebErrorKind::detect(cloudflare_landing),
            Some(OpenAIWebErrorKind::CloudflareChallenge)
        );
    }

    #[test]
    fn test_detect_rate_limited() {
        let html = "429 Too Many Requests";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::RateLimited)
        );
    }

    #[test]
    fn test_detect_server_error() {
        let html = "500 Internal Server Error";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::ServerError)
        );

        let html = "Something went wrong. Please try again later.";
        assert_eq!(
            OpenAIWebErrorKind::detect(html),
            Some(OpenAIWebErrorKind::ServerError)
        );
    }

    #[test]
    fn test_friendly_error_empty() {
        let msg = friendly_error("", None, None);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("empty page"));
    }

    #[test]
    fn test_friendly_error_with_email() {
        let msg = friendly_error("Sign in to continue", Some("user@example.com"), None);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("user@example.com"));
    }

    #[test]
    fn test_friendly_error_cloudflare() {
        let msg = friendly_error("Just a moment...", None, None);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("Cloudflare"));
    }

    #[test]
    fn test_normal_page_no_error() {
        let html = "<html><head></head><body>Welcome to your dashboard</body></html>";
        assert_eq!(OpenAIWebErrorKind::detect(html), None);
        assert!(friendly_error(html, None, None).is_none());
    }
}
