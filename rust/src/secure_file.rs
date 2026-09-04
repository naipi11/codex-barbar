//! Small helper for storing local secret-bearing JSON files.

use std::io;
use std::path::Path;

#[cfg(not(target_os = "linux"))]
use base64::Engine;
use serde::{Deserialize, Serialize};

const FORMAT: &str = "codexbar.secure-file";
const VERSION: u32 = 1;
const WINDOWS_DPAPI_USER: &str = "windows-dpapi-user";
const WINDOWS_DPAPI_MACHINE: &str = "windows-dpapi-machine";
const LINUX_SECRET_SERVICE: &str = "linux-secret-service";

#[derive(Debug, Serialize, Deserialize)]
struct ProtectedFile {
    format: String,
    version: u32,
    protection: String,
    payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecureFileStatus {
    Missing,
    Plaintext,
    Protected(String),
    Unreadable(String),
}

/// Return a non-secret storage status for diagnostics/UI surfaces.
pub fn status(path: &Path) -> SecureFileStatus {
    if !path.exists() {
        return SecureFileStatus::Missing;
    }

    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => return SecureFileStatus::Unreadable(e.to_string()),
    };

    let Ok(file) = serde_json::from_str::<ProtectedFile>(&raw) else {
        return SecureFileStatus::Plaintext;
    };

    if file.format != FORMAT {
        return SecureFileStatus::Plaintext;
    }
    if file.version != VERSION {
        return SecureFileStatus::Unreadable(format!(
            "unsupported secure file version {}",
            file.version
        ));
    }

    match file.protection.as_str() {
        WINDOWS_DPAPI_USER | WINDOWS_DPAPI_MACHINE => SecureFileStatus::Protected(file.protection),
        LINUX_SECRET_SERVICE => {
            #[cfg(target_os = "linux")]
            {
                SecureFileStatus::Protected(file.protection)
            }
            #[cfg(not(target_os = "linux"))]
            {
                SecureFileStatus::Unreadable(
                    "Linux Secret Service file cannot be read on this platform".to_string(),
                )
            }
        }
        other => {
            SecureFileStatus::Unreadable(format!("unsupported secure file protection {other}"))
        }
    }
}

/// Read a UTF-8 file that may be protected by this module.
#[cfg(not(target_os = "linux"))]
pub fn read_string(path: &Path) -> io::Result<String> {
    let raw = std::fs::read_to_string(path)?;
    let Ok(file) = serde_json::from_str::<ProtectedFile>(&raw) else {
        return Ok(raw);
    };

    if file.format != FORMAT {
        return Ok(raw);
    }
    if file.version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported secure file version {}", file.version),
        ));
    }

    match file.protection.as_str() {
        WINDOWS_DPAPI_USER | WINDOWS_DPAPI_MACHINE => {
            let encrypted = base64::engine::general_purpose::STANDARD
                .decode(file.payload.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let plain = unprotect(&encrypted)?;
            String::from_utf8(plain).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported secure file protection {other}"),
        )),
    }
}

/// Linux secret-bearing files contain only a path-bound marker locally. The
/// JSON payload itself lives in the logged-in user's Secret Service keyring.
#[cfg(target_os = "linux")]
pub fn read_string(path: &Path) -> io::Result<String> {
    let raw = std::fs::read_to_string(path)?;
    let file = serde_json::from_str::<ProtectedFile>(&raw).map_err(|_| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "plaintext secret-bearing files are not supported on Linux",
        )
    })?;
    if file.format != FORMAT {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "plaintext secret-bearing files are not supported on Linux",
        ));
    }
    if file.version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported secure file version {}", file.version),
        ));
    }
    if file.protection != LINUX_SECRET_SERVICE {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure file protection is unavailable on this platform",
        ));
    }
    let key = linux_storage_key(path)?;
    if file.payload != key {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "secure file marker does not match its path",
        ));
    }
    let entry = linux_secret_service_entry(&key)?;
    let secret = entry.get_secret().map_err(map_linux_keyring_error)?;
    String::from_utf8(secret).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Secret Service value is not valid UTF-8",
        )
    })
}

/// Write a UTF-8 file, protecting it with Windows DPAPI when available.
#[cfg(not(target_os = "linux"))]
pub fn write_string(path: &Path, contents: &str) -> io::Result<()> {
    let bytes = protected_file_bytes(contents)?;
    std::fs::write(path, bytes)?;
    restrict_file_permissions(path)?;
    Ok(())
}

/// Read a file that is explicitly known not to contain authentication
/// material. Windows keeps the existing DPAPI behavior; Linux reads the
/// owner-only public file without requiring Secret Service.
#[cfg(target_os = "linux")]
pub fn read_non_secret_string(path: &Path) -> io::Result<String> {
    if matches!(status(path), SecureFileStatus::Protected(_)) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "protected secret files cannot be used as public settings",
        ));
    }
    std::fs::read_to_string(path)
}

#[cfg(not(target_os = "linux"))]
pub fn read_non_secret_string(path: &Path) -> io::Result<String> {
    read_string(path)
}

/// Write content whose caller has already proved contains no authentication
/// material. This is the Linux escape hatch that keeps ordinary preferences
/// usable when Secret Service is absent; secret-bearing stores must use
/// [`write_string`] instead.
#[cfg(target_os = "linux")]
pub fn write_non_secret_string(path: &Path, contents: &str) -> io::Result<()> {
    write_linux_owner_only(path, contents.as_bytes())
}

#[cfg(not(target_os = "linux"))]
pub fn write_non_secret_string(path: &Path, contents: &str) -> io::Result<()> {
    write_string(path, contents)
}

#[cfg(target_os = "linux")]
pub fn write_string(path: &Path, contents: &str) -> io::Result<()> {
    let key = linux_storage_key(path)?;
    validate_linux_secret_file_for_write(path, &key)?;
    let entry = linux_secret_service_entry(&key)?;
    let previous = match entry.get_secret() {
        Ok(secret) => Some(crate::accounts::secret_bytes::SensitiveBytes::new(secret)),
        Err(keyring::Error::NoEntry) => None,
        Err(error) => return Err(map_linux_keyring_error(error)),
    };
    entry
        .set_secret(contents.as_bytes())
        .map_err(map_linux_keyring_error)?;

    let marker = linux_marker_bytes(path)?;
    if let Err(write_error) = write_linux_marker(path, &marker) {
        let rollback = match previous {
            Some(secret) => entry.set_secret(secret.as_slice()),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(error),
            },
        };
        if rollback.is_err() {
            return Err(io::Error::other(
                "secure file marker write and Secret Service rollback failed",
            ));
        }
        return Err(write_error);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_linux_secret_file_for_write(path: &Path, expected_key: &str) -> io::Result<()> {
    match status(path) {
        SecureFileStatus::Missing => Ok(()),
        SecureFileStatus::Protected(protection) if protection == LINUX_SECRET_SERVICE => {
            let raw = std::fs::read_to_string(path)?;
            let marker: ProtectedFile = serde_json::from_str(&raw).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "secure file marker is invalid")
            })?;
            if marker.payload == expected_key {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "secure file marker does not match its path",
                ))
            }
        }
        SecureFileStatus::Plaintext => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to rewrite a plaintext secret-bearing file on Linux",
        )),
        SecureFileStatus::Protected(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "refusing to rewrite a foreign protected secret file on Linux",
        )),
        SecureFileStatus::Unreadable(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "refusing to rewrite an unreadable secure file on Linux",
        )),
    }
}

#[cfg(target_os = "linux")]
fn linux_storage_key(path: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let digest = Sha256::digest(absolute.as_os_str().as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(format!("secure-file-v1:{encoded}"))
}

#[cfg(target_os = "linux")]
fn linux_secret_service_entry(key: &str) -> io::Result<keyring::Entry> {
    keyring::Entry::new("com.naipi11.codexbarbar", key).map_err(map_linux_keyring_error)
}

#[cfg(target_os = "linux")]
fn map_linux_keyring_error(error: keyring::Error) -> io::Error {
    let kind = match error {
        keyring::Error::NoStorageAccess(_) => io::ErrorKind::PermissionDenied,
        keyring::Error::NoEntry => io::ErrorKind::NotFound,
        _ => io::ErrorKind::Unsupported,
    };
    io::Error::new(kind, "Linux Secret Service secure storage is unavailable")
}

#[cfg(target_os = "linux")]
fn linux_marker_bytes(path: &Path) -> io::Result<Vec<u8>> {
    serde_json::to_vec_pretty(&ProtectedFile {
        format: FORMAT.to_string(),
        version: VERSION,
        protection: LINUX_SECRET_SERVICE.to_string(),
        payload: linux_storage_key(path)?,
    })
    .map_err(io::Error::other)
}

#[cfg(target_os = "linux")]
fn write_linux_marker(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_linux_owner_only(path, bytes)
}

#[cfg(target_os = "linux")]
fn write_linux_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "secure file has no parent"))?;
    let temporary = parent.join(format!(".secure-file-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
        std::fs::rename(&temporary, path)?;
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o777 != 0o600
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "secure file marker permissions are not owner-only",
            ));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(windows)]
fn protected_file_bytes(contents: &str) -> io::Result<Vec<u8>> {
    let (protection, encrypted) = protect(contents.as_bytes())?;
    let file = ProtectedFile {
        format: FORMAT.to_string(),
        version: VERSION,
        protection: protection.to_string(),
        payload: base64::engine::general_purpose::STANDARD.encode(encrypted),
    };
    serde_json::to_vec_pretty(&file).map_err(io::Error::other)
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn protected_file_bytes(contents: &str) -> io::Result<Vec<u8>> {
    Ok(contents.as_bytes().to_vec())
}

#[cfg(windows)]
fn protect(plain: &[u8]) -> io::Result<(&'static str, Vec<u8>)> {
    use windows::Win32::Security::Cryptography::{
        CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN,
    };

    match protect_with_flags(plain, CRYPTPROTECT_UI_FORBIDDEN) {
        Ok(encrypted) => Ok((WINDOWS_DPAPI_USER, encrypted)),
        Err(user_error) => protect_with_flags(
            plain,
            CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE,
        )
        .map(|encrypted| (WINDOWS_DPAPI_MACHINE, encrypted))
        .map_err(|machine_error| {
            io::Error::other(format!(
                "CryptProtectData failed with user scope ({user_error}) and machine scope ({machine_error})"
            ))
        }),
    }
}

#[cfg(windows)]
fn protect_with_flags(plain: &[u8], flags: u32) -> io::Result<Vec<u8>> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData};

    unsafe {
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        CryptProtectData(&input_blob, None, None, None, None, flags, &mut output_blob)
            .map_err(|e| io::Error::other(format!("CryptProtectData failed: {e:?}")))?;

        if output_blob.pbData.is_null() {
            return Err(io::Error::other("CryptProtectData returned null output"));
        }

        let encrypted =
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output_blob.pbData as *mut _));
        Ok(encrypted)
    }
}

#[cfg(windows)]
fn unprotect(encrypted: &[u8]) -> io::Result<Vec<u8>> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    unsafe {
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        CryptUnprotectData(
            &input_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        )
        .map_err(|e| io::Error::other(format!("CryptUnprotectData failed: {e:?}")))?;

        if output_blob.pbData.is_null() {
            return Err(io::Error::other("CryptUnprotectData returned null output"));
        }

        let plain =
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output_blob.pbData as *mut _));
        Ok(plain)
    }
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn unprotect(_encrypted: &[u8]) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows DPAPI-protected files can only be read on Windows by the same user",
    ))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn restrict_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn reads_plaintext_json_without_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.json");
        std::fs::write(&path, r#"{"hello":"world"}"#).unwrap();

        assert_eq!(read_string(&path).unwrap(), r#"{"hello":"world"}"#);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn write_roundtrips_on_this_platform() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secure.json");
        write_string(&path, r#"{"secret":"value"}"#).unwrap();

        assert_eq!(read_string(&path).unwrap(), r#"{"secret":"value"}"#);
    }

    #[test]
    fn status_reports_missing_plaintext_and_protected_files() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json");
        assert_eq!(status(&missing), SecureFileStatus::Missing);

        let plain = dir.path().join("plain.json");
        std::fs::write(&plain, r#"{"secret":"value"}"#).unwrap();
        assert_eq!(status(&plain), SecureFileStatus::Plaintext);

        let protected = dir.path().join("protected.json");
        std::fs::write(
            &protected,
            serde_json::to_string(&ProtectedFile {
                format: FORMAT.to_string(),
                version: VERSION,
                protection: WINDOWS_DPAPI_USER.to_string(),
                payload: "AA==".to_string(),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            status(&protected),
            SecureFileStatus::Protected(WINDOWS_DPAPI_USER.to_string())
        );
    }

    #[test]
    fn status_reports_unsupported_wrappers_as_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protected.json");
        std::fs::write(
            &path,
            serde_json::to_string(&ProtectedFile {
                format: FORMAT.to_string(),
                version: VERSION + 1,
                protection: WINDOWS_DPAPI_USER.to_string(),
                payload: "AA==".to_string(),
            })
            .unwrap(),
        )
        .unwrap();

        assert!(matches!(status(&path), SecureFileStatus::Unreadable(_)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_windows_does_not_rewrite_windows_dpapi_as_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protected.json");
        let original = serde_json::to_vec(&ProtectedFile {
            format: FORMAT.to_string(),
            version: VERSION,
            protection: WINDOWS_DPAPI_USER.to_string(),
            payload: "AA==".to_string(),
        })
        .unwrap();
        std::fs::write(&path, &original).unwrap();

        let error = write_string(&path, r#"{"secret":"value"}"#).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_plaintext_secret_file_is_never_returned_as_a_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.json");
        std::fs::write(&path, r#"{"secret":"value"}"#).unwrap();

        let error = read_string(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_plaintext_secret_file_is_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.json");
        let original = br#"{"secret":"legacy"}"#;
        std::fs::write(&path, original).unwrap();

        let error = write_string(&path, r#"{"secret":"new"}"#).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_non_secret_file_remains_readable_and_owner_only_without_keyring() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        write_non_secret_string(&path, r#"{"theme":"dark"}"#).unwrap();

        assert_eq!(
            read_non_secret_string(&path).unwrap(),
            r#"{"theme":"dark"}"#
        );
        assert_eq!(
            std::fs::symlink_metadata(path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_local_marker_is_path_bound_and_contains_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secure.json");
        let marker = linux_marker_bytes(&path).unwrap();
        let text = String::from_utf8(marker).unwrap();

        assert!(text.contains(LINUX_SECRET_SERVICE));
        assert!(text.contains("secure-file-v1:"));
        assert!(!text.contains("secret-value"));

        let other = dir.path().join("other.json");
        std::fs::write(&path, linux_marker_bytes(&other).unwrap()).unwrap();
        let error = read_string(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(windows)]
    #[test]
    fn windows_write_uses_protected_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secure.json");
        write_string(&path, r#"{"secret":"value"}"#).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let file: ProtectedFile = serde_json::from_str(&raw).unwrap();

        assert_eq!(file.format, FORMAT);
        assert_eq!(file.version, VERSION);
        assert!(matches!(
            file.protection.as_str(),
            WINDOWS_DPAPI_USER | WINDOWS_DPAPI_MACHINE
        ));
        assert!(
            !raw.contains("secret") && !raw.contains("value"),
            "protected Windows file must not contain plaintext JSON"
        );
    }
}
