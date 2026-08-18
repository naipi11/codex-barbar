//! Protected Windows DACL helpers for guarded runtime directories.

use std::path::Path;

use super::runtime_home::RuntimeHomeError;

/// A directory created with an exact Current User + SYSTEM DACL and no
/// inherited ACEs.
pub struct GuardedRuntimeDir {
    pub(crate) path: std::path::PathBuf,
}

impl GuardedRuntimeDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Create `path` with a protected DACL granting full control only to the
/// current user and SYSTEM, then verify the security descriptor.
#[cfg(windows)]
pub fn protect_directory(path: &Path) -> Result<GuardedRuntimeDir, RuntimeHomeError> {
    let dir = super::runtime_home::RuntimeHomeManager::create_guarded_dir(path)?;
    super::runtime_home::RuntimeHomeManager::verify_protected_directory(path)?;
    Ok(dir)
}

#[cfg(not(windows))]
pub fn protect_directory(path: &Path) -> Result<GuardedRuntimeDir, RuntimeHomeError> {
    std::fs::create_dir_all(path).map_err(|_| RuntimeHomeError::Io)?;
    Ok(GuardedRuntimeDir {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::RuntimeHomeManager;

    #[test]
    fn reparse_point_detection_works_on_normal_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        let metadata = std::fs::symlink_metadata(&file).unwrap();
        assert!(!is_reparse_point(&metadata));
    }

    #[cfg(windows)]
    #[test]
    fn protected_directory_has_exact_dacl() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("guarded");
        let guarded = protect_directory(&target).unwrap();
        assert!(guarded.path().is_dir());
        RuntimeHomeManager::verify_protected_directory(&target).unwrap();
    }
}
