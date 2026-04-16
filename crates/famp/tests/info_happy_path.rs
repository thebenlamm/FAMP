//! Tests for `famp info` command — output peer card.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::cli::info::InfoArgs;
use famp::cli::setup::PeerCard;

#[test]
fn info_outputs_peer_card_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // First, set up an identity via setup
    let setup_args = famp::cli::setup::SetupArgs {
        name: "carol".to_string(),
        port: Some(9990),
        home: Some(home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let setup_card = famp::cli::setup::run_with_io(&setup_args, &mut out, &mut err).expect("setup");

    // Now test info
    let info_args = InfoArgs {
        format: "json".to_string(),
    };
    let mut info_out = Vec::<u8>::new();
    let info_card = famp::cli::info::run_at(&home, &info_args, &mut info_out).expect("info");

    // Verify info outputs same data as setup
    assert_eq!(info_card.pubkey, setup_card.pubkey);
    assert_eq!(info_card.endpoint, setup_card.endpoint);
    assert_eq!(info_card.principal, setup_card.principal);

    // Verify JSON output is valid
    let out_str = String::from_utf8(info_out).unwrap();
    let parsed: PeerCard = serde_json::from_str(&out_str).expect("valid JSON");
    assert_eq!(parsed.pubkey, setup_card.pubkey);
}

#[test]
fn info_extracts_alias_from_any_principal_format() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Init the identity
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    // Manually write a config with a non-localhost principal
    let config = r#"listen_addr = "127.0.0.1:8443"
principal = "agent:myhost.example.com/specialname"
"#;
    std::fs::write(home.join("config.toml"), config).unwrap();

    // Run info
    let info_args = InfoArgs {
        format: "json".to_string(),
    };
    let mut info_out = Vec::<u8>::new();
    let info_card = famp::cli::info::run_at(&home, &info_args, &mut info_out).expect("info");

    // Alias should be extracted from after the last slash
    assert_eq!(info_card.alias, "specialname");
    assert_eq!(info_card.principal, "agent:myhost.example.com/specialname");
}

#[test]
fn info_fails_without_identity() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");
    std::fs::create_dir_all(&home).unwrap();

    let info_args = InfoArgs {
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let result = famp::cli::info::run_at(&home, &info_args, &mut out);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("identity incomplete") || err_msg.contains("missing"));
}
