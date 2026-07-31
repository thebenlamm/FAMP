//! KEYR-02/KEYR-03: `famp peer rotate` / `famp peer retire`.
//!
//! In-process `run_at` cases mirror `peer_roundtrip.rs`'s idiom; two
//! process-level `assert_cmd` cases prove the exit-code contract that
//! makes KEYR-03's "different exit code" claim falsifiable: exit 2 +
//! `KEY CHANGED` on stderr for an unconfirmed key change, exit 0 for the
//! same rotation with `--confirm-rotation`.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Cursor;

use assert_cmd::Command as AssertCommand;
use famp::cli::error::CliError;
use famp::cli::peer::identity::{self, gateway_peers_keyring_path};
use famp::cli::peer::{export, import, import_revocation, retire, revoke, rotate};
use famp_core::Principal;
use famp_keyring::revocation::SignedRevocation;
use famp_keyring::{KeyState, Keyring};

const PRINCIPAL_A: &str = "agent:machine-a.local/gateway";

const NOW: &str = "2026-07-31T12:00:00Z";

fn export_blob(gateway_home: &std::path::Path, principal: &str) -> String {
    let args = export::PeerExportArgs {
        as_principal: principal.to_string(),
    };
    let mut buf: Vec<u8> = Vec::new();
    export::run_at(gateway_home, &args, &mut buf).expect("export::run_at must succeed");
    String::from_utf8(buf).expect("export blob must be valid UTF-8")
}

/// Seed `import_home`'s keyring with a fresh pin for `PRINCIPAL_A`,
/// returning the export blob for a SECOND, DIFFERENT keypair under the
/// same principal — the "changed key for a known peer" scenario.
fn seed_pin_and_produce_a_changed_key_blob(import_home: &std::path::Path) -> String {
    let first_home = tempfile::tempdir().unwrap();
    let second_home = tempfile::tempdir().unwrap();

    let first_blob = export_blob(first_home.path(), PRINCIPAL_A);
    import::run_at(import_home, &mut Cursor::new(first_blob.into_bytes()))
        .expect("initial import must succeed");

    export_blob(second_home.path(), PRINCIPAL_A)
}

#[test]
fn rotate_unconfirmed_key_change_exits_2_and_mutates_nothing() {
    let import_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(import_home.path());

    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let before = std::fs::read(&keyring_path).unwrap();

    let err = rotate::run_at(
        import_home.path(),
        &mut Cursor::new(changed_blob.into_bytes()),
        false,
        NOW,
    )
    .unwrap_err();
    assert!(
        matches!(err, CliError::Exit(2)),
        "expected Exit(2), got {err:?}"
    );

    let after = std::fs::read(&keyring_path).unwrap();
    assert_eq!(
        before, after,
        "an unconfirmed rotation must leave the keyring file byte-identical"
    );
}

#[test]
fn rotate_confirmed_key_change_succeeds_and_retains_previous_as_retired() {
    let import_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(import_home.path());

    rotate::run_at(
        import_home.path(),
        &mut Cursor::new(changed_blob.into_bytes()),
        true,
        NOW,
    )
    .expect("confirmed rotation must succeed");

    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let entries = keyring.entries(&principal);
    assert_eq!(entries.len(), 2, "previous key must not be dropped");
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.state() == KeyState::Active)
            .count(),
        1
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.state() == KeyState::Retired)
            .count(),
        1
    );
}

#[test]
fn rotate_on_a_never_pinned_principal_refuses_and_creates_nothing() {
    let import_home = tempfile::tempdir().unwrap();
    let blob = export_blob(tempfile::tempdir().unwrap().path(), PRINCIPAL_A);

    let err = rotate::run_at(
        import_home.path(),
        &mut Cursor::new(blob.into_bytes()),
        true,
        NOW,
    )
    .unwrap_err();
    assert!(
        matches!(err, CliError::PeerNotPinned { .. }),
        "expected PeerNotPinned, got {err:?}"
    );

    let keyring_path = gateway_peers_keyring_path(import_home.path());
    assert!(
        !keyring_path.exists(),
        "rotate on an unpinned principal must not create the keyring file"
    );
}

#[test]
fn import_still_fails_closed_on_a_changed_key_for_a_known_peer() {
    let import_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(import_home.path());

    let err = import::run_at(
        import_home.path(),
        &mut Cursor::new(changed_blob.into_bytes()),
    )
    .unwrap_err();
    assert!(
        matches!(err, CliError::PeerKeyConflict { .. }),
        "expected PeerKeyConflict (regression guard, unchanged by this phase), got {err:?}"
    );
}

#[test]
fn import_still_succeeds_silently_on_a_brand_new_peer() {
    let export_home = tempfile::tempdir().unwrap();
    let import_home = tempfile::tempdir().unwrap();
    let blob = export_blob(export_home.path(), PRINCIPAL_A);

    import::run_at(import_home.path(), &mut Cursor::new(blob.into_bytes()))
        .expect("first-sight import must succeed with no confirmation");
}

#[test]
fn retire_removes_the_retired_entry_and_pubkey_no_longer_saved() {
    let import_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(import_home.path());
    rotate::run_at(
        import_home.path(),
        &mut Cursor::new(changed_blob.into_bytes()),
        true,
        NOW,
    )
    .unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let retired_key_id = keyring
        .entries(&principal)
        .iter()
        .find(|e| e.state() == KeyState::Retired)
        .expect("must have a retired entry after rotation")
        .key_id();
    let retired_pubkey = keyring
        .entries(&principal)
        .iter()
        .find(|e| e.state() == KeyState::Retired)
        .unwrap()
        .key()
        .to_b64url();

    retire::run_at(import_home.path(), &principal, &retired_key_id)
        .expect("retire of a retired entry must succeed");

    let saved = std::fs::read_to_string(&keyring_path).unwrap();
    assert!(
        !saved.contains(&retired_pubkey),
        "retired key material must no longer appear in the saved keyring"
    );
}

#[test]
fn retire_against_the_active_key_refuses() {
    let export_home = tempfile::tempdir().unwrap();
    let import_home = tempfile::tempdir().unwrap();
    let blob = export_blob(export_home.path(), PRINCIPAL_A);
    import::run_at(import_home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let active_key_id = keyring
        .entries(&principal)
        .iter()
        .find(|e| e.state() == KeyState::Active)
        .unwrap()
        .key_id();

    let err = retire::run_at(import_home.path(), &principal, &active_key_id).unwrap_err();
    assert!(
        matches!(err, CliError::PeerCannotRetire { .. }),
        "expected PeerCannotRetire, got {err:?}"
    );
}

// ── REVK-02: `famp peer revoke` / `famp peer import-revocation` ────────

#[test]
fn revoke_run_at_transitions_entry_to_revoked() {
    let home = tempfile::tempdir().unwrap();
    // Home pins ITS OWN gateway identity under `PRINCIPAL_A` — the "this
    // machine owns the principal being revoked" scenario `revoke.rs`'s
    // module doc names as the only case `--emit` is meaningful for.
    let blob = export_blob(home.path(), PRINCIPAL_A);
    import::run_at(home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let key_id = keyring.entries(&principal)[0].key_id();

    revoke::run_at(home.path(), &principal, &key_id, None, NOW, None)
        .expect("revoke::run_at must succeed");

    let reloaded = Keyring::load_from_file(&keyring_path).unwrap();
    let entries = reloaded.entries(&principal);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state(), KeyState::Revoked);
}

#[test]
fn revoke_run_at_with_emit_writes_a_blob_verifiable_against_this_machines_own_gateway_key() {
    let home = tempfile::tempdir().unwrap();
    let blob = export_blob(home.path(), PRINCIPAL_A);
    import::run_at(home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let key_id = keyring.entries(&principal)[0].key_id();

    let emit_path = home.path().join("revocation.blob");
    revoke::run_at(
        home.path(),
        &principal,
        &key_id,
        Some("compromised"),
        NOW,
        Some(&emit_path),
    )
    .expect("revoke::run_at with --emit must succeed");

    let emitted = std::fs::read_to_string(&emit_path).unwrap();
    let signed = SignedRevocation::from_blob(&emitted).expect("emitted blob must parse");

    let gateway_sk =
        identity::load_or_generate(&identity::gateway_identity_path(home.path())).unwrap();
    let gateway_vk = gateway_sk.verifying_key();
    let sig = famp_crypto::FampSignature::from_b64url(&signed.sig).unwrap();
    famp_crypto::verify_value(&gateway_vk, &signed.statement, &sig)
        .expect("emitted statement must verify against this machine's own gateway key");
}

#[test]
fn import_revocation_on_valid_blob_revokes_the_entry() {
    let alice_home = tempfile::tempdir().unwrap();
    let blob = export_blob(alice_home.path(), PRINCIPAL_A);
    import::run_at(
        alice_home.path(),
        &mut Cursor::new(blob.clone().into_bytes()),
    )
    .unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(alice_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let key_id = keyring.entries(&principal)[0].key_id();

    let emit_path = alice_home.path().join("revocation.blob");
    revoke::run_at(
        alice_home.path(),
        &principal,
        &key_id,
        Some("compromised"),
        NOW,
        Some(&emit_path),
    )
    .unwrap();
    let signed_blob = std::fs::read_to_string(&emit_path).unwrap();

    // A SEPARATE peer who independently pinned the same key under the
    // same principal — has NOT locally revoked anything yet.
    let bob_home = tempfile::tempdir().unwrap();
    import::run_at(bob_home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();

    import_revocation::run_at(
        bob_home.path(),
        &mut Cursor::new(signed_blob.into_bytes()),
        NOW,
    )
    .expect("a validly-signed revocation must be accepted");

    let bob_keyring_path = gateway_peers_keyring_path(bob_home.path());
    let bob_keyring = Keyring::load_from_file(&bob_keyring_path).unwrap();
    let entries = bob_keyring.entries(&principal);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state(), KeyState::Revoked);
}

#[test]
fn import_revocation_on_tampered_blob_rejects_and_keyring_byte_identical() {
    let alice_home = tempfile::tempdir().unwrap();
    let blob = export_blob(alice_home.path(), PRINCIPAL_A);
    import::run_at(
        alice_home.path(),
        &mut Cursor::new(blob.clone().into_bytes()),
    )
    .unwrap();

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let keyring_path = gateway_peers_keyring_path(alice_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let key_id = keyring.entries(&principal)[0].key_id();

    let emit_path = alice_home.path().join("revocation.blob");
    revoke::run_at(
        alice_home.path(),
        &principal,
        &key_id,
        None,
        NOW,
        Some(&emit_path),
    )
    .unwrap();
    let signed_blob = std::fs::read_to_string(&emit_path).unwrap();

    // Tamper: flip the `reason` field inside the JSON blob AFTER signing.
    let mut value: serde_json::Value = serde_json::from_str(&signed_blob).unwrap();
    value["statement"]["reason"] = serde_json::json!("tampered");
    let tampered_blob = serde_json::to_string(&value).unwrap();

    let bob_home = tempfile::tempdir().unwrap();
    import::run_at(bob_home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();
    let bob_keyring_path = gateway_peers_keyring_path(bob_home.path());
    let before = std::fs::read(&bob_keyring_path).unwrap();

    let err = import_revocation::run_at(
        bob_home.path(),
        &mut Cursor::new(tampered_blob.into_bytes()),
        NOW,
    )
    .unwrap_err();
    assert!(
        matches!(err, CliError::PeerRevocationRejected { .. }),
        "expected PeerRevocationRejected, got {err:?}"
    );

    let after = std::fs::read(&bob_keyring_path).unwrap();
    assert_eq!(
        before, after,
        "a tampered revocation must leave the keyring byte-identical"
    );
}

// ── Process-level exit-code contract (KEYR-03) ──────────────────────────

#[test]
fn process_level_unconfirmed_rotation_exits_2_with_key_changed_on_stderr() {
    let famp_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(famp_home.path());

    let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
    let assert = cmd
        .env("FAMP_HOME", famp_home.path())
        .arg("peer")
        .arg("rotate")
        .write_stdin(changed_blob)
        .assert();

    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("KEY CHANGED"),
        "expected 'KEY CHANGED' on stderr, got: {stderr}"
    );
}

#[test]
fn process_level_confirmed_rotation_exits_0_with_gateway_restart_note() {
    let famp_home = tempfile::tempdir().unwrap();
    let changed_blob = seed_pin_and_produce_a_changed_key_blob(famp_home.path());

    let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
    let assert = cmd
        .env("FAMP_HOME", famp_home.path())
        .arg("peer")
        .arg("rotate")
        .arg("--confirm-rotation")
        .write_stdin(changed_blob)
        .assert();

    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit code 0, got: {output:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("NOTE:") && stderr.contains("restart"),
        "expected a NOTE: line naming the gateway-restart remedy, got: {stderr}"
    );
}
