//! Single-iteration runtime body.
//!
//! Sequence (D-D3 + D-D5 + Pitfall 2):
//!   1. `peek_sender` — extract `from` from raw wire bytes.
//!   2. keyring lookup — `get(&sender)` or fail `UnknownSender`.
//!   3. **Canonical pre-check** — re-canonicalize wire bytes via
//!      `famp_canonical::canonicalize`; if `canonical != wire_bytes`,
//!      return `CanonicalDivergence` BEFORE decode runs. This is the
//!      load-bearing CONF-06 vs CONF-07 distinction.
//!   4. `AnySignedEnvelope::decode(bytes, pinned)` — signature verification
//!      happens inside; map error -> `RuntimeError::Decode`.
//!   5. Recipient cross-check — `envelope_recipient(&env) == msg.recipient`
//!      or fail `RecipientMismatch`.
//!   6. If class != Ack: call `fsm_input_from_envelope` -> `task_fsm.step`.
//!      If class == Ack: skip FSM (D-D4).

use crate::runtime::{
    adapter::{envelope_recipient, fsm_input_from_envelope},
    error::RuntimeError,
    peek::peek_sender,
};
use famp_canonical::{canonicalize, from_slice_strict};
use famp_envelope::{AnySignedEnvelope, EnvelopeDecodeError};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::TransportMessage;

/// Process a single received `TransportMessage` end-to-end.
///
/// Returns the decoded envelope on success. Every error path returns a
/// distinct typed [`RuntimeError`] variant — no generic `Other`, no
/// panics, no silent drops.
pub fn process_one_message(
    msg: &TransportMessage,
    keyring: &Keyring,
    task_fsm: &mut TaskFsm,
) -> Result<AnySignedEnvelope, RuntimeError> {
    // Step 1: peek sender (strict-parse; rejects duplicate keys).
    let sender = peek_sender(&msg.bytes)?;

    // Step 2: keyring lookup.
    let pinned = keyring
        .get(&sender)
        .ok_or_else(|| RuntimeError::UnknownSender(sender.clone()))?;

    // Step 3: canonical pre-check (CONF-07). Re-parse the wire bytes,
    // re-canonicalize, compare byte-wise to the original wire bytes. If the
    // producer did not emit RFC 8785 canonical bytes, reject BEFORE the
    // signature verifier runs — otherwise CONF-06 (bad signature) and
    // CONF-07 (non-canonical bytes) would be indistinguishable.
    let parsed: serde_json::Value = from_slice_strict(&msg.bytes)
        .map_err(|e| RuntimeError::Decode(EnvelopeDecodeError::MalformedJson(e)))?;
    let re_canonical = canonicalize(&parsed)
        .map_err(|e| RuntimeError::Decode(EnvelopeDecodeError::MalformedJson(e)))?;
    if re_canonical != msg.bytes {
        return Err(RuntimeError::CanonicalDivergence);
    }

    // Step 4: decode (signature verification happens here).
    let env = AnySignedEnvelope::decode(&msg.bytes, pinned).map_err(RuntimeError::Decode)?;

    // Step 5: recipient cross-check (D-D5).
    let env_to = envelope_recipient(&env);
    if env_to != &msg.recipient {
        return Err(RuntimeError::RecipientMismatch {
            transport: msg.recipient.clone(),
            envelope: env_to.clone(),
        });
    }

    // Step 6: FSM step (ack is wire-only per D-D4).
    if let Some(input) = fsm_input_from_envelope(&env) {
        task_fsm.step(input).map_err(RuntimeError::Fsm)?;
    }

    Ok(env)
}
