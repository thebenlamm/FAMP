//! Domain-separation prefix per FAMP-v0.5.1 §7.1a.
//! Hex: 46 41 4d 50 2d 73 69 67 2d 76 31 00

/// Public constant — exposed for test/fixture use only.
/// Callers MUST NOT assemble signing input manually; use
/// `canonicalize_for_signature` (added in Plan 02).
pub const DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\0";

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
