//! Fixed-location, redacted diagnostics summary and export.
//!
//! Export writes to
//! `%LOCALAPPDATA%\codex-barbar\diagnostics\codex-barbar-diagnostics-yyyyMMddTHHmmssZ.json`.
//! The model is redacted before serialization and the completed temporary
//! file is scanned again before atomic publish; a failed final scan removes
//! the temporary file and preserves any previous export.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::SecretRedactor;

/// Bounded number of log lines included in an export.
pub const LOG_TAIL_LINES: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSummaryDto {
    pub product_name: &'static str,
    pub version: String,
    pub os: String,
    pub codex_version: Option<String>,
    pub resolved_path_class: String,
    pub capabilities: DiagnosticsCapabilitiesDto,
    pub profile_kinds: BTreeMap<String, usize>,
    pub profile_count: usize,
    pub refresh_times: Vec<String>,
    pub error_kinds: Vec<String>,
    pub vault_status: String,
    pub recovery_status: String,
    pub storage_status: String,
    pub tested_versions: Vec<String>,
    pub log_tail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsCapabilitiesDto {
    pub account_read: bool,
    pub rate_limits_read: bool,
    pub managed_login: bool,
}

/// Collects redacted diagnostic state from the current process.
#[derive(Debug, Default)]
pub struct Diagnostics {
    pub codex_version: Option<String>,
    pub capabilities: DiagnosticsCapabilitiesDto,
    pub profile_kinds: BTreeMap<String, usize>,
    pub refresh_times: Vec<String>,
    pub error_kinds: Vec<String>,
    pub vault_status: String,
    pub recovery_status: String,
    pub storage_status: String,
    pub tested_versions: Vec<String>,
}

impl Diagnostics {
    pub fn summary(&self, log_dir: Option<&Path>) -> DiagnosticsSummaryDto {
        DiagnosticsSummaryDto {
            product_name: "codex-barbar",
            version: env!("CARGO_PKG_VERSION").to_string(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            codex_version: self.codex_version.clone(),
            resolved_path_class: resolved_path_class(),
            capabilities: self.capabilities.clone(),
            profile_kinds: self.profile_kinds.clone(),
            profile_count: self.profile_kinds.values().sum(),
            refresh_times: self.refresh_times.clone(),
            error_kinds: self.error_kinds.clone(),
            vault_status: self.vault_status.clone(),
            recovery_status: self.recovery_status.clone(),
            storage_status: self.storage_status.clone(),
            tested_versions: self.tested_versions.clone(),
            log_tail: read_log_tail(log_dir),
        }
    }

    /// Export the redacted summary to the canonical diagnostics directory.
    pub fn export(&self) -> Result<PathBuf, String> {
        let dir = crate::app_paths::AppPaths::discover()
            .map(|paths| paths.root.join("diagnostics"))
            .unwrap_or_else(|_| PathBuf::from("codex-barbar-diagnostics"));
        self.export_to(&dir, scan_file)
    }

    /// Write a temporary file, scan it, then atomically publish. Any failure
    /// removes the temporary file and leaves the previous export intact.
    pub fn export_to(
        &self,
        dir: &Path,
        final_scan: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<PathBuf, String> {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        let final_path = dir.join(format!("codex-barbar-diagnostics-{stamp}.json"));
        let temp_path = dir.join(format!("codex-barbar-diagnostics-{stamp}.json.tmp"));

        let summary = self.summary(Some(dir.parent().and_then(|p| p.parent()).unwrap_or(dir)));
        let json = serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())?;
        let redacted = SecretRedactor::redact(&json);
        if let Err(error) = fs::write(&temp_path, redacted) {
            let _ = fs::remove_file(&temp_path);
            return Err(error.to_string());
        }

        let result = final_scan(&temp_path);
        if let Err(error) = result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        fs::rename(&temp_path, &final_path).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            error.to_string()
        })?;
        Ok(final_path)
    }
}

fn resolved_path_class() -> String {
    let Some(home) = dirs::home_dir() else {
        return "home-unavailable".to_string();
    };
    match crate::app_paths::AppPaths::discover() {
        Ok(paths) => {
            let root = paths.root.to_string_lossy().replace('\\', "/");
            let home = home.to_string_lossy().replace('\\', "/");
            if let Some((_, rest)) = root.split_once(&format!("{home}/")) {
                format!("%USERPROFILE%/{rest}")
            } else {
                "custom-root".to_string()
            }
        }
        Err(_) => "app-paths-unavailable".to_string(),
    }
}

fn read_log_tail(log_dir: Option<&Path>) -> String {
    let Some(dir) = log_dir else {
        return String::new();
    };
    let path = dir.join("codex-barbar.log");
    let Ok(text) = fs::read_to_string(&path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().rev().take(LOG_TAIL_LINES).collect();
    let mut tail = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
    tail = SecretRedactor::redact(&tail);
    tail
}

fn scan_file(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let redacted = SecretRedactor::redact(&content);
    if redacted != content {
        return Err("final scan found unredacted secret material".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Diagnostics) {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics {
            vault_status: "ok".to_string(),
            recovery_status: "ok".to_string(),
            storage_status: "ok".to_string(),
            ..Diagnostics::default()
        };
        (dir, diagnostics)
    }

    #[test]
    fn export_writes_fixed_named_redacted_file() {
        let (dir, diagnostics) = fixture();
        let exported = diagnostics.export_to(dir.path(), scan_file).unwrap();
        assert!(
            exported
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("codex-barbar-diagnostics-")
        );
        assert!(exported.to_string_lossy().ends_with(".json"));
        let text = fs::read_to_string(&exported).unwrap();
        assert!(text.contains("\"productName\": \"codex-barbar\""));
        assert!(!text.contains("secret"));
    }

    #[test]
    fn failed_final_scan_removes_temporary_export_and_preserves_previous_file() {
        let (dir, diagnostics) = fixture();
        let previous = dir
            .path()
            .join("codex-barbar-diagnostics-20200101T000000Z.json");
        fs::write(&previous, b"previous").unwrap();

        let result = diagnostics.export_to(dir.path(), |_path| {
            Err("injected final scan failure".to_string())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&previous).unwrap(), b"previous");
        let temp_files = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temp_files, 0);
    }
}
