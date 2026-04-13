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

/// Sign an arbitrary `Serialize` value: canonicalize, prepend `DOMAIN_PREFIX`,
/// sign via Ed25519.
pub fn sign_value<T: serde::Serialize + ?Sized>(
    signing_key: &FampSigningKey,
    value: &T,
) -> Result<FampSignature, CryptoError> {
    let canonical = famp_canonical::canonicalize(value)?;
    Ok(sign_canonical_bytes(signing_key, &canonical))
}

/// Sign already-canonical bytes: prepend `DOMAIN_PREFIX` internally, then sign.
/// The caller MUST pass bytes produced by `famp_canonical::canonicalize`.
#[must_use]
pub fn sign_canonical_bytes(
    signing_key: &FampSigningKey,
    canonical_bytes: &[u8],
) -> FampSignature {
    let mut input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical_bytes.len());
    input.extend_from_slice(DOMAIN_PREFIX);
    input.extend_from_slice(canonical_bytes);
    let sig = signing_key.0.sign(&input);
    FampSignature(sig)
}
