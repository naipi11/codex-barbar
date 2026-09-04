//! Versioned vault envelope and canonical credential-bundle encoding.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::accounts::secret_bytes::SensitiveBytes;
use crate::core::{AuthMode, ProfileId};

use super::crypto::VaultError;

pub const VAULT_FORMAT: &str = "codex-barbar-vault";
pub const VAULT_VERSION: u32 = 1;
#[cfg(target_os = "linux")]
pub const VAULT_PROTECTION: &str = "linux-secret-service-v1";
#[cfg(not(target_os = "linux"))]
pub const VAULT_PROTECTION: &str = "windows-dpapi-current-user";

/// One credential file stored inside a managed profile's bundle.
pub struct CredentialFile {
    pub relative_path: String,
    pub contents: SensitiveBytes,
}

/// Private profile metadata kept out of SQLite.
pub struct PrivateProfileMetadata {
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub auth_mode: AuthMode,
}

/// Credentials collected from a managed runtime, ready to seal.
pub struct ManagedCredentialBundle {
    pub files: Vec<CredentialFile>,
    pub private_metadata: PrivateProfileMetadata,
}

/// Outer JSON envelope. Only the ciphertext payload is protected; metadata is
/// intentionally non-secret but never includes paths or raw credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultEnvelope {
    pub format: String,
    pub version: u32,
    pub protection: String,
    pub profile_id: ProfileId,
    pub generation: u64,
    pub sealed_at: DateTime<Utc>,
    pub ciphertext_base64: String,
}

impl VaultEnvelope {
    pub fn new(
        profile_id: ProfileId,
        generation: u64,
        sealed_at: DateTime<Utc>,
        ciphertext: &[u8],
    ) -> Self {
        Self {
            format: VAULT_FORMAT.to_string(),
            version: VAULT_VERSION,
            protection: VAULT_PROTECTION.to_string(),
            profile_id,
            generation,
            sealed_at,
            ciphertext_base64: BASE64.encode(ciphertext),
        }
    }

    pub fn validate(&self) -> Result<(), VaultError> {
        if self.format != VAULT_FORMAT
            || self.version != VAULT_VERSION
            || self.protection != VAULT_PROTECTION
        {
            return Err(VaultError::InvalidEnvelope);
        }
        Ok(())
    }
}

/// Canonical plaintext layout for one bundle:
/// `codex-barbar-bundle\0` + profile UUID + auth-mode byte + metadata JSON +
/// file record count + per-file relative path + length + bytes.
#[allow(dead_code)] // consumed by accounts::vault::store (Task 3)
pub(crate) fn encode_bundle(
    bundle: &ManagedCredentialBundle,
) -> Result<SensitiveBytes, VaultError> {
    let metadata = serde_json::json!({
        "email": bundle.private_metadata.email,
        "planType": bundle.private_metadata.plan_type,
        "authMode": match bundle.private_metadata.auth_mode {
            AuthMode::Unknown => "unknown",
            AuthMode::ChatGpt => "chatgpt",
            AuthMode::ApiKey => "apiKey",
        },
    });
    let metadata_bytes = serde_json::to_vec(&metadata).map_err(|_| VaultError::InvalidEnvelope)?;
    let mut out = Vec::new();
    out.extend_from_slice(b"codex-barbar-bundle\0");
    out.extend_from_slice(metadata_bytes.as_slice());
    out.push(0);
    out.extend_from_slice(&(bundle.files.len() as u32).to_le_bytes());
    for file in &bundle.files {
        let path = file.relative_path.as_bytes();
        if path.is_empty() || path.contains(&0) || file.contents.len() > u32::MAX as usize {
            return Err(VaultError::InvalidEnvelope);
        }
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path);
        out.extend_from_slice(&(file.contents.len() as u32).to_le_bytes());
        out.extend_from_slice(file.contents.as_slice());
    }
    Ok(SensitiveBytes::new(out))
}

#[allow(dead_code)] // consumed by accounts::vault::store (Task 3)
pub(crate) fn decode_bundle(plaintext: &[u8]) -> Result<ManagedCredentialBundle, VaultError> {
    const MAGIC: &[u8] = b"codex-barbar-bundle\0";
    if !plaintext.starts_with(MAGIC) {
        return Err(VaultError::InvalidEnvelope);
    }
    let mut cursor = MAGIC.len();
    let metadata_end = plaintext[cursor..]
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(VaultError::InvalidEnvelope)?
        + cursor;
    let metadata: serde_json::Value = serde_json::from_slice(&plaintext[cursor..metadata_end])
        .map_err(|_| VaultError::InvalidEnvelope)?;
    let auth_mode = match metadata.get("authMode").and_then(|value| value.as_str()) {
        Some("chatgpt") => AuthMode::ChatGpt,
        Some("apiKey") => AuthMode::ApiKey,
        _ => AuthMode::Unknown,
    };
    let private_metadata = PrivateProfileMetadata {
        email: metadata
            .get("email")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        plan_type: metadata
            .get("planType")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned),
        auth_mode,
    };
    cursor = metadata_end + 1;
    let count = read_u32(plaintext, &mut cursor)? as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let path_len = read_u32(plaintext, &mut cursor)? as usize;
        let path_end = cursor
            .checked_add(path_len)
            .ok_or(VaultError::InvalidEnvelope)?;
        let path = plaintext
            .get(cursor..path_end)
            .ok_or(VaultError::InvalidEnvelope)?;
        let relative_path = std::str::from_utf8(path).map_err(|_| VaultError::InvalidEnvelope)?;
        if relative_path.is_empty() || relative_path.contains('\0') {
            return Err(VaultError::InvalidEnvelope);
        }
        cursor = path_end;
        let content_len = read_u32(plaintext, &mut cursor)? as usize;
        let content_end = cursor
            .checked_add(content_len)
            .ok_or(VaultError::InvalidEnvelope)?;
        let content = plaintext
            .get(cursor..content_end)
            .ok_or(VaultError::InvalidEnvelope)?;
        cursor = content_end;
        files.push(CredentialFile {
            relative_path: relative_path.to_string(),
            contents: SensitiveBytes::new(content.to_vec()),
        });
    }
    if cursor != plaintext.len() {
        return Err(VaultError::InvalidEnvelope);
    }
    Ok(ManagedCredentialBundle {
        files,
        private_metadata,
    })
}

#[allow(dead_code)] // consumed by accounts::vault::store (Task 3)
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, VaultError> {
    let end = cursor.checked_add(4).ok_or(VaultError::InvalidEnvelope)?;
    let raw = bytes.get(*cursor..end).ok_or(VaultError::InvalidEnvelope)?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture_bundle() -> ManagedCredentialBundle {
        ManagedCredentialBundle {
            files: vec![
                CredentialFile {
                    relative_path: "auth.json".to_string(),
                    contents: SensitiveBytes::new(b"{\"tokens\":{}}".to_vec()),
                },
                CredentialFile {
                    relative_path: "config.toml".to_string(),
                    contents: SensitiveBytes::new(
                        b"cli_auth_credentials_store = \"file\"".to_vec(),
                    ),
                },
            ],
            private_metadata: PrivateProfileMetadata {
                email: Some("user@example.com".to_string()),
                plan_type: Some("plus".to_string()),
                auth_mode: AuthMode::ChatGpt,
            },
        }
    }

    #[test]
    fn bundle_round_trips_without_losing_files_or_metadata() {
        let encoded = encode_bundle(&fixture_bundle()).unwrap();
        let decoded = decode_bundle(encoded.as_slice()).unwrap();
        assert_eq!(decoded.files.len(), 2);
        assert_eq!(decoded.files[0].relative_path, "auth.json");
        assert_eq!(decoded.files[0].contents.as_slice(), b"{\"tokens\":{}}");
        assert_eq!(
            decoded.files[1].contents.as_slice(),
            b"cli_auth_credentials_store = \"file\""
        );
        assert_eq!(
            decoded.private_metadata.email.as_deref(),
            Some("user@example.com")
        );
        assert_eq!(decoded.private_metadata.plan_type.as_deref(), Some("plus"));
        assert_eq!(decoded.private_metadata.auth_mode, AuthMode::ChatGpt);
    }

    #[test]
    fn envelope_validates_exact_format_and_rejects_unknown_protection() {
        let envelope = VaultEnvelope::new(Uuid::nil(), 1, Utc::now(), b"ciphertext");
        assert!(envelope.validate().is_ok());
        let mut wrong = envelope.clone();
        wrong.protection = "plaintext".to_string();
        assert_eq!(wrong.validate(), Err(VaultError::InvalidEnvelope));
        let mut wrong_version = envelope;
        wrong_version.version = 99;
        assert_eq!(wrong_version.validate(), Err(VaultError::InvalidEnvelope));
    }

    #[test]
    fn truncated_bundle_is_rejected() {
        let encoded = encode_bundle(&fixture_bundle()).unwrap();
        assert!(decode_bundle(&encoded.as_slice()[..encoded.len() - 3]).is_err());
    }
}
