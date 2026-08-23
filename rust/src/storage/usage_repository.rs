//! Last-success snapshot and current-error repository.

use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use std::sync::Arc;
use uuid::Uuid;

use crate::core::{
    AppError, AppErrorKind, Freshness, ProfileId, ProfileUsageSnapshot, ProfileUsageState,
    RefreshStatus,
};

use super::database::{AppDatabase, StorageError};

/// Composite cache key for the only shipping provider (Codex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsageCacheKey {
    pub profile_id: ProfileId,
    pub provider_id: crate::core::ProviderId,
}

impl UsageCacheKey {
    pub const fn codex(profile_id: ProfileId) -> Self {
        Self {
            profile_id,
            provider_id: crate::core::ProviderId::Codex,
        }
    }
}

/// Persistence boundary for profile usage state.
pub trait UsageRepository: Send + Sync {
    fn load_state(&self, profile_id: ProfileId) -> Result<ProfileUsageState, StorageError>;
    fn load_all_states(&self) -> Result<Vec<ProfileUsageState>, StorageError>;
    fn save_success(&self, snapshot: &ProfileUsageSnapshot) -> Result<(), StorageError>;
    fn save_error(&self, profile_id: ProfileId, error: &AppError) -> Result<(), StorageError>;
    fn delete_profile(&self, profile_id: ProfileId) -> Result<(), StorageError>;
}

/// SQLite-backed implementation of [`UsageRepository`].
pub struct SqliteUsageRepository {
    db: Arc<AppDatabase>,
}

impl SqliteUsageRepository {
    pub fn new(db: Arc<AppDatabase>) -> Self {
        Self { db }
    }
}

impl UsageRepository for SqliteUsageRepository {
    fn load_state(&self, profile_id: ProfileId) -> Result<ProfileUsageState, StorageError> {
        let snapshot_json = self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT snapshot_json FROM usage_snapshots
                     WHERE profile_id = ?1 AND provider_id = 'codex'",
                    params![profile_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage_error)
        })?;
        let snapshot = snapshot_json
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| {
                StorageError::new(
                    AppErrorKind::StorageFailure,
                    "DB_SNAPSHOT_DECODE_FAILED",
                    error.to_string(),
                )
            })?;

        let error_json = self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT error_json FROM profile_refresh_state
                     WHERE profile_id = ?1 AND provider_id = 'codex'",
                    params![profile_id.to_string()],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(storage_error)
        })?;
        let error_json = error_json.flatten();
        let current_error = error_json
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|error| {
                StorageError::new(
                    AppErrorKind::StorageFailure,
                    "DB_ERROR_DECODE_FAILED",
                    error.to_string(),
                )
            })?;

        let (refresh_status, freshness) = match (&snapshot, &current_error) {
            (Some(_), Some(_)) => (RefreshStatus::Idle, Freshness::Fresh),
            (Some(_), None) => (RefreshStatus::Idle, Freshness::Fresh),
            (None, Some(_)) => (RefreshStatus::Blocked, Freshness::Missing),
            (None, None) => (RefreshStatus::Idle, Freshness::Missing),
        };
        Ok(ProfileUsageState {
            profile_id,
            snapshot,
            current_error,
            refresh_status,
            freshness,
            manual_cooldown_until: None,
        })
    }

    fn load_all_states(&self) -> Result<Vec<ProfileUsageState>, StorageError> {
        let ids = self.db.with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT id FROM account_profiles WHERE lifecycle = 'ready'")
                .map_err(storage_error)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(storage_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(storage_error)
        })?;
        ids.into_iter()
            .map(|id| {
                Uuid::parse_str(&id)
                    .map_err(|error| {
                        StorageError::new(
                            AppErrorKind::StorageFailure,
                            "DB_PROFILE_ID_INVALID",
                            error.to_string(),
                        )
                    })
                    .and_then(|profile_id| self.load_state(profile_id))
            })
            .collect()
    }

    fn save_success(&self, snapshot: &ProfileUsageSnapshot) -> Result<(), StorageError> {
        let snapshot_json = serde_json::to_string(snapshot).map_err(|error| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_SNAPSHOT_ENCODE_FAILED",
                error.to_string(),
            )
        })?;
        self.db.with_connection_mut(|connection| {
            let transaction = connection.unchecked_transaction().map_err(storage_error)?;
            transaction
                .execute(
                    "INSERT INTO usage_snapshots(profile_id, provider_id, snapshot_json, fetched_at)
                     VALUES (?1, 'codex', ?2, ?3)
                     ON CONFLICT(profile_id, provider_id) DO UPDATE SET
                        snapshot_json = excluded.snapshot_json,
                        fetched_at = excluded.fetched_at",
                    params![
                        snapshot.profile_id.to_string(),
                        snapshot_json,
                        snapshot.fetched_at.timestamp()
                    ],
                )
                .map_err(storage_error)?;
            transaction
                .execute(
                    "INSERT INTO profile_refresh_state(profile_id, provider_id, error_json, attempted_at)
                     VALUES (?1, 'codex', NULL, ?2)
                     ON CONFLICT(profile_id, provider_id) DO UPDATE SET
                        error_json = NULL,
                        attempted_at = excluded.attempted_at",
                    params![snapshot.profile_id.to_string(), Utc::now().timestamp()],
                )
                .map_err(storage_error)?;
            transaction.commit().map_err(storage_error)
        })
    }

    fn save_error(&self, profile_id: ProfileId, error: &AppError) -> Result<(), StorageError> {
        let error_json = serde_json::to_string(error).map_err(|encode_error| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_ERROR_ENCODE_FAILED",
                encode_error.to_string(),
            )
        })?;
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO profile_refresh_state(profile_id, provider_id, error_json, attempted_at)
                     VALUES (?1, 'codex', ?2, ?3)
                     ON CONFLICT(profile_id, provider_id) DO UPDATE SET
                        error_json = excluded.error_json,
                        attempted_at = excluded.attempted_at",
                    params![profile_id.to_string(), error_json, Utc::now().timestamp()],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }

    fn delete_profile(&self, profile_id: ProfileId) -> Result<(), StorageError> {
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM account_profiles WHERE id = ?1",
                    params![profile_id.to_string()],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }
}

fn storage_error(error: rusqlite::Error) -> StorageError {
    StorageError::new(
        AppErrorKind::StorageFailure,
        "DB_QUERY_FAILED",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use uuid::Uuid;

    use crate::core::{
        AppError, AppErrorKind, ProfileUsageSnapshot, RecoveryAction, UsageSource, UsageWindow,
    };
    use crate::storage::test_usage_repository;

    fn profile_id() -> Uuid {
        Uuid::nil()
    }

    fn snapshot() -> ProfileUsageSnapshot {
        ProfileUsageSnapshot {
            profile_id: profile_id(),
            plan_type: Some("plus".to_string()),
            primary: Some(UsageWindow::normalized("codex", None, 25.0, Some(300), None, None).0),
            secondary: None,
            additional_windows: Vec::new(),
            fetched_at: DateTime::from_timestamp(1_750_000_000, 0).unwrap(),
            source: UsageSource::AppServer,
            protocol_anomaly: false,
            reset_credits: None,
        }
    }

    fn offline_error() -> AppError {
        AppError::new(
            AppErrorKind::OfflineOrTimeout,
            "errors.offlineOrTimeout",
            RecoveryAction::Retry,
            "APP_SERVER_RPC_TIMEOUT",
        )
    }

    #[test]
    fn refresh_error_does_not_replace_last_success() {
        let repo = test_usage_repository();
        repo.save_success(&snapshot()).unwrap();
        repo.save_error(profile_id(), &offline_error()).unwrap();
        let state = repo.load_state(profile_id()).unwrap();
        assert_eq!(state.snapshot, Some(snapshot()));
        assert_eq!(
            state.current_error.unwrap().kind,
            AppErrorKind::OfflineOrTimeout
        );
    }
    #[test]
    fn reset_credit_snapshot_round_trips_through_storage() {
        let repo = test_usage_repository();
        let mut snapshot = snapshot();
        snapshot.reset_credits = Some(crate::core::ResetCreditsSummary { available_count: 3 });
        repo.save_success(&snapshot).unwrap();
        let state = repo.load_state(profile_id()).unwrap();
        assert_eq!(
            state
                .snapshot
                .unwrap()
                .reset_credits
                .unwrap()
                .available_count,
            3
        );
    }
}
