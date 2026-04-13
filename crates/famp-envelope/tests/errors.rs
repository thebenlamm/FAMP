//! Plan 01-01 Task 3 — `EnvelopeDecodeError` → `ProtocolError` mapping tests.

#![allow(clippy::unwrap_used)]

use famp_canonical as _;
use famp_core::{ProtocolError, ProtocolErrorKind};
use famp_crypto as _;
use famp_envelope::EnvelopeDecodeError;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use serde_json as _;
use thiserror as _;

#[test]
fn missing_signature_maps_to_unauthorized() {
    let err = EnvelopeDecodeError::MissingSignature;
    let proto: ProtocolError = err.into();
    assert_eq!(proto.kind, ProtocolErrorKind::Unauthorized);
}

#[test]
fn unsupported_version_maps_to_unsupported() {
    let err = EnvelopeDecodeError::UnsupportedVersion {
        found: "0.6.0".into(),
    };
    let proto: ProtocolError = err.into();
    assert_eq!(proto.kind, ProtocolErrorKind::Unsupported);
}

#[test]
fn missing_field_maps_to_malformed() {
    let err = EnvelopeDecodeError::MissingField { field: "to" };
    let proto: ProtocolError = err.into();
    assert_eq!(proto.kind, ProtocolErrorKind::Malformed);
}

#[test]
fn error_impls_std_error() {
    // Compile-time check: EnvelopeDecodeError must implement std::error::Error.
    fn assert_err<E: std::error::Error>() {}
    assert_err::<EnvelopeDecodeError>();

    // Display goes through thiserror's `#[error(...)]` format strings.
    let e = EnvelopeDecodeError::MissingField { field: "ts" };
    assert!(format!("{e}").contains("ts"));
}

#[test]
fn no_variant_maps_to_other_or_unmapped() {
    // Sample every declared variant and assert mapping lands in exactly the
    // three sanctioned kinds — Unauthorized, Unsupported, or Malformed. Any
    // future variant added without a From arm will fail to compile because
    // the match in `From<EnvelopeDecodeError> for ProtocolError` is
    // exhaustive, so this test catches "variant present but forgotten".
    use famp_envelope::{EnvelopeScope, MessageClass};

    let samples: Vec<EnvelopeDecodeError> = vec![
        EnvelopeDecodeError::MissingField { field: "x" },
        EnvelopeDecodeError::UnknownEnvelopeField { field: "x".into() },
        EnvelopeDecodeError::UnknownBodyField {
            class: MessageClass::Ack,
            field: "x".into(),
        },
        EnvelopeDecodeError::UnsupportedVersion { found: "0.9".into() },
        EnvelopeDecodeError::UnknownClass { found: "x".into() },
        EnvelopeDecodeError::ClassMismatch {
            expected: MessageClass::Request,
            got: MessageClass::Ack,
        },
        EnvelopeDecodeError::ScopeMismatch {
            class: MessageClass::Request,
            expected: EnvelopeScope::Standalone,
            got: EnvelopeScope::Task,
        },
        EnvelopeDecodeError::MissingSignature,
        EnvelopeDecodeError::SignatureInvalid,
        EnvelopeDecodeError::InvalidControlAction {
            found: "supersede".into(),
        },
        EnvelopeDecodeError::InterimWithTerminalStatus,
        EnvelopeDecodeError::TerminalWithoutStatus,
        EnvelopeDecodeError::MissingErrorDetail,
        EnvelopeDecodeError::MissingProvenance,
        EnvelopeDecodeError::InsufficientBounds { count: 1 },
        EnvelopeDecodeError::BodyValidation("x".into()),
    ];
    for e in samples {
        let proto: ProtocolError = e.into();
        assert!(matches!(
            proto.kind,
            ProtocolErrorKind::Unauthorized
                | ProtocolErrorKind::Unsupported
                | ProtocolErrorKind::Malformed
        ));
    }
}
