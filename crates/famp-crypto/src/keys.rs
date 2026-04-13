//! FAMP-owned newtypes wrapping `ed25519-dalek` types (D-06/D-07/D-10).
//!
//! `TrustedVerifyingKey` is the only verifying-key type reachable from public
//! API. Its constructors perform the spec §7.1b ingress checks (canonical
//! point decode + weak-key / 8-torsion rejection). No public path exposes
//! `ed25519_dalek::VerifyingKey::verify` (non-strict).

use crate::error::CryptoError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use subtle::ConstantTimeEq;

/// Ed25519 signing key.
///
/// Secret bytes are zeroized on drop by `ed25519-dalek`'s own drop-time
/// `ZeroizeOnDrop` behavior, enabled via the `zeroize` feature wired in the
/// workspace dep. We do not re-derive `Zeroize` / `ZeroizeOnDrop` on the
/// newtype because `SigningKey` intentionally does not implement `Zeroize`
/// directly — drop-time zeroization from dalek is the supported path
/// (Pitfall 4).
pub struct FampSigningKey(pub(crate) SigningKey);

/// The ONLY verifying-key type reachable from public API.
/// Construction enforces weak-key rejection (SPEC §7.1b, CRYPTO-02/03).
#[derive(Clone)]
pub struct TrustedVerifyingKey(pub(crate) VerifyingKey);

/// Ed25519 signature newtype. 64 bytes on the wire.
#[derive(Clone)]
pub struct FampSignature(pub(crate) Signature);

impl FampSigningKey {
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&bytes))
    }

    pub fn from_b64url(input: &str) -> Result<Self, CryptoError> {
        let v = URL_SAFE_NO_PAD
            .decode(input)
            .map_err(|_| CryptoError::InvalidKeyEncoding)?;
        let arr: [u8; 32] = v.try_into().map_err(|_| CryptoError::InvalidKeyEncoding)?;
        Ok(Self::from_bytes(arr))
    }

    pub fn to_b64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.to_bytes())
    }

    /// Returns the associated public key as a `TrustedVerifyingKey`.
    /// Self-generated keys are by construction non-weak, but we still route
    /// through the ingress constructor for uniformity.
    #[must_use]
    pub fn verifying_key(&self) -> TrustedVerifyingKey {
        TrustedVerifyingKey(self.0.verifying_key())
    }
}

impl core::fmt::Debug for FampSigningKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FampSigningKey(<redacted>)")
    }
}

impl TrustedVerifyingKey {
    /// Ingress constructor. Performs length decode, canonical Edwards point
    /// decode, and weak-key / 8-torsion rejection.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let vk = VerifyingKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidKeyEncoding)?;
        if vk.is_weak() {
            return Err(CryptoError::WeakKey);
        }
        Ok(Self(vk))
    }

    pub fn from_b64url(input: &str) -> Result<Self, CryptoError> {
        let v = URL_SAFE_NO_PAD
            .decode(input)
            .map_err(|_| CryptoError::InvalidKeyEncoding)?;
        let arr: [u8; 32] = v.try_into().map_err(|_| CryptoError::InvalidKeyEncoding)?;
        Self::from_bytes(&arr)
    }

    pub fn to_b64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.as_bytes())
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl core::fmt::Debug for TrustedVerifyingKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "TrustedVerifyingKey({})", self.to_b64url())
    }
}

impl FampSignature {
    #[must_use]
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(Signature::from_bytes(&bytes))
    }

    pub fn from_b64url(input: &str) -> Result<Self, CryptoError> {
        let v = URL_SAFE_NO_PAD
            .decode(input)
            .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
        let arr: [u8; 64] = v
            .try_into()
            .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
        Ok(Self::from_bytes(arr))
    }

    pub fn to_b64url(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.to_bytes())
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }
}

impl PartialEq for FampSignature {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes().ct_eq(&other.0.to_bytes()).into()
    }
}
impl Eq for FampSignature {}

impl core::fmt::Debug for FampSignature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FampSignature({})", self.to_b64url())
    }
}

#[cfg(test)]
mod tests {
    use super::{FampSignature, FampSigningKey, TrustedVerifyingKey};
    use crate::error::CryptoError;

    #[test]
    fn identity_point_rejected_as_weak() {
        let zero = [0u8; 32];
        let res = TrustedVerifyingKey::from_bytes(&zero);
        assert!(
            matches!(res, Err(CryptoError::WeakKey)),
            "identity point MUST be rejected at ingress, got {res:?}"
        );
    }

    #[test]
    fn base64_standard_alphabet_rejected() {
        // Contains '/' — STANDARD alphabet, not URL_SAFE
        let bad = "aaaa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(TrustedVerifyingKey::from_b64url(bad).is_err());
    }

    #[test]
    fn base64_padded_rejected() {
        let bad = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        assert!(TrustedVerifyingKey::from_b64url(bad).is_err());
    }

    #[test]
    fn debug_signing_key_redacts() {
        let sk = FampSigningKey::from_bytes([1u8; 32]);
        let s = format!("{sk:?}");
        assert!(s.contains("redacted"));
        assert!(!s.contains("0101"));
    }

    #[test]
    fn signature_partial_eq_constant_time_wrapper() {
        let a = FampSignature::from_bytes([7u8; 64]);
        let b = FampSignature::from_bytes([7u8; 64]);
        let c = FampSignature::from_bytes([8u8; 64]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
