//! Sign free functions (D-01/D-03). Every FAMP signature is produced here.
//!
//! `sign_canonical_bytes` prepends `DOMAIN_PREFIX` internally — callers never
//! assemble the signing input themselves.

use crate::{
    error::CryptoError,
    keys::{FampSignature, FampSigningKey},
    prefix::DOMAIN_PREFIX,
};
use ed25519_dalek::Signer as _;

/// Primary signing entry point. Canonicalizes `value` via `famp-canonical`
/// (RFC 8785), prepends [`DOMAIN_PREFIX`], then Ed25519-signs.
///
/// See the crate-level quick-start example in `lib.rs` for the full
/// sign/verify round-trip.
///
/// # Pitfalls
///
/// If you already hold canonical bytes (e.g. in a hot loop that
/// canonicalizes once and signs many times), call [`sign_canonical_bytes`]
/// instead to skip the re-canonicalization cost. Both paths produce
/// byte-identical signatures for byte-identical input.
pub fn sign_value<T: serde::Serialize + ?Sized>(
    signing_key: &FampSigningKey,
    value: &T,
) -> Result<FampSignature, CryptoError> {
    let canonical = famp_canonical::canonicalize(value)?;
    Ok(sign_canonical_bytes(signing_key, &canonical))
}

/// Sign already-canonical bytes. Internally prepends [`DOMAIN_PREFIX`] and
/// calls Ed25519.
///
/// # Precondition
///
/// `canonical_bytes` MUST be the output of `famp_canonical::canonicalize`
/// (RFC 8785 JCS). There is no internal canonicalization step. Passing raw
/// `serde_json::to_vec` output produces a perfectly valid Ed25519 signature
/// that will not round-trip across implementations. Use [`sign_value`] if
/// you want the canonicalize step done for you.
#[must_use]
pub fn sign_canonical_bytes(signing_key: &FampSigningKey, canonical_bytes: &[u8]) -> FampSignature {
    let mut input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical_bytes.len());
    input.extend_from_slice(DOMAIN_PREFIX);
    input.extend_from_slice(canonical_bytes);
    let sig = signing_key.0.sign(&input);
    FampSignature(sig)
}
