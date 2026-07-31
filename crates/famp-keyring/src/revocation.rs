//! Signed revocation statements (REVK-02) — defense-in-depth on top of
//! REVK-01 expiry (D-06/D-07).
//!
//! See
//! `.planning/phases/15-keyring-multi-key-extension-revocation/15-DECISIONS.md`
//! (D15-B) for the authorized-signer rule [`authorized_signer_for`]
//! encodes, and [`crate::error::KeyringError`] for the three fresh error
//! variants this module produces.

use famp_core::Principal;
use famp_crypto::{sign_value, FampSigningKey, TrustedVerifyingKey};

use crate::entry::KeyState;
use crate::error::KeyringError;
use crate::Keyring;

/// The unsigned statement content: "key `revoked_key_id` for `principal` is
/// revoked as of `revoked_at`, for reason `reason`".
///
/// `#[serde(deny_unknown_fields)]`: the Phase 14 objection to
/// `deny_unknown_fields` (`.planning/phases/14-inbound-content-is-data-quarantine/14-CONTEXT.md`)
/// was about additive forward-compat on a WIRE type that must keep working
/// across versions as fields are added. That reasoning does not transfer
/// here: this struct is SIGNED (`RevocationStatement::sign` ->
/// `famp_crypto::sign_value`, RFC 8785 JCS canonicalization) — any additive
/// field, forward-compat or not, already changes the canonicalized bytes
/// and invalidates the signature the moment a producer and consumer's
/// schemas diverge. Rejecting an unknown field loudly at parse time beats
/// silently dropping it and then failing with a confusing
/// signature-verification error three steps downstream.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationStatement {
    pub principal: String,
    pub revoked_key_id: String,
    pub revoked_at: String,
    pub reason: Option<String>,
}

impl RevocationStatement {
    /// Sign this statement, producing a [`SignedRevocation`] ready for
    /// [`SignedRevocation::to_blob`]. Delegates entirely to
    /// `famp_crypto::sign_value` — RFC 8785 JCS canonicalization and the
    /// `FAMP-sig-v1\0` domain prefix are both handled there; no second
    /// signing path is introduced.
    pub fn sign(self, sk: &FampSigningKey) -> Result<SignedRevocation, KeyringError> {
        let sig = sign_value(sk, &self)?;
        Ok(SignedRevocation {
            statement: self,
            sig: sig.to_b64url(),
        })
    }
}

/// A [`RevocationStatement`] plus its detached Ed25519 signature
/// (b64url-encoded `FampSignature`) — the unit that travels over the same
/// out-of-band channel as a `famp peer export` blob.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRevocation {
    pub statement: RevocationStatement,
    pub sig: String,
}

impl SignedRevocation {
    /// Serialize to a single-line JSON blob, mirroring how the `famp peer
    /// export` blob travels out-of-band.
    pub fn to_blob(&self) -> Result<String, KeyringError> {
        serde_json::to_string(self).map_err(|e| KeyringError::RevocationBlobMalformed {
            reason: e.to_string(),
        })
    }

    /// Parse a blob produced by [`SignedRevocation::to_blob`]. An unknown
    /// field anywhere in the structure is a parse error
    /// (`#[serde(deny_unknown_fields)]` on both this type and
    /// [`RevocationStatement`]), never a silently dropped field.
    pub fn from_blob(s: &str) -> Result<Self, KeyringError> {
        serde_json::from_str(s).map_err(|e| KeyringError::RevocationBlobMalformed {
            reason: e.to_string(),
        })
    }
}

/// The authorized-signer rule for a revocation statement (D15-B).
///
/// **Selected option: `signer-self-allowed`** (`15-DECISIONS.md` D15-B).
/// Candidates are the principal's `Active` entry (if any) PLUS the entry
/// whose `key_id()` equals `revoked_key_id`, provided that entry is not
/// itself already `Revoked` — i.e. self-revocation is permitted. A
/// `Revoked` entry is NEVER a candidate under either option
/// (`signer-different-active` or `signer-self-allowed`).
///
/// This is deliberately the single function encoding the rule, so a later
/// flip to `signer-different-active` (candidates = the `Active` entry ONLY
/// when its `key_id()` differs from `revoked_key_id`) is a one-function
/// change.
///
/// **Accepted rationale for `signer-self-allowed`** (orchestrator's
/// decision message, famp-lead-730, 2026-07-31 — recorded here IN FULL per
/// this phase's Task 2 instruction, not summarized away, because a future
/// reader who sees "a compromised key can sign its own death warrant"
/// without this context will assume it is a bug):
///
/// 1. **Precedent:** this matches the standing PGP convention — revocation
///    certificates are self-signed with the key being revoked, for exactly
///    this no-CA bilateral case. FAMP's keyring trust model has no CA
///    either.
/// 2. **Dominated threat:** the admitted DoS (an attacker holding a
///    compromised key pre-emptively revokes it to sever the channel before
///    detection) is strictly dominated by the capability that attacker
///    already holds — full impersonation of that peer. Severing the
///    channel is strictly less damage than silently speaking as them.
/// 3. **Dead-code avoidance:** disallowing self-sign
///    (`signer-different-active`) makes REVK-02 dead code in its primary
///    scenario — a peer whose SOLE key is compromised, the overwhelmingly
///    common case given the keyring was single-key-per-principal before
///    this phase. A peer with only one compromised key could never
///    produce a verifiable revocation under that rule.
///
/// **REVK-01 (expiry) remains the PRIMARY revocation mechanism regardless
/// of this rule** — self-revocation under D15-B does not change that
/// ordering (D-06).
#[must_use]
pub fn authorized_signer_for<'a>(
    keyring: &'a Keyring,
    principal: &Principal,
    revoked_key_id: &str,
) -> Vec<&'a TrustedVerifyingKey> {
    keyring
        .entries(principal)
        .iter()
        .filter(|e| {
            e.state() != KeyState::Revoked
                && (e.state() == KeyState::Active || e.key_id() == revoked_key_id)
        })
        .map(super::entry::KeyEntry::key)
        .collect()
}
