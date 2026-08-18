//! Versioned SQLite migrations with WAL checkpointing and bounded backups.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::core::AppErrorKind;

use super::database::StorageError;

pub const SCHEMA_VERSION: i64 = 1;
const BACKUP_KEEP: usize = 3;

pub fn migrate(connection: &mut Connection, path: &Path) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_meta (
                component TEXT PRIMARY KEY,
                version INTEGER NOT NULL
            );",
        )
        .map_err(migration_error)?;
    let current: i64 = connection
        .query_row(
            "SELECT version FROM schema_meta WHERE component = 'main'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current > SCHEMA_VERSION {
        return Err(StorageError::new(
            AppErrorKind::StorageFailure,
            "DB_SCHEMA_NEWER_THAN_BINARY",
            format!("database schema {current} is newer than supported {SCHEMA_VERSION}"),
        ));
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }

    // Version-changing migration: checkpoint the WAL, snapshot the current
    // database beside it, retain the three newest backups, then migrate in a
    // transaction. Any failure rolls back and leaves the source untouched.
    let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    create_backup(path)?;

    let transaction = connection.transaction().map_err(migration_error)?;
    transaction
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS account_profiles (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK(kind IN ('currentCli','managed')),
                label TEXT NOT NULL,
                auth_mode TEXT NOT NULL,
                lifecycle TEXT NOT NULL CHECK(lifecycle IN ('pending','ready','removing')),
                email_fingerprint BLOB,
                created_at INTEGER NOT NULL,
                last_selected_at INTEGER,
                last_success_at INTEGER
            );
            CREATE UNIQUE INDEX IF NOT EXISTS one_current_cli
                ON account_profiles(kind) WHERE kind = 'currentCli';
            CREATE TABLE IF NOT EXISTS usage_snapshots (
                profile_id TEXT NOT NULL REFERENCES account_profiles(id) ON DELETE CASCADE,
                provider_id TEXT NOT NULL CHECK(provider_id = 'codex'),
                snapshot_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY(profile_id, provider_id)
            );
            CREATE TABLE IF NOT EXISTS profile_refresh_state (
                profile_id TEXT NOT NULL REFERENCES account_profiles(id) ON DELETE CASCADE,
                provider_id TEXT NOT NULL CHECK(provider_id = 'codex'),
                error_json TEXT,
                attempted_at INTEGER,
                PRIMARY KEY(profile_id, provider_id)
            );
            INSERT OR REPLACE INTO schema_meta(component, version) VALUES ('main', 1);
            "#,
        )
        .map_err(migration_error)?;
    transaction.commit().map_err(migration_error)
}

/// Copy the current database file to a timestamped backup beside it and trim
/// to the three newest backups. The source file is never modified by this
/// helper.
fn create_backup(path: &Path) -> Result<PathBuf, StorageError> {
    let base = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "codex-barbar.db".to_string());
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let backup = path.with_file_name(format!("{base}.bak-{stamp}"));
    fs::copy(path, &backup).map_err(|error| {
        StorageError::new(
            AppErrorKind::StorageFailure,
            "DB_BACKUP_COPY_FAILED",
            error.to_string(),
        )
    })?;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut backups = fs::read_dir(parent)
        .map_err(|error| {
            StorageError::new(
                AppErrorKind::StorageFailure,
                "DB_BACKUP_LIST_FAILED",
                error.to_string(),
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("{base}.bak-")))
        })
        .collect::<Vec<_>>();
    backups.sort();
    for stale in backups
        .iter()
        .take(backups.len().saturating_sub(BACKUP_KEEP))
    {
        let _ = fs::remove_file(stale);
    }
    Ok(backup)
}

fn migration_error(error: rusqlite::Error) -> StorageError {
    StorageError::new(
        AppErrorKind::StorageFailure,
        "DB_MIGRATE_FAILED",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_migration_creates_schema_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let mut connection = Connection::open(&path).unwrap();
        migrate(&mut connection, &path).unwrap();
        let version: i64 = connection
            .query_row(
                "SELECT version FROM schema_meta WHERE component = 'main'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn reopen_is_idempotent_and_does_not_create_backup_for_unchanged_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let mut connection = Connection::open(&path).unwrap();
            migrate(&mut connection, &path).unwrap();
        }
        let backups_after_first_open = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .count();
        assert_eq!(backups_after_first_open, 1);
        {
            let mut connection = Connection::open(&path).unwrap();
            migrate(&mut connection, &path).unwrap();
        }
        let backups = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .count();
        assert_eq!(backups, backups_after_first_open);
    }

    #[test]
    fn backup_retention_keeps_only_three_newest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codex-barbar.db");
        fs::write(&path, b"db").unwrap();
        for _ in 0..5 {
            create_backup(&path).unwrap();
        }
        let backups = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("codex-barbar.db.bak-")
            })
            .count();
        assert_eq!(backups, BACKUP_KEEP);
    }
}
