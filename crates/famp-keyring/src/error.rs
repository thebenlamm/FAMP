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

    #[error("duplicate key entry at line {line}: pubkey already present for {principal}")]
    DuplicateKeyEntry { principal: Principal, line: usize },

    #[error("malformed entry at line {line}: {reason}")]
    MalformedEntry { line: usize, reason: String },

    #[error("key conflict: principal {principal} already pinned to a different key")]
    KeyConflict { principal: Principal },

    /// `Keyring::rotate_to` refused a real key change for a known
    /// principal because the caller did not pass `confirmed = true`
    /// (KEYR-03). Zero mutation on this path — the CLI turns this into
    /// an operator-facing `KEY CHANGED` block plus exit code 2.
    #[error(
        "key change requires confirmation for {principal}: currently pinned to \
         {previous_key_id}, incoming {new_key_id}"
    )]
    KeyChangeRequiresConfirmation {
        principal: Principal,
        previous_key_id: String,
        new_key_id: String,
    },

    /// `rotate_to`/`pin_tofu` refused to (re-)pin a pubkey that already
    /// exists as a `revoked` entry for this principal. Revocation is
    /// monotonic — a revoked key can never return via rotation or TOFU
    /// re-import (T-15-04).
    #[error("key revoked: {key_id} is a revoked entry for {principal} and cannot be re-pinned")]
    KeyRevoked {
        principal: Principal,
        key_id: String,
    },

    /// `Keyring::retire` was asked to remove a `key_id` that does not
    /// exist for this principal.
    #[error("no such key entry {key_id} for principal {principal}")]
    NoSuchKeyEntry {
        principal: Principal,
        key_id: String,
    },

    /// `Keyring::retire` refuses to remove an `Active` entry (would sever
    /// the channel) or a `Revoked` entry (a revocation tombstone is
    /// permanent). `state` names which.
    #[error("cannot retire {key_id} for {principal}: entry is {state}")]
    CannotRetire {
        principal: Principal,
        key_id: String,
        state: &'static str,
    },

    /// A caller-supplied `now` or `valid_until` passed to `rotate_to` is
    /// not itself in the canonical 20-byte UTC `YYYY-MM-DDTHH:MM:SSZ`
    /// form — fail closed rather than risk an unsound lexical comparison
    /// (mirrors `KeyLookupOutcome::NonCanonicalNow`, T-15-10).
    #[error("non-canonical timestamp: {value:?}")]
    NonCanonicalTimestamp { value: String },

    #[error("invalid --peer flag: {reason}")]
    InvalidPeerFlag { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {0}")]
    Crypto(#[from] famp_crypto::CryptoError),
}
