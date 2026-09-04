//! Safe user-data purge for the per-user NSIS uninstaller.
//!
//! `--purge-user-data` is a fixed internal mode: it derives
//! `%LOCALAPPDATA%\codex-barbar` itself, verifies canonical exact equality,
//! requires ordinary non-reparse directories at every component, refuses to
//! run while the desktop app is running, and deletes no other target. It
//! never accepts a path argument.

use std::path::{Path, PathBuf};

/// Canonical data-root directory name under `%LOCALAPPDATA%`.
pub const DATA_DIR_NAME: &str = "codex-barbar";

/// Mutex name created by `tauri-plugin-single-instance` (no `semver`
/// feature), used to refuse purging while the desktop app is running.
pub const SINGLE_INSTANCE_MUTEX_NAME: &str = "com.naipi11.codexbarbar-sim";

/// Result of probing one filesystem path component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Missing,
    OrdinaryDirectory,
    ReparsePoint,
    NotDirectory,
}

/// Injectable path probe so the safety checks are testable cross-platform.
pub trait PathProbe: Send + Sync {
    fn kind(&self, path: &Path) -> PathKind;
}

/// Windows implementation backed by `GetFileAttributesW`.
pub struct WindowsPathProbe;

#[cfg(windows)]
impl PathProbe for WindowsPathProbe {
    fn kind(&self, path: &Path) -> PathKind {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, GetFileAttributesW,
            INVALID_FILE_ATTRIBUTES,
        };
        use windows::core::PCWSTR;

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let attributes = unsafe { GetFileAttributesW(PCWSTR::from_raw(wide.as_ptr())) };
        if attributes == INVALID_FILE_ATTRIBUTES {
            PathKind::Missing
        } else if attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            PathKind::ReparsePoint
        } else if attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
            PathKind::OrdinaryDirectory
        } else {
            PathKind::NotDirectory
        }
    }
}

#[cfg(not(windows))]
impl PathProbe for WindowsPathProbe {
    fn kind(&self, _path: &Path) -> PathKind {
        PathKind::Missing
    }
}

/// Injectable single-instance check so the purge path is testable.
pub trait RunningCheck: Send + Sync {
    fn is_running(&self) -> Result<bool, PurgeError>;
}

/// Production check against the `tauri-plugin-single-instance` mutex.
pub struct WindowsSingleInstanceCheck;

#[cfg(windows)]
impl RunningCheck for WindowsSingleInstanceCheck {
    fn is_running(&self) -> Result<bool, PurgeError> {
        use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND};
        use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
        use windows::core::PCWSTR;

        let wide: Vec<u16> = SINGLE_INSTANCE_MUTEX_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            OpenMutexW(
                SYNCHRONIZATION_SYNCHRONIZE,
                false,
                PCWSTR::from_raw(wide.as_ptr()),
            )
        };
        match result {
            Ok(handle) => {
                let _ = unsafe { CloseHandle(handle) };
                Ok(true)
            }
            Err(error)
                if error.code() == windows::core::HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) =>
            {
                Ok(false)
            }
            Err(_) => Err(PurgeError::RunningCheckFailed),
        }
    }
}

#[cfg(not(windows))]
impl RunningCheck for WindowsSingleInstanceCheck {
    fn is_running(&self) -> Result<bool, PurgeError> {
        Ok(false)
    }
}

/// Stable, user-visible failure modes for the purge-only mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeError {
    LocalAppDataUnavailable,
    TargetNotAbsolute,
    TargetNotCanonicalRoot,
    AncestorNotOrdinaryDirectory,
    TargetNotDirectory,
    ReparsePoint,
    AppRunning,
    RunningCheckFailed,
    Io,
}

impl std::fmt::Display for PurgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalAppDataUnavailable => write!(f, "LocalAppData is unavailable"),
            Self::TargetNotAbsolute => write!(f, "purge target must be absolute"),
            Self::TargetNotCanonicalRoot => {
                write!(
                    f,
                    "purge target is not the canonical codex-barbar data root"
                )
            }
            Self::AncestorNotOrdinaryDirectory => {
                write!(
                    f,
                    "an ancestor of the purge target is not an ordinary directory"
                )
            }
            Self::TargetNotDirectory => write!(f, "purge target is not a directory"),
            Self::ReparsePoint => write!(f, "purge target or ancestor is a reparse point"),
            Self::AppRunning => write!(f, "codex-barbar is running; close it before purging"),
            Self::RunningCheckFailed => {
                write!(f, "could not verify that codex-barbar is not running")
            }
            Self::Io => write!(f, "failed to remove the purge target"),
        }
    }
}

impl std::error::Error for PurgeError {}

/// Purger that only ever operates on the exact `%LOCALAPPDATA%\codex-barbar`
/// root. The expected root is fixed at construction and is never replaced by
/// a caller-supplied path.
pub struct DataPurger {
    expected_root: Option<PathBuf>,
    probe: Box<dyn PathProbe>,
    running_check: Box<dyn RunningCheck>,
}

impl DataPurger {
    /// Build the production purger for the current user's LocalAppData root.
    pub fn new() -> Self {
        Self {
            expected_root: None,
            probe: Box::new(WindowsPathProbe),
            running_check: Box::new(WindowsSingleInstanceCheck),
        }
    }

    /// Build a purger against an injected root, probe, and running check
    /// (tests only).
    pub fn for_root(
        root: PathBuf,
        probe: Box<dyn PathProbe>,
        running_check: Box<dyn RunningCheck>,
    ) -> Self {
        Self {
            expected_root: Some(root),
            probe,
            running_check,
        }
    }

    /// Canonical expected root derived from the current user's LocalAppData.
    pub fn canonical_local_app_data_root() -> Result<PathBuf, PurgeError> {
        let base = dirs::data_local_dir().ok_or(PurgeError::LocalAppDataUnavailable)?;
        Ok(base.join(DATA_DIR_NAME))
    }

    fn resolve_target(&self) -> Result<PathBuf, PurgeError> {
        match &self.expected_root {
            Some(root) => Ok(root.clone()),
            None => Self::canonical_local_app_data_root(),
        }
    }

    /// Verify the target is exactly the canonical root and every existing
    /// component is an ordinary directory (no reparse points).
    pub fn validate_target(&self, target: &Path) -> Result<(), PurgeError> {
        let expected = self.resolve_target()?;
        if !target.is_absolute() {
            return Err(PurgeError::TargetNotAbsolute);
        }
        if target != expected.as_path() {
            return Err(PurgeError::TargetNotCanonicalRoot);
        }

        let mut ancestors = target.ancestors();
        let target_path = ancestors.next().ok_or(PurgeError::TargetNotCanonicalRoot)?;
        match self.probe.kind(target_path) {
            // A missing root is a success: there is nothing to purge.
            PathKind::Missing => {}
            PathKind::OrdinaryDirectory => {}
            PathKind::ReparsePoint => return Err(PurgeError::ReparsePoint),
            PathKind::NotDirectory => return Err(PurgeError::TargetNotDirectory),
        }
        for ancestor in ancestors {
            match self.probe.kind(ancestor) {
                PathKind::OrdinaryDirectory => {}
                PathKind::Missing | PathKind::NotDirectory => {
                    return Err(PurgeError::AncestorNotOrdinaryDirectory);
                }
                PathKind::ReparsePoint => return Err(PurgeError::ReparsePoint),
            }
        }
        Ok(())
    }

    /// Purge the exact canonical data root; a missing root is a success.
    pub fn purge_exact_local_app_data_root(&self) -> Result<(), PurgeError> {
        let target = self.resolve_target()?;
        self.validate_target(&target)?;
        if self.running_check.is_running()? {
            return Err(PurgeError::AppRunning);
        }
        match std::fs::remove_dir_all(&target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(PurgeError::Io),
        }
    }
}

impl Default for DataPurger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{DataPurger, PathKind, PathProbe, PurgeError, RunningCheck};

    #[derive(Debug, Default)]
    struct TestPathProbe {
        missing: Option<PathBuf>,
        reparse: Option<PathBuf>,
        not_directory: Option<PathBuf>,
    }

    impl PathProbe for TestPathProbe {
        fn kind(&self, path: &Path) -> PathKind {
            if self.missing.as_deref() == Some(path) {
                PathKind::Missing
            } else if self.reparse.as_deref() == Some(path) {
                PathKind::ReparsePoint
            } else if self.not_directory.as_deref() == Some(path) {
                PathKind::NotDirectory
            } else {
                PathKind::OrdinaryDirectory
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct TestRunningCheck {
        running: bool,
    }

    impl RunningCheck for TestRunningCheck {
        fn is_running(&self) -> Result<bool, PurgeError> {
            Ok(self.running)
        }
    }

    fn fake_base() -> PathBuf {
        std::env::temp_dir().join("codex-barbar-data-cleanup-tests")
    }

    fn data_root() -> PathBuf {
        fake_base().join(super::DATA_DIR_NAME)
    }

    fn purger() -> DataPurger {
        DataPurger::for_root(
            data_root(),
            Box::new(TestPathProbe::default()),
            Box::new(TestRunningCheck { running: false }),
        )
    }

    fn purger_with_probe(probe: TestPathProbe) -> DataPurger {
        DataPurger::for_root(
            data_root(),
            Box::new(probe),
            Box::new(TestRunningCheck { running: false }),
        )
    }

    #[test]
    fn purge_rejects_parent_relative_and_reparse_targets() {
        assert_eq!(
            purger().validate_target(Path::new(r"C:\Users\A\AppData\Local")),
            Err(PurgeError::TargetNotCanonicalRoot)
        );
        assert_eq!(
            purger().validate_target(Path::new("..")),
            Err(PurgeError::TargetNotAbsolute)
        );

        let reparse_root = data_root();
        let purger = purger_with_probe(TestPathProbe {
            reparse: Some(reparse_root.clone()),
            ..Default::default()
        });
        assert_eq!(
            purger.validate_target(&reparse_root),
            Err(PurgeError::ReparsePoint)
        );
    }

    #[test]
    fn accepts_exact_root_and_missing_target() {
        assert_eq!(purger().validate_target(&data_root()), Ok(()));

        let root = data_root();
        let purger = purger_with_probe(TestPathProbe {
            missing: Some(root.clone()),
            ..Default::default()
        });
        assert_eq!(purger.validate_target(&root), Ok(()));
    }

    #[test]
    fn rejects_missing_ancestor_reparse_ancestor_and_file_target() {
        let missing_ancestor = purger_with_probe(TestPathProbe {
            missing: Some(fake_base()),
            ..Default::default()
        });
        assert_eq!(
            missing_ancestor.validate_target(&data_root()),
            Err(PurgeError::AncestorNotOrdinaryDirectory)
        );

        let reparse_ancestor = purger_with_probe(TestPathProbe {
            reparse: Some(fake_base()),
            ..Default::default()
        });
        assert_eq!(
            reparse_ancestor.validate_target(&data_root()),
            Err(PurgeError::ReparsePoint)
        );

        let file_target = purger_with_probe(TestPathProbe {
            not_directory: Some(data_root()),
            ..Default::default()
        });
        assert_eq!(
            file_target.validate_target(&data_root()),
            Err(PurgeError::TargetNotDirectory)
        );
    }

    #[test]
    fn purge_removes_only_the_exact_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("codex-barbar");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("cache.bin"), b"x").unwrap();
        let sibling = temp.path().join("keep.txt");
        std::fs::write(&sibling, b"y").unwrap();

        let purger = DataPurger::for_root(
            root.clone(),
            Box::new(TestPathProbe::default()),
            Box::new(TestRunningCheck { running: false }),
        );
        assert_eq!(purger.purge_exact_local_app_data_root(), Ok(()));
        assert!(!root.exists());
        assert!(sibling.exists());
    }

    #[test]
    fn purge_refuses_while_app_is_running() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("codex-barbar");
        std::fs::create_dir(&root).unwrap();

        let purger = DataPurger::for_root(
            root.clone(),
            Box::new(TestPathProbe::default()),
            Box::new(TestRunningCheck { running: true }),
        );
        assert_eq!(
            purger.purge_exact_local_app_data_root(),
            Err(PurgeError::AppRunning)
        );
        assert!(root.exists());
    }

    #[test]
    fn purge_missing_root_is_success() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("codex-barbar");
        assert!(!root.exists());

        let purger = DataPurger::for_root(
            root.clone(),
            Box::new(TestPathProbe {
                missing: Some(root.clone()),
                ..Default::default()
            }),
            Box::new(TestRunningCheck { running: false }),
        );
        assert_eq!(purger.purge_exact_local_app_data_root(), Ok(()));
    }
}
