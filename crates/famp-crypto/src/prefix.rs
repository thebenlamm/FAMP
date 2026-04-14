//! Domain-separation prefix per FAMP-v0.5.1 §7.1a.
//! Hex: 46 41 4d 50 2d 73 69 67 2d 76 31 00

/// Domain-separation prefix: the 12 bytes `b"FAMP-sig-v1\0"`. Prepended to
/// every canonicalized payload before Ed25519 sign/verify.
///
/// # Invariants
///
/// - Exactly 12 bytes, including the trailing NUL. Any transformation that
///   drops or adds bytes silently breaks interop.
/// - `v1` is the wire version. If the protocol ever needs to rotate signing
///   semantics, the mechanism is a new prefix (`FAMP-sig-v2\0`), NOT a field
///   rename and NOT an alternate code path.
///
/// # Pitfalls
///
/// Never sign or verify without this prefix. Signing a raw canonical body
/// with Ed25519 produces a perfectly valid Ed25519 signature that will not
/// verify through any FAMP implementation — and will not verify through
/// `famp-crypto` either, because every entry point in this crate prepends
/// the prefix internally.
///
/// # Spec
///
/// §7.1a, §Δ08.
pub const DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\0";

use crate::error::CryptoError;

/// Canonicalize `unsigned_value` via `famp-canonical` and prepend
/// [`DOMAIN_PREFIX`], yielding the exact byte sequence passed to Ed25519
/// sign/verify per spec §7.1a.
///
/// # Precondition
///
/// `unsigned_value` must be the envelope with the `signature` field already
/// removed. Envelope field-strip policy lives in `famp-envelope`, not here.
///
/// # Pitfalls
///
/// The function name contains "canonicalize" for historical reasons — it
/// really means "prepare canonical bytes for signature by prepending the
/// domain prefix". It is NOT a drop-in replacement for
/// `famp_canonical::canonicalize`. Do not call it on raw
/// `serde_json::to_vec` output.
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
            &[0x46, 0x41, 0x4d, 0x50, 0x2d, 0x73, 0x69, 0x67, 0x2d, 0x76, 0x31, 0x00,]
        );
    }
}
