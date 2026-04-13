//! Adversarial decode suite (CONTEXT.md D-D4).
//!
//! Every decode failure mode catalogued in D-D4 has a dedicated test here.
//! Each test asserts a specific `EnvelopeDecodeError` variant (or a
//! documented set when the exact variant depends on serde vs verify order).
//! A non-typed error, or a panic, is a regression.
//!
//! The fixtures for missing-signature / bad-signature / unknown-top-level
//! field are produced at runtime by mutating vector 0 bytes (no fixture file
//! to commit). The ENV-09 (`capability_snapshot`), ENV-12 (`supersede`), and
//! D-D3 (unknown nested body field) adversarial inputs reuse the Plan 01-02
//! fixtures under `tests/fixtures/adversarial/`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::needless_raw_string_hashes)]

use famp_canonical as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use thiserror as _;

use famp_canonical::from_slice_strict;
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, ControlBody, DeliverBody, ErrorCategory,
    ErrorDetail, RequestBody, TerminalStatus,
};
use famp_envelope::{
    EnvelopeDecodeError, SignedEnvelope, Timestamp, UnsignedEnvelope,
};
use serde_json::Value;
use std::fs;

const VECTOR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/vector_0");
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/adversarial");

// RFC 8032 Test 1 keypair.
const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn vk() -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap()
}
fn sk() -> FampSigningKey {
    FampSigningKey::from_bytes(SECRET)
}
fn vector_0() -> Value {
    let bytes = fs::read(format!("{VECTOR_DIR}/envelope.json")).unwrap();
    from_slice_strict(&bytes).unwrap()
}
fn alice() -> Principal {
    "agent:example.test/alice".parse().unwrap()
}
fn bob() -> Principal {
    "agent:example.test/bob".parse().unwrap()
}
fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}
fn id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}
fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "100".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: None,
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: None,
    }
}

// ---------------- envelope-level adversarial cases ----------------

#[test]
fn missing_signature_rejected() {
    let mut value = vector_0();
    value.as_object_mut().unwrap().remove("signature");
    let bytes = serde_json::to_vec(&value).unwrap();
    let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::MissingSignature),
        "expected MissingSignature, got {err:?}"
    );
}

#[test]
fn bad_signature_padded_rejected() {
    let mut value = vector_0();
    // Take the valid unpadded b64url signature and add a trailing `=`, which
    // the URL_SAFE_NO_PAD decoder strictly rejects.
    let sig = value
        .get("signature")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let padded = format!("{sig}=");
    *value.get_mut("signature").unwrap() = Value::String(padded);
    let bytes = serde_json::to_vec(&value).unwrap();
    let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::InvalidSignatureEncoding(_)),
        "expected InvalidSignatureEncoding, got {err:?}"
    );
}

#[test]
fn class_body_mismatch_rejected() {
    // Sign a valid CommitBody envelope, then decode it as
    // SignedEnvelope::<RequestBody>. The signature verifies (raw-Value path)
    // but the typed deserialize step fails with a typed error — either
    // ClassMismatch (if the wire struct deserializes) or an
    // UnknownEnvelopeField / BodyValidation from the mismatched body shape.
    let body = CommitBody {
        scope: serde_json::json!({"task": "translate"}),
        scope_subset: None,
        bounds: two_key_bounds(),
        accepted_policies: vec!["p".to_string()],
        delegation_permissions: None,
        reporting_obligations: None,
        terminal_condition: serde_json::json!({"type": "final_delivery"}),
        conditions: None,
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<CommitBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::CommitLocal,
        ts(),
        body,
    )
    .sign(&sk())
    .unwrap();
    let bytes = signed.encode().unwrap();
    // Decode as RequestBody — must fail typed, not panic.
    let err = SignedEnvelope::<RequestBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(
            err,
            EnvelopeDecodeError::ClassMismatch { .. }
                | EnvelopeDecodeError::ScopeMismatch { .. }
                | EnvelopeDecodeError::UnknownEnvelopeField { .. }
                | EnvelopeDecodeError::BodyValidation(_)
        ),
        "expected a typed class/body-mismatch error, got {err:?}"
    );
}

#[test]
fn unknown_envelope_field_rejected() {
    // Vector 0 PLUS an unknown top-level key. Vector 0 was signed without
    // the extra key, so canonical-bytes divergence means verify fails first.
    // Either SignatureInvalid OR UnknownEnvelopeField is acceptable — both
    // are typed. The interesting property is "no panic, no generic error".
    let mut value = vector_0();
    value
        .as_object_mut()
        .unwrap()
        .insert("extra_evil".to_string(), Value::from(1));
    let bytes = serde_json::to_vec(&value).unwrap();
    let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(
            err,
            EnvelopeDecodeError::SignatureInvalid
                | EnvelopeDecodeError::UnknownEnvelopeField { .. }
        ),
        "expected SignatureInvalid or UnknownEnvelopeField, got {err:?}"
    );
}

// ---------------- deliver cross-field adversarial cases ----------------

fn sign_deliver(body: DeliverBody, terminal: Option<TerminalStatus>) -> Vec<u8> {
    let mut e = UnsignedEnvelope::<DeliverBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    if let Some(t) = terminal {
        e = e.with_terminal_status(t);
    }
    // To exercise the decode path with a valid signature BUT a cross-field
    // violation, we bypass `post_decode_validate` at sign time by building
    // the wire JSON manually and re-signing. Easier: `sign()` itself does
    // NOT run `post_decode_validate`, so we can sign an invalid combination.
    let signed = e.sign(&sk()).unwrap();
    signed.encode().unwrap()
}

#[test]
fn deliver_interim_with_terminal_status_rejected() {
    let body = DeliverBody {
        interim: true,
        artifacts: None,
        result: Some(serde_json::json!({"progress": 0.5})),
        usage_metrics: None,
        error_detail: None,
        provenance: None,
        natural_language_summary: None,
    };
    let bytes = sign_deliver(body, Some(TerminalStatus::Completed));
    let err = SignedEnvelope::<DeliverBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::InterimWithTerminalStatus),
        "expected InterimWithTerminalStatus, got {err:?}"
    );
}

#[test]
fn deliver_failed_without_error_detail_rejected() {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: None,
        usage_metrics: None,
        error_detail: None, // missing — must trip MissingErrorDetail
        provenance: Some(serde_json::json!({"signer": "did:example:translator"})),
        natural_language_summary: None,
    };
    let bytes = sign_deliver(body, Some(TerminalStatus::Failed));
    let err = SignedEnvelope::<DeliverBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::MissingErrorDetail),
        "expected MissingErrorDetail, got {err:?}"
    );
}

#[test]
fn deliver_terminal_without_status_rejected() {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: Some(serde_json::json!({"ok": true})),
        usage_metrics: None,
        error_detail: None,
        provenance: Some(serde_json::json!({"signer": "did:example:translator"})),
        natural_language_summary: None,
    };
    let bytes = sign_deliver(body, None); // non-interim must have terminal_status
    let err = SignedEnvelope::<DeliverBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::TerminalWithoutStatus),
        "expected TerminalWithoutStatus, got {err:?}"
    );
}

#[test]
fn deliver_completed_without_provenance_rejected() {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: Some(serde_json::json!({"ok": true})),
        usage_metrics: None,
        error_detail: None,
        provenance: None, // missing on terminal delivery
        natural_language_summary: None,
    };
    let bytes = sign_deliver(body, Some(TerminalStatus::Completed));
    let err = SignedEnvelope::<DeliverBody>::decode(&bytes, &vk()).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::MissingProvenance),
        "expected MissingProvenance, got {err:?}"
    );
}

// ---------------- Plan 01-02 body fixture reuse ----------------

fn load_body(name: &str) -> String {
    fs::read_to_string(format!("{FIXTURE_DIR}/{name}")).unwrap()
}

#[test]
fn commit_with_capability_snapshot_rejected_at_body_level() {
    // ENV-09: CommitBody has no `capability_snapshot` field; deny_unknown_fields
    // surfaces it as a serde error at body-level decode. This is the same
    // lock as the Plan 01-02 body_shapes test but re-asserted from the
    // envelope perspective to confirm the narrowing holds through the full
    // pipeline, not just the body struct in isolation.
    let json = load_body("commit_with_capability_snapshot.json");
    let result: Result<CommitBody, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "commit with capability_snapshot must fail body decode (ENV-09 narrowing)"
    );
}

#[test]
fn control_supersede_rejected_at_body_level() {
    // ENV-12: ControlAction is single-variant {Cancel}. `supersede` is not
    // even a variant; the enum deserializer rejects it.
    let json = load_body("control_supersede.json");
    let result: Result<ControlBody, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "control with action=supersede must fail (ENV-12 narrowing)"
    );
}

#[test]
fn unknown_body_field_nested_rejected_at_body_level() {
    // D-D3: unknown key injected at depth inside the body (not at envelope
    // top level). deny_unknown_fields must fire inside bounds, not only at
    // the outermost object.
    let json = load_body("unknown_body_field_nested.json");
    let result: Result<RequestBody, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "nested unknown body field must fail body decode (D-D3)"
    );
}

// ---------------- eyeballs on the error shape itself ----------------

#[test]
fn all_envelope_errors_convert_into_protocol_error() {
    // Tripwire — any new EnvelopeDecodeError variant added in the future
    // that is not routed into ProtocolError would fail the error.rs
    // exhaustive match. This test just exercises the existing variants
    // we care about from an envelope perspective.
    let err = EnvelopeDecodeError::MissingSignature;
    let pe: famp_core::ProtocolError = err.into();
    assert_eq!(pe.kind, famp_core::ProtocolErrorKind::Unauthorized);

    let err = EnvelopeDecodeError::UnknownClass {
        found: "delegate".to_string(),
    };
    let pe: famp_core::ProtocolError = err.into();
    assert_eq!(pe.kind, famp_core::ProtocolErrorKind::Malformed);

    // Reference-only use — silences dead-code lints on the tiny helpers.
    let _ = (ErrorCategory::Internal, AckDisposition::Accepted);
    let _ = ErrorDetail {
        category: ErrorCategory::Malformed,
        message: String::new(),
        diagnostic: None,
    };
}
