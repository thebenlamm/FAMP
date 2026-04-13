//! `EnvelopeDecodeError` — phase-local narrow error enum for envelope decode.
//!
//! Follows the v0.6 pattern (Plans 01-01 D-16, 02-01): a narrow typed enum
//! that converts into `famp_core::ProtocolError` at the crate boundary.
//!
//! The full variant list is shipped now (Plan 01-01 Task 3) so that Plans 02
//! and 03 can reference variants by name without editing `error.rs` again.
//! Body-level variants are `dead_code` until their Plan 02/03 call sites land.

#![allow(dead_code)] // variants wired progressively across Plans 02 and 03

use crate::{EnvelopeScope, MessageClass};
use famp_canonical::CanonicalError;
use famp_core::{ProtocolError, ProtocolErrorKind};
use famp_crypto::CryptoError;

/// Every typed failure mode the envelope decoder can produce.
///
/// Each adversarial case from CONTEXT.md D-D4 has a dedicated variant.
/// `From<EnvelopeDecodeError> for ProtocolError` is compile-time exhaustive
/// (no `_ =>` arm) and never routes to `ProtocolErrorKind::Other`.
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeDecodeError {
    #[error("malformed envelope JSON: {0}")]
    MalformedJson(#[from] CanonicalError),

    #[error("missing required envelope field: {field}")]
    MissingField { field: &'static str },

    #[error("unknown envelope field: {field}")]
    UnknownEnvelopeField { field: String },

    #[error("unknown body field at depth: {class}.{field}")]
    UnknownBodyField { class: MessageClass, field: String },

    #[error("envelope.famp = {found:?}; expected \"0.5.1\"")]
    UnsupportedVersion { found: String },

    #[error("envelope.class = {found:?} not a known message class")]
    UnknownClass { found: String },

    #[error("envelope.class = {got} does not match expected {expected}")]
    ClassMismatch {
        expected: MessageClass,
        got: MessageClass,
    },

    #[error("envelope.scope = {got} does not match expected {expected} for class {class}")]
    ScopeMismatch {
        class: MessageClass,
        expected: EnvelopeScope,
        got: EnvelopeScope,
    },

    #[error("envelope is unsigned — signature field absent")]
    MissingSignature,

    #[error("signature encoding malformed")]
    InvalidSignatureEncoding(#[from] CryptoError),

    #[error("signature verification failed (verify_strict)")]
    SignatureInvalid,

    #[error("control.action = {found:?}; v0.7 supports only `cancel`")]
    InvalidControlAction { found: String },

    #[error("deliver.interim = true but envelope.terminal_status is set")]
    InterimWithTerminalStatus,

    #[error("deliver.interim = false but envelope.terminal_status is absent")]
    TerminalWithoutStatus,

    #[error("deliver.error_detail required when terminal_status = failed")]
    MissingErrorDetail,

    #[error("deliver.provenance required on terminal delivery")]
    MissingProvenance,

    #[error("bounds requires ≥2 keys from §9.3 set; got {count}")]
    InsufficientBounds { count: usize },

    #[error("body field validation failed: {0}")]
    BodyValidation(String),
}

impl From<EnvelopeDecodeError> for ProtocolError {
    fn from(e: EnvelopeDecodeError) -> Self {
        // Exhaustive by design — adding a variant must be a compile error,
        // never a silent fallthrough into `ProtocolErrorKind::Malformed`.
        let kind = match &e {
            EnvelopeDecodeError::MissingSignature
            | EnvelopeDecodeError::InvalidSignatureEncoding(_)
            | EnvelopeDecodeError::SignatureInvalid => ProtocolErrorKind::Unauthorized,

            EnvelopeDecodeError::UnsupportedVersion { .. } => ProtocolErrorKind::Unsupported,

            EnvelopeDecodeError::MalformedJson(_)
            | EnvelopeDecodeError::MissingField { .. }
            | EnvelopeDecodeError::UnknownEnvelopeField { .. }
            | EnvelopeDecodeError::UnknownBodyField { .. }
            | EnvelopeDecodeError::UnknownClass { .. }
            | EnvelopeDecodeError::ClassMismatch { .. }
            | EnvelopeDecodeError::ScopeMismatch { .. }
            | EnvelopeDecodeError::InvalidControlAction { .. }
            | EnvelopeDecodeError::InterimWithTerminalStatus
            | EnvelopeDecodeError::TerminalWithoutStatus
            | EnvelopeDecodeError::MissingErrorDetail
            | EnvelopeDecodeError::MissingProvenance
            | EnvelopeDecodeError::InsufficientBounds { .. }
            | EnvelopeDecodeError::BodyValidation(_) => ProtocolErrorKind::Malformed,
        };
        Self::with_detail(kind, e.to_string())
    }
}
