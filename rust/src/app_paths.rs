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
    pub notification_state: PathBuf,
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
            notification_state: root.join("runtime").join("notification-state.json"),
            root,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::AppPaths;

    #[test]
    fn derives_every_v1_path_from_local_app_data() {
        let paths = AppPaths::from_local_app_data(Path::new(r"C:\Users\A\AppData\Local"));
        assert_eq!(
            paths.root,
            PathBuf::from(r"C:\Users\A\AppData\Local\codex-barbar")
        );
        assert_eq!(paths.database, paths.root.join(r"data\codex-barbar.db"));
        assert_eq!(paths.vault, paths.root.join("vault"));
        assert_eq!(paths.runtime, paths.root.join("runtime"));
        assert_eq!(paths.logs, paths.root.join("logs"));
        assert_eq!(
            paths.notification_state,
            paths.root.join("runtime").join("notification-state.json")
        );
    }

    #[test]
    fn derives_identity_cache_path_from_canonical_root() {
        let paths = AppPaths::from_local_app_data(Path::new(r"C:\Users\A\AppData\Local"));
        assert_eq!(
            paths.identity_cache,
            paths.root.join("identity").join("profiles.json")
        );
    }
}
