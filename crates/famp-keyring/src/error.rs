//! Phase-local narrow error enum.
//!
//! Following the v0.6 / Phase 1 / Phase 2 precedent, `KeyringError` does NOT
//! convert into `famp_core::ProtocolErrorKind` inside this crate. Mapping to
//! the protocol boundary happens in `crates/famp/src/runtime/` (Phase 3
//! runtime glue, Plan 03-03).

use famp_core::Principal;

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("duplicate principal at line {line}: {principal}")]
    DuplicatePrincipal { principal: Principal, line: usize },

    #[error("duplicate pubkey at line {line}: already pinned to {existing}")]
    DuplicatePubkey { existing: Principal, line: usize },

    #[error("malformed entry at line {line}: {reason}")]
    MalformedEntry { line: usize, reason: String },

    #[error("key conflict: principal {principal} already pinned to a different key")]
    KeyConflict { principal: Principal },

    #[error("invalid --peer flag: {reason}")]
    InvalidPeerFlag { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {0}")]
    Crypto(#[from] famp_crypto::CryptoError),
}
