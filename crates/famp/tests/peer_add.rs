//! Phase 3 Plan 03-02 Task 1 — `famp peer add` integration tests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

use std::path::Path;

use famp::cli::config::read_peers;
use famp::cli::error::CliError;
use famp::cli::peer::add::run_add_at;

const VALID_PUBKEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"; // 32 zero bytes

fn init_home(path: &Path) {
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(path, false, &mut out, &mut err).expect("famp init");
}

#[test]
fn peer_add_creates_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home(&home);

    run_add_at(
        &home,
        "alice".to_string(),
        "https://127.0.0.1:9443".to_string(),
        VALID_PUBKEY_B64.to_string(),
        None,
    )
    .expect("peer add");

    let peers = read_peers(&home.join("peers.toml")).unwrap();
    assert_eq!(peers.peers.len(), 1);
    assert_eq!(peers.peers[0].alias, "alice");
    assert_eq!(peers.peers[0].endpoint, "https://127.0.0.1:9443");
    assert_eq!(peers.peers[0].pubkey_b64, VALID_PUBKEY_B64);
    assert!(peers.peers[0].tls_fingerprint_sha256.is_none());
    assert!(peers.peers[0].principal.is_none());
}

#[test]
fn peer_add_rejects_duplicate() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home(&home);

    run_add_at(
        &home,
        "alice".to_string(),
        "https://127.0.0.1:9443".to_string(),
        VALID_PUBKEY_B64.to_string(),
        None,
    )
    .unwrap();

    match run_add_at(
        &home,
        "alice".to_string(),
        "https://127.0.0.1:9444".to_string(),
        VALID_PUBKEY_B64.to_string(),
        None,
    ) {
        Err(CliError::PeerDuplicate { alias }) => assert_eq!(alias, "alice"),
        other => panic!("expected PeerDuplicate, got {other:?}"),
    }

    // File should still have exactly one entry.
    let peers = read_peers(&home.join("peers.toml")).unwrap();
    assert_eq!(peers.peers.len(), 1);
    assert_eq!(peers.peers[0].endpoint, "https://127.0.0.1:9443");
}

#[test]
fn peer_add_rejects_http_endpoint() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home(&home);

    match run_add_at(
        &home,
        "bob".to_string(),
        "http://127.0.0.1:9443".to_string(),
        VALID_PUBKEY_B64.to_string(),
        None,
    ) {
        Err(CliError::PeerEndpointInvalid { value }) => {
            assert_eq!(value, "http://127.0.0.1:9443");
        }
        other => panic!("expected PeerEndpointInvalid, got {other:?}"),
    }

    let peers = read_peers(&home.join("peers.toml")).unwrap();
    assert!(peers.peers.is_empty());
}

#[test]
fn peer_add_rejects_short_pubkey() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home(&home);

    // 16 bytes of zero → 22 chars base64url-unpadded.
    let short = "AAAAAAAAAAAAAAAAAAAAAA";
    match run_add_at(
        &home,
        "carol".to_string(),
        "https://127.0.0.1:9443".to_string(),
        short.to_string(),
        None,
    ) {
        Err(CliError::PeerPubkeyInvalid { value }) => assert_eq!(value, short),
        other => panic!("expected PeerPubkeyInvalid, got {other:?}"),
    }
}

#[test]
fn peer_add_rejects_garbage_pubkey() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home(&home);

    match run_add_at(
        &home,
        "dave".to_string(),
        "https://127.0.0.1:9443".to_string(),
        "not base64!!!".to_string(),
        None,
    ) {
        Err(CliError::PeerPubkeyInvalid { .. }) => {}
        other => panic!("expected PeerPubkeyInvalid, got {other:?}"),
    }
}

// Silencers for workspace deps not referenced in this test binary.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_taskdir as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use rand as _;
use rcgen as _;
use reqwest as _;
use rustls as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use thiserror as _;
use time as _;
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
