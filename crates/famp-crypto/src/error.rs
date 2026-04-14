//! Phase 2 `CryptoError` — narrow, phase-appropriate (D-26/D-27).

use thiserror::Error;

/// Typed errors for `famp-crypto` sign / verify / encoding paths.
///
/// Narrow by design: each variant names the exact invariant it protects.
/// New variants are a breaking change and must carry a spec citation in the
/// commit body.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Base64url decode failed, wrong length, or wrong alphabet. Strict
    /// `URL_SAFE_NO_PAD` decoding rejects padded or standard-alphabet input
    /// by design (spec §7.1b encoding rules).
    #[error("invalid key encoding")]
    InvalidKeyEncoding,
    /// Signature bytes failed strict base64url decode or were not exactly
    /// 64 bytes. Spec §7.1b.
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,
    /// `VerifyingKey::is_weak()` rejected the point at ingress: identity or
    /// low-order 8-torsion element. Constructing a [`crate::TrustedVerifyingKey`]
    /// that reached this variant means the trust boundary held. Spec §7.1b.
    #[error("weak public key rejected at ingress")]
    WeakKey,
    /// RFC 8785 canonicalization failed upstream in `famp-canonical`. Bubbled
    /// through so sign/verify callers see a single error type.
    #[error("canonicalization failure: {0}")]
    Canonicalization(#[from] famp_canonical::CanonicalError),
    /// Ed25519 `verify_strict` rejected the signature: wrong key, tampered
    /// payload, non-canonical encoding, or small-order point. Never plain
    /// `verify` — this variant is the one that makes INV-10 + non-repudiation
    /// a real property rather than an aspiration. Spec §7.1b.
    #[error("signature verification failed")]
    VerificationFailed,
    /// Signing input was structurally invalid before Ed25519 was reached
    /// (reserved for future use; currently unused but kept for forward-compat).
    #[error("invalid signing input")]
    InvalidSigningInput,
}
