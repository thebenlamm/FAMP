//! TRUST-01: single-machine `famp peer export` -> `import` -> TOFU-pin
//! round-trip.
//!
//! Proves the export/import/pin mechanism in-process, per CONTEXT.md
//! `<specifics>` — no second physical machine is needed for this phase's
//! own gate (the real two-machine run is Phase 9/10). Also proves the
//! fail-closed conflict behavior (T-08-11) and that a never-imported
//! principal stays absent (the TRUST-02 precondition `verify_inbound`
//! relies on).
//!
//! Fully in-process (`run_at` calls directly) — no broker/gateway child
//! process is spawned, so `ChildGuard` is not needed here.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Cursor;

use famp::cli::error::CliError;
use famp::cli::peer::identity::gateway_peers_keyring_path;
use famp::cli::peer::{export, import};
use famp_core::Principal;
use famp_keyring::Keyring;

const PRINCIPAL_A: &str = "agent:machine-a.local/gateway";
const PRINCIPAL_B: &str = "agent:machine-b.local/gateway";

fn export_blob(gateway_home: &std::path::Path, principal: &str) -> String {
    let args = export::PeerExportArgs {
        as_principal: principal.to_string(),
    };
    let mut buf: Vec<u8> = Vec::new();
    export::run_at(gateway_home, &args, &mut buf).expect("export::run_at must succeed");
    String::from_utf8(buf).expect("export blob must be valid UTF-8")
}

#[test]
fn export_import_pins_the_exact_key() {
    let export_home = tempfile::tempdir().unwrap();
    let import_home = tempfile::tempdir().unwrap();

    let blob = export_blob(export_home.path(), PRINCIPAL_A);

    // Sanity: the blob is exactly the 3-field Signal-paste-safe line.
    let fields: Vec<&str> = blob.split_whitespace().collect();
    assert_eq!(
        fields.len(),
        3,
        "export blob must have 3 fields: {fields:?}"
    );
    assert_eq!(fields[0], PRINCIPAL_A);
    let exported_pubkey = fields[1];

    import::run_at(
        import_home.path(),
        &mut Cursor::new(blob.clone().into_bytes()),
    )
    .expect("import::run_at must succeed on a freshly exported blob");

    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).expect("peer keyring must be on disk");

    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    let pinned = keyring
        .get(&principal)
        .expect("principal must be pinned after import");

    assert_eq!(
        pinned.to_b64url(),
        exported_pubkey,
        "pinned key must equal the exported pubkey — the round-trip must preserve the exact key"
    );
}

#[test]
fn conflicting_repin_fails_closed() {
    let export_home_first = tempfile::tempdir().unwrap();
    let export_home_second = tempfile::tempdir().unwrap();
    let import_home = tempfile::tempdir().unwrap();

    // First export/import under PRINCIPAL_A establishes the pin.
    let blob_first = export_blob(export_home_first.path(), PRINCIPAL_A);
    import::run_at(
        import_home.path(),
        &mut Cursor::new(blob_first.into_bytes()),
    )
    .expect("first import must succeed");

    // A SECOND, DIFFERENT gateway keypair exported under the SAME
    // principal name must be rejected on import — TOFU fails closed,
    // never silently overwriting a pinned trust anchor (T-08-11).
    let blob_second = export_blob(export_home_second.path(), PRINCIPAL_A);
    let result = import::run_at(
        import_home.path(),
        &mut Cursor::new(blob_second.into_bytes()),
    );

    match result {
        Err(CliError::PeerKeyConflict { principal }) => {
            assert_eq!(principal, PRINCIPAL_A);
        }
        other => panic!("expected Err(PeerKeyConflict), got {other:?}"),
    }

    // The original pin must remain intact after the rejected re-pin
    // attempt (no partial/corrupted write).
    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();
    let principal: Principal = PRINCIPAL_A.parse().unwrap();
    assert!(
        keyring.get(&principal).is_some(),
        "the original pin must survive a rejected conflicting re-pin"
    );
}

#[test]
fn never_imported_principal_is_absent() {
    let export_home = tempfile::tempdir().unwrap();
    let import_home = tempfile::tempdir().unwrap();

    // Establish some pin so the keyring file exists on disk at all.
    let blob = export_blob(export_home.path(), PRINCIPAL_A);
    import::run_at(import_home.path(), &mut Cursor::new(blob.into_bytes())).unwrap();

    let keyring_path = gateway_peers_keyring_path(import_home.path());
    let keyring = Keyring::load_from_file(&keyring_path).unwrap();

    let never_imported: Principal = PRINCIPAL_B.parse().unwrap();
    assert!(
        keyring.get(&never_imported).is_none(),
        "a principal that was never imported must be absent from the keyring \
         (the TRUST-02 precondition verify_inbound relies on)"
    );
}
