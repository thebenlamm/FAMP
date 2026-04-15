//! CLI-01 `--force` atomic replace path.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

#[test]
fn force_atomically_replaces_existing_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // First init
    let mut o1 = Vec::<u8>::new();
    let mut e1 = Vec::<u8>::new();
    let first = famp::cli::init::run_at(&home, false, &mut o1, &mut e1).expect("first init");
    let first_key = std::fs::read(home.join("key.ed25519")).unwrap();

    // Second init with --force: must succeed and generate NEW keys
    let mut o2 = Vec::<u8>::new();
    let mut e2 = Vec::<u8>::new();
    let second = famp::cli::init::run_at(&home, true, &mut o2, &mut e2).expect("force init");
    let second_key = std::fs::read(home.join("key.ed25519")).unwrap();

    assert_ne!(first.pubkey_b64url, second.pubkey_b64url);
    assert_ne!(first_key, second_key);

    // All six files still present
    for name in [
        "key.ed25519",
        "pub.ed25519",
        "tls.cert.pem",
        "tls.key.pem",
        "config.toml",
        "peers.toml",
    ] {
        assert!(home.join(name).exists(), "missing after force: {name}");
    }
}
