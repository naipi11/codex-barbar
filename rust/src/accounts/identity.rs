//! Local, DPAPI-protected cache for non-secret Codex account identity.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::accounts::presentation::{PresentationIdentity, presentation_identity};
use crate::core::ProfileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    SignedIn,
    SignedOut,
    Unavailable,
}

fn default_account_status() -> AccountStatus {
    AccountStatus::Unavailable
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountIdentityRecord {
    #[serde(default)]
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    #[serde(default = "default_account_status")]
    pub status: AccountStatus,
    #[serde(default)]
    pub presentation: PresentationIdentity,
    pub updated_at: DateTime<Utc>,
}

impl AccountIdentityRecord {
    fn normalize_presentation_name(&mut self) {
        self.presentation.display_name = presentation_identity(
            self.username.as_deref(),
            self.display_name.as_deref(),
            self.email.as_deref(),
            self.status,
        )
        .display_name;
        if self.presentation.avatar_kind == crate::accounts::avatar::AvatarKind::Default {
            self.presentation.avatar_revision = None;
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityCacheError {
    #[error("identity cache I/O failed")]
    Io(#[source] std::io::Error),
    #[error("identity cache encoding failed")]
    Encode(#[source] serde_json::Error),
    #[error("identity cache decoding failed")]
    Decode(#[source] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct AccountIdentityCache {
    path: PathBuf,
}

impl AccountIdentityCache {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(
        &self,
        profile_id: ProfileId,
    ) -> Result<Option<AccountIdentityRecord>, IdentityCacheError> {
        let records = self.load_records()?;
        Ok(records.get(&profile_id.to_string()).cloned())
    }

    pub fn save(
        &self,
        profile_id: ProfileId,
        record: &AccountIdentityRecord,
    ) -> Result<(), IdentityCacheError> {
        let mut records = self.load_records()?;
        records.insert(profile_id.to_string(), record.clone());
        self.save_records(&records)
    }

    pub fn remove(&self, profile_id: ProfileId) -> Result<(), IdentityCacheError> {
        let mut records = self.load_records()?;
        if records.remove(&profile_id.to_string()).is_some() {
            self.save_records(&records)?;
        }
        Ok(())
    }

    fn load_records(&self) -> Result<BTreeMap<String, AccountIdentityRecord>, IdentityCacheError> {
        if !self.path.exists() {
            return Ok(BTreeMap::new());
        }
        let raw = crate::secure_file::read_non_secret_string(&self.path)
            .map_err(IdentityCacheError::Io)?;
        let mut records: BTreeMap<String, AccountIdentityRecord> =
            serde_json::from_str(&raw).map_err(IdentityCacheError::Decode)?;
        for record in records.values_mut() {
            record.normalize_presentation_name();
        }
        Ok(records)
    }

    fn save_records(
        &self,
        records: &BTreeMap<String, AccountIdentityRecord>,
    ) -> Result<(), IdentityCacheError> {
        let json = serde_json::to_string_pretty(records).map_err(IdentityCacheError::Encode)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(IdentityCacheError::Io)?;
        }
        let temporary = self
            .path
            .with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
        crate::secure_file::write_non_secret_string(&temporary, &json)
            .map_err(IdentityCacheError::Io)?;
        let result = replace_file(&temporary, &self.path);
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(IdentityCacheError::Io)
    }
}

fn replace_file(temporary: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        if target.exists() {
            let backup = target.with_extension(format!("bak-{}", Uuid::new_v4().simple()));
            fs::rename(target, &backup)?;
            match fs::rename(temporary, target) {
                Ok(()) => {
                    let _ = fs::remove_file(backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, target);
                    Err(error)
                }
            }
        } else {
            fs::rename(temporary, target)
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(temporary, target)
    }
}

#[cfg(test)]
fn atomic_write_with<F>(path: &Path, contents: &[u8], replace: F) -> std::io::Result<()>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    let temporary = path.with_extension("test-tmp");
    fs::write(&temporary, contents)?;
    let result = replace(&temporary, path);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn old_cache_record_defaults_to_unavailable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let profile_id = Uuid::from_u128(99);
        fs::write(
            &path,
            format!(
                r#"{{"{profile_id}":{{"display_name":"Ming Zhao","email":"user@example.com","plan_type":"plus","updated_at":"2026-08-08T00:00:00Z"}}}}"#
            ),
        )
        .unwrap();

        let record = AccountIdentityCache::new(path)
            .load(profile_id)
            .unwrap()
            .unwrap();

        assert_eq!(record.status, AccountStatus::Unavailable);
        assert_eq!(record.presentation.display_name, "Ming Zhao");
        assert_eq!(
            record.presentation.avatar_kind,
            crate::accounts::avatar::AvatarKind::Default
        );
    }

    fn record(name: &str) -> AccountIdentityRecord {
        AccountIdentityRecord {
            username: Some("handle".to_string()),
            display_name: Some(name.to_string()),
            email: Some("user@example.com".to_string()),
            plan_type: Some("plus".to_string()),
            status: AccountStatus::SignedIn,
            presentation: crate::accounts::presentation::presentation_identity(
                Some("handle"),
                Some(name),
                Some("user@example.com"),
                AccountStatus::SignedIn,
            ),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn cache_round_trips_a_profile_identity() {
        let dir = tempdir().unwrap();
        let cache = AccountIdentityCache::new(dir.path().join("identity").join("profiles.json"));
        let profile_id = Uuid::from_u128(1);
        let expected = record("Ming Zhao");

        cache.save(profile_id, &expected).unwrap();

        assert_eq!(cache.load(profile_id).unwrap(), Some(expected));
    }

    #[test]
    fn cache_round_trip_keeps_only_resolved_avatar_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let cache = AccountIdentityCache::new(path.clone());
        let profile_id = Uuid::from_u128(3);
        let mut expected = record("Safe Name");
        expected.presentation.avatar_kind = crate::accounts::avatar::AvatarKind::Manual;
        expected.presentation.avatar_revision = Some("opaque-revision".to_string());

        cache.save(profile_id, &expected).unwrap();

        let raw = crate::secure_file::read_non_secret_string(&path).unwrap();
        assert!(raw.contains("opaque-revision"));
        for forbidden in ["avatarUrl", "avatar_url", "C:\\", "file://"] {
            assert!(
                !raw.contains(forbidden),
                "identity cache leaked {forbidden}"
            );
        }
        assert_eq!(cache.load(profile_id).unwrap(), Some(expected));
    }

    #[test]
    fn removing_a_profile_removes_only_that_identity() {
        let dir = tempdir().unwrap();
        let cache = AccountIdentityCache::new(dir.path().join("profiles.json"));
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        cache.save(first, &record("First")).unwrap();
        cache.save(second, &record("Second")).unwrap();

        cache.remove(first).unwrap();

        assert_eq!(cache.load(first).unwrap(), None);
        assert_eq!(
            cache.load(second).unwrap().unwrap().display_name.as_deref(),
            Some("Second")
        );
    }

    #[test]
    fn failed_atomic_replace_keeps_previous_cache_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        fs::write(&path, br#"{"old":true}"#).unwrap();

        let error = atomic_write_with(&path, br#"{"new":true}"#, |_temporary, _target| {
            Err(std::io::Error::other("simulated replace failure"))
        })
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(fs::read(&path).unwrap(), br#"{"old":true}"#);
    }
}
