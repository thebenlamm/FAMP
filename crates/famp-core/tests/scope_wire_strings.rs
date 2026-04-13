//! Wire-string fixture gate for `AuthorityScope` (spec §5.3).
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::AuthorityScope;
use std::str::FromStr;

const FIXTURE: &[(AuthorityScope, &str)] = &[
    (AuthorityScope::Advisory, "advisory"),
    (AuthorityScope::Negotiate, "negotiate"),
    (AuthorityScope::CommitLocal, "commit_local"),
    (AuthorityScope::CommitDelegate, "commit_delegate"),
    (AuthorityScope::Transfer, "transfer"),
];

#[test]
fn fixture_has_five_entries() {
    assert_eq!(FIXTURE.len(), 5);
}

#[test]
fn serialize_each_variant() {
    for (scope, wire) in FIXTURE {
        let v = serde_json::to_value(scope).unwrap();
        assert_eq!(v, serde_json::json!(wire));
    }
}

#[test]
fn deserialize_each_wire_string() {
    for (scope, wire) in FIXTURE {
        let parsed: AuthorityScope = serde_json::from_value(serde_json::json!(wire)).unwrap();
        assert_eq!(parsed, *scope);
    }
}

#[test]
fn display_and_fromstr_roundtrip() {
    for (scope, wire) in FIXTURE {
        assert_eq!(scope.to_string(), *wire);
        assert_eq!(AuthorityScope::from_str(wire).unwrap(), *scope);
    }
}

#[test]
fn unknown_wire_string_rejected_serde() {
    let res: Result<AuthorityScope, _> = serde_json::from_str(r#""root""#);
    assert!(res.is_err());
}

#[test]
fn unknown_wire_string_rejected_fromstr() {
    assert!(AuthorityScope::from_str("root").is_err());
}
