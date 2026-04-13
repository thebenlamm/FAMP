//! Round-trip + validation tests for `Principal` and `Instance` (Phase 3 D-01..D-09).
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::{Instance, ParseInstanceError, ParsePrincipalError, Principal};
use std::str::FromStr;

// ---------- Principal happy path ----------

#[test]
fn principal_parses_and_displays_byte_for_byte() {
    let input = "agent:example.com/alice";
    let p: Principal = input.parse().unwrap();
    assert_eq!(p.authority(), "example.com");
    assert_eq!(p.name(), "alice");
    assert_eq!(p.to_string(), input);
}

#[test]
fn principal_preserves_case_no_normalization() {
    // D-03, D-07: no auto-lowercase
    let input = "agent:Example.COM/Alice";
    let p: Principal = input.parse().unwrap();
    assert_eq!(p.to_string(), input);
    assert_eq!(p.authority(), "Example.COM");
    assert_eq!(p.name(), "Alice");
}

#[test]
fn principal_rejects_instance_tail() {
    // D-01: Principal parser MUST reject instance-bearing strings.
    let err = "agent:example.com/alice#i1".parse::<Principal>().unwrap_err();
    assert_eq!(err, ParsePrincipalError::UnexpectedInstanceTail);
}

#[test]
fn principal_missing_scheme() {
    assert_eq!(
        "example.com/alice".parse::<Principal>().unwrap_err(),
        ParsePrincipalError::MissingScheme
    );
}

#[test]
fn principal_missing_name_separator() {
    assert_eq!(
        "agent:example.com".parse::<Principal>().unwrap_err(),
        ParsePrincipalError::MissingNameSeparator
    );
}

#[test]
fn principal_empty_authority() {
    assert!(matches!(
        "agent:/alice".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_empty_name() {
    assert!(matches!(
        "agent:example.com/".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidName(_))
    ));
}

#[test]
fn principal_rejects_underscore_in_authority() {
    // D-04: underscore rejected.
    assert!(matches!(
        "agent:ex_ample.com/alice".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_rejects_leading_dash_label() {
    assert!(matches!(
        "agent:-bad.com/alice".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_rejects_trailing_dash_label() {
    assert!(matches!(
        "agent:bad-.com/alice".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_rejects_authority_over_253_bytes() {
    let mut auth = String::new();
    while auth.len() < 260 {
        if !auth.is_empty() {
            auth.push('.');
        }
        auth.push_str("abcdefghij");
    }
    let input = format!("agent:{auth}/alice");
    assert!(matches!(
        input.parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_rejects_name_with_slash_or_whitespace() {
    assert!(matches!(
        "agent:example.com/ali ce".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidName(_))
    ));
}

#[test]
fn principal_name_length_boundaries() {
    // length 1 OK
    let p1: Principal = "agent:example.com/a".parse().unwrap();
    assert_eq!(p1.name(), "a");

    // length 64 OK
    let n64: String = "a".repeat(64);
    let input_ok = format!("agent:example.com/{n64}");
    let p64: Principal = input_ok.parse().unwrap();
    assert_eq!(p64.name().len(), 64);

    // length 65 rejected
    let n65: String = "a".repeat(65);
    let input_bad = format!("agent:example.com/{n65}");
    assert!(matches!(
        input_bad.parse::<Principal>(),
        Err(ParsePrincipalError::InvalidName(_))
    ));
}

#[test]
fn principal_rejects_non_ascii() {
    assert!(matches!(
        "agent:\u{4f8b}\u{3048}.com/alice".parse::<Principal>(),
        Err(ParsePrincipalError::InvalidAuthority(_))
    ));
}

#[test]
fn principal_serde_roundtrip() {
    let input_json = r#""agent:example.com/alice""#;
    let p: Principal = serde_json::from_str(input_json).unwrap();
    let out = serde_json::to_string(&p).unwrap();
    assert_eq!(out, input_json);
}

#[test]
fn principal_serde_rejects_instance_tail() {
    let bad = r#""agent:example.com/alice#i1""#;
    assert!(serde_json::from_str::<Principal>(bad).is_err());
}

// ---------- Instance happy path ----------

#[test]
fn instance_parses_and_displays() {
    let input = "agent:example.com/alice#i1";
    let i: Instance = input.parse().unwrap();
    assert_eq!(i.authority(), "example.com");
    assert_eq!(i.name(), "alice");
    assert_eq!(i.instance_id(), "i1");
    assert_eq!(i.to_string(), input);
}

#[test]
fn instance_rejects_principal_only() {
    // D-01: Instance parser MUST reject principal-only strings.
    let err = "agent:example.com/alice".parse::<Instance>().unwrap_err();
    assert_eq!(err, ParseInstanceError::MissingInstanceTail);
}

#[test]
fn instance_empty_instance_id() {
    assert!(matches!(
        "agent:example.com/alice#".parse::<Instance>(),
        Err(ParseInstanceError::InvalidInstanceId(_))
    ));
}

#[test]
fn instance_id_length_boundaries() {
    let id64: String = "z".repeat(64);
    let ok = format!("agent:example.com/alice#{id64}");
    let inst: Instance = ok.parse().unwrap();
    assert_eq!(inst.instance_id().len(), 64);

    let id65: String = "z".repeat(65);
    let bad = format!("agent:example.com/alice#{id65}");
    assert!(matches!(
        bad.parse::<Instance>(),
        Err(ParseInstanceError::InvalidInstanceId(_))
    ));
}

#[test]
fn instance_serde_roundtrip() {
    let input = r#""agent:example.com/alice#i1""#;
    let i: Instance = serde_json::from_str(input).unwrap();
    let out = serde_json::to_string(&i).unwrap();
    assert_eq!(out, input);
}

// ---------- Table-driven round trip ----------

#[test]
fn principal_table_roundtrip() {
    let table = [
        "agent:example.com/alice",
        "agent:a/b",
        "agent:sub.example.com/service-1",
        "agent:EXAMPLE.COM/Alice",
        "agent:x.y.z/worker_42",
        "agent:node-1.example.io/name.with.dots",
        "agent:a.b/_hidden",
        "agent:a-b.c/NAME",
        "agent:10.example/agent-name",
        "agent:host/a1",
        "agent:host/a-b-c",
        "agent:host/a_b_c",
        "agent:host/a.b.c",
        "agent:one.two.three.four/x",
        "agent:host/Z",
    ];
    for input in table {
        let p: Principal = input.parse().unwrap_or_else(|e| panic!("parse {input}: {e:?}"));
        assert_eq!(p.to_string(), input, "round trip: {input}");
        let again: Principal = Principal::from_str(&p.to_string()).unwrap();
        assert_eq!(again, p);
    }
}

#[test]
fn instance_table_roundtrip() {
    let table = [
        "agent:example.com/alice#i1",
        "agent:a/b#c",
        "agent:sub.example.com/service-1#inst.42",
        "agent:EXAMPLE.COM/Alice#Instance_A",
        "agent:a.b/c#d-e-f",
    ];
    for input in table {
        let i: Instance = input.parse().unwrap();
        assert_eq!(i.to_string(), input);
    }
}
