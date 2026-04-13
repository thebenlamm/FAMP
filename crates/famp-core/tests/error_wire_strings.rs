//! Wire-string fixture gate for `ProtocolErrorKind`.
//!
//! Locks every variant's serde wire form against spec §15.1. A rename in the
//! enum that would change an on-the-wire string is a compile-checked failure.
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::{ProtocolError, ProtocolErrorKind};

const FIXTURE: &[(ProtocolErrorKind, &str)] = &[
    (ProtocolErrorKind::Malformed, "malformed"),
    (ProtocolErrorKind::Unsupported, "unsupported"),
    (ProtocolErrorKind::Unauthorized, "unauthorized"),
    (ProtocolErrorKind::Stale, "stale"),
    (ProtocolErrorKind::Duplicate, "duplicate"),
    (ProtocolErrorKind::Orphaned, "orphaned"),
    (ProtocolErrorKind::OutOfScope, "out_of_scope"),
    (ProtocolErrorKind::CapacityExceeded, "capacity_exceeded"),
    (ProtocolErrorKind::PolicyBlocked, "policy_blocked"),
    (ProtocolErrorKind::CommitmentMissing, "commitment_missing"),
    (ProtocolErrorKind::DelegationForbidden, "delegation_forbidden"),
    (ProtocolErrorKind::ProvenanceIncomplete, "provenance_incomplete"),
    (ProtocolErrorKind::Conflict, "conflict"),
    (ProtocolErrorKind::ConditionFailed, "condition_failed"),
    (ProtocolErrorKind::Expired, "expired"),
];

#[test]
fn fixture_has_fifteen_entries() {
    assert_eq!(FIXTURE.len(), 15);
}

#[test]
fn serialize_each_variant_to_wire_string() {
    for (kind, wire) in FIXTURE {
        let json = serde_json::to_value(kind).unwrap();
        assert_eq!(json, serde_json::json!(wire), "serialize {kind:?}");
    }
}

#[test]
fn deserialize_each_wire_string_to_variant() {
    for (kind, wire) in FIXTURE {
        let json = serde_json::json!(wire);
        let parsed: ProtocolErrorKind = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, *kind, "deserialize {wire}");
    }
}

#[test]
fn display_matches_wire_string() {
    for (kind, wire) in FIXTURE {
        assert_eq!(kind.to_string(), *wire);
    }
}

#[test]
fn unknown_variant_rejected() {
    let result: Result<ProtocolErrorKind, _> =
        serde_json::from_str(r#""invented_kind""#);
    assert!(result.is_err());
}

#[test]
fn protocol_error_wrapper_impls_error_trait() {
    let err = ProtocolError::new(ProtocolErrorKind::Unauthorized);
    // Sanity: source() is callable; wrapper is a std::error::Error.
    let _source: Option<&(dyn std::error::Error + 'static)> =
        std::error::Error::source(&err);
    let with_detail =
        ProtocolError::with_detail(ProtocolErrorKind::Malformed, "bad envelope");
    assert_eq!(with_detail.kind, ProtocolErrorKind::Malformed);
    assert_eq!(with_detail.detail.as_deref(), Some("bad envelope"));
}
