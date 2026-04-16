//! Tests for `famp peer import` command — import peer card from JSON.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

#[test]
fn peer_import_from_card_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Init the identity first
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    // Create a peer card JSON
    let card_json = r#"{
        "alias": "remote-agent",
        "endpoint": "https://192.168.1.100:8443",
        "pubkey": "RapCUIhAboZb0lQajuzzRVDyYOYOyln4OvpqLhefqso",
        "principal": "agent:remote.example.com/agent1"
    }"#;

    // Import it
    famp::cli::peer::import::run_import_at(&home, Some(card_json.to_string())).expect("import");

    // Verify peers.toml was updated
    let peers_content = std::fs::read_to_string(home.join("peers.toml")).unwrap();
    assert!(peers_content.contains("remote-agent"));
    assert!(peers_content.contains("https://192.168.1.100:8443"));
    assert!(peers_content.contains("RapCUIhAboZb0lQajuzzRVDyYOYOyln4OvpqLhefqso"));
    assert!(peers_content.contains("agent:remote.example.com/agent1"));
}

#[test]
fn peer_import_rejects_invalid_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Init the identity first
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    // Try to import invalid JSON
    let result = famp::cli::peer::import::run_import_at(&home, Some("not valid json".to_string()));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("peer card"));
}

#[test]
fn peer_import_rejects_invalid_pubkey() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Init the identity first
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    // Create a peer card with invalid pubkey (not 32 bytes when decoded)
    let card_json = r#"{
        "alias": "bad-agent",
        "endpoint": "https://127.0.0.1:8443",
        "pubkey": "tooshort",
        "principal": "agent:localhost/bad"
    }"#;

    let result = famp::cli::peer::import::run_import_at(&home, Some(card_json.to_string()));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("pubkey"));
}

#[test]
fn peer_import_roundtrip_with_setup_and_info() {
    let tmp = tempfile::TempDir::new().unwrap();
    let alice_home = tmp.path().join("alice");
    let bob_home = tmp.path().join("bob");

    // Setup Alice
    let alice_args = famp::cli::setup::SetupArgs {
        name: "alice".to_string(),
        port: Some(9980),
        home: Some(alice_home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::setup::run_with_io(alice_args, &mut out, &mut err).expect("setup alice");

    // Setup Bob
    let bob_args = famp::cli::setup::SetupArgs {
        name: "bob".to_string(),
        port: Some(9981),
        home: Some(bob_home.display().to_string()),
        force: false,
        format: "json".to_string(),
    };
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::setup::run_with_io(bob_args, &mut out, &mut err).expect("setup bob");

    // Get Alice's peer card via info
    let info_args = famp::cli::info::InfoArgs {
        format: "json".to_string(),
    };
    let mut alice_card_out = Vec::<u8>::new();
    famp::cli::info::run_at(&alice_home, info_args, &mut alice_card_out).expect("info alice");
    let alice_card_json = String::from_utf8(alice_card_out).unwrap();

    // Import Alice to Bob
    famp::cli::peer::import::run_import_at(&bob_home, Some(alice_card_json)).expect("import alice to bob");

    // Verify Bob has Alice as a peer
    let bob_peers = std::fs::read_to_string(bob_home.join("peers.toml")).unwrap();
    assert!(bob_peers.contains("alice"));
    assert!(bob_peers.contains("https://127.0.0.1:9980"));
    assert!(bob_peers.contains("agent:localhost/alice"));
}
