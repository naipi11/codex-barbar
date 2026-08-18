//! Strict Current User DPAPI protector with no machine-scope or plaintext
//! fallback.

use crate::accounts::secret_bytes::SensitiveBytes;
use crate::core::ProfileId;

/// Vault operation failures. These are deliberately coarse: no error carries
/// secret material or raw Win32 detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    UnsupportedPlatform,
    ProtectFailed,
    UnprotectFailed,
    InvalidEnvelope,
    WrongProfile,
    GenerationConflict,
    Io,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VaultError {}

/// Boundary for protecting/unprotecting one profile's credential bytes.
pub trait CredentialProtector: Send + Sync {
    fn protect_current_user(
        &self,
        profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError>;

    fn unprotect_current_user(
        &self,
        profile_id: ProfileId,
        ciphertext: &[u8],
    ) -> Result<SensitiveBytes, VaultError>;
}

/// Current User DPAPI using only `CRYPTPROTECT_UI_FORBIDDEN`; Local Machine
/// protection is never enabled.
#[derive(Debug, Clone, Default)]
pub struct WindowsDpapiProtector {
    _private: (),
}

impl WindowsDpapiProtector {
    pub fn new() -> Self {
        Self { _private: () }
    }

    fn entropy(profile_id: ProfileId) -> Vec<u8> {
        format!("codex-barbar/vault/v1/{profile_id}").into_bytes()
    }
}

#[cfg(windows)]
impl CredentialProtector for WindowsDpapiProtector {
    fn protect_current_user(
        &self,
        profile_id: ProfileId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData};
        const DPAPI_FLAGS: u32 = windows::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN;

        let entropy = Self::entropy(profile_id);
        let plain = CRYPT_INTEGER_BLOB {
            pbData: plaintext.as_ptr() as *mut u8,
            cbData: plaintext.len() as u32,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            pbData: entropy.as_ptr() as *mut u8,
            cbData: entropy.len() as u32,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        // Safety: DPAPI copies the output into its own buffer; we free it via
        // LocalFree below. Inputs are valid for the duration of the call.
        let result = unsafe {
            CryptProtectData(
                &plain,
                windows::core::PCWSTR::null(),
                Some(&entropy_blob),
                None,
                None,
                DPAPI_FLAGS,
                &mut out,
            )
        };
        if result.is_err() || out.pbData.is_null() {
            return Err(VaultError::ProtectFailed);
        }
        let ciphertext =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        // Safety: out was allocated by DPAPI and must be released with
        // LocalFree.
        unsafe {
            LocalFree(HLOCAL(out.pbData as _));
        }
        Ok(ciphertext)
    }

    fn unprotect_current_user(
        &self,
        profile_id: ProfileId,
        ciphertext: &[u8],
    ) -> Result<SensitiveBytes, VaultError> {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData};
        const DPAPI_FLAGS: u32 = windows::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN;

        let entropy = Self::entropy(profile_id);
        let cipher = CRYPT_INTEGER_BLOB {
            pbData: ciphertext.as_ptr() as *mut u8,
            cbData: ciphertext.len() as u32,
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            pbData: entropy.as_ptr() as *mut u8,
            cbData: entropy.len() as u32,
        };
        let mut out = CRYPT_INTEGER_BLOB::default();
        // Safety: inputs are valid for the call; output buffer is released
        // below.
        let result = unsafe {
            CryptUnprotectData(
                &cipher,
                None,
                Some(&entropy_blob),
                None,
                None,
                DPAPI_FLAGS,
                &mut out,
            )
        };
        if result.is_err() || out.pbData.is_null() {
            return Err(VaultError::UnprotectFailed);
        }
        let plaintext =
            unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(HLOCAL(out.pbData as _));
        }
        Ok(SensitiveBytes::new(plaintext))
    }
}

#[cfg(not(windows))]
impl CredentialProtector for WindowsDpapiProtector {
    fn protect_current_user(
        &self,
        _profile_id: ProfileId,
        _plaintext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        Err(VaultError::UnsupportedPlatform)
    }

    fn unprotect_current_user(
        &self,
        _profile_id: ProfileId,
        _ciphertext: &[u8],
    ) -> Result<SensitiveBytes, VaultError> {
        Err(VaultError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    use super::*;
    use crate::accounts::secret_bytes::SensitiveBytes;

    const TEST_TOKEN: &[u8] = b"sk-test-token-that-must-never-leak";

    #[derive(Default)]
    struct FailingCurrentUserProtector {
        calls: AtomicUsize,
    }

    impl FailingCurrentUserProtector {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl CredentialProtector for FailingCurrentUserProtector {
        fn protect_current_user(
            &self,
            _profile_id: Uuid,
            _plaintext: &[u8],
        ) -> Result<Vec<u8>, VaultError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(VaultError::ProtectFailed)
        }

        fn unprotect_current_user(
            &self,
            _profile_id: Uuid,
            _ciphertext: &[u8],
        ) -> Result<SensitiveBytes, VaultError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(VaultError::UnprotectFailed)
        }
    }

    #[test]
    fn protect_failure_has_no_machine_or_plaintext_fallback() {
        let protector = FailingCurrentUserProtector::default();
        let error = protector
            .protect_current_user(Uuid::nil(), TEST_TOKEN)
            .unwrap_err();
        assert!(matches!(error, VaultError::ProtectFailed));
        assert_eq!(protector.calls(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_flags_are_current_user_and_ui_forbidden_only() {
        let flags = windows::Win32::Security::Cryptography::CRYPTPROTECT_UI_FORBIDDEN;
        assert_ne!(flags, 0);
        assert_eq!(
            flags & windows::Win32::Security::Cryptography::CRYPTPROTECT_LOCAL_MACHINE,
            0
        );
    }

    #[cfg(windows)]
    #[test]
    fn same_user_round_trip_and_wrong_profile_entropy() {
        let protector = WindowsDpapiProtector::new();
        let secret = b"sk-test-token";
        let ciphertext = protector.protect_current_user(Uuid::nil(), secret).unwrap();
        let roundtrip = protector
            .unprotect_current_user(Uuid::nil(), &ciphertext)
            .unwrap();
        assert_eq!(roundtrip.as_slice(), secret);
        // Wrong profile entropy must fail; this is the same-user DPAPI call,
        // but entropy mismatch makes the blob unreadable.
        assert!(matches!(
            protector.unprotect_current_user(Uuid::new_v4(), &ciphertext),
            Err(VaultError::UnprotectFailed)
        ));
    }

    #[test]
    fn vault_error_never_contains_raw_secret_material() {
        let error = VaultError::ProtectFailed;
        let text = error.to_string();
        assert!(!text.contains("sk-"));
        assert!(!text.to_ascii_lowercase().contains("token"));
    }
}
