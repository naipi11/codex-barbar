//! Profile-scoped avatar validation, download, and local storage.

use std::collections::HashMap;
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
const MAX_DECODED_AVATAR_BYTES: usize =
    MAX_AVATAR_DIMENSION as usize * MAX_AVATAR_DIMENSION as usize * 4;
const MAX_INFLATED_AVATAR_BYTES: usize =
    MAX_AVATAR_DIMENSION as usize * MAX_AVATAR_DIMENSION as usize * 8
        + MAX_AVATAR_DIMENSION as usize * 7
        + 64;
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

struct AvatarHttpClientBuilder {
    inner: reqwest::ClientBuilder,
    #[cfg(test)]
    retry_policy: AvatarRetryPolicy,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AvatarRetryPolicy {
    Never,
}

impl AvatarHttpClientBuilder {
    fn new(host: &str, addresses: &[SocketAddr]) -> Self {
        Self {
            inner: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .retry(avatar_retry_policy())
                .cookie_store(false)
                .no_proxy()
                .https_only(true)
                .connect_timeout(DNS_TIMEOUT)
                .timeout(DOWNLOAD_TIMEOUT)
                .resolve_to_addrs(host, addresses),
            #[cfg(test)]
            retry_policy: AvatarRetryPolicy::Never,
        }
    }

    fn build(self) -> Result<reqwest::Client, AvatarError> {
        self.inner.build().map_err(|_| AvatarError::Unavailable)
    }

    #[cfg(test)]
    fn retry_policy(&self) -> AvatarRetryPolicy {
        self.retry_policy
    }
}

fn avatar_retry_policy() -> reqwest::retry::Builder {
    reqwest::retry::never()
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
    root: Option<PathBuf>,
}

struct AvatarDirectoryGuards {
    _root: DirectoryGuard,
    _kind: DirectoryGuard,
}

#[cfg(windows)]
struct DirectoryGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(not(windows))]
struct DirectoryGuard;

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
        Self { root: Some(root) }
    }

    pub fn disabled() -> Self {
        Self { root: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.root.is_some()
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
            .map(|value| value.map(|(asset, _bytes)| asset))
    }

    pub fn read_asset(
        &self,
        profile_id: ProfileId,
        revision: &str,
    ) -> Result<Option<Vec<u8>>, AvatarError> {
        let Some((asset, bytes)) = self.resolve_asset(profile_id)? else {
            return Ok(None);
        };
        if asset.revision != revision {
            return Ok(None);
        }
        Ok(Some(bytes))
    }

    fn write_asset(
        &self,
        profile_id: ProfileId,
        kind: AvatarKind,
        bytes: &[u8],
    ) -> Result<AvatarAsset, AvatarError> {
        validate_png(bytes)?;
        let directory = self.directory(kind)?;
        let _guards = self
            .directory_guards(kind, true)?
            .ok_or(AvatarError::Storage)?;
        let target = directory.join(format!("{profile_id}.png"));
        ensure_ordinary_file_if_present(&target)?;
        atomic_write(&target, bytes)?;
        Ok(AvatarAsset {
            kind,
            revision: revision_for(bytes),
        })
    }

    fn remove_asset(&self, profile_id: ProfileId, kind: AvatarKind) -> Result<(), AvatarError> {
        if !self.is_enabled() {
            return Ok(());
        }
        let Some(_guards) = self.directory_guards(kind, false)? else {
            return Ok(());
        };
        let path = self.directory(kind)?.join(format!("{profile_id}.png"));
        match fs::symlink_metadata(&path) {
            Ok(_) => delete_avatar_leaf(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AvatarError::Storage),
        }
    }

    fn resolve_asset(
        &self,
        profile_id: ProfileId,
    ) -> Result<Option<(AvatarAsset, Vec<u8>)>, AvatarError> {
        if !self.is_enabled() {
            return Ok(None);
        }
        for kind in [AvatarKind::Manual, AvatarKind::Official] {
            let Some(_guards) = self.directory_guards(kind, false)? else {
                continue;
            };
            let path = self.directory(kind)?.join(format!("{profile_id}.png"));
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let bytes = read_validated_png(&path)?;
                    return Ok(Some((
                        AvatarAsset {
                            kind,
                            revision: revision_for(&bytes),
                        },
                        bytes,
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(AvatarError::Storage),
            }
        }
        Ok(None)
    }

    fn directory_guards(
        &self,
        kind: AvatarKind,
        create: bool,
    ) -> Result<Option<AvatarDirectoryGuards>, AvatarError> {
        let Some(root) = self.root.as_ref() else {
            return if create {
                Err(AvatarError::Unavailable)
            } else {
                Ok(None)
            };
        };
        let directory = self.directory(kind)?;
        if create {
            ensure_ordinary_directory(root)?;
            ensure_ordinary_directory(&directory)?;
        } else if !ordinary_directory_is_present(root)?
            || !ordinary_directory_is_present(&directory)?
        {
            return Ok(None);
        }
        let root = open_directory_guard(root)?;
        let kind = open_directory_guard(&directory)?;
        Ok(Some(AvatarDirectoryGuards {
            _root: root,
            _kind: kind,
        }))
    }

    fn directory(&self, kind: AvatarKind) -> Result<PathBuf, AvatarError> {
        let root = self.root.as_ref().ok_or(AvatarError::Unavailable)?;
        match kind {
            AvatarKind::Manual => Ok(root.join("manual")),
            AvatarKind::Official => Ok(root.join("official")),
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

    let client = AvatarHttpClientBuilder::new(&host, &addresses).build()?;
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
    !(a == 0
        || ip.is_private()
        || (a == 100 && b & 0b1100_0000 == 0b0100_0000)
        || ip.is_loopback()
        || ip.is_link_local()
        || (a == 192 && b == 0 && c == 0)
        || ip.is_documentation()
        || (a == 192 && b == 31 && c == 196)
        || (a == 192 && b == 52 && c == 193)
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 175 && c == 48)
        || (a == 198 && (b == 18 || b == 19))
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.to_ipv4_mapped().is_some()
        || matches!(segments, [0x64, 0xff9b, 0, 0, 0, 0, _, _])
        || matches!(segments, [0x64, 0xff9b, 1, _, _, _, _, _])
        || matches!(segments, [0x100, 0, 0, 0, _, _, _, _])
        || matches!(segments, [0x2001, b, _, _, _, _, _, _] if b < 0x200)
        || matches!(segments, [0x2002, _, _, _, _, _, _, _])
        || matches!(segments, [0x2001, 0xdb8, _, _, _, _, _, _])
        || segments[0] & 0xfff0 == 0x3ff0
        || matches!(segments, [0x5f00, ..])
        || matches!(segments, [0x2620, 0x4f, 0x8000, _, _, _, _, _])
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || (segments[0] & 0xffc0 == 0xfec0)
        || ip.is_multicast())
}

fn validate_png(bytes: &[u8]) -> Result<(), AvatarError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(AvatarError::TooLarge);
    }
    if bytes.len() < 8 || &bytes[..8] != SIGNATURE {
        return Err(AvatarError::Invalid);
    }

    let mut offset = 8usize;
    let mut header = None;
    let mut saw_palette = false;
    let mut saw_idat = false;
    let mut saw_nonempty_idat = false;
    let mut idat_ended = false;
    let mut idat_data = Vec::new();
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(AvatarError::Invalid)?;
        if header_end > bytes.len() {
            return Err(AvatarError::Invalid);
        }
        let length = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| AvatarError::Invalid)?,
        ) as usize;
        if length > MAX_AVATAR_BYTES {
            return Err(AvatarError::TooLarge);
        }
        let chunk_type: [u8; 4] = bytes[offset + 4..header_end]
            .try_into()
            .map_err(|_| AvatarError::Invalid)?;
        if !chunk_type.iter().all(u8::is_ascii_alphabetic) || !chunk_type[2].is_ascii_uppercase() {
            return Err(AvatarError::Invalid);
        }
        let data_end = header_end.checked_add(length).ok_or(AvatarError::Invalid)?;
        let chunk_end = data_end.checked_add(4).ok_or(AvatarError::Invalid)?;
        if chunk_end > bytes.len() {
            return Err(AvatarError::Invalid);
        }
        let stored_crc = u32::from_be_bytes(
            bytes[data_end..chunk_end]
                .try_into()
                .map_err(|_| AvatarError::Invalid)?,
        );
        if stored_crc != png_chunk_crc(&chunk_type, &bytes[header_end..data_end]) {
            return Err(AvatarError::Invalid);
        }

        match &chunk_type {
            b"IHDR" => {
                if offset != 8 || header.is_some() || length != 13 {
                    return Err(AvatarError::Invalid);
                }
                header = Some(validate_ihdr(&bytes[header_end..data_end])?);
            }
            b"PLTE" => {
                let Some(header) = header else {
                    return Err(AvatarError::Invalid);
                };
                let palette_entries = length / 3;
                if saw_palette
                    || saw_idat
                    || length == 0
                    || length > 256 * 3
                    || !length.is_multiple_of(3)
                    || matches!(header.color_type, 0 | 4)
                    || (header.color_type == 3 && palette_entries > (1usize << header.bit_depth))
                {
                    return Err(AvatarError::Invalid);
                }
                saw_palette = true;
            }
            b"IDAT" => {
                let Some(header) = header else {
                    return Err(AvatarError::Invalid);
                };
                if idat_ended || (header.color_type == 3 && !saw_palette) {
                    return Err(AvatarError::Invalid);
                }
                saw_idat = true;
                saw_nonempty_idat |= length > 0;
                idat_data.extend_from_slice(&bytes[header_end..data_end]);
            }
            b"IEND" => {
                let Some(header) = header else {
                    return Err(AvatarError::Invalid);
                };
                if length != 0 || !saw_idat || !saw_nonempty_idat || chunk_end != bytes.len() {
                    return Err(AvatarError::Invalid);
                }
                validate_zlib_stream(&idat_data)?;
                validate_decoded_png(bytes, header)?;
                return Ok(());
            }
            _ => {
                if header.is_none() || chunk_type[0].is_ascii_uppercase() {
                    return Err(AvatarError::Invalid);
                }
                if saw_idat {
                    idat_ended = true;
                }
            }
        }
        offset = chunk_end;
    }
    Err(AvatarError::Invalid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PngHeader {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
}

fn validate_ihdr(data: &[u8]) -> Result<PngHeader, AvatarError> {
    if data.len() != 13 {
        return Err(AvatarError::Invalid);
    }
    let width = u32::from_be_bytes(data[0..4].try_into().map_err(|_| AvatarError::Invalid)?);
    let height = u32::from_be_bytes(data[4..8].try_into().map_err(|_| AvatarError::Invalid)?);
    let bit_depth = data[8];
    let color_type = data[9];
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
        || data[10] != 0
        || data[11] != 0
        || data[12] > 1
    {
        return Err(AvatarError::Invalid);
    }
    Ok(PngHeader {
        width,
        height,
        bit_depth,
        color_type,
    })
}

#[cfg(windows)]
fn validate_decoded_png(bytes: &[u8], expected: PngHeader) -> Result<(), AvatarError> {
    use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICImagingFactory, IWICPalette,
        WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };

    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let _com = if result.is_ok() {
        ComGuard(true)
    } else if result == RPC_E_CHANGED_MODE {
        ComGuard(false)
    } else {
        return Err(AvatarError::Invalid);
    };

    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| AvatarError::Invalid)?;
    let stream = unsafe { factory.CreateStream() }.map_err(|_| AvatarError::Invalid)?;
    unsafe { stream.InitializeFromMemory(bytes) }.map_err(|_| AvatarError::Invalid)?;
    let decoder = unsafe {
        factory.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)
    }
    .map_err(|_| AvatarError::Invalid)?;
    if unsafe { decoder.GetFrameCount() }.map_err(|_| AvatarError::Invalid)? != 1 {
        return Err(AvatarError::Invalid);
    }
    let frame = unsafe { decoder.GetFrame(0) }.map_err(|_| AvatarError::Invalid)?;
    let mut width = 0;
    let mut height = 0;
    unsafe { frame.GetSize(&mut width, &mut height) }.map_err(|_| AvatarError::Invalid)?;
    if width != expected.width || height != expected.height {
        return Err(AvatarError::Invalid);
    }
    let stride = width.checked_mul(4).ok_or(AvatarError::TooLarge)?;
    let decoded_length = usize::try_from(stride)
        .ok()
        .and_then(|stride| {
            usize::try_from(height)
                .ok()
                .and_then(|height| stride.checked_mul(height))
        })
        .filter(|length| *length <= MAX_DECODED_AVATAR_BYTES)
        .ok_or(AvatarError::TooLarge)?;
    let converter = unsafe { factory.CreateFormatConverter() }.map_err(|_| AvatarError::Invalid)?;
    unsafe {
        converter.Initialize(
            &frame,
            &GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone,
            None::<&IWICPalette>,
            0.0,
            WICBitmapPaletteTypeCustom,
        )
    }
    .map_err(|_| AvatarError::Invalid)?;
    let mut pixels = vec![0u8; decoded_length];
    unsafe { converter.CopyPixels(std::ptr::null(), stride, &mut pixels) }
        .map_err(|_| AvatarError::Invalid)?;
    Ok(())
}

#[cfg(not(windows))]
fn validate_decoded_png(_bytes: &[u8], _expected: PngHeader) -> Result<(), AvatarError> {
    Ok(())
}

fn png_chunk_crc(chunk_type: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in chunk_type.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn validate_zlib_stream(bytes: &[u8]) -> Result<(), AvatarError> {
    if bytes.len() < 6 {
        return Err(AvatarError::Invalid);
    }
    let cmf = bytes[0];
    let flags = bytes[1];
    if cmf & 0x0f != 8
        || cmf >> 4 > 7
        || (u16::from(cmf) << 8 | u16::from(flags)) % 31 != 0
        || flags & 0x20 != 0
    {
        return Err(AvatarError::Invalid);
    }
    let compressed_end = bytes.len() - 4;
    let mut reader = DeflateBits::new(&bytes[2..compressed_end]);
    let mut output = Vec::new();
    loop {
        let final_block = reader.read_bits(1)? != 0;
        match reader.read_bits(2)? {
            0 => inflate_stored_block(&mut reader, &mut output)?,
            1 => {
                let (literal, distance) = fixed_huffman_tables()?;
                inflate_huffman_block(&mut reader, &mut output, &literal, &distance)?;
            }
            2 => {
                let (literal, distance) = dynamic_huffman_tables(&mut reader)?;
                inflate_huffman_block(&mut reader, &mut output, &literal, &distance)?;
            }
            _ => return Err(AvatarError::Invalid),
        }
        if final_block {
            break;
        }
    }
    if !reader.at_stream_end() || output.len() > MAX_INFLATED_AVATAR_BYTES {
        return Err(AvatarError::Invalid);
    }
    let expected_adler = u32::from_be_bytes(
        bytes[compressed_end..]
            .try_into()
            .map_err(|_| AvatarError::Invalid)?,
    );
    if adler32(&output) != expected_adler {
        return Err(AvatarError::Invalid);
    }
    Ok(())
}

struct DeflateBits<'a> {
    bytes: &'a [u8],
    byte_index: usize,
    bit_index: u8,
}

impl<'a> DeflateBits<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_index: 0,
            bit_index: 0,
        }
    }

    fn read_bits(&mut self, count: u8) -> Result<u32, AvatarError> {
        if count > 16 {
            return Err(AvatarError::Invalid);
        }
        let mut value = 0u32;
        for shift in 0..count {
            let byte = *self
                .bytes
                .get(self.byte_index)
                .ok_or(AvatarError::Invalid)?;
            value |= u32::from((byte >> self.bit_index) & 1) << shift;
            self.bit_index += 1;
            if self.bit_index == 8 {
                self.bit_index = 0;
                self.byte_index += 1;
            }
        }
        Ok(value)
    }

    fn align_byte(&mut self) {
        if self.bit_index != 0 {
            self.bit_index = 0;
            self.byte_index += 1;
        }
    }

    fn read_aligned_bytes(&mut self, count: usize) -> Result<&'a [u8], AvatarError> {
        if self.bit_index != 0 {
            return Err(AvatarError::Invalid);
        }
        let end = self
            .byte_index
            .checked_add(count)
            .ok_or(AvatarError::Invalid)?;
        let bytes = self
            .bytes
            .get(self.byte_index..end)
            .ok_or(AvatarError::Invalid)?;
        self.byte_index = end;
        Ok(bytes)
    }

    fn at_stream_end(&self) -> bool {
        self.byte_index == self.bytes.len()
            || (self.bit_index != 0 && self.byte_index + 1 == self.bytes.len())
    }
}

struct DeflateHuffman {
    codes: Vec<HashMap<u16, u16>>,
    max_bits: u8,
}

impl DeflateHuffman {
    fn from_lengths(lengths: &[u8]) -> Result<Self, AvatarError> {
        let max_bits = lengths.iter().copied().max().unwrap_or(0);
        if max_bits > 15 {
            return Err(AvatarError::Invalid);
        }
        let mut counts = [0u16; 16];
        for &length in lengths {
            if length > 15 {
                return Err(AvatarError::Invalid);
            }
            if length != 0 {
                counts[usize::from(length)] += 1;
            }
        }
        let mut left = 1i32;
        for &count in counts.iter().skip(1) {
            left = (left << 1) - i32::from(count);
            if left < 0 {
                return Err(AvatarError::Invalid);
            }
        }
        let mut next_code = [0u16; 16];
        let mut code = 0u16;
        for bits in 1..=15 {
            code = (code + counts[bits - 1]) << 1;
            next_code[bits] = code;
        }
        let mut codes = (0..=usize::from(max_bits))
            .map(|_| HashMap::new())
            .collect::<Vec<_>>();
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let canonical = next_code[usize::from(length)];
            next_code[usize::from(length)] += 1;
            let reversed = canonical.reverse_bits() >> (16 - length);
            if codes[usize::from(length)]
                .insert(
                    reversed,
                    u16::try_from(symbol).map_err(|_| AvatarError::Invalid)?,
                )
                .is_some()
            {
                return Err(AvatarError::Invalid);
            }
        }
        Ok(Self { codes, max_bits })
    }

    fn decode(&self, reader: &mut DeflateBits<'_>) -> Result<u16, AvatarError> {
        let mut code = 0u16;
        for length in 1..=self.max_bits {
            code |= u16::try_from(reader.read_bits(1)?).map_err(|_| AvatarError::Invalid)?
                << (length - 1);
            if let Some(symbol) = self.codes[usize::from(length)].get(&code) {
                return Ok(*symbol);
            }
        }
        Err(AvatarError::Invalid)
    }
}

fn inflate_stored_block(
    reader: &mut DeflateBits<'_>,
    output: &mut Vec<u8>,
) -> Result<(), AvatarError> {
    reader.align_byte();
    let header = reader.read_aligned_bytes(4)?;
    let length = u16::from_le_bytes([header[0], header[1]]);
    let inverse = u16::from_le_bytes([header[2], header[3]]);
    if length != !inverse {
        return Err(AvatarError::Invalid);
    }
    let bytes = reader.read_aligned_bytes(usize::from(length))?;
    extend_inflated(output, bytes)
}

fn fixed_huffman_tables() -> Result<(DeflateHuffman, DeflateHuffman), AvatarError> {
    let mut literal_lengths = vec![0u8; 288];
    literal_lengths[..144].fill(8);
    literal_lengths[144..256].fill(9);
    literal_lengths[256..280].fill(7);
    literal_lengths[280..].fill(8);
    let distance_lengths = vec![5u8; 32];
    Ok((
        DeflateHuffman::from_lengths(&literal_lengths)?,
        DeflateHuffman::from_lengths(&distance_lengths)?,
    ))
}

fn dynamic_huffman_tables(
    reader: &mut DeflateBits<'_>,
) -> Result<(DeflateHuffman, DeflateHuffman), AvatarError> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let literal_count =
        usize::try_from(reader.read_bits(5)?).map_err(|_| AvatarError::Invalid)? + 257;
    let distance_count =
        usize::try_from(reader.read_bits(5)?).map_err(|_| AvatarError::Invalid)? + 1;
    let code_length_count =
        usize::try_from(reader.read_bits(4)?).map_err(|_| AvatarError::Invalid)? + 4;
    if literal_count > 286 || distance_count > 32 {
        return Err(AvatarError::Invalid);
    }
    let mut code_lengths = [0u8; 19];
    for &index in ORDER.iter().take(code_length_count) {
        code_lengths[index] =
            u8::try_from(reader.read_bits(3)?).map_err(|_| AvatarError::Invalid)?;
    }
    let code_length_table = DeflateHuffman::from_lengths(&code_lengths)?;
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);
    while lengths.len() < total {
        match code_length_table.decode(reader)? {
            value @ 0..=15 => lengths.push(value as u8),
            16 => {
                let previous = *lengths.last().ok_or(AvatarError::Invalid)?;
                let count =
                    usize::try_from(reader.read_bits(2)?).map_err(|_| AvatarError::Invalid)? + 3;
                append_code_lengths(&mut lengths, previous, count, total)?;
            }
            17 => {
                let count =
                    usize::try_from(reader.read_bits(3)?).map_err(|_| AvatarError::Invalid)? + 3;
                append_code_lengths(&mut lengths, 0, count, total)?;
            }
            18 => {
                let count =
                    usize::try_from(reader.read_bits(7)?).map_err(|_| AvatarError::Invalid)? + 11;
                append_code_lengths(&mut lengths, 0, count, total)?;
            }
            _ => return Err(AvatarError::Invalid),
        }
    }
    if lengths.get(256).copied().unwrap_or(0) == 0 {
        return Err(AvatarError::Invalid);
    }
    let literal = DeflateHuffman::from_lengths(&lengths[..literal_count])?;
    let distance = DeflateHuffman::from_lengths(&lengths[literal_count..])?;
    Ok((literal, distance))
}

fn append_code_lengths(
    lengths: &mut Vec<u8>,
    value: u8,
    count: usize,
    limit: usize,
) -> Result<(), AvatarError> {
    if lengths.len().saturating_add(count) > limit {
        return Err(AvatarError::Invalid);
    }
    lengths.resize(lengths.len() + count, value);
    Ok(())
}

fn inflate_huffman_block(
    reader: &mut DeflateBits<'_>,
    output: &mut Vec<u8>,
    literal: &DeflateHuffman,
    distance: &DeflateHuffman,
) -> Result<(), AvatarError> {
    const LENGTH_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LENGTH_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DISTANCE_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DISTANCE_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    loop {
        match literal.decode(reader)? {
            value @ 0..=255 => extend_inflated(output, &[value as u8])?,
            256 => return Ok(()),
            value @ 257..=285 => {
                let length_index = usize::from(value - 257);
                let length = LENGTH_BASE[length_index]
                    + usize::try_from(reader.read_bits(LENGTH_EXTRA[length_index])?)
                        .map_err(|_| AvatarError::Invalid)?;
                let distance_symbol = usize::from(distance.decode(reader)?);
                if distance_symbol >= DISTANCE_BASE.len() {
                    return Err(AvatarError::Invalid);
                }
                let back = DISTANCE_BASE[distance_symbol]
                    + usize::try_from(reader.read_bits(DISTANCE_EXTRA[distance_symbol])?)
                        .map_err(|_| AvatarError::Invalid)?;
                if back == 0
                    || back > output.len()
                    || output.len().saturating_add(length) > MAX_INFLATED_AVATAR_BYTES
                {
                    return Err(AvatarError::Invalid);
                }
                for _ in 0..length {
                    let byte = output[output.len() - back];
                    output.push(byte);
                }
            }
            _ => return Err(AvatarError::Invalid),
        }
    }
}

fn extend_inflated(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), AvatarError> {
    if output.len().saturating_add(bytes.len()) > MAX_INFLATED_AVATAR_BYTES {
        return Err(AvatarError::TooLarge);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

fn adler32(bytes: &[u8]) -> u32 {
    const MODULUS: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in bytes {
        a = (a + u32::from(byte)) % MODULUS;
        b = (b + a) % MODULUS;
    }
    b << 16 | a
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
    #[cfg(windows)]
    let (file, length) = open_avatar_leaf_for_read(path)?;
    #[cfg(not(windows))]
    let (mut file, length) = {
        ensure_ordinary_file_if_present(path)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| AvatarError::Storage)?;
        (
            File::open(path).map_err(|_| AvatarError::Storage)?,
            metadata.len(),
        )
    };
    if length > MAX_AVATAR_BYTES as u64 {
        return Err(AvatarError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take((MAX_AVATAR_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AvatarError::Storage)?;
    validate_png(&bytes)?;
    Ok(bytes)
}

#[cfg(windows)]
fn validate_avatar_leaf_attributes(attributes: u32) -> Result<(), AvatarError> {
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    };

    if attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0
        || attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    {
        return Err(AvatarError::Storage);
    }
    Ok(())
}

#[cfg(windows)]
fn open_avatar_leaf_handle(
    path: &Path,
    desired_access: u32,
) -> Result<(DirectoryGuard, u64), AvatarError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        GetFileInformationByHandle, OPEN_EXISTING,
    };
    use windows::core::PCWSTR;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide.as_ptr()),
            desired_access,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|_| AvatarError::Storage)?;
    let guard = DirectoryGuard(handle);
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(guard.0, &mut information) }
        .map_err(|_| AvatarError::Storage)?;
    validate_avatar_leaf_attributes(information.dwFileAttributes)?;
    let length = u64::from(information.nFileSizeHigh) << 32 | u64::from(information.nFileSizeLow);
    Ok((guard, length))
}

#[cfg(windows)]
fn open_avatar_leaf_for_read(path: &Path) -> Result<(File, u64), AvatarError> {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::Foundation::GENERIC_READ;

    let (guard, length) = open_avatar_leaf_handle(path, GENERIC_READ.0)?;
    let raw = guard.0.0;
    std::mem::forget(guard);
    let file = unsafe { File::from_raw_handle(raw) };
    Ok((file, length))
}

#[cfg(windows)]
fn delete_avatar_leaf(path: &Path) -> Result<(), AvatarError> {
    use windows::Win32::Foundation::BOOLEAN;
    use windows::Win32::Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_INFO, FILE_READ_ATTRIBUTES, FileDispositionInfo,
        SetFileInformationByHandle,
    };

    let (guard, _length) = open_avatar_leaf_handle(path, DELETE.0 | FILE_READ_ATTRIBUTES.0)?;
    let disposition = FILE_DISPOSITION_INFO {
        DeleteFile: BOOLEAN(1),
    };
    unsafe {
        SetFileInformationByHandle(
            guard.0,
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>())
                .map_err(|_| AvatarError::Storage)?,
        )
    }
    .map_err(|_| AvatarError::Storage)
}

#[cfg(not(windows))]
fn delete_avatar_leaf(path: &Path) -> Result<(), AvatarError> {
    ensure_ordinary_file_if_present(path)?;
    fs::remove_file(path).map_err(|_| AvatarError::Storage)
}

fn ensure_ordinary_directory(path: &Path) -> Result<(), AvatarError> {
    fs::create_dir_all(path).map_err(|_| AvatarError::Storage)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| AvatarError::Storage)?;
    if !metadata.is_dir() || super::windows_acl::is_reparse_point(&metadata) {
        return Err(AvatarError::Storage);
    }
    Ok(())
}

fn ordinary_directory_is_present(path: &Path) -> Result<bool, AvatarError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !super::windows_acl::is_reparse_point(&metadata) => {
            Ok(true)
        }
        Ok(_) => Err(AvatarError::Storage),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AvatarError::Storage),
    }
}

#[cfg(windows)]
fn open_directory_guard(path: &Path) -> Result<DirectoryGuard, AvatarError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, GetFileInformationByHandle, OPEN_EXISTING,
    };
    use windows::core::PCWSTR;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide.as_ptr()),
            FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|_| AvatarError::Storage)?;
    let guard = DirectoryGuard(handle);
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(guard.0, &mut information) }
        .map_err(|_| AvatarError::Storage)?;
    if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 == 0
        || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    {
        return Err(AvatarError::Storage);
    }
    Ok(guard)
}

#[cfg(not(windows))]
fn open_directory_guard(path: &Path) -> Result<DirectoryGuard, AvatarError> {
    if ordinary_directory_is_present(path)? {
        Ok(DirectoryGuard)
    } else {
        Err(AvatarError::Storage)
    }
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
    use std::net::IpAddr;

    use tempfile::tempdir;
    use uuid::Uuid;

    use super::{
        AvatarHttpClientBuilder, AvatarKind, AvatarRetryPolicy, AvatarStore, avatar_retry_policy,
        decode_png_data_url, is_public_avatar_ip, validate_avatar_leaf_attributes,
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
            192, 240, 31, 0, 5, 0, 1, 255, 114, 156, 82, 103, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ]
    }

    fn header_only_png_bytes() -> Vec<u8> {
        valid_png_bytes()[..33].to_vec()
    }

    fn png_with_chunks(ihdr: [u8; 13], palette: Option<&[u8]>, idat: &[u8]) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        append_png_chunk(&mut bytes, *b"IHDR", &ihdr);
        if let Some(palette) = palette {
            append_png_chunk(&mut bytes, *b"PLTE", palette);
        }
        append_png_chunk(&mut bytes, *b"IDAT", idat);
        append_png_chunk(&mut bytes, *b"IEND", &[]);
        bytes
    }

    fn append_png_chunk(bytes: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&chunk_type);
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(&super::png_chunk_crc(&chunk_type, data).to_be_bytes());
    }

    fn crc_valid_png_with_invalid_zlib() -> Vec<u8> {
        png_with_chunks(
            [0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0],
            None,
            &[0x78, 0x9c, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0],
        )
    }

    fn indexed_png_with_too_many_palette_entries() -> Vec<u8> {
        png_with_chunks(
            [0, 0, 0, 1, 0, 0, 0, 1, 1, 3, 0, 0, 0],
            Some(&[0, 0, 0, 255, 255, 255, 127, 127, 127]),
            &[120, 156, 99, 96, 0, 0, 0, 2, 0, 1],
        )
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
    fn official_avatar_client_constructor_uses_retry_never() {
        let address = "8.8.8.8:443".parse().unwrap();
        let builder = AvatarHttpClientBuilder::new("cdn.openai.com", &[address]);

        assert_eq!(builder.retry_policy(), AvatarRetryPolicy::Never);
        let policy = format!("{:?}", avatar_retry_policy());
        assert!(policy.contains("classifier: Never"), "{policy}");
        assert!(policy.contains("budget: None"), "{policy}");
    }

    #[test]
    fn only_native_globally_routable_addresses_are_avatar_targets() {
        for value in [
            "0.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.0.0.1",
            "192.0.2.1",
            "192.31.196.1",
            "192.52.193.1",
            "192.88.99.1",
            "192.168.0.1",
            "192.175.48.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",
            "64:ff9b:1::1",
            "100::1",
            "2001::1",
            "2001:2::1",
            "2001:db8::1",
            "2002:808:808::1",
            "3fff::1",
            "5f00::1",
            "fc00::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
        ] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(!is_public_avatar_ip(ip), "accepted {ip}");
        }
        for value in ["8.8.8.8", "2606:4700:4700::1111"] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(is_public_avatar_ip(ip), "rejected {ip}");
        }
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
    fn header_only_and_truncated_pngs_are_not_stored() {
        for bytes in [
            header_only_png_bytes(),
            valid_png_bytes()[..valid_png_bytes().len() - 1].to_vec(),
        ] {
            let dir = tempdir().unwrap();
            let store = AvatarStore::for_test(dir.path());

            assert!(store.write_manual(profile_a(), &bytes).is_err());
            assert!(
                !dir.path()
                    .join("manual")
                    .join(format!("{}.png", profile_a()))
                    .exists()
            );
            assert!(store.asset_for(profile_a()).unwrap().is_none());
        }
    }

    #[test]
    fn crc_corruption_is_rejected_before_storage() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());
        let mut corrupt = valid_png_bytes();
        corrupt[29] ^= 1;

        assert!(store.write_manual(profile_a(), &corrupt).is_err());
        assert!(store.asset_for(profile_a()).unwrap().is_none());
    }

    #[test]
    fn directly_planted_header_only_png_is_never_served() {
        let dir = tempdir().unwrap();
        let store = AvatarStore::for_test(dir.path());
        let manual = dir.path().join("manual");
        std::fs::create_dir_all(&manual).unwrap();
        let bytes = header_only_png_bytes();
        std::fs::write(manual.join(format!("{}.png", profile_a())), &bytes).unwrap();

        let revision = super::revision_for(&bytes);
        assert!(store.read_asset(profile_a(), &revision).is_err());
    }

    #[test]
    fn decoded_pixels_and_indexed_palette_limits_are_required_before_store_or_serve() {
        for (label, bytes) in [
            ("invalid zlib", crc_valid_png_with_invalid_zlib()),
            (
                "oversized indexed palette",
                indexed_png_with_too_many_palette_entries(),
            ),
        ] {
            let dir = tempdir().unwrap();
            let store = AvatarStore::for_test(dir.path());

            assert!(
                store.write_manual(profile_a(), &bytes).is_err(),
                "stored {label}"
            );
            assert!(store.asset_for(profile_a()).unwrap().is_none());

            let manual = dir.path().join("manual");
            std::fs::create_dir_all(&manual).unwrap();
            std::fs::write(manual.join(format!("{}.png", profile_a())), &bytes).unwrap();
            let revision = super::revision_for(&bytes);
            assert!(
                store.read_asset(profile_a(), &revision).is_err(),
                "served {label}"
            );
        }
    }

    #[test]
    fn dynamic_huffman_zlib_stream_is_fully_decoded() {
        let compressed = [
            120, 218, 237, 202, 89, 22, 64, 32, 0, 64, 209, 45, 33, 146, 229, 100, 8, 101, 150, 33,
            171, 183, 143, 206, 187, 223, 87, 215, 77, 219, 153, 126, 24, 173, 155, 230, 101, 221,
            246, 227, 244, 215, 253, 188, 225, 75, 210, 76, 228, 133, 44, 85, 165, 57, 28, 14, 135,
            195, 225, 112, 56, 28, 14, 135, 19, 221, 249, 1, 44, 161, 37, 124,
        ];

        assert!(super::validate_zlib_stream(&compressed).is_ok());
    }

    #[test]
    fn reparse_kind_directories_cannot_read_or_delete_outside_avatar_root() {
        for (kind, manual) in [("manual", true), ("official", false)] {
            let dir = tempdir().unwrap();
            let root = dir.path().join("avatars");
            let outside = dir.path().join(format!("outside-{kind}"));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::create_dir_all(&outside).unwrap();
            let outside_asset = outside.join(format!("{}.png", profile_a()));
            let bytes = valid_png_bytes();
            std::fs::write(&outside_asset, &bytes).unwrap();
            link_directory(&outside, &root.join(kind));
            let store = AvatarStore::new(root);
            let revision = super::revision_for(&bytes);

            let read = store.read_asset(profile_a(), &revision);
            let remove = if manual {
                store.clear_manual(profile_a())
            } else {
                store.clear_official(profile_a())
            };

            assert!(read.is_err(), "read through {kind} reparse directory");
            assert!(remove.is_err(), "removed through {kind} reparse directory");
            assert_eq!(std::fs::read(outside_asset).unwrap(), bytes);
        }
    }

    #[cfg(windows)]
    #[test]
    fn file_reparse_handle_is_rejected_before_serve_or_clear_touches_outside_file() {
        use windows::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
        };

        let dir = tempdir().unwrap();
        let outside = dir.path().join("outside.png");
        let bytes = valid_png_bytes();
        std::fs::write(&outside, &bytes).unwrap();

        assert!(
            validate_avatar_leaf_attributes(
                FILE_ATTRIBUTE_NORMAL.0 | FILE_ATTRIBUTE_REPARSE_POINT.0
            )
            .is_err()
        );
        assert!(validate_avatar_leaf_attributes(FILE_ATTRIBUTE_REPARSE_POINT.0).is_err());
        assert_eq!(std::fs::read(outside).unwrap(), bytes);
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

    #[test]
    fn disabled_store_returns_default_without_creating_a_relative_root() {
        let store = AvatarStore::disabled();

        assert!(!store.is_enabled());
        assert!(store.asset_for(profile_a()).unwrap().is_none());
        assert!(
            store
                .read_asset(profile_a(), &"ab".repeat(32))
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            store.write_manual(profile_a(), &valid_png_bytes()),
            Err(super::AvatarError::Unavailable)
        ));
        assert!(store.clear_manual(profile_a()).is_ok());
    }

    #[cfg(windows)]
    fn link_directory(target: &std::path::Path, link: &std::path::Path) {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return;
        }
        let output = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(output.status.success(), "junction creation failed");
    }

    #[cfg(unix)]
    fn link_directory(target: &std::path::Path, link: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
}
