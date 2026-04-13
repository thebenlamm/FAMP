#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic
)]

//! Duplicate-key rejection on strict-parse path (CANON-01, D-04..D-07).
//!
//! Verbatim from `.planning/phases/01-canonical-json-foundations/01-RESEARCH.md`
//! §"Duplicate Key Rejection Test".
//!
//! Gated behind `wave2_impl` until Plan 02 lands `from_str_strict` and
//! `CanonicalError::DuplicateKey`.

#![cfg(feature = "wave2_impl")]

#[test]
fn duplicate_key_is_error() {
    let input = r#"{"a":1,"b":2,"a":3}"#;
    let result = famp_canonical::from_str_strict::<serde_json::Value>(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        famp_canonical::CanonicalError::DuplicateKey { key } => {
            assert_eq!(key, "a");
        }
        other => panic!("expected DuplicateKey, got {:?}", other),
    }
}

#[test]
fn non_duplicate_is_ok() {
    let input = r#"{"a":1,"b":2,"c":3}"#;
    let result = famp_canonical::from_str_strict::<serde_json::Value>(input);
    assert!(result.is_ok());
}
