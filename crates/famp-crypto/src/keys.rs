//! FAMP-owned newtypes wrapping `ed25519-dalek` types (D-06/D-07/D-10).
//!
//! `TrustedVerifyingKey` is the only verifying-key type reachable from public
//! API. Its constructors perform the spec §7.1b ingress checks (canonical
//! point decode + weak-key / 8-torsion rejection). No public path exposes
//! `ed25519_dalek::VerifyingKey::verify` (non-strict).

use crate::error::CryptoError;
use crate::hash::sha256_digest;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use subtle::ConstantTimeEq;

/// Ed25519 signing key — 32-byte seed, wraps `ed25519_dalek::SigningKey`.
///
/// # Invariants
///
/// - 32-byte seed; never logged (`Debug` is redacted); never serialized to
///   the wire. Signing keys live only in memory or on disk under operator
///   control.
/// - Drop-time zeroization is inherited from `ed25519-dalek`'s `zeroize`
///   feature (wired in the workspace dep). The newtype intentionally does
///   NOT re-derive `Zeroize` — dalek's own `ZeroizeOnDrop` is the supported
///   path.
///
/// # Pitfalls
///
/// `FampSigningKey::from_bytes([0u8; 32])` and other all-constant seeds are
/// test fixtures only. The crate-level quick-start doctest uses `[0u8; 32]`
/// for illustration; production code must source 32 bytes from a CSPRNG.
///
/// # Security contract (FAMP D-17 mechanism #1)
///
/// `FampSigningKey` must never expose private-key bytes via `Debug` or
/// `Display`. Phase 1 of v0.8 locks this contract with the tests below.
/// The `Debug` impl returns a fixed redacted string (no seed bytes); there
/// is no `Display` impl, and the `compile_fail` block below is the forcing
/// function that keeps it that way.
///
/// ```
/// use famp_crypto::FampSigningKey;
/// let sk = FampSigningKey::from_bytes([7u8; 32]);
/// let dbg = format!("{:?}", sk);
/// assert!(dbg.contains("redacted"));
/// // The raw seed byte 7 must not leak through Debug.
/// assert!(!dbg.contains('7'));
/// ```
///
/// ```compile_fail
/// use famp_crypto::FampSigningKey;
/// let sk = FampSigningKey::from_bytes([0u8; 32]);
/// // There must be no Display impl — this must fail to compile.
/// let _ = format!("{}", sk);
/// ```
pub struct FampSigningKey(pub(crate) SigningKey);

/// The only verifying-key type reachable from public API.
///
/// The word "Trusted" is load-bearing: a `TrustedVerifyingKey` is one that
/// has already passed canonical-point decode and weak-key / 8-torsion
/// rejection at ingress (spec §7.1b, CRYPTO-02/03). Verification code
/// downstream of this type may assume the key is safe to use.
///
/// # Invariants
///
/// - Constructed only via [`TrustedVerifyingKey::from_bytes`] or
///   [`TrustedVerifyingKey::from_b64url`], both of which run the
///   `is_weak()` check.
/// - The underlying `VerifyingKey` is never exposed; the trust boundary
///   only holds through this newtype.
///
/// # Pitfalls
///
/// Do NOT construct an `ed25519_dalek::VerifyingKey` directly and pass it
/// to raw dalek APIs. The whole point of this wrapper is that the ingress
/// check happens exactly once, here. Bypassing it reintroduces the 8-torsion
/// hole that `verify_strict` alone does not close.
#[derive(Clone)]
pub struct TrustedVerifyingKey(pub(crate) VerifyingKey);

/// Raw 64-byte Ed25519 signature.
///
/// Wire encoding is base64url unpadded (`URL_SAFE_NO_PAD`, strict alphabet),
/// yielding an 86-character string per spec §7.1b.
///
/// # Pitfalls
///
/// Strict decoding rejects trailing `=` padding and the standard (`+/`)
/// alphabet by design. A signature that decodes under a lax decoder but
/// fails strict decoding is a protocol bug in the *producer*, not a codec
/// bug here — do not "fix" it by relaxing the decoder.
#[derive(Clone)]
pub struct FampSignature(pub(crate) Signature);

impl FampSigningKey {
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&bytes))
    }

    /// Generates a fresh signing key from the OS CSPRNG (`OsRng`).
    ///
    /// # Security contract
    ///
    /// This is the ONLY sanctioned way to create a signing key outside of
    /// test fixtures (see the `from_bytes` pitfall note above). Every call
    /// draws fresh entropy from the operating system's CSPRNG — there is no
    /// fixed-seed, time-based, or PID-based path in production code
    /// (TRUST-01 prerequisite, T-08-04).
    #[must_use]
    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        Self(SigningKey::generate(&mut OsRng))
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

/// Stable, human-comparable fingerprint of an Ed25519 verifying key
/// (WIRE-02 `sender_key_id`, D-03).
///
/// Derivation: `SHA-256(raw 32-byte pubkey)` -> `base64url` (unpadded) ->
/// first 16 characters (~96 bits of the 256-bit digest). This length is
/// locked by Phase 8 D-03 / RESEARCH Assumption A1.
///
/// # Non-anchor contract
///
/// `key_id` is diagnostic/UX metadata ONLY — an eyeball-verification aid for
/// `famp peer export`/`import` (D-05) and a wire-carried label
/// (`sender_key_id`). It is NOT a trust anchor: no code may make a trust
/// decision on `key_id` alone. The sole trust anchor is the full 32-byte
/// pinned verifying key in the keyring (T-08-05).
#[must_use]
pub fn key_id(vk: &TrustedVerifyingKey) -> String {
    let digest = sha256_digest(vk.as_bytes());
    let full = URL_SAFE_NO_PAD.encode(digest);
    full.chars().take(16).collect()
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
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
    fn generate_produces_distinct_keys() {
        let a = FampSigningKey::generate();
        let b = FampSigningKey::generate();
        assert_ne!(
            a.verifying_key().to_b64url(),
            b.verifying_key().to_b64url(),
            "two generate() calls must not yield the same key"
        );
    }

    #[test]
    fn generated_key_signs_and_verifies() {
        use crate::sign::sign_value;
        use crate::verify::verify_value;
        use serde_json::json;

        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        let v = json!({"hello": "generated"});
        let sig = sign_value(&sk, &v).expect("sign_value should succeed");
        verify_value(&vk, &v, &sig).expect("verify_value should succeed for generated key");
    }

    #[test]
    fn key_id_is_deterministic_and_16_chars() {
        use super::key_id;

        let sk1 = FampSigningKey::from_bytes([3u8; 32]);
        let vk1 = sk1.verifying_key();
        let sk2 = FampSigningKey::from_bytes([9u8; 32]);
        let vk2 = sk2.verifying_key();

        let id1a = key_id(&vk1);
        let id1b = key_id(&vk1);
        let id2 = key_id(&vk2);

        assert_eq!(id1a.len(), 16, "key_id must be exactly 16 characters");
        assert_eq!(id1a, id1b, "key_id must be deterministic for the same key");
        assert_ne!(id1a, id2, "key_id must differ for distinct keys");
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
