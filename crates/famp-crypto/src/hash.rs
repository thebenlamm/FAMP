//! SHA-256 content-addressing (CRYPTO-07).
//!
//! FAMP represents content-addressed artifacts with the form
//! `sha256:<lowercase-hex>` per the spec `artifact-id` scheme. This module
//! is the single sanctioned path to produce that string — callers MUST NOT
//! re-implement the hash or the encoding.
//!
//! Backed by the `RustCrypto` `sha2` crate (workspace-pinned at 0.11.0).
//! See `README.md` `## Content addressing (CRYPTO-07)` and
//! `tests/sha256_vectors.rs` for the NIST KAT conformance gate.

use sha2::{Digest, Sha256};

/// Raw SHA-256 digest of `bytes` as a 32-byte array.
///
/// Infallible. Empty input is well-defined and returns the standard
/// `e3b0c442...` digest.
#[must_use]
pub fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// SHA-256 content-addressed identifier in FAMP `sha256:<lowercase-hex>`
/// form, matching the spec `artifact-id` scheme (§3.6a).
///
/// Returns a 71-character `String`: the literal prefix `sha256:` (7 bytes)
/// followed by exactly 64 lowercase hex characters.
///
/// # Pitfalls
///
/// The full 71-character string is the wire identifier. Callers MUST NOT
/// uppercase the hex, strip the `sha256:` prefix, or re-add it somewhere
/// else in the stack. Comparing "just the hex" against a peer is the
/// easiest way to ship a bug that only surfaces when a future digest family
/// (`sha3:`, `blake3:`, …) lands.
#[must_use]
pub fn sha256_artifact_id(bytes: &[u8]) -> String {
    let digest = sha256_digest(bytes);
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for b in digest {
        // Lowercase hex, two chars per byte, no separators.
        // `u32::from(b >> 4)` and `u32::from(b & 0x0f)` are always 0..=15,
        // so `char::from_digit(.., 16)` never returns None; the
        // `unwrap_or('0')` branch is statically unreachable but lint-clean
        // under the workspace's `clippy::unwrap_used = deny`.
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    out
}
