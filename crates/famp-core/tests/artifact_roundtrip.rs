//! Tests for `ArtifactId` (Phase 3 D-14..D-18).
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::{ArtifactId, ParseArtifactIdError};
use std::str::FromStr;

const EMPTY_SHA256: &str =
    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn parses_valid_lowercase() {
    let a: ArtifactId = EMPTY_SHA256.parse().unwrap();
    assert_eq!(a.as_str(), EMPTY_SHA256);
    assert_eq!(a.to_string(), EMPTY_SHA256);
}

#[test]
fn rejects_uppercase_hex() {
    let bad = "sha256:E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn rejects_mixed_case_hex() {
    let bad = "sha256:e3B0c44298FC1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn rejects_63_char_hex() {
    let bad = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn rejects_65_char_hex() {
    let bad = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8555";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn rejects_non_sha256_algorithms() {
    for bad in [
        "sha1:da39a3ee5e6b4b0d3255bfef95601890afd80709",
        "blake3:af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        "md5:d41d8cd98f00b204e9800998ecf8427e",
        "SHA256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ] {
        assert_eq!(
            ArtifactId::from_str(bad).unwrap_err(),
            ParseArtifactIdError::UnsupportedAlgorithm,
            "input: {bad}"
        );
    }
}

#[test]
fn rejects_missing_colon() {
    let bad = "sha256e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::MissingPrefix
    );
}

#[test]
fn rejects_empty_hex() {
    assert_eq!(
        ArtifactId::from_str("sha256:").unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn rejects_non_hex_chars() {
    let bad = "sha256:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
    assert_eq!(
        ArtifactId::from_str(bad).unwrap_err(),
        ParseArtifactIdError::InvalidHex
    );
}

#[test]
fn serde_roundtrip() {
    let json = format!("\"{EMPTY_SHA256}\"");
    let a: ArtifactId = serde_json::from_str(&json).unwrap();
    let out = serde_json::to_string(&a).unwrap();
    assert_eq!(out, json);
}

#[test]
fn try_from_str_and_string() {
    let a = ArtifactId::try_from(EMPTY_SHA256).unwrap();
    assert_eq!(a.as_str(), EMPTY_SHA256);

    let b = ArtifactId::try_from(EMPTY_SHA256.to_owned()).unwrap();
    assert_eq!(b.as_str(), EMPTY_SHA256);
}

#[test]
fn nist_empty_kat_roundtrip() {
    // The SHA-256 hex of empty input — locked in Phase 2 KAT (02-04).
    // This asserts that when Phase 2's `sha256_artifact_id("")` produces
    // `sha256:e3b0...`, Phase 3's `ArtifactId` parses it cleanly.
    let id: ArtifactId = EMPTY_SHA256.parse().unwrap();
    assert_eq!(id.as_str(), EMPTY_SHA256);
}
