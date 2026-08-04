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

pub mod consent;
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
/// Seven pairwise-distinct, plain-language, next-action-bearing failure
/// messages (PAIR-05, mechanical half — see this module's
/// `pair_errors_are_pairwise_distinct` test) plus [`Self::StoreBusy`] and
/// [`Self::UnknownInvite`], which are operator/CLI-internal rather than
/// part of the redeemer-facing seven. None of the seven names a term from
/// the jargon list `crates/famp/tests/pair_cli.rs`'s
/// `pair_errors_avoid_jargon` asserts against — no "public key", no
/// "fingerprint", no signature-scheme name, no trust-store file name, no
/// "base64". Whether a real non-expert can ACT on this wording is not
/// mechanically assertable; that closes only at Phase 20's UAT-02 (see
/// `18-03-SUMMARY.md`).
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    /// The five-word code the human typed does not parse: wrong word
    /// count, or a word not in the list. `reason` is retained on the
    /// variant for `Debug`/log-line detail but the `Display` text below is
    /// what a human sees, and is fixed regardless of `reason`'s content.
    #[error(
        "That does not look like a pairing code. A pairing code is exactly five lowercase \
         words separated by spaces. Check the message you were sent and type it again."
    )]
    CodeMalformed { reason: String },
    #[error("no pending invite")]
    NoPendingInvite,
    #[error(
        "Another famp command is using the pairing files right now. Wait a few seconds and \
         try again."
    )]
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
    /// PAIR-03's expiry clause.
    #[error("This code has expired. Codes last 24 hours. Ask the person who invited you to send a new one.")]
    Expired,
    #[error(
        "This code has already been used. If that was not you, tell the person who invited \
         you right away and ask them to run: famp pair revoke --all-pending"
    )]
    AlreadyRedeemed,
    /// PAIR-02's attempt-budget clause.
    #[error(
        "Too many wrong tries, so this code is now locked. Ask the person who invited you to \
         send a new one."
    )]
    AttemptsExhausted,
    #[error(
        "That code did not match. Check for a typo, then try again. If you run out of tries, \
         ask the person who invited you to send a new code."
    )]
    WrongCode,
    /// The redeemer's gateway could not be reached over the network — a
    /// transport-level failure, not a rejection from the endpoint.
    #[error(
        "Could not reach {url}. Check that you copied the address exactly, then ask the \
         person who invited you whether their FAMP gateway is running."
    )]
    GatewayUnreachable { url: String },
    /// The redeemer's own domain equals the inviter's own domain — the
    /// endpoint's `own_domain_refused` rejection (T-18-07).
    #[error(
        "This code cannot be redeemed on the same machine that created it. Run this on the \
         machine you want to connect."
    )]
    SameMachineRefusal,
    // Operator-facing (revoke by id, an internal lookup miss) — not one of
    // the seven redeemer-facing failure modes.
    #[error("unknown invite: {id}")]
    UnknownInvite { id: String },
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

/// Map a reject reason slug to its operator-facing [`PairingError`].
///
/// `reason` is the wire vocabulary
/// `crates/famp-gateway/src/pairing_ingress.rs::reject_status` matches
/// on; the returned value's `Display` is the operator-facing message for
/// it. `crates/famp/src/cli/pair/redeem.rs` is the sole production
/// caller.
///
/// Five reasons map 1:1 onto five of the seven redeemer-facing failure
/// modes (`code_mismatch`, `expired`, `already_redeemed`,
/// `attempts_exhausted`, `own_domain_refused`); the sixth and seventh
/// (malformed code, gateway unreachable) never reach this function — the
/// former is caught client-side by `parse_code` before any network call,
/// the latter is a transport failure, not an HTTP rejection. Any reason
/// this endpoint can return that isn't one of those five
/// (`no_pending_invite`, `invalid_signature`, `internal_error`, or an
/// unrecognized string) falls back to the wrong-code message: T-18-09
/// already groups `no_pending_invite` and `code_mismatch` under the same
/// oracle-avoidance HTTP status, and no separate non-jargon wording exists
/// for "no invite is currently outstanding" that isn't itself the
/// wrong-code message in different words.
#[must_use]
pub fn reject_reason_to_pairing_error(reason: &str) -> PairingError {
    match reason {
        "expired" => PairingError::Expired,
        "already_redeemed" => PairingError::AlreadyRedeemed,
        "attempts_exhausted" => PairingError::AttemptsExhausted,
        "own_domain_refused" => PairingError::SameMachineRefusal,
        // "code_mismatch" and every other/unrecognized reason.
        _ => PairingError::WrongCode,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::PairingError;

    /// The seven redeemer-facing failure modes (PAIR-05's mechanical
    /// half), in the order `.planning/REQUIREMENTS.md`'s `18-03-PLAN.md`
    /// lists them.
    fn seven_failure_messages() -> [String; 7] {
        [
            PairingError::CodeMalformed {
                reason: "any".to_string(),
            }
            .to_string(),
            PairingError::WrongCode.to_string(),
            PairingError::Expired.to_string(),
            PairingError::AlreadyRedeemed.to_string(),
            PairingError::AttemptsExhausted.to_string(),
            PairingError::GatewayUnreachable {
                url: "https://gateway.example.test:8443".to_string(),
            }
            .to_string(),
            PairingError::SameMachineRefusal.to_string(),
        ]
    }

    #[test]
    fn pair_errors_are_pairwise_distinct() {
        let messages = seven_failure_messages();
        for i in 0..messages.len() {
            for j in 0..messages.len() {
                if i == j {
                    continue;
                }
                assert_ne!(
                    messages[i], messages[j],
                    "messages at index {i} and {j} must differ: {messages:?}",
                );
            }
        }
    }

    #[test]
    fn each_failure_message_names_an_imperative_next_action() {
        // A cheap proxy for "names an imperative next-action clause":
        // every message contains at least one of the verbs the plan's own
        // seven exact texts use to tell the human what to do next.
        let verbs = ["Ask", "Check", "Run", "try again", "type it again", "tell"];
        for message in seven_failure_messages() {
            assert!(
                verbs.iter().any(|v| message.contains(v)),
                "message has no imperative next-action clause: {message}"
            );
        }
    }
}
