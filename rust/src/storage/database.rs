//! SQLite database bootstrap with WAL, foreign keys, and a read-only failure
//! mode that never destroys the source database.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::core::{AppError, AppErrorKind, RecoveryAction};

use super::migrations;

/// Storage failure produced by the database layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    pub kind: AppErrorKind,
    pub diagnostic_code: &'static str,
    pub message: String,
}

impl StorageError {
    pub(crate) fn new(
        kind: AppErrorKind,
        diagnostic_code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            diagnostic_code,
            message: message.into(),
        }
    }

    pub fn into_app_error(self) -> AppError {
        AppError::new(
            self.kind,
            "errors.storageFailure",
            RecoveryAction::ExportDiagnostics,
            self.diagnostic_code,
        )
    }

    pub fn code(&self) -> &'static str {
        self.diagnostic_code
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.diagnostic_code)
    }
}

impl std::error::Error for StorageError {}

/// A writable application database.
pub struct AppDatabase {
    connection: Mutex<Connection>,
    path: PathBuf,
}

impl AppDatabase {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let parent = path.parent().ok_or_else(|| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_PATH_INVALID",
                "database path has no parent",
            )
        })?;
        std::fs::create_dir_all(parent).map_err(|error| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_DIR_CREATE_FAILED",
                error.to_string(),
            )
        })?;
        let mut connection = Connection::open(path).map_err(|error| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_OPEN_FAILED",
                error.to_string(),
            )
        })?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| {
                StorageError::new(
                    AppErrorKind::StorageFailure,
                    "DB_FK_PRAGMA_FAILED",
                    error.to_string(),
                )
            })?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| {
                StorageError::new(
                    AppErrorKind::StorageFailure,
                    "DB_WAL_PRAGMA_FAILED",
                    error.to_string(),
                )
            })?;
        migrations::migrate(&mut connection, path)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path: path.to_path_buf(),
        })
    }

    /// Run one database operation against the guarded connection.
    pub(crate) fn with_connection<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let connection = self.connection.lock().map_err(|_| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_LOCK_POISONED",
                "database connection lock was poisoned",
            )
        })?;
        f(&connection)
    }

    /// Run one database operation requiring a mutable connection (transactions).
    pub(crate) fn with_connection_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let mut connection = self.connection.lock().map_err(|_| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_LOCK_POISONED",
                "database connection lock was poisoned",
            )
        })?;
        f(&mut connection)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_open_preserves_source_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-db.db");
        std::fs::write(&path, b"original bytes").unwrap();
        assert!(AppDatabase::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original bytes");
    }
}
