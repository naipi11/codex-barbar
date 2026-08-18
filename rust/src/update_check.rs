//! Manual-only update checking against the public codex-barbar GitHub feed.
//!
//! This module performs one anonymous GET when the user asks, compares
//! release versions, and reports whether a newer public release exists.
//! It never fetches assets, installs updates, or exits the app.

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const RELEASE_API_URL: &str =
    "https://api.github.com/repos/naipi11/codex-barbar/releases/latest";
pub const RELEASE_PAGE_URL: &str = "https://github.com/naipi11/codex-barbar/releases";
pub const CODEX_USAGE_PAGE_URL: &str = "https://chatgpt.com/codex/settings/usage";

const GITHUB_HOST: &str = "github.com";
const GITHUB_RELEASE_PATH_PREFIX: &str = "/naipi11/codex-barbar/releases/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ManualUpdateResult {
    #[serde(rename_all = "camelCase")]
    Current { current_version: String },
    #[serde(rename_all = "camelCase")]
    Available {
        current_version: String,
        latest_version: String,
    },
    #[serde(rename_all = "camelCase")]
    ReleaseFeedUnavailable { current_version: String },
}

pub struct ManualUpdateChecker {
    endpoint: String,
    client: reqwest::Client,
}

impl ManualUpdateChecker {
    pub fn new() -> Self {
        Self::with_endpoint(RELEASE_API_URL)
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("codex-barbar/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("update-check HTTP client must build");
        Self {
            endpoint: endpoint.into(),
            client,
        }
    }

    pub async fn check(&self) -> Result<ManualUpdateResult, String> {
        let response = self
            .client
            .get(&self.endpoint)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Ok(ManualUpdateResult::ReleaseFeedUnavailable {
                current_version: current_version(),
            });
        }
        let release: GitHubRelease = response.json().await.map_err(|error| error.to_string())?;
        Ok(parse_release(release))
    }
}

impl Default for ManualUpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

fn parse_release(release: GitHubRelease) -> ManualUpdateResult {
    let current = current_version();
    if !release_html_url_is_valid(&release.html_url) {
        return ManualUpdateResult::ReleaseFeedUnavailable {
            current_version: current,
        };
    }
    let Some(remote) = parse_release_tag(&release.tag_name) else {
        return ManualUpdateResult::ReleaseFeedUnavailable {
            current_version: current,
        };
    };
    if remote > current_release_version() {
        ManualUpdateResult::Available {
            current_version: current,
            latest_version: release.tag_name,
        }
    } else {
        ManualUpdateResult::Current {
            current_version: current,
        }
    }
}

fn release_html_url_is_valid(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.host_str() == Some(GITHUB_HOST)
        && parsed.path().starts_with(GITHUB_RELEASE_PATH_PREFIX)
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Open the fixed public release page. No URL crosses the WebView boundary.
pub fn open_release_page() -> Result<(), String> {
    open::that(RELEASE_PAGE_URL).map_err(|error| error.to_string())
}

/// Open the fixed Codex usage page. No URL crosses the WebView boundary.
pub fn open_codex_usage_page() -> Result<(), String> {
    open::that(CODEX_USAGE_PAGE_URL).map_err(|error| error.to_string())
}

fn current_release_version() -> ReleaseVersion {
    parse_release_tag(&format!("v{}", env!("CARGO_PKG_VERSION"))).unwrap_or(ReleaseVersion {
        major: 0,
        minor: 1,
        patch: 0,
        prerelease: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PrereleaseChannel {
    Alpha,
    Beta,
    Rc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prerelease {
    channel: PrereleaseChannel,
    number: u32,
}

impl PartialOrd for Prerelease {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Prerelease {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.channel
            .cmp(&other.channel)
            .then_with(|| self.number.cmp(&other.number))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseVersion {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Option<Prerelease>,
}

impl PartialOrd for ReleaseVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReleaseVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| match (self.prerelease, other.prerelease) {
                (None, None) => std::cmp::Ordering::Equal,
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}

/// Accepted public tags are `vMAJOR.MINOR.PATCH` or
/// `vMAJOR.MINOR.PATCH-(alpha|beta|rc).N`; the `v` prefix is required.
fn parse_release_tag(tag: &str) -> Option<ReleaseVersion> {
    let core = tag.strip_prefix('v')?;
    let (numbers, prerelease) = match core.split_once('-') {
        Some((numbers, prerelease)) => (numbers, Some(parse_prerelease(prerelease)?)),
        None => (core, None),
    };
    let mut parts = numbers.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ReleaseVersion {
        major,
        minor,
        patch,
        prerelease,
    })
}

fn parse_prerelease(value: &str) -> Option<Prerelease> {
    let (channel, number) = value.split_once('.')?;
    let channel = match channel {
        "alpha" => PrereleaseChannel::Alpha,
        "beta" => PrereleaseChannel::Beta,
        "rc" => PrereleaseChannel::Rc,
        _ => return None,
    };
    Some(Prerelease {
        channel,
        number: number.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, ServerGuard};

    async fn release_server(status: usize, body: &str) -> (ServerGuard, mockito::Mock) {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/naipi11/codex-barbar/releases/latest")
            .match_header("authorization", Matcher::Missing)
            .with_status(status)
            .with_body(body)
            .create();
        (server, mock)
    }

    fn endpoint(server: &ServerGuard) -> String {
        format!(
            "{}/repos/naipi11/codex-barbar/releases/latest",
            server.url()
        )
    }

    #[tokio::test]
    async fn private_release_feed_degrades_without_credentials() {
        let (server, mock) = release_server(404, "").await;
        let result = ManualUpdateChecker::with_endpoint(endpoint(&server))
            .check()
            .await
            .unwrap();
        assert_eq!(
            result,
            ManualUpdateResult::ReleaseFeedUnavailable {
                current_version: current_version(),
            }
        );
        mock.assert();
    }

    #[test]
    fn update_module_exports_no_download_or_apply_function() {
        // Scan only the production portion of this file; the test section
        // itself legitimately names the forbidden tokens.
        let full_source = include_str!("update_check.rs");
        let source = full_source
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(full_source);
        assert!(!source.contains("download_update"));
        assert!(!source.contains("apply_update"));
    }

    #[tokio::test]
    async fn public_newer_release_reports_available() {
        let (server, mock) = release_server(
            200,
            r#"{"tag_name":"v99.0.0","html_url":"https://github.com/naipi11/codex-barbar/releases/tag/v99.0.0"}"#,
        )
        .await;
        let result = ManualUpdateChecker::with_endpoint(endpoint(&server))
            .check()
            .await
            .unwrap();
        assert_eq!(
            result,
            ManualUpdateResult::Available {
                current_version: current_version(),
                latest_version: "v99.0.0".to_string(),
            }
        );
        mock.assert();
    }

    #[tokio::test]
    async fn same_or_older_release_is_current() {
        let (server, mock) = release_server(
            200,
            r#"{"tag_name":"v1.0.0","html_url":"https://github.com/naipi11/codex-barbar/releases/tag/v1.0.0"}"#,
        )
        .await;
        let result = ManualUpdateChecker::with_endpoint(endpoint(&server))
            .check()
            .await
            .unwrap();
        assert_eq!(
            result,
            ManualUpdateResult::Current {
                current_version: current_version(),
            }
        );
        mock.assert();
    }

    #[tokio::test]
    async fn invalid_release_url_degrades_to_unavailable() {
        let (server, mock) = release_server(
            200,
            r#"{"tag_name":"v1.0.1","html_url":"https://evil.example/releases/v1.0.1"}"#,
        )
        .await;
        let result = ManualUpdateChecker::with_endpoint(endpoint(&server))
            .check()
            .await
            .unwrap();
        assert_eq!(
            result,
            ManualUpdateResult::ReleaseFeedUnavailable {
                current_version: current_version(),
            }
        );
        mock.assert();
    }

    #[test]
    fn release_versions_compare_core_then_prerelease_channels() {
        let alpha_1 = parse_release_tag("v1.0.0-alpha.1").unwrap();
        let alpha_2 = parse_release_tag("v1.0.0-alpha.2").unwrap();
        let beta_1 = parse_release_tag("v1.0.0-beta.1").unwrap();
        let rc_1 = parse_release_tag("v1.0.0-rc.1").unwrap();
        let final_1 = parse_release_tag("v1.0.0").unwrap();
        let next_patch = parse_release_tag("v1.0.1").unwrap();

        assert!(alpha_1 < alpha_2);
        assert!(alpha_2 < beta_1);
        assert!(beta_1 < rc_1);
        assert!(rc_1 < final_1);
        assert!(final_1 < next_patch);
    }

    #[test]
    fn malformed_and_foreign_tags_are_rejected() {
        for tag in [
            "v1.2",
            "v1.2.3.4",
            "v1.2.3-foo.1",
            "v1.2.3-alpha",
            "1.2.3",
            "v1.2.3-beta.x",
        ] {
            assert!(parse_release_tag(tag).is_none(), "{tag}");
        }
    }

    #[test]
    fn fixed_page_urls_are_exact() {
        assert_eq!(
            RELEASE_PAGE_URL,
            "https://github.com/naipi11/codex-barbar/releases"
        );
        assert_eq!(
            CODEX_USAGE_PAGE_URL,
            "https://chatgpt.com/codex/settings/usage"
        );
    }
}
