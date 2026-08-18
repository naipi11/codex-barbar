//! Current-user Windows startup registration.

use std::path::{Path, PathBuf};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "codex-barbar";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutostartError {
    ExecutablePathUnavailable,
    ExecutablePathNotAbsolute,
    UnexpectedExecutableName,
    Registry(String),
}

impl std::fmt::Display for AutostartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutablePathUnavailable => write!(f, "current executable path unavailable"),
            Self::ExecutablePathNotAbsolute => write!(f, "current executable path is not absolute"),
            Self::UnexpectedExecutableName => {
                write!(f, "current executable must be named codex-barbar.exe")
            }
            Self::Registry(message) => write!(f, "autostart registry operation failed: {message}"),
        }
    }
}

impl std::error::Error for AutostartError {}

pub fn command_for_executable(path: &Path) -> Result<String, AutostartError> {
    if !path.is_absolute() {
        return Err(AutostartError::ExecutablePathNotAbsolute);
    }
    let name = path.file_name().and_then(|value| value.to_str());
    if !name.is_some_and(|value| value.eq_ignore_ascii_case("codex-barbar.exe")) {
        return Err(AutostartError::UnexpectedExecutableName);
    }
    Ok(format!("\"{}\" --background", path.display()))
}

pub fn current_command() -> Result<(PathBuf, String), AutostartError> {
    let path = std::env::current_exe().map_err(|_| AutostartError::ExecutablePathUnavailable)?;
    let command = command_for_executable(&path)?;
    Ok((path, command))
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)
        .map_err(|error| AutostartError::Registry(error.to_string()))?;
    if enabled {
        let (_, command) = current_command()?;
        run_key
            .set_value(VALUE_NAME, &command)
            .map_err(|error| AutostartError::Registry(error.to_string()))?;
    } else {
        let _ = run_key.delete_value(VALUE_NAME);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<(), AutostartError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_the_expected_codex_barbar_executable() {
        let path = Path::new(r"C:\Program Files\codex-barbar\codex-barbar.exe");
        assert_eq!(
            command_for_executable(path).unwrap(),
            r#""C:\Program Files\codex-barbar\codex-barbar.exe" --background"#
        );
    }

    #[test]
    fn relative_paths_and_wrong_names_are_rejected() {
        assert_eq!(
            command_for_executable(Path::new("codex-barbar.exe")),
            Err(AutostartError::ExecutablePathNotAbsolute)
        );
        assert_eq!(
            command_for_executable(Path::new(r"C:\Program Files\codexbar.exe")),
            Err(AutostartError::UnexpectedExecutableName)
        );
    }
}
