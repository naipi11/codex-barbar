//! Shared AWS SigV4 / Volcengine signing primitives.
//!
//! Bedrock and Doubao both build SigV4-style requests over the same
//! `sha256_hex` / `hmac_sha256` / `hex` helpers. Hoisting them here keeps the
//! two providers byte-identical in their crypto and lets each provider keep
//! its own `sanitized_body` truncation policy.

use sha2::{Digest, Sha256};

/// Lowercase hex encoding of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SHA-256 digest of `data` as a lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// RFC 2104 HMAC-SHA256 of `data` keyed with `key` (raw bytes).
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut outer = [0x5cu8; BLOCK_SIZE];
    let mut inner = [0x36u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        outer[i] ^= key_block[i];
        inner[i] ^= key_block[i];
    }

    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(data);
    let inner_digest = inner_hash.finalize();

    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_digest);
    outer_hash.finalize().to_vec()
}

/// Collapse whitespace and truncate the body preview to `max_chars` for
/// safe logging / debug output. Returns `"empty body"` for blank input.
pub fn sanitized_body(body: &str, max_chars: usize) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > max_chars {
        let preview: String = collapsed.chars().take(max_chars).collect();
        format!("{preview}... [truncated]")
    } else if collapsed.is_empty() {
        "empty body".to_string()
    } else {
        collapsed
    }
}
