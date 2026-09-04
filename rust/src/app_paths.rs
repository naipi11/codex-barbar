use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum AppPathError {
    #[error("LocalAppData is unavailable")]
    LocalAppDataUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub vault: PathBuf,
    pub runtime: PathBuf,
    pub logs: PathBuf,
    pub identity_cache: PathBuf,
    pub avatars: PathBuf,
    pub notification_state: PathBuf,
    pub cache: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathError> {
        let base = dirs::data_local_dir().ok_or(AppPathError::LocalAppDataUnavailable)?;
        Ok(Self::from_local_app_data(&base))
    }

    pub fn from_local_app_data(base: &Path) -> Self {
        let root = base.join("codex-barbar");
        Self {
            database: root.join("data").join("codex-barbar.db"),
            vault: root.join("vault"),
            runtime: root.join("runtime"),
            logs: root.join("logs"),
            identity_cache: root.join("identity").join("profiles.json"),
            avatars: root.join("avatars"),
            notification_state: root.join("runtime").join("notification-state.json"),
            cache: root.join("cache"),
            root,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::AppPaths;

    #[test]
    fn derives_every_v1_path_from_local_app_data() {
        let base = PathBuf::from("fixtures").join("local-app-data");
        let paths = AppPaths::from_local_app_data(&base);
        assert_eq!(paths.root, base.join("codex-barbar"));
        assert_eq!(
            paths.database,
            paths.root.join("data").join("codex-barbar.db")
        );
        assert_eq!(paths.vault, paths.root.join("vault"));
        assert_eq!(paths.runtime, paths.root.join("runtime"));
        assert_eq!(paths.logs, paths.root.join("logs"));
        assert_eq!(
            paths.notification_state,
            paths.root.join("runtime").join("notification-state.json")
        );
        assert_eq!(paths.cache, paths.root.join("cache"));
    }

    #[test]
    fn derives_identity_cache_path_from_canonical_root() {
        let base = PathBuf::from("fixtures").join("local-app-data");
        let paths = AppPaths::from_local_app_data(&base);
        assert_eq!(
            paths.identity_cache,
            paths.root.join("identity").join("profiles.json")
        );
    }

    #[test]
    fn derives_avatar_store_path_from_canonical_root() {
        let base = PathBuf::from("fixtures").join("local-app-data");
        let paths = AppPaths::from_local_app_data(&base);
        assert_eq!(paths.avatars, paths.root.join("avatars"));
    }
}
