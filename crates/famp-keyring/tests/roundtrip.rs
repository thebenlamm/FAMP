//! Keyring file format round-trip + validation tests (D-B2, D-B5).
//!
//! RT-1/RT-2: byte-identical round-trip against committed canonical fixture.
//! RT-3: duplicate principal line rejected with line number.
//! RT-4: same pubkey under two principals rejected with line number.
//! RT-5: inline `#` comment rejected as `MalformedEntry`.
//! RT-6: `\r\n` line endings tolerated on load.

#![allow(clippy::unwrap_used, unused_crate_dependencies)]

use famp_core::Principal;
use famp_keyring::{Keyring, KeyringError};
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

const CANONICAL_FIXTURE: &str = "tests/fixtures/two_peers.canonical.keyring";
const HUMAN_FIXTURE: &str = "tests/fixtures/two_peers.keyring";

fn alice() -> Principal {
    Principal::from_str("agent:local/alice").unwrap()
}

fn bob() -> Principal {
    Principal::from_str("agent:local/bob").unwrap()
}

#[test]
fn rt1_human_fixture_saves_to_canonical_form() {
    // Load the human-readable fixture (with comments), save it, and assert
    // the saved output matches the committed canonical form byte-for-byte.
    let loaded = Keyring::load_from_file(Path::new(HUMAN_FIXTURE)).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    loaded.save_to_file(tmp.path()).unwrap();
    let saved = std::fs::read(tmp.path()).unwrap();
    let expected = std::fs::read(CANONICAL_FIXTURE).unwrap();
    assert_eq!(
        saved, expected,
        "save output must match canonical fixture byte-for-byte"
    );
}

#[test]
fn rt1b_canonical_fixture_round_trips_byte_identical() {
    // The real "load -> save byte-identical" assertion required by KEY-02.
    let canonical = Keyring::load_from_file(Path::new(CANONICAL_FIXTURE)).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    canonical.save_to_file(tmp.path()).unwrap();
    let saved = std::fs::read(tmp.path()).unwrap();
    let original = std::fs::read(CANONICAL_FIXTURE).unwrap();
    assert_eq!(
        saved, original,
        "canonical fixture must round-trip byte-identical"
    );
}

#[test]
fn rt2_fixture_loads_expected_principals() {
    let k = Keyring::load_from_file(Path::new(HUMAN_FIXTURE)).unwrap();
    assert_eq!(k.len(), 2);
    assert!(k.get(&alice()).is_some());
    assert!(k.get(&bob()).is_some());
    let carol = Principal::from_str("agent:local/carol").unwrap();
    assert!(k.get(&carol).is_none());
}

#[test]
fn rt3_duplicate_principal_rejected_with_line_number() {
    let content = "\
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w
agent:local/bob  gTl3Dqh9F19Wo1Rmw0x-zMuNipG07jeiXfYPW4_Js5Q
# comment line
# another comment
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w
";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let err = Keyring::load_from_file(tmp.path()).unwrap_err();
    match err {
        KeyringError::DuplicatePrincipal { principal, line } => {
            assert_eq!(principal, alice());
            assert_eq!(line, 5);
        }
        other => panic!("expected DuplicatePrincipal, got {other:?}"),
    }
}

#[test]
fn rt4_duplicate_pubkey_rejected() {
    // Two distinct principals mapped to the same 32-byte pubkey.
    let content = "\
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w
agent:local/bob  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w
";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let err = Keyring::load_from_file(tmp.path()).unwrap_err();
    match err {
        KeyringError::DuplicatePubkey { existing, line } => {
            assert_eq!(existing, alice());
            assert_eq!(line, 2);
        }
        other => panic!("expected DuplicatePubkey, got {other:?}"),
    }
}

#[test]
fn rt5_inline_comment_rejected() {
    let content = "\
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w # inline comment
";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let err = Keyring::load_from_file(tmp.path()).unwrap_err();
    match err {
        KeyringError::MalformedEntry { line, reason } => {
            assert_eq!(line, 1);
            assert!(
                reason.contains("inline '#' comments"),
                "reason should mention inline comments, got: {reason}"
            );
        }
        other => panic!("expected MalformedEntry, got {other:?}"),
    }
}

#[test]
fn rt6_crlf_line_endings_tolerated() {
    let content =
        "# header\r\nagent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w\r\nagent:local/bob  gTl3Dqh9F19Wo1Rmw0x-zMuNipG07jeiXfYPW4_Js5Q\r\n";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let k = Keyring::load_from_file(tmp.path()).unwrap();
    assert_eq!(k.len(), 2);
    assert!(k.get(&alice()).is_some());
    assert!(k.get(&bob()).is_some());
}
