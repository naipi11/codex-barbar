//! Stable product error kinds with recovery actions and redacted diagnostics.
//!
//! `AppError` is the only error type that crosses from provider internals into
//! product surfaces (tray panel, settings, logs). It deliberately has no field
//! capable of carrying raw protocol text, credentials, tokens, or file
//! contents: diagnostics are limited to a short redacted code string.

use serde::{Deserialize, Serialize};

/// Stable, user-actionable error categories for the codex-barbar product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppErrorKind {
    /// No Codex CLI / app-server executable could be resolved.
    CodexNotFound,
    /// The resolved Codex version is outside the tested compatibility matrix.
    UnsupportedCodexVersion,
    /// The current CLI has no signed-in account.
    NotSignedIn,
    /// The account authenticates with an API key, which exposes no quota data.
    ApiKeyNoQuota,
    /// Stored or CLI-side authentication has expired.
    AuthExpired,
    /// Network failure, process hang, or RPC timeout.
    OfflineOrTimeout,
    /// The server asked us to back off.
    RateLimited,
    /// The app-server wire protocol did not match the frozen schema.
    ProtocolMismatch,
    /// DPAPI or vault read/write failed (fatal for the vault operation).
    VaultFailure,
    /// Local settings/database storage failed.
    StorageFailure,
}

/// What the UI should offer the user to recover from an [`AppError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryAction {
    /// No user action available; wait or report a bug.
    None,
    /// Install (or repair) a Codex version from the tested matrix.
    InstallTestedCodex,
    /// Sign in with the Codex CLI, then return here.
    SignInWithCli,
    /// Re-authenticate (token refresh failed or expired).
    Reauthenticate,
    /// Retry the failed operation.
    Retry,
    /// Wait for the rate-limit/backoff window to pass.
    WaitForReset,
    /// Show recovery/export-diagnostics UI.
    ExportDiagnostics,
}

/// A stable, redacted product error.
///
/// Serialization carries only `kind`, a localization `message_key`, a
/// `recovery` hint, and an optional short redacted `diagnosticCode`. There is
/// intentionally no field for raw protocol lines, HTTP bodies, cookies, or
/// token material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    /// i18n key, e.g. "errors.protocolMismatch".
    pub message_key: String,
    pub recovery: RecoveryAction,
    /// Short redacted diagnostic code (e.g. "APP_SERVER_REQUIRED_FIELD_MISSING").
    /// Never contains raw protocol text or secret material.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub diagnostic_code: String,
}

impl AppError {
    pub fn new(
        kind: AppErrorKind,
        message_key: impl Into<String>,
        recovery: RecoveryAction,
        diagnostic: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message_key: message_key.into(),
            recovery,
            diagnostic_code: diagnostic.into(),
        }
    }

    /// Error without a diagnostic code.
    pub fn bare(
        kind: AppErrorKind,
        message_key: impl Into<String>,
        recovery: RecoveryAction,
    ) -> Self {
        Self {
            kind,
            message_key: message_key.into(),
            recovery,
            diagnostic_code: String::new(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ({})", self.kind, self.message_key)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serialization_contains_only_stable_redacted_fields() {
        let error = AppError::new(
            AppErrorKind::ProtocolMismatch,
            "errors.protocolMismatch",
            RecoveryAction::InstallTestedCodex,
            "APP_SERVER_REQUIRED_FIELD_MISSING",
        );
        let value = serde_json::to_value(&error).unwrap();
        assert_eq!(value["kind"], "protocolMismatch");
        assert_eq!(value["messageKey"], "errors.protocolMismatch");
        assert_eq!(value["recovery"], "installTestedCodex");
        assert_eq!(value["diagnosticCode"], "APP_SERVER_REQUIRED_FIELD_MISSING");
        assert!(value.get("rawLine").is_none());
        assert!(value.get("source").is_none());
    }

    #[test]
    fn bare_error_omits_diagnostic() {
        let error = AppError::bare(
            AppErrorKind::NotSignedIn,
            "errors.notSignedIn",
            RecoveryAction::SignInWithCli,
        );
        let value = serde_json::to_value(&error).unwrap();
        assert!(value.get("diagnosticCode").is_none());
        assert_eq!(value["kind"], "notSignedIn");
    }

    #[test]
    fn error_kind_round_trips_all_variants() {
        let kinds = [
            AppErrorKind::CodexNotFound,
            AppErrorKind::UnsupportedCodexVersion,
            AppErrorKind::NotSignedIn,
            AppErrorKind::ApiKeyNoQuota,
            AppErrorKind::AuthExpired,
            AppErrorKind::OfflineOrTimeout,
            AppErrorKind::RateLimited,
            AppErrorKind::ProtocolMismatch,
            AppErrorKind::VaultFailure,
            AppErrorKind::StorageFailure,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let back: AppErrorKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn app_error_implements_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        let e = AppError::bare(
            AppErrorKind::OfflineOrTimeout,
            "errors.offlineOrTimeout",
            RecoveryAction::Retry,
        );
        assert_error(&e);
        assert!(e.to_string().contains("offlineOrTimeout"));
    }
}
