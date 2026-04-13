//! `sha256:<hex>` artifact-ID helper (SPEC-18, CONTEXT.md D-19..D-22).
//!
//! Phase 1 ships a `String`-backed placeholder type. Phase 3 (`famp-core`)
//! refactors this into a strongly-typed `ArtifactId`. The helpers here
//! constitute the byte-to-`sha256:<hex>` primitive that the rest of the
//! codebase will depend on, so the byte-level behavior must stay frozen.

use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::canonical::canonicalize;
use crate::error::CanonicalError;

/// Phase 1 placeholder for `ArtifactId` (D-21).
///
/// Wraps a `String` of the form `sha256:<64 lowercase hex>`. Phase 3 will
/// replace this with a strongly-typed alternative; consumers should call
/// [`ArtifactIdString::as_str`] (or rely on the `AsRef<str>` / `Display`
/// impls) rather than poking at the inner field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactIdString(pub String);

impl ArtifactIdString {
    /// Borrow the underlying `sha256:<hex>` string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ArtifactIdString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArtifactIdString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Lowercase-hex encode `bytes` without pulling in the `hex` crate.
///
/// Inlined here per RESEARCH §"SHA-256 Artifact ID (Inline, No Hex Crate)"
/// to keep the dependency tree minimal.
fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("writing to String never fails");
    }
    s
}

/// Compute `sha256:<hex>` over an already-canonicalized byte string.
///
/// Use this when you have canonical bytes in hand (e.g., the same bytes you
/// just signed). For arbitrary `Serialize` values, use
/// [`artifact_id_for_value`] instead, which canonicalizes first.
pub fn artifact_id_for_canonical_bytes(bytes: &[u8]) -> ArtifactIdString {
    let hash = Sha256::digest(bytes);
    ArtifactIdString(format!("sha256:{}", bytes_to_lower_hex(&hash)))
}

/// Canonicalize `value` then compute its `sha256:<hex>` artifact ID.
///
/// This is the convenience path: it ensures the artifact ID is computed over
/// exactly the bytes that `canonicalize` would produce, eliminating any risk
/// of "hashed something different than what we signed".
///
/// # Errors
///
/// Propagates any [`CanonicalError`] from [`canonicalize`].
pub fn artifact_id_for_value<T: Serialize + ?Sized>(
    value: &T,
) -> Result<ArtifactIdString, CanonicalError> {
    let bytes = canonicalize(value)?;
    Ok(artifact_id_for_canonical_bytes(&bytes))
}
