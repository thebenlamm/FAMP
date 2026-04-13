//! Domain-separation prefix per FAMP-v0.5.1 §7.1a.
//! Hex: 46 41 4d 50 2d 73 69 67 2d 76 31 00

/// Public constant — exposed for test/fixture use only.
/// Callers MUST NOT assemble signing input manually; use
/// `canonicalize_for_signature`.
pub const DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\0";

use crate::error::CryptoError;

/// Returns `DOMAIN_PREFIX || famp_canonical::canonicalize(value)`.
///
/// This is the exact byte sequence passed to Ed25519 sign/verify for FAMP
/// signatures per spec §7.1a. Callers MUST provide the envelope with the
/// `signature` field already removed (envelope field-strip policy lives in
/// `famp-envelope`, not here).
pub fn canonicalize_for_signature(
    unsigned_value: &serde_json::Value,
) -> Result<Vec<u8>, CryptoError> {
    let canonical = famp_canonical::canonicalize(unsigned_value)?;
    let mut buf = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical.len());
    buf.extend_from_slice(DOMAIN_PREFIX);
    buf.extend_from_slice(&canonical);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::DOMAIN_PREFIX;

    #[test]
    fn prefix_bytes_match_spec() {
        assert_eq!(DOMAIN_PREFIX.len(), 12);
        assert_eq!(
            DOMAIN_PREFIX.as_slice(),
            &[
                0x46, 0x41, 0x4d, 0x50, 0x2d, 0x73, 0x69, 0x67, 0x2d, 0x76, 0x31, 0x00,
            ]
        );
    }
}
