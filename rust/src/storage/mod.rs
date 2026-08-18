//! Persistent storage for account metadata and usage snapshots (Phase 2).

pub mod account_repository;
pub mod database;
pub mod migrations;
pub mod settings_repository;
pub mod usage_repository;

use std::path::Path;
use std::sync::Arc;

use crate::core::ProfileId;

pub use account_repository::AccountRepository;
pub use database::{AppDatabase, StorageError};
pub use settings_repository::{
    AppSettings, DisplayMode, LanguagePreference, SettingsPatch, SettingsRepository,
    ThemePreference,
};
pub use usage_repository::{SqliteUsageRepository, UsageCacheKey, UsageRepository};

/// Repositories opened against one application database.
pub struct AccountRepositories {
    pub accounts: AccountRepository,
    pub settings: SettingsRepository,
    pub usage: Box<dyn UsageRepository>,
}

impl AccountRepositories {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let db = Arc::new(AppDatabase::open(path)?);
        Ok(Self {
            accounts: AccountRepository::new(Arc::clone(&db)),
            settings: SettingsRepository::new(Arc::clone(&db)),
            usage: Box::new(SqliteUsageRepository::new(db)),
        })
    }
}

/// Bootstrap result for the desktop shell.
pub enum DatabaseBootstrap {
    Ready(AccountRepositories),
    ReadOnlyFailure {
        database_path: std::path::PathBuf,
        backup_path: Option<std::path::PathBuf>,
        error: StorageError,
    },
}

impl DatabaseBootstrap {
    pub fn open(path: &Path) -> Self {
        match AccountRepositories::open(path) {
            Ok(repositories) => Self::Ready(repositories),
            Err(error) => Self::ReadOnlyFailure {
                database_path: path.to_path_buf(),
                backup_path: None,
                error,
            },
        }
    }

    pub fn ready_profile_ids(&self) -> Vec<ProfileId> {
        Vec::new()
    }
}

/// Deterministic temporary repository pair for storage tests.
#[cfg(test)]
pub(crate) fn test_repositories() -> AccountRepositories {
    let dir = tempfile::tempdir().unwrap();
    AccountRepositories::open(&dir.path().join("test.db")).unwrap()
}

/// Deterministic temporary usage repository with a seeded managed profile.
#[cfg(test)]
pub(crate) fn test_usage_repository() -> Box<dyn UsageRepository> {
    let dir = tempfile::tempdir().unwrap();
    let db = AppDatabase::open(&dir.path().join("usage.db")).unwrap();
    db.with_connection(|connection| {
        connection
            .execute(
                "INSERT INTO account_profiles
                    (id, kind, label, auth_mode, lifecycle, email_fingerprint, created_at,
                     last_selected_at, last_success_at)
                 VALUES ('00000000-0000-0000-0000-000000000000', 'managed', 'Test', 'unknown',
                         'ready', NULL, 0, NULL, NULL)",
                [],
            )
            .map_err(|error| {
                StorageError::new(
                    crate::core::AppErrorKind::StorageFailure,
                    "DB_FIXTURE_SEED_FAILED",
                    error.to_string(),
                )
            })?;
        Ok(())
    })
    .unwrap();
    Box::new(SqliteUsageRepository::new(Arc::new(db)))
}
