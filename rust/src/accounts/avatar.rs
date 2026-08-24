//! Profile-scoped avatar validation, download, and local storage.

use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::ProfileId;

const PNG_DATA_URL_PREFIX: &str = "data:image/png;base64,";
const MAX_AVATAR_BYTES: usize = 1024 * 1024;
const MAX_AVATAR_DIMENSION: u32 = 2048;
const MAX_AVATAR_URL_BYTES: usize = 2048;
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(5);
const OFFICIAL_HOST_SUFFIXES: [&str; 3] = ["openai.com", "chatgpt.com", "oaistatic.com"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AvatarKind {
    #[default]
    Default,
    Official,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarAsset {
    pub kind: AvatarKind,
    pub revision: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AvatarError {
    #[error("PROFILE_AVATAR_INVALID")]
    Invalid,
    #[error("PROFILE_AVATAR_TOO_LARGE")]
    TooLarge,
    #[error("PROFILE_AVATAR_UNAVAILABLE")]
    Unavailable,
    #[error("PROFILE_AVATAR_STORAGE_FAILED")]
    Storage,
    #[error("PROFILE_AVATAR_PROFILE_NOT_FOUND")]
    ProfileNotFound,
}

#[derive(Clone)]
pub struct AvatarStore {
    root: PathBuf,
}

impl std::fmt::Debug for AvatarStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarStore")
            .field("root", &"<redacted>")
            .finish()
    }
}

impl AvatarStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[cfg(test)]
    pub fn for_test(root: &Path) -> Self {
        Self::new(root.to_path_buf())
    }

    pub fn write_manual(
        &self,
        profile_id: ProfileId,
        bytes: &[u8],
    ) -> Result<AvatarAsset, AvatarError> {
        self.write_asset(profile_id, AvatarKind::Manual, bytes)
    }

    pub fn write_official(
        &self,
        profile_id: ProfileId,
        bytes: &[u8],
    ) -> Result<AvatarAsset, AvatarError> {
        self.write_asset(profile_id, AvatarKind::Official, bytes)
    }

    pub fn clear_manual(&self, profile_id: ProfileId) -> Result<(), AvatarError> {
        self.remove_asset(profile_id, AvatarKind::Manual)
    }

    pub fn clear_official(&self, profile_id: ProfileId) -> Result<(), AvatarError> {
        self.remove_asset(profile_id, AvatarKind::Official)
    }

    pub fn remove_profile(&self, profile_id: ProfileId) -> Result<(), AvatarError> {
        self.clear_manual(profile_id)?;
        self.clear_official(profile_id)
    }

    pub fn asset_for(&self, profile_id: ProfileId) -> Result<Option<AvatarAsset>, AvatarError> {
        self.resolve_asset(profile_id)
            .map(|value| value.map(|(asset, _path)| asset))
    }

    pub fn read_asset(
        &self,
        profile_id: ProfileId,
        revision: &str,
    ) -> Result<Option<Vec<u8>>, AvatarError> {
        let Some((asset, path)) = self.resolve_asset(profile_id)? else {
            return Ok(None);
        };
        if asset.revision != revision {
            return Ok(None);
        }
        read_validated_png(&path).map(Some)
    }

    fn write_asset(
        &self,
        profile_id: ProfileId,
        kind: AvatarKind,
        bytes: &[u8],
    ) -> Result<AvatarAsset, AvatarError> {
        validate_png(bytes)?;
        let directory = self.directory(kind)?;
        ensure_ordinary_directory(&self.root)?;
        ensure_ordinary_directory(&directory)?;
        let target = directory.join(format!("{profile_id}.png"));
        ensure_ordinary_file_if_present(&target)?;
        atomic_write(&target, bytes)?;
        Ok(AvatarAsset {
            kind,
            revision: revision_for(bytes),
        })
    }

    fn remove_asset(&self, profile_id: ProfileId, kind: AvatarKind) -> Result<(), AvatarError> {
        let path = self.directory(kind)?.join(format!("{profile_id}.png"));
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() || super::windows_acl::is_reparse_point(&metadata) {
                    return Err(AvatarError::Storage);
                }
                fs::remove_file(path).map_err(|_| AvatarError::Storage)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AvatarError::Storage),
        }
    }

    fn resolve_asset(
        &self,
        profile_id: ProfileId,
    ) -> Result<Option<(AvatarAsset, PathBuf)>, AvatarError> {
        for kind in [AvatarKind::Manual, AvatarKind::Official] {
            let path = self.directory(kind)?.join(format!("{profile_id}.png"));
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let bytes = read_validated_png(&path)?;
                    return Ok(Some((
                        AvatarAsset {
                            kind,
                            revision: revision_for(&bytes),
                        },
                        path,
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(AvatarError::Storage),
            }
        }
        Ok(None)
    }

    fn directory(&self, kind: AvatarKind) -> Result<PathBuf, AvatarError> {
        match kind {
            AvatarKind::Manual => Ok(self.root.join("manual")),
            AvatarKind::Official => Ok(self.root.join("official")),
            AvatarKind::Default => Err(AvatarError::Invalid),
        }
    }
}

pub fn decode_png_data_url(value: &str) -> Result<Vec<u8>, AvatarError> {
    let payload = value
        .strip_prefix(PNG_DATA_URL_PREFIX)
        .ok_or(AvatarError::Invalid)?;
    let max_encoded_len = MAX_AVATAR_BYTES.div_ceil(3) * 4;
    if payload.is_empty() || payload.len() > max_encoded_len || !payload.is_ascii() {
        return Err(if payload.len() > max_encoded_len {
            AvatarError::TooLarge
        } else {
            AvatarError::Invalid
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| AvatarError::Invalid)?;
    validate_png(&bytes)?;
    Ok(bytes)
}

pub fn validate_official_avatar_url(value: &str) -> Result<Url, AvatarError> {
    if value.len() > MAX_AVATAR_URL_BYTES {
        return Err(AvatarError::Invalid);
    }
    let url = Url::parse(value).map_err(|_| AvatarError::Invalid)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(AvatarError::Invalid);
    }
    let host = url.host_str().ok_or(AvatarError::Invalid)?;
    if !OFFICIAL_HOST_SUFFIXES
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
    {
        return Err(AvatarError::Invalid);
    }
    Ok(url)
}

pub async fn download_official_avatar(value: &str) -> Result<Vec<u8>, AvatarError> {
    let url = validate_official_avatar_url(value)?;
    let host = url.host_str().ok_or(AvatarError::Invalid)?.to_string();
    let resolved = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((&host[..], 443)))
        .await
        .map_err(|_| AvatarError::Unavailable)?
        .map_err(|_| AvatarError::Unavailable)?;
    let mut addresses = resolved.collect::<Vec<SocketAddr>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !is_public_avatar_ip(address.ip()))
    {
        return Err(AvatarError::Unavailable);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .no_proxy()
        .https_only(true)
        .connect_timeout(DNS_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|_| AvatarError::Unavailable)?;
    let mut response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "image/png")
        .send()
        .await
        .map_err(|_| AvatarError::Unavailable)?;
    if !response.status().is_success() || response.status().is_redirection() {
        return Err(AvatarError::Unavailable);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .ok_or(AvatarError::Invalid)?;
    if !content_type.eq_ignore_ascii_case("image/png")
        && !content_type
            .split_once(';')
            .is_some_and(|(kind, _)| kind.trim().eq_ignore_ascii_case("image/png"))
    {
        return Err(AvatarError::Invalid);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_AVATAR_BYTES as u64)
    {
        return Err(AvatarError::TooLarge);
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AvatarError::Unavailable)?
    {
        if bytes.len() + chunk.len() > MAX_AVATAR_BYTES {
            return Err(AvatarError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    validate_png(&bytes)?;
    Ok(bytes)
}

pub(crate) fn is_public_avatar_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _d] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || a >= 240
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (b == 18 || b == 19)))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn validate_png(bytes: &[u8]) -> Result<(), AvatarError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(AvatarError::TooLarge);
    }
    if bytes.len() < 33
        || &bytes[..8] != SIGNATURE
        || u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| AvatarError::Invalid)?) != 13
        || &bytes[12..16] != b"IHDR"
    {
        return Err(AvatarError::Invalid);
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| AvatarError::Invalid)?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| AvatarError::Invalid)?);
    let bit_depth = bytes[24];
    let color_type = bytes[25];
    let valid_bit_depth = match color_type {
        0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
        2 | 4 | 6 => matches!(bit_depth, 8 | 16),
        3 => matches!(bit_depth, 1 | 2 | 4 | 8),
        _ => false,
    };
    if width == 0
        || height == 0
        || width > MAX_AVATAR_DIMENSION
        || height > MAX_AVATAR_DIMENSION
        || !valid_bit_depth
        || bytes[26] != 0
        || bytes[27] != 0
        || bytes[28] > 1
    {
        return Err(AvatarError::Invalid);
    }
    Ok(())
}

fn revision_for(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut revision = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut revision, "{byte:02x}");
    }
    revision
}

fn read_validated_png(path: &Path) -> Result<Vec<u8>, AvatarError> {
    ensure_ordinary_file_if_present(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| AvatarError::Storage)?;
    if metadata.len() > MAX_AVATAR_BYTES as u64 {
        return Err(AvatarError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .map_err(|_| AvatarError::Storage)?
        .take((MAX_AVATAR_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AvatarError::Storage)?;
    validate_png(&bytes)?;
    Ok(bytes)
}

fn ensure_ordinary_directory(path: &Path) -> Result<(), AvatarError> {
    fs::create_dir_all(path).map_err(|_| AvatarError::Storage)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| AvatarError::Storage)?;
    if !metadata.is_dir() || super::windows_acl::is_reparse_point(&metadata) {
        return Err(AvatarError::Storage);
    }
    Ok(())
}

fn ensure_ordinary_file_if_present(path: &Path) -> Result<(), AvatarError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !super::windows_acl::is_reparse_point(&metadata) => {
            Ok(())
        }
        Ok(_) => Err(AvatarError::Storage),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AvatarError::Storage),
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), AvatarError> {
    let parent = target.parent().ok_or(AvatarError::Storage)?;
    let temporary = parent.join(format!(
        "{}.tmp.{}",
        target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(AvatarError::Storage)?,
        Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| AvatarError::Storage)?;
        file.write_all(bytes).map_err(|_| AvatarError::Storage)?;
        file.sync_all().map_err(|_| AvatarError::Storage)?;
        drop(file);
        publish_atomic(&temporary, target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publish_atomic(temporary: &Path, target: &Path) -> Result<(), AvatarError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACEFILE_WRITE_THROUGH, ReplaceFileW,
        };
        use windows::core::PCWSTR;

        fn wide(path: &Path) -> Vec<u16> {
            path.as_os_str().encode_wide().chain(Some(0)).collect()
        }

        let temporary = wide(temporary);
        let target_wide = wide(target);
        let result = if target.exists() {
            unsafe {
                ReplaceFileW(
                    PCWSTR::from_raw(target_wide.as_ptr()),
                    PCWSTR::from_raw(temporary.as_ptr()),
                    PCWSTR::null(),
                    REPLACEFILE_WRITE_THROUGH,
                    None,
                    None,
                )
            }
        } else {
            unsafe {
                MoveFileExW(
                    PCWSTR::from_raw(temporary.as_ptr()),
                    PCWSTR::from_raw(target_wide.as_ptr()),
                    MOVEFILE_WRITE_THROUGH,
                )
            }
        };
        result.map_err(|_| AvatarError::Storage)
    }
    #[cfg(not(windows))]
    {
        fs::rename(temporary, target).map_err(|_| AvatarError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use tempfile::tempdir;
    use uuid::Uuid;

    use super::{
        AvatarKind, AvatarStore, decode_png_data_url, is_public_avatar_ip,
        validate_official_avatar_url,
    };

    fn profile_a() -> Uuid {
        Uuid::from_u128(1)
    }

    fn profile_b() -> Uuid {
        Uuid::from_u128(2)
    }

    fn valid_png_bytes() -> Vec<u8> {
        vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 211, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ]
    }

    #[test]
    fn avatar_url_rejects_http_unknown_hosts_and_direct_ip_targets() {
        for value in [
            "http://cdn.openai.com/a.png",
            "https://127.0.0.1/a.png",
            "https://images.example.com/a.png",
            "file:///C:/avatar.png",
            "https://openai.com.evil.example/a.png",
        ] {
            assert!(
                validate_official_avatar_url(value).is_err(),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn avatar_url_accepts_only_exact_approved_official_suffixes() {
        for value in [
            "https://openai.com/a.png",
            "https://cdn.openai.com/a.png",
            "https://chatgpt.com/a.png",
            "https://images.chatgpt.com/a.png",
            "https://oaistatic.com/a.png",
            "https://cdn.oaistatic.com/a.png",
        ] {
            assert!(
                validate_official_avatar_url(value).is_ok(),
                "rejected {value}"
            );
        }
    }

    #[test]
    fn private_loopback_link_local_and_unspecified_addresses_are_never_targets() {
        for ip in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fc00::1".parse().unwrap()),
            IpAddr::V6("fe80::1".parse().unwrap()),
        ] {
            assert!(!is_public_avatar_ip(ip), "accepted {ip}");
        }
        assert!(is_public_avatar_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_avatar_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn manual_avatar_is_scoped_to_its_profile() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());

        store.write_manual(profile_a(), &valid_png_bytes()).unwrap();

        assert_eq!(
            store.asset_for(profile_a()).unwrap().unwrap().kind,
            AvatarKind::Manual
        );
        assert!(store.asset_for(profile_b()).unwrap().is_none());
    }

    #[test]
    fn clearing_manual_avatar_reveals_profiles_official_avatar() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());
        store
            .write_official(profile_a(), &valid_png_bytes())
            .unwrap();
        store.write_manual(profile_a(), &valid_png_bytes()).unwrap();

        store.clear_manual(profile_a()).unwrap();

        assert_eq!(
            store.asset_for(profile_a()).unwrap().unwrap().kind,
            AvatarKind::Official
        );
    }

    #[test]
    fn invalid_manual_avatar_retains_current_asset_and_leaves_no_partial_file() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());
        let first = store.write_manual(profile_a(), &valid_png_bytes()).unwrap();

        assert!(store.write_manual(profile_a(), b"not a png").is_err());

        let current = store.asset_for(profile_a()).unwrap().unwrap();
        assert_eq!(current.revision, first.revision);
        assert_eq!(
            store
                .read_asset(profile_a(), &current.revision)
                .unwrap()
                .unwrap(),
            valid_png_bytes()
        );
        let leftovers = std::fs::read_dir(dir.path().join("manual"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn png_data_url_decoder_rejects_wrong_type_and_oversized_input() {
        assert!(decode_png_data_url("data:image/jpeg;base64,AA==").is_err());
        let oversized = format!("data:image/png;base64,{}", "A".repeat(1_500_000));
        assert!(decode_png_data_url(&oversized).is_err());
    }

    #[test]
    fn avatar_store_debug_never_exposes_its_local_root() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());
        let debug = format!("{store:?}");
        let unique_directory = dir.path().file_name().unwrap().to_string_lossy();

        assert!(!debug.contains(unique_directory.as_ref()));
    }
}
