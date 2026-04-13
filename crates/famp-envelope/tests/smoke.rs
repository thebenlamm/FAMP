//! Smoke tests for Plan 01-01 Task 2 primitive types.
//!
//! Verifies byte-stable wire round-trip for `FampVersion`, `MessageClass`,
//! `EnvelopeScope`, and `Timestamp`.

#![allow(clippy::unwrap_used)]

use famp_envelope::{EnvelopeScope, FampVersion, MessageClass, Timestamp};

#[test]
fn version_literal_roundtrip() {
    let s = serde_json::to_string(&FampVersion).unwrap();
    assert_eq!(s, "\"0.5.1\"");
    let back: FampVersion = serde_json::from_str("\"0.5.1\"").unwrap();
    assert_eq!(back, FampVersion);
}

#[test]
fn version_rejects_wrong_literal() {
    assert!(serde_json::from_str::<FampVersion>("\"0.5.2\"").is_err());
    assert!(serde_json::from_str::<FampVersion>("\"V0_5_1\"").is_err());
    assert!(serde_json::from_str::<FampVersion>("\"0.6.0\"").is_err());
}

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
