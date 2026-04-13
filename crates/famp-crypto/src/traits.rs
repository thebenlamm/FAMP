//! Signer / Verifier traits — thin sugar over the free functions in
//! `sign` / `verify` (D-01). No direct `ed25519_dalek::Signer` impl is
//! provided for FAMP newtypes; FAMP signing rules (domain prefix,
//! canonical JSON input) are protocol-specific.

use crate::{
    error::CryptoError,
    keys::{FampSignature, FampSigningKey, TrustedVerifyingKey},
};

/// Thin sugar over `sign_value` / `sign_canonical_bytes`.
pub trait Signer {
    fn sign_value<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
    ) -> Result<FampSignature, CryptoError>;
    fn sign_canonical_bytes(&self, canonical_bytes: &[u8]) -> FampSignature;
}

/// Thin sugar over `verify_value` / `verify_canonical_bytes`.
pub trait Verifier {
    fn verify_value<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        signature: &FampSignature,
    ) -> Result<(), CryptoError>;
    fn verify_canonical_bytes(
        &self,
        canonical_bytes: &[u8],
        signature: &FampSignature,
    ) -> Result<(), CryptoError>;
}

impl Signer for FampSigningKey {
    fn sign_value<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
    ) -> Result<FampSignature, CryptoError> {
        crate::sign::sign_value(self, value)
    }
    fn sign_canonical_bytes(&self, canonical_bytes: &[u8]) -> FampSignature {
        crate::sign::sign_canonical_bytes(self, canonical_bytes)
    }
}

impl Verifier for TrustedVerifyingKey {
    fn verify_value<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
        signature: &FampSignature,
    ) -> Result<(), CryptoError> {
        crate::verify::verify_value(self, value, signature)
    }
    fn verify_canonical_bytes(
        &self,
        canonical_bytes: &[u8],
        signature: &FampSignature,
    ) -> Result<(), CryptoError> {
        crate::verify::verify_canonical_bytes(self, canonical_bytes, signature)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trait_sugar_matches_free_fn() {
        let sk = FampSigningKey::from_bytes([7u8; 32]);
        let vk = sk.verifying_key();
        let v = json!({"hello": "world"});
        let sig_trait = <FampSigningKey as Signer>::sign_value(&sk, &v).unwrap();
        let sig_free = crate::sign::sign_value(&sk, &v).unwrap();
        assert_eq!(sig_trait, sig_free);
        <TrustedVerifyingKey as Verifier>::verify_value(&vk, &v, &sig_trait).unwrap();
    }

    #[test]
    fn trait_canonical_bytes_sugar_matches_free_fn() {
        let sk = FampSigningKey::from_bytes([9u8; 32]);
        let vk = sk.verifying_key();
        let bytes = b"{\"k\":42}";
        let sig = <FampSigningKey as Signer>::sign_canonical_bytes(&sk, bytes);
        <TrustedVerifyingKey as Verifier>::verify_canonical_bytes(&vk, bytes, &sig).unwrap();
    }
}
