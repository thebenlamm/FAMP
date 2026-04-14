#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic
)]

//! `sha256:<hex>` artifact ID helper (SPEC-18, CANON-06, D-19..D-22).
//!
//! Asserts:
//!  1. Empty-input SHA-256 matches the well-known constant.
//!  2. Output is always lowercase hex per spec §3.6a (no uppercase).

#[test]
fn sha256_known_input() {
    // SHA-256 of the empty byte string is a well-known constant.
    let id = famp_canonical::artifact_id_for_canonical_bytes(b"");
    assert_eq!(
        id.as_ref(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_lowercase_only() {
    // Spec §3.6a mandates 64 lowercase hex characters. One uppercase letter
    // produces a different artifact ID and breaks interop.
    let id = famp_canonical::artifact_id_for_canonical_bytes(b"hello world");
    let s: &str = id.as_ref();
    assert!(
        s.starts_with("sha256:"),
        "artifact ID must start with literal 'sha256:'"
    );
    let hex = &s["sha256:".len()..];
    assert_eq!(hex.len(), 64, "hex digest must be exactly 64 characters");
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "hex digest must be lowercase only (no uppercase per spec §3.6a): {hex}"
    );
}
