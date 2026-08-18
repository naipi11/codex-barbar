//! Sensitive byte ownership with redacted debug output and best-effort
//! zeroing on drop.

use std::fmt;

/// Owns secret bytes. Never prints, serializes, or otherwise exposes its
/// contents; the live allocation is overwritten on drop.
pub struct SensitiveBytes {
    bytes: Vec<u8>,
}

impl SensitiveBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for SensitiveBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitiveBytes([REDACTED])")
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        // Volatile-ish overwrite so an optimizer cannot elide the wipe.
        for byte in &mut self.bytes {
            *byte = 0;
        }
        std::hint::black_box(&self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_bytes_debug_never_prints_content() {
        let bytes = SensitiveBytes::new(b"sk-super-secret".to_vec());
        assert_eq!(format!("{bytes:?}"), "SensitiveBytes([REDACTED])");
        assert_eq!(bytes.as_slice(), b"sk-super-secret");
        assert_eq!(bytes.len(), 15);
    }
}
