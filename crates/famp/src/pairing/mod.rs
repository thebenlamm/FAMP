//! `famp::pairing` — cross-person trust bootstrap (Phase 18, PAIR-01/06).
//!
//! A five-word texted code lets two people with no prior shared secret pin
//! each other's Ed25519 key, without pasting a raw key blob (`famp peer
//! export`/`import`, Phase 8) and without reading a fingerprint aloud.
//! Security rests on entropy (`wordlist::WORDLIST_LEN.pow(5)` — roughly 55
//! bits), single-use invite state (Plan 02), and a bounded server-side
//! attempt counter (Plan 02) — never on cryptography beyond the wire
//! signing below (T-18-01).
//!
//! This module is deliberately a PARALLEL, non-envelope wire: it never
//! constructs an `AnySignedEnvelope`, never rides the local bus, and never
//! touches the frozen `famp-envelope`/`famp-canonical`/`famp-crypto`
//! internals directly — only their public `sign_value`/`verify_value`
//! entry points, the same substrate every other signed FAMP record
//! (`famp_keyring::revocation::SignedRevocation`) already uses.
//!
//! Submodules:
//! - [`wordlist`] — the vendored BIP-39 English word list and the uniform
//!   CSPRNG draw/parse.
//! - [`invite`] — the on-disk invite record and its atomic, locked store.

pub mod invite;
pub mod wordlist;

use famp_crypto::{
    sign_value, verify_value, CryptoError, FampSignature, FampSigningKey, TrustedVerifyingKey,
};
use serde::{Deserialize, Serialize};

/// Errors this module's library surface can produce.
///
/// CLI-layer and gateway-layer callers translate these into their own
/// typed errors (`CliError`, `famp_gateway::pairing_ingress::RedemptionReject`)
/// rather than propagating this enum across a crate boundary.
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    #[error("pairing code is malformed: {reason}")]
    CodeMalformed { reason: String },
    #[error("no pending invite")]
    NoPendingInvite,
    #[error("pairing store is busy (locked by another process)")]
    StoreBusy,
    #[error("io error at {}: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cryptographic error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("wire error: {reason}")]
    Wire { reason: String },
}

/// A statement plus its detached Ed25519 signature — mirrors
/// `famp_keyring::revocation::SignedRevocation`'s exact shape.
///
/// `#[serde(deny_unknown_fields)]` is unconditional here (as it is on
/// every signed FAMP type): an additive field changes the canonicalized
/// bytes and would silently invalidate the signature, so an unknown field
/// must be rejected loudly at parse time rather than dropped. Generic
/// over the statement type so both [`RedemptionRequest`] and
/// [`RedemptionResponse`] share one wrapper and one signing/verification
/// call site — never a second signing path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed<T> {
    pub statement: T,
    pub sig: String,
}

impl<T: Serialize> Signed<T> {
    /// Sign `statement`, producing a `Signed<T>`. Delegates entirely to
    /// `famp_crypto::sign_value` — RFC 8785 JCS canonicalization and the
    /// `FAMP-sig-v1\0` domain prefix are both handled there.
    pub fn new(statement: T, sk: &FampSigningKey) -> Result<Self, PairingError> {
        let sig = sign_value(sk, &statement)?;
        Ok(Self {
            statement,
            sig: sig.to_b64url(),
        })
    }

    /// Verify this envelope's signature against `vk`. Never routes through
    /// anything but `famp_crypto::verify_value` (`verify_strict`
    /// internally) — see that crate's module docs for why plain
    /// `ed25519_dalek::verify` must never be reachable here.
    pub fn verify(&self, vk: &TrustedVerifyingKey) -> Result<(), PairingError> {
        let sig = FampSignature::from_b64url(&self.sig).map_err(|_| PairingError::Wire {
            reason: "malformed signature encoding".to_string(),
        })?;
        verify_value(vk, &self.statement, &sig).map_err(|_| PairingError::Wire {
            reason: "signature verification failed".to_string(),
        })
    }
}

/// The redemption request body.
///
/// The redeemer proves possession of the fresh key it wants pinned by
/// signing this statement with that SAME key (T-18-04) — a relay cannot
/// swap in a pubkey it does not hold the private half of.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedemptionRequest {
    pub code: String,
    pub principal: String,
    pub pubkey_b64url: String,
    pub nonce: String,
}

/// The redemption response body.
///
/// The inviter's gateway signs this with its OWN gateway key, carrying
/// its own principal + pubkey so the redeemer can verify
/// proof-of-possession (T-18-04) rather than trusting the transport
/// alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedemptionResponse {
    pub invite_id: String,
    pub inviter_principal: String,
    pub inviter_pubkey_b64url: String,
    pub redeemer_key_id: String,
    pub at: String,
}

/// An UNSIGNED rejection body (T-18-09: the only fact this can leak is
/// "no invite is currently outstanding" or "the code was malformed" —
/// never key material, never which specific invite was close).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedemptionReject {
    pub reason: String,
}
