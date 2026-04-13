//! Phase-local narrow `RuntimeError`.
//!
//! Each adversarial case (CONF-05, -06, -07) bottoms out in a DISTINCT
//! variant so the adversarial test matrix can `matches!` each
//! independently — this is the load-bearing guarantee for D-D8.

use famp_core::Principal;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Sender principal is not in the keyring. Runtime rejects before
    /// decode so signature verification never sees an unpinned key.
    #[error("unknown sender: {0}")]
    UnknownSender(Principal),

    /// Envelope decode OR signature verification failed inside
    /// `AnySignedEnvelope::decode`. CONF-05 (unsigned -> `MissingSignature`)
    /// and CONF-06 (wrong-key -> `SignatureInvalid`) both surface here — the
    /// inner `EnvelopeDecodeError` discriminant distinguishes them.
    #[error("envelope decode error")]
    Decode(#[source] famp_envelope::EnvelopeDecodeError),

    /// CONF-07: wire bytes did not equal `canonicalize(parsed_wire_bytes)`.
    /// Detected by the pre-decode canonical re-check in `process_one_message`
    /// — distinct from `Decode(SignatureInvalid)` because it is emitted
    /// BEFORE signature verification runs.
    #[error("canonicalization divergence detected")]
    CanonicalDivergence,

    /// Transport-layer recipient does not match the envelope's `to` field
    /// (D-D5). Prevents the keyring from degrading to "any valid key
    /// accepted anywhere."
    #[error("transport recipient {transport} does not match envelope recipient {envelope}")]
    RecipientMismatch {
        transport: Principal,
        envelope: Principal,
    },

    /// Opaque wrapper for the concrete transport's error type. Boxed to
    /// avoid a generic parameter leaking into `RuntimeError`.
    #[error("transport error")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("keyring error")]
    Keyring(#[source] famp_keyring::KeyringError),

    #[error("fsm error")]
    Fsm(#[source] famp_fsm::TaskFsmError),
}
