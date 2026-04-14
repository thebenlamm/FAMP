//! Smoke tests for Plan 01-01 Task 2 primitive types.
//!
//! Verifies byte-stable wire round-trip for `MessageClass`, `EnvelopeScope`,
//! and `Timestamp`. Version-string handling is covered end-to-end by
//! `tests/adversarial.rs::tampered_famp_version_rejected_*`.

#![allow(clippy::unwrap_used)]

use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope::{EnvelopeScope, MessageClass, Timestamp};
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use thiserror as _;

#[test]
fn message_class_snake_case_roundtrip() {
    let cases = [
        ("\"request\"", MessageClass::Request),
        ("\"commit\"", MessageClass::Commit),
        ("\"deliver\"", MessageClass::Deliver),
        ("\"ack\"", MessageClass::Ack),
        ("\"control\"", MessageClass::Control),
    ];
    for (wire, variant) in cases {
        let decoded: MessageClass = serde_json::from_str(wire).unwrap();
        assert_eq!(decoded, variant);
        let re = serde_json::to_string(&variant).unwrap();
        assert_eq!(re, wire);
    }
    assert!(serde_json::from_str::<MessageClass>("\"propose\"").is_err());
    assert!(serde_json::from_str::<MessageClass>("\"Request\"").is_err());
}

#[test]
fn envelope_scope_snake_case_roundtrip() {
    let cases = [
        ("\"standalone\"", EnvelopeScope::Standalone),
        ("\"conversation\"", EnvelopeScope::Conversation),
        ("\"task\"", EnvelopeScope::Task),
    ];
    for (wire, variant) in cases {
        let decoded: EnvelopeScope = serde_json::from_str(wire).unwrap();
        assert_eq!(decoded, variant);
        let re = serde_json::to_string(&variant).unwrap();
        assert_eq!(re, wire);
    }
    assert!(serde_json::from_str::<EnvelopeScope>("\"global\"").is_err());
}

#[test]
fn timestamp_preserves_bytes() {
    let wire = "\"2026-04-13T00:00:00Z\"";
    let ts: Timestamp = serde_json::from_str(wire).unwrap();
    assert_eq!(ts.0, "2026-04-13T00:00:00Z");
    let re = serde_json::to_string(&ts).unwrap();
    assert_eq!(re, wire);

    // Offset form preserved byte-for-byte.
    let wire2 = "\"2026-04-13T12:34:56+05:30\"";
    let ts2: Timestamp = serde_json::from_str(wire2).unwrap();
    assert_eq!(serde_json::to_string(&ts2).unwrap(), wire2);

    // Malformed rejected.
    assert!(serde_json::from_str::<Timestamp>("\"2026/04/13\"").is_err());
    assert!(serde_json::from_str::<Timestamp>("\"short\"").is_err());
}
