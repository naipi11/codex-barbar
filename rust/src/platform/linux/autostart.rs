//! XDG current-user autostart registration.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DESKTOP_FILE_NAME: &str = "com.naipi11.codexbarbar.desktop";
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartError {
    ConfigDirectoryUnavailable,
    ExecutablePathUnavailable,
    ExecutablePathNotAbsolute,
    UnexpectedExecutableName,
    UnsafeExecutablePath,
    FileSystem(String),
}

impl std::fmt::Display for AutostartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigDirectoryUnavailable => write!(formatter, "config directory unavailable"),
            Self::ExecutablePathUnavailable => write!(formatter, "current executable unavailable"),
            Self::ExecutablePathNotAbsolute => write!(formatter, "executable path is not absolute"),
            Self::UnexpectedExecutableName => {
                write!(formatter, "current executable must be named codex-barbar")
            }
            Self::UnsafeExecutablePath => write!(formatter, "executable path is not safe for Exec"),
            Self::FileSystem(message) => {
                write!(
                    formatter,
                    "autostart filesystem operation failed: {message}"
                )
            }
        }
    }
}

impl std::error::Error for AutostartError {}

pub fn path(config_root: &Path) -> PathBuf {
    config_root.join("autostart").join(DESKTOP_FILE_NAME)
}

pub fn desktop_entry(executable: &Path) -> Result<String, AutostartError> {
    let executable = executable
        .to_str()
        .ok_or(AutostartError::UnsafeExecutablePath)?;
    if !executable.starts_with('/') {
        return Err(AutostartError::ExecutablePathNotAbsolute);
    }
    if executable.rsplit('/').next() != Some("codex-barbar") {
        return Err(AutostartError::UnexpectedExecutableName);
    }
    let executable = desktop_exec_token(executable)?;
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=codex-barbar\nExec={executable} --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    ))
}

pub fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
    let config_root = dirs::config_dir().ok_or(AutostartError::ConfigDirectoryUnavailable)?;
    set_enabled_at(&config_root, enabled)
}

pub fn set_enabled_at(config_root: &Path, enabled: bool) -> Result<(), AutostartError> {
    if !enabled {
        return remove_fixed_entry(config_root);
    }
    let executable =
        std::env::current_exe().map_err(|_| AutostartError::ExecutablePathUnavailable)?;
    set_enabled_at_with_executable(config_root, true, &executable)
}

fn set_enabled_at_with_executable(
    config_root: &Path,
    enabled: bool,
    executable: &Path,
) -> Result<(), AutostartError> {
    if !enabled {
        return remove_fixed_entry(config_root);
    }
    let target = path(config_root);
    let parent = target.parent().ok_or_else(filesystem_error)?;
    std::fs::create_dir_all(parent).map_err(filesystem_error_from)?;
    let contents = desktop_entry(executable)?;
    atomic_write(&target, contents.as_bytes())
}

fn remove_fixed_entry(config_root: &Path) -> Result<(), AutostartError> {
    match std::fs::remove_file(path(config_root)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem_error_from(error)),
    }
}

fn atomic_write(target: &Path, contents: &[u8]) -> Result<(), AutostartError> {
    let parent = target.parent().ok_or_else(filesystem_error)?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".{DESKTOP_FILE_NAME}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&temporary, target)
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(filesystem_error_from(error));
    }
    Ok(())
}

fn desktop_exec_token(path: &str) -> Result<String, AutostartError> {
    if path
        .chars()
        .any(|character| character.is_control() || character == '=')
    {
        return Err(AutostartError::UnsafeExecutablePath);
    }
    let exec_escaped = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    // Desktop Entry string-value escaping runs before Exec argument
    // unquoting, so every Exec-layer backslash must itself be doubled.
    let escaped = exec_escaped.replace('\\', "\\\\").replace('%', "%%");
    let needs_quotes = path.chars().any(|character| {
        matches!(
            character,
            ' ' | '\t'
                | '"'
                | '\''
                | '\\'
                | '>'
                | '<'
                | '~'
                | '|'
                | '&'
                | ';'
                | '$'
                | '*'
                | '?'
                | '#'
                | '('
                | ')'
                | '`'
        )
    });
    if needs_quotes {
        Ok(format!("\"{escaped}\""))
    } else {
        Ok(escaped)
    }
}

fn filesystem_error() -> AutostartError {
    AutostartError::FileSystem("invalid destination".to_string())
}

fn filesystem_error_from(error: std::io::Error) -> AutostartError {
    AutostartError::FileSystem(error.kind().to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn linux_autostart_path_uses_the_fixed_desktop_entry_id() {
        assert_eq!(
            path(Path::new("/home/test/.config"))
                .file_name()
                .and_then(|name| name.to_str()),
            Some("com.naipi11.codexbarbar.desktop")
        );
    }

    #[test]
    fn linux_autostart_entry_uses_only_an_absolute_executable() {
        let text = desktop_entry(Path::new("/opt/codex-barbar/codex-barbar")).unwrap();
        assert!(text.contains("Type=Application"));
        assert!(text.contains("Exec=/opt/codex-barbar/codex-barbar --background"));
        assert!(!text.contains("sh -c"));
    }

    #[test]
    fn linux_autostart_rejects_relative_and_unexpected_executables() {
        assert_eq!(
            desktop_entry(Path::new("codex-barbar")),
            Err(AutostartError::ExecutablePathNotAbsolute)
        );
        assert_eq!(
            desktop_entry(Path::new("/opt/codexbar")),
            Err(AutostartError::UnexpectedExecutableName)
        );
    }

    #[test]
    fn desktop_exec_serialization_preserves_reserved_literal_path_characters() {
        let cases = [
            (
                "/opt/with space/codex-barbar",
                r#"Exec="/opt/with space/codex-barbar" --background"#,
            ),
            (
                "/opt/with\\slash/codex-barbar",
                r#"Exec="/opt/with\\\\slash/codex-barbar" --background"#,
            ),
            (
                "/opt/with\"quote/codex-barbar",
                r#"Exec="/opt/with\\"quote/codex-barbar" --background"#,
            ),
            (
                "/opt/with$cash/codex-barbar",
                r#"Exec="/opt/with\\$cash/codex-barbar" --background"#,
            ),
        ];

        for (executable, expected_exec) in cases {
            let entry = desktop_entry(Path::new(executable)).unwrap();
            assert!(
                entry.lines().any(|line| line == expected_exec),
                "expected {expected_exec:?} for {executable:?}, got {entry:?}"
            );
            assert!(!entry.contains("sh -c"));
        }
        assert_eq!(
            desktop_entry(Path::new("/opt/with=equals/codex-barbar")),
            Err(AutostartError::UnsafeExecutablePath)
        );
    }

    #[test]
    fn enabling_autostart_atomically_writes_the_fixed_desktop_file() {
        let root = tempfile::tempdir().unwrap();
        set_enabled_at_with_executable(
            root.path(),
            true,
            Path::new("/opt/codex-barbar/codex-barbar"),
        )
        .unwrap();

        let text = std::fs::read_to_string(path(root.path())).unwrap();
        assert!(text.contains("Exec=/opt/codex-barbar/codex-barbar --background"));
        assert_eq!(
            std::fs::read_dir(root.path().join("autostart"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn disabling_autostart_removes_only_the_fixed_desktop_file() {
        let root = tempfile::tempdir().unwrap();
        let autostart_dir = root.path().join("autostart");
        std::fs::create_dir_all(&autostart_dir).unwrap();
        std::fs::write(path(root.path()), "owned").unwrap();
        let sibling = autostart_dir.join("keep.desktop");
        std::fs::write(&sibling, "keep").unwrap();

        set_enabled_at(root.path(), false).unwrap();

        assert!(!path(root.path()).exists());
        assert_eq!(std::fs::read_to_string(sibling).unwrap(), "keep");
    }
}
