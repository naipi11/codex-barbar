//! Profile, login, lifecycle, and service event types for codex-barbar V1.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::{AuthMode, ProfileId};

/// How a profile was provisioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileKind {
    CurrentCli,
    Managed,
}

/// Lifecycle state of an account profile row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileLifecycle {
    Pending,
    Ready,
    Removing,
}

/// Non-secret profile metadata persisted in SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountProfile {
    pub id: ProfileId,
    pub kind: ProfileKind,
    pub label: String,
    pub auth_mode: AuthMode,
    pub lifecycle: ProfileLifecycle,
    /// SHA-256 fingerprint of the normalized email; identity is never stored.
    pub email_fingerprint: Option<[u8; 32]>,
    pub created_at: DateTime<Utc>,
    pub last_selected_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
}

/// One view of all ready profiles plus the selected profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountProfilesSnapshot {
    pub profiles: Vec<AccountProfile>,
    pub selected_profile_id: ProfileId,
}

/// Redacted bootstrap cache exposed to the desktop shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapState {
    pub profiles: Vec<AccountProfile>,
    pub selected_profile_id: ProfileId,
    pub database_ready: bool,
}

impl AccountProfilesSnapshot {
    pub fn current_cli_removable(&self) -> bool {
        false
    }
}

/// Login operation types surfaced through the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedLoginMethod {
    Browser,
    DeviceCode,
}

#[derive(Debug, Clone)]
pub struct StartManagedLogin {
    pub label: String,
    pub method: ManagedLoginMethod,
    pub replace_profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedLoginStage {
    Starting,
    AwaitingUser,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ManagedLoginStatus {
    pub operation_id: Uuid,
    pub profile_id: ProfileId,
    pub stage: ManagedLoginStage,
    pub verification_url: Option<String>,
    pub user_code: Option<String>,
    pub error_kind: Option<crate::core::AppErrorKind>,
}

/// Events emitted by the account service for the shell/UI.
#[derive(Debug, Clone)]
pub enum AccountServiceEvent {
    ProfilesChanged(AccountProfilesSnapshot),
    LoginChanged(ManagedLoginStatus),
    SelectedProfileChanged {
        profile_id: ProfileId,
    },
    UsageStateChanged(Box<crate::core::ProfileUsageState>),
    RefreshStateChanged {
        profile_id: ProfileId,
        status: crate::core::RefreshStatus,
    },
}

/// Errors returned by the account lifecycle service.
#[derive(Debug, thiserror::Error)]
pub enum AccountServiceError {
    #[error("application operation failed")]
    App(#[from] crate::core::AppError),
    #[error("profile label is invalid")]
    InvalidLabel,
    #[error("Current CLI profile is immutable")]
    CurrentCliImmutable,
    #[error("another account operation is active")]
    Busy,
    #[error("account already exists")]
    DuplicateProfile { existing_profile_id: ProfileId },
    #[error("login operation was not found")]
    LoginOperationNotFound,
}
