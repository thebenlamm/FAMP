//! Compile-checked exhaustive match over every `ProtocolErrorKind` and
//! `AuthorityScope` variant. Adding a variant without updating the match arms
//! is a hard compile error — this is the promise from Phase 3 success
//! criteria #3 and #5.
#![deny(unreachable_patterns)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::{
    invariants::INV_10, ArtifactId, AuthorityScope, Instance, MessageId, Principal,
    ProtocolErrorKind,
};
use std::str::FromStr;

const fn describe_error(kind: ProtocolErrorKind) -> &'static str {
    match kind {
        ProtocolErrorKind::Malformed => "malformed",
        ProtocolErrorKind::Unsupported => "unsupported",
        ProtocolErrorKind::Unauthorized => "unauthorized",
        ProtocolErrorKind::Stale => "stale",
        ProtocolErrorKind::Duplicate => "duplicate",
        ProtocolErrorKind::Orphaned => "orphaned",
        ProtocolErrorKind::OutOfScope => "out_of_scope",
        ProtocolErrorKind::CapacityExceeded => "capacity_exceeded",
        ProtocolErrorKind::PolicyBlocked => "policy_blocked",
        ProtocolErrorKind::CommitmentMissing => "commitment_missing",
        ProtocolErrorKind::DelegationForbidden => "delegation_forbidden",
        ProtocolErrorKind::ProvenanceIncomplete => "provenance_incomplete",
        ProtocolErrorKind::Conflict => "conflict",
        ProtocolErrorKind::ConditionFailed => "condition_failed",
        ProtocolErrorKind::Expired => "expired",
    }
}

const fn describe_scope(scope: AuthorityScope) -> &'static str {
    match scope {
        AuthorityScope::Advisory => "advisory",
        AuthorityScope::Negotiate => "negotiate",
        AuthorityScope::CommitLocal => "commit_local",
        AuthorityScope::CommitDelegate => "commit_delegate",
        AuthorityScope::Transfer => "transfer",
    }
}

const ALL_ERROR_KINDS: [ProtocolErrorKind; 15] = [
    ProtocolErrorKind::Malformed,
    ProtocolErrorKind::Unsupported,
    ProtocolErrorKind::Unauthorized,
    ProtocolErrorKind::Stale,
    ProtocolErrorKind::Duplicate,
    ProtocolErrorKind::Orphaned,
    ProtocolErrorKind::OutOfScope,
    ProtocolErrorKind::CapacityExceeded,
    ProtocolErrorKind::PolicyBlocked,
    ProtocolErrorKind::CommitmentMissing,
    ProtocolErrorKind::DelegationForbidden,
    ProtocolErrorKind::ProvenanceIncomplete,
    ProtocolErrorKind::Conflict,
    ProtocolErrorKind::ConditionFailed,
    ProtocolErrorKind::Expired,
];

const ALL_SCOPES: [AuthorityScope; 5] = [
    AuthorityScope::Advisory,
    AuthorityScope::Negotiate,
    AuthorityScope::CommitLocal,
    AuthorityScope::CommitDelegate,
    AuthorityScope::Transfer,
];

#[test]
fn describe_error_handles_all_fifteen_kinds() {
    for kind in ALL_ERROR_KINDS {
        let s = describe_error(kind);
        assert!(!s.is_empty(), "empty description for {kind:?}");
    }
}

#[test]
fn describe_scope_handles_all_five_scopes() {
    for scope in ALL_SCOPES {
        let s = describe_scope(scope);
        assert!(!s.is_empty(), "empty description for {scope:?}");
    }
}

#[test]
fn phase_3_surface_smoke_test() {
    // Exercise the full Phase 3 public API surface in a single compile
    // unit to prove crate-level re-exports resolve cleanly.
    let principal = Principal::from_str("agent:example.com/alice").unwrap();
    let instance = Instance::from_str("agent:example.com/alice#node-1").unwrap();
    let message_id = MessageId::new_v7();
    let artifact = ArtifactId::from_str(
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    )
    .unwrap();
    let kind = ProtocolErrorKind::Malformed;
    let scope = AuthorityScope::CommitLocal;

    // Round-trip back through Display to ensure the types render.
    assert_eq!(principal.to_string(), "agent:example.com/alice");
    assert!(instance.to_string().ends_with("#node-1"));
    assert!(!message_id.as_uuid().is_nil());
    assert!(artifact.as_str().starts_with("sha256:"));
    assert_eq!(describe_error(kind), "malformed");
    assert_eq!(describe_scope(scope), "commit_local");
    assert_eq!(INV_10, "INV-10");
}
