//! Phase 2 `CryptoError` — narrow, phase-appropriate (D-26/D-27).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid key encoding")]
    InvalidKeyEncoding,
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,
    #[error("weak public key rejected at ingress")]
    WeakKey,
    #[error("canonicalization failure: {0}")]
    Canonicalization(#[from] famp_canonical::CanonicalError),
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("invalid signing input")]
    InvalidSigningInput,
}
