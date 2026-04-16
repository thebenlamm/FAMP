//! Tests for `famp setup` command — one-command onboarding.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::cli::setup::{PeerCard, SetupArgs};

#[test]
fn setup_creates_identity_and_outputs_peer_card() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let args = SetupArgs {
        name: "alice".to_string(),
        port: Some(9999),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let card = famp::cli::setup::run_with_io(args, &mut out, &mut err).expect("setup happy path");

    // Verify peer card fields
    assert_eq!(card.alias, "alice");
    assert_eq!(card.endpoint, "https://127.0.0.1:9999");
    assert_eq!(card.principal, "agent:localhost/alice");
    assert!(!card.pubkey.is_empty());
    assert!(!card.pubkey.contains('='), "pubkey must be unpadded base64url");

    // Verify JSON output
    let out_str = String::from_utf8(out).unwrap();
    let parsed: PeerCard = serde_json::from_str(&out_str).expect("output should be valid JSON");
    assert_eq!(parsed.alias, card.alias);
    assert_eq!(parsed.pubkey, card.pubkey);

    // Verify config.toml was updated
    let cfg = std::fs::read_to_string(home.join("config.toml")).unwrap();
    assert!(cfg.contains("listen_addr = \"127.0.0.1:9999\""));
    assert!(cfg.contains("principal = \"agent:localhost/alice\""));

    // Verify identity files exist
    assert!(home.join("key.ed25519").exists());
    assert!(home.join("pub.ed25519").exists());
}

#[test]
fn setup_rejects_invalid_names() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Test path traversal
    let args = SetupArgs {
        name: "../etc".to_string(),
        port: Some(9998),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let result = famp::cli::setup::run_with_io(args, &mut out, &mut err);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("invalid agent name"));

    // Test slash in name
    let args = SetupArgs {
        name: "foo/bar".to_string(),
        port: Some(9998),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let result = famp::cli::setup::run_with_io(args, &mut out, &mut err);
    assert!(result.is_err());

    // Test empty name
    let args = SetupArgs {
        name: "".to_string(),
        port: Some(9998),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let result = famp::cli::setup::run_with_io(args, &mut out, &mut err);
    assert!(result.is_err());
}

#[test]
fn setup_accepts_valid_name_characters() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let args = SetupArgs {
        name: "valid-name_123".to_string(),
        port: Some(9997),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let card = famp::cli::setup::run_with_io(args, &mut out, &mut err).expect("valid name should work");
    assert_eq!(card.alias, "valid-name_123");
}

#[test]
fn setup_text_format_outputs_readable_text() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let args = SetupArgs {
        name: "bob".to_string(),
        port: Some(9996),
        home: Some(home.display().to_string()),
        force: false,
        format: "text".to_string(),
    };

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::setup::run_with_io(args, &mut out, &mut err).expect("setup");

    let out_str = String::from_utf8(out).unwrap();
    assert!(out_str.contains("Alias:"));
    assert!(out_str.contains("Endpoint:"));
    assert!(out_str.contains("Pubkey:"));
    assert!(out_str.contains("Principal:"));
}
