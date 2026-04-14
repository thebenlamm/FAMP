//! CLI-01 happy path + IDENT-01 (32 bytes + modes 0600/0644) + IDENT-02
//! (cross-phase TLS conformance gate) + narrowed IDENT-03 / IDENT-04.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

use std::os::unix::fs::PermissionsExt;

#[test]
fn init_creates_all_six_files_with_correct_modes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    let outcome =
        famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init happy path");

    // File existence + byte lengths
    let key = std::fs::read(home.join("key.ed25519")).unwrap();
    let pubk = std::fs::read(home.join("pub.ed25519")).unwrap();
    assert_eq!(key.len(), 32, "key.ed25519 must be 32 raw bytes");
    assert_eq!(pubk.len(), 32, "pub.ed25519 must be 32 raw bytes");

    // Modes
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&home.join("key.ed25519")), 0o600);
    assert_eq!(mode(&home.join("pub.ed25519")), 0o644);
    assert_eq!(mode(&home.join("tls.cert.pem")), 0o644);
    assert_eq!(mode(&home.join("tls.key.pem")), 0o600);
    assert_eq!(mode(&home.join("config.toml")), 0o644);
    assert_eq!(mode(&home.join("peers.toml")), 0o644);

    // config.toml contains exactly the single listen_addr line
    let cfg = std::fs::read_to_string(home.join("config.toml")).unwrap();
    assert_eq!(cfg, "listen_addr = \"127.0.0.1:8443\"\n");

    // peers.toml is zero bytes
    assert_eq!(std::fs::metadata(home.join("peers.toml")).unwrap().len(), 0);

    // D-15 stdout: exactly one line = pubkey base64url unpadded
    let out_str = String::from_utf8(out).unwrap();
    assert_eq!(out_str.lines().count(), 1);
    assert_eq!(out_str.trim_end_matches('\n'), outcome.pubkey_b64url);
    assert!(!outcome.pubkey_b64url.contains('='), "must be unpadded");

    // D-15 stderr: exactly `initialized FAMP home at <home>\n`
    let err_str = String::from_utf8(err).unwrap();
    assert_eq!(
        err_str,
        format!("initialized FAMP home at {}\n", home.display())
    );
}

/// IDENT-02 cross-phase conformance gate: the generated PEMs must load
/// through `famp-transport-http`'s rustls setup without modification.
#[test]
fn init_tls_output_loads_via_transport_http() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    let certs = famp_transport_http::tls::load_pem_cert(&home.join("tls.cert.pem"))
        .expect("load_pem_cert");
    let key =
        famp_transport_http::tls::load_pem_key(&home.join("tls.key.pem")).expect("load_pem_key");
    let _cfg = famp_transport_http::tls::build_server_config(certs, key)
        .expect("build_server_config");
}
