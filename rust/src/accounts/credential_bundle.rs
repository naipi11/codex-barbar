//! Credential bundle path validation, no-follow restoration, and collection.
//!
//! Managed runtime credentials are restored and collected only through
//! relative, ordinary, non-reparse paths inside one guarded `CODEX_HOME`.

use crate::accounts::vault::{CredentialFile, ManagedCredentialBundle, PrivateProfileMetadata};

use super::runtime_home::RuntimeHomeError;

/// Reject any bundle path that could escape the guarded runtime root.
pub(crate) fn validate_credential_entry(relative_path: &str) -> Result<(), RuntimeHomeError> {
    let has_windows_volume_prefix = relative_path
        .as_bytes()
        .get(1)
        .is_some_and(|character| *character == b':');
    if relative_path.is_empty()
        || relative_path.contains('\0')
        || relative_path.starts_with('\\')
        || has_windows_volume_prefix
        || std::path::Path::new(relative_path).is_absolute()
    {
        return Err(RuntimeHomeError::UnsafeBundlePath);
    }
    let path = std::path::Path::new(relative_path);
    for component in path.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(RuntimeHomeError::UnsafeBundlePath);
        }
    }
    Ok(())
}

/// Restore one credential file into `dest_root` without following links.
pub(crate) fn restore_entry(
    dest_root: &std::path::Path,
    entry: &CredentialFile,
) -> Result<(), RuntimeHomeError> {
    validate_credential_entry(&entry.relative_path)?;
    let target = dest_root.join(&entry.relative_path);
    let parent = target.parent().ok_or(RuntimeHomeError::UnsafeBundlePath)?;
    super::runtime_home::reject_symlinked_ancestors(parent)?;
    if let Ok(metadata) = std::fs::symlink_metadata(parent)
        && super::windows_acl::is_reparse_point(&metadata)
    {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&target)
        && (!metadata.is_file() || super::windows_acl::is_reparse_point(&metadata))
    {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    super::runtime_home::ensure_restricted_directory(parent)?;
    super::runtime_home::write_restricted_file(&target, entry.contents.as_slice())?;
    Ok(())
}

/// Collect credential files from a managed runtime root. `config.toml` and
/// the token-free `manifest.json` are deliberately excluded.
#[allow(dead_code)] // consumed by recovery/refresh in later Phase 2 tasks
pub(crate) fn collect_bundle(
    home: &std::path::Path,
    _profile_id: crate::core::ProfileId,
) -> Result<ManagedCredentialBundle, RuntimeHomeError> {
    let mut files = Vec::new();
    collect_dir(home, home, &mut files)?;
    Ok(ManagedCredentialBundle {
        files,
        private_metadata: PrivateProfileMetadata {
            email: None,
            plan_type: None,
            auth_mode: crate::core::AuthMode::Unknown,
        },
    })
}

#[allow(dead_code)] // consumed by recovery/refresh in later Phase 2 tasks
fn collect_dir(
    root: &std::path::Path,
    dir: &std::path::Path,
    files: &mut Vec<CredentialFile>,
) -> Result<(), RuntimeHomeError> {
    let metadata = std::fs::symlink_metadata(dir).map_err(|_| RuntimeHomeError::Io)?;
    if super::windows_acl::is_reparse_point(&metadata) {
        return Err(RuntimeHomeError::ReparsePointRejected);
    }
    super::runtime_home::verify_restricted_directory(dir)?;
    let entries = std::fs::read_dir(dir).map_err(|_| RuntimeHomeError::Io)?;
    for entry in entries {
        let entry = entry.map_err(|_| RuntimeHomeError::Io)?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|_| RuntimeHomeError::Io)?;
        if super::windows_acl::is_reparse_point(&meta) {
            return Err(RuntimeHomeError::ReparsePointRejected);
        }
        if meta.is_dir() {
            collect_dir(root, &path, files)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        super::runtime_home::verify_restricted_file(&path)?;
        let name = entry.file_name();
        if name == "config.toml" || name == "manifest.json" {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| RuntimeHomeError::UnsafeBundlePath)?
            .to_string_lossy()
            .replace('\\', "/");
        let contents = std::fs::read(&path).map_err(|_| RuntimeHomeError::Io)?;
        files.push(CredentialFile {
            relative_path: relative,
            contents: crate::accounts::secret_bytes::SensitiveBytes::new(contents),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::secret_bytes::SensitiveBytes;

    fn entry(path: &str) -> CredentialFile {
        CredentialFile {
            relative_path: path.to_string(),
            contents: SensitiveBytes::new(b"secret".to_vec()),
        }
    }

    #[test]
    fn bundle_restore_rejects_parent_absolute_and_reparse_paths() {
        for path in [
            "../auth.json",
            r"C:\outside\auth.json",
            r"\\server\share\auth.json",
            "a/../../auth.json",
        ] {
            assert!(validate_credential_entry(path).is_err(), "{path}");
        }
        assert!(validate_credential_entry("auth.json").is_ok());
        assert!(validate_credential_entry("nested/auth.json").is_ok());
    }

    #[test]
    fn restore_creates_relative_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        restore_entry(dir.path(), &entry("auth.json")).unwrap();
        assert!(dir.path().join("auth.json").is_file());
    }
}
