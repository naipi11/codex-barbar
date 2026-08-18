//! Account profile metadata repository.

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params, types::Type};
use std::sync::Arc;
use uuid::Uuid;

use crate::accounts::model::{AccountProfile, ProfileKind, ProfileLifecycle};
use crate::core::{AuthMode, ProfileId};

use super::database::{AppDatabase, StorageError};

fn profile_id_to_db(id: ProfileId) -> String {
    id.to_string()
}

fn kind_from_db(value: &str) -> ProfileKind {
    if value == "managed" {
        ProfileKind::Managed
    } else {
        ProfileKind::CurrentCli
    }
}

fn lifecycle_from_db(value: &str) -> ProfileLifecycle {
    match value {
        "pending" => ProfileLifecycle::Pending,
        "removing" => ProfileLifecycle::Removing,
        _ => ProfileLifecycle::Ready,
    }
}

fn auth_mode_from_db(value: &str) -> AuthMode {
    match value {
        "chatgpt" => AuthMode::ChatGpt,
        "apiKey" => AuthMode::ApiKey,
        _ => AuthMode::Unknown,
    }
}

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<AccountProfile> {
    let id_text: String = row.get(0)?;
    let kind: String = row.get(1)?;
    let label: String = row.get(2)?;
    let auth_mode: String = row.get(3)?;
    let lifecycle: String = row.get(4)?;
    let email_fingerprint: Option<Vec<u8>> = row.get(5)?;
    let created_at: i64 = row.get(6)?;
    let last_selected_at: Option<i64> = row.get(7)?;
    let last_success_at: Option<i64> = row.get(8)?;
    Ok(AccountProfile {
        id: Uuid::parse_str(&id_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
        })?,
        kind: kind_from_db(&kind),
        label,
        auth_mode: auth_mode_from_db(&auth_mode),
        lifecycle: lifecycle_from_db(&lifecycle),
        email_fingerprint: email_fingerprint.and_then(|bytes| bytes.try_into().ok()),
        created_at: DateTime::from_timestamp(created_at, 0).unwrap_or_default(),
        last_selected_at: last_selected_at.and_then(|v| DateTime::from_timestamp(v, 0)),
        last_success_at: last_success_at.and_then(|v| DateTime::from_timestamp(v, 0)),
    })
}

/// Repository for non-secret profile metadata.
pub struct AccountRepository {
    db: Arc<AppDatabase>,
}

impl AccountRepository {
    pub fn new(db: Arc<AppDatabase>) -> Self {
        Self { db }
    }

    pub fn ensure_current_cli(&self, now: DateTime<Utc>) -> Result<AccountProfile, StorageError> {
        let existing = self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                            last_selected_at, last_success_at
                     FROM account_profiles WHERE kind = 'currentCli' LIMIT 1",
                    [],
                    row_to_profile,
                )
                .optional()
                .map_err(storage_error)
        })?;
        if let Some(profile) = existing {
            return Ok(profile);
        }
        let id = Uuid::new_v4();
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO account_profiles
                        (id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                         last_selected_at, last_success_at)
                     VALUES (?1, 'currentCli', 'Current CLI', 'unknown', 'ready', NULL, ?2, ?2, NULL)",
                    params![profile_id_to_db(id), now.timestamp()],
                )
                .map_err(storage_error)?;
            Ok(())
        })?;
        self.ensure_selected(id)?;
        self.get(id)?.ok_or_else(|| {
            StorageError::new(
                crate::core::AppErrorKind::StorageFailure,
                "DB_PROFILE_MISSING_AFTER_INSERT",
                "inserted profile was not found",
            )
        })
    }

    pub fn get(&self, id: ProfileId) -> Result<Option<AccountProfile>, StorageError> {
        self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                            last_selected_at, last_success_at
                     FROM account_profiles WHERE id = ?1",
                    params![profile_id_to_db(id)],
                    row_to_profile,
                )
                .optional()
                .map_err(storage_error)
        })
    }

    pub fn list_ready(&self) -> Result<Vec<AccountProfile>, StorageError> {
        self.db.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                            last_selected_at, last_success_at
                     FROM account_profiles WHERE lifecycle = 'ready' ORDER BY created_at",
                )
                .map_err(storage_error)?;
            let rows = statement
                .query_map([], row_to_profile)
                .map_err(storage_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(storage_error)
        })
    }

    pub fn list_all(&self) -> Result<Vec<AccountProfile>, StorageError> {
        self.db.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                            last_selected_at, last_success_at
                     FROM account_profiles ORDER BY created_at",
                )
                .map_err(storage_error)?;
            let rows = statement
                .query_map([], row_to_profile)
                .map_err(storage_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(storage_error)
        })
    }

    pub fn insert_pending(
        &self,
        id: ProfileId,
        label: String,
        now: DateTime<Utc>,
    ) -> Result<AccountProfile, StorageError> {
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO account_profiles
                        (id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                         last_selected_at, last_success_at)
                     VALUES (?1, 'managed', ?2, 'unknown', 'pending', NULL, ?3, NULL, NULL)",
                    params![profile_id_to_db(id), label, now.timestamp()],
                )
                .map_err(storage_error)?;
            Ok(())
        })?;
        self.get(id)?.ok_or_else(|| {
            StorageError::new(
                crate::core::AppErrorKind::StorageFailure,
                "DB_PROFILE_MISSING_AFTER_INSERT",
                "inserted profile was not found",
            )
        })
    }

    pub fn update_profile(
        &self,
        id: ProfileId,
        label: &str,
        auth_mode: AuthMode,
        lifecycle: ProfileLifecycle,
        email_fingerprint: Option<[u8; 32]>,
    ) -> Result<(), StorageError> {
        let auth_mode_db = match auth_mode {
            AuthMode::Unknown => "unknown",
            AuthMode::ChatGpt => "chatgpt",
            AuthMode::ApiKey => "apiKey",
        };
        let lifecycle_db = match lifecycle {
            ProfileLifecycle::Pending => "pending",
            ProfileLifecycle::Ready => "ready",
            ProfileLifecycle::Removing => "removing",
        };
        let fingerprint = email_fingerprint.map(|bytes| bytes.to_vec());
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE account_profiles
                     SET label = ?2, auth_mode = ?3, lifecycle = ?4, email_fingerprint = ?5
                     WHERE id = ?1",
                    params![
                        profile_id_to_db(id),
                        label,
                        auth_mode_db,
                        lifecycle_db,
                        fingerprint
                    ],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }

    pub fn delete_profile(&self, id: ProfileId) -> Result<(), StorageError> {
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM account_profiles WHERE id = ?1",
                    params![profile_id_to_db(id)],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }

    pub fn selected_profile_id(&self) -> Result<ProfileId, StorageError> {
        let value = self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT value_json FROM app_settings WHERE key = 'selected_profile_id'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage_error)
        })?;
        match value {
            Some(json) => serde_json::from_str(&json).map_err(|error| {
                StorageError::new(
                    crate::core::AppErrorKind::StorageFailure,
                    "DB_SELECTED_ID_INVALID",
                    error.to_string(),
                )
            }),
            None => Err(StorageError::new(
                crate::core::AppErrorKind::StorageFailure,
                "DB_SELECTED_ID_MISSING",
                "no selected profile",
            )),
        }
    }

    pub fn set_selected(
        &self,
        profile_id: ProfileId,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        self.ensure_selected(profile_id)?;
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE account_profiles SET last_selected_at = ?1 WHERE id = ?2",
                    params![now.timestamp(), profile_id_to_db(profile_id)],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }

    fn ensure_selected(&self, profile_id: ProfileId) -> Result<(), StorageError> {
        let encoded = serde_json::to_string(&profile_id).map_err(|error| {
            StorageError::new(
                crate::core::AppErrorKind::StorageFailure,
                "DB_SELECTED_ID_ENCODE_FAILED",
                error.to_string(),
            )
        })?;
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings(key, value_json) VALUES ('selected_profile_id', ?1)
                     ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                    params![encoded],
                )
                .map_err(storage_error)?;
            Ok(())
        })
    }
}

fn storage_error(error: rusqlite::Error) -> StorageError {
    StorageError::new(
        crate::core::AppErrorKind::StorageFailure,
        "DB_QUERY_FAILED",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::accounts::model::ProfileKind;
    use crate::core::AuthMode;
    use crate::storage::test_repositories;

    #[test]
    fn current_cli_is_unique_and_selected_by_default() {
        let repos = test_repositories();
        let first = repos.accounts.ensure_current_cli(Utc::now()).unwrap();
        let second = repos.accounts.ensure_current_cli(Utc::now()).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(repos.accounts.selected_profile_id().unwrap(), first.id);
        assert_eq!(
            repos
                .accounts
                .list_ready()
                .unwrap()
                .iter()
                .filter(|profile| profile.kind == ProfileKind::CurrentCli)
                .count(),
            1
        );
        assert_eq!(first.auth_mode, AuthMode::Unknown);
    }
}
