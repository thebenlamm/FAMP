//! Verify free functions (D-01/D-03). All verification routes through
//! `ed25519_dalek::VerifyingKey::verify_strict` — no public path reaches the
//! non-strict `verify`.

use crate::{
    error::CryptoError,
    keys::{FampSignature, TrustedVerifyingKey},
    prefix::DOMAIN_PREFIX,
};

/// Verify a signature over an arbitrary `Serialize` value.
pub fn verify_value<T: serde::Serialize + ?Sized>(
    verifying_key: &TrustedVerifyingKey,
    value: &T,
    signature: &FampSignature,
) -> Result<(), CryptoError> {
    let canonical = famp_canonical::canonicalize(value)?;
    verify_canonical_bytes(verifying_key, &canonical, signature)
}

/// Verify a signature over already-canonical bytes.
/// Routes exclusively through `VerifyingKey::verify_strict`.
pub fn verify_canonical_bytes(
    verifying_key: &TrustedVerifyingKey,
    canonical_bytes: &[u8],
    signature: &FampSignature,
) -> Result<(), CryptoError> {
    let mut input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical_bytes.len());
    input.extend_from_slice(DOMAIN_PREFIX);
    input.extend_from_slice(canonical_bytes);
    verifying_key
        .0
        .verify_strict(&input, &signature.0)
        .map_err(|_| CryptoError::VerificationFailed)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sign::{sign_canonical_bytes, sign_value};
    use crate::{FampSignature, FampSigningKey, TrustedVerifyingKey};
    use serde_json::json;

    fn keypair() -> (FampSigningKey, TrustedVerifyingKey) {
        let sk = FampSigningKey::from_bytes([42u8; 32]);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    #[test]
    fn roundtrip_value() {
        let (sk, vk) = keypair();
        let v = json!({"a": 1, "b": [1, 2, 3], "c": "hello"});
        let sig = sign_value(&sk, &v).unwrap();
        verify_value(&vk, &v, &sig).unwrap();
    }

    #[test]
    fn roundtrip_canonical_bytes() {
        let (sk, vk) = keypair();
        let canonical = b"{\"x\":1}";
        let sig = sign_canonical_bytes(&sk, canonical);
        verify_canonical_bytes(&vk, canonical, &sig).unwrap();
    }

    #[test]
    fn tampered_payload_fails() {
        let (sk, vk) = keypair();
        let sig = sign_canonical_bytes(&sk, b"{\"x\":1}");
        let res = verify_canonical_bytes(&vk, b"{\"x\":2}", &sig);
        assert!(matches!(res, Err(CryptoError::VerificationFailed)));
    }

    #[test]
    fn tampered_signature_fails() {
        let (sk, vk) = keypair();
        let sig = sign_canonical_bytes(&sk, b"{\"x\":1}");
        let mut bytes = sig.to_bytes();
        bytes[0] ^= 0x01;
        let bad = FampSignature::from_bytes(bytes);
        let res = verify_canonical_bytes(&vk, b"{\"x\":1}", &bad);
        assert!(matches!(res, Err(CryptoError::VerificationFailed)));
    }

    #[test]
    fn canonicalize_for_signature_starts_with_prefix() {
        let v = json!({"z": 0});
        let out = crate::prefix::canonicalize_for_signature(&v).unwrap();
        assert_eq!(&out[..12], DOMAIN_PREFIX.as_slice());
        let canonical = famp_canonical::canonicalize(&v).unwrap();
        assert_eq!(out.len(), 12 + canonical.len());
        assert_eq!(&out[12..], canonical.as_slice());
    }
}
