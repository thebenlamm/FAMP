//! Keyring file format round-trip + validation tests (D-B2, D-B5).
//!
//! RT-1/RT-2: byte-identical round-trip against committed canonical fixture.
//! RT-3: duplicate principal line rejected with line number.
//! RT-4: same pubkey under two principals rejected with line number.
//! RT-5: inline `#` comment rejected as `MalformedEntry`.
//! RT-6: `\r\n` line endings tolerated on load.
//!
//! `keyr01_*`/`multi_key_*`/`*_rejected*` (Phase 15, plan 02 Task 2):
//! fixture-prove KEYR-01 backward compatibility against the UNMODIFIED,
//! pre-existing `two_peers*.keyring` fixtures, plus the v1.1 multi-key
//! grammar's load-time invariants.

#![allow(clippy::unwrap_used, unused_crate_dependencies)]

use famp_core::Principal;
use famp_keyring::{Keyring, KeyringError};
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

const CANONICAL_FIXTURE: &str = "tests/fixtures/two_peers.canonical.keyring";
const HUMAN_FIXTURE: &str = "tests/fixtures/two_peers.keyring";
const MULTI_KEY_FIXTURE: &str = "tests/fixtures/multi_key.canonical.keyring";
use famp_keyring::KeyState;

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

// --- Phase 15 plan 02 Task 2: KEYR-01 backward-compat + multi-key fixture proof ---

#[test]
fn keyr01_legacy_fixture_loads_as_single_active_key() {
    let k = Keyring::load_from_file(Path::new(HUMAN_FIXTURE)).unwrap();
    assert_eq!(k.len(), 2);
    for p in [alice(), bob()] {
        let entries = k.entries(&p);
        assert_eq!(entries.len(), 1, "principal {p} must have exactly 1 entry");
        let e = &entries[0];
        assert_eq!(e.state(), KeyState::Active);
        assert!(e.valid_until().is_none());
        assert!(e.pinned_at().is_none());
        assert!(e.state_since().is_none());
    }
}

#[test]
fn keyr01_legacy_fixture_still_verifies_at_any_now() {
    let k = Keyring::load_from_file(Path::new(HUMAN_FIXTURE)).unwrap();
    for p in [alice(), bob()] {
        for now in ["1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z"] {
            let outcome = k.active_key(&p, now);
            assert!(
                matches!(outcome, famp_keyring::KeyLookupOutcome::Active(_)),
                "principal {p} at now={now}: expected Active, got {outcome:?}"
            );
        }
    }
}

#[test]
fn keyr01_legacy_fixture_saves_back_byte_identical() {
    let canonical = Keyring::load_from_file(Path::new(CANONICAL_FIXTURE)).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    canonical.save_to_file(tmp.path()).unwrap();
    let saved = std::fs::read(tmp.path()).unwrap();
    let original = std::fs::read(CANONICAL_FIXTURE).unwrap();
    assert_eq!(
        saved, original,
        "the v1.1 writer must not upgrade an untouched legacy file's shape"
    );
}

#[test]
fn multi_key_canonical_fixture_round_trips_byte_identical() {
    let k = Keyring::load_from_file(Path::new(MULTI_KEY_FIXTURE)).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    k.save_to_file(tmp.path()).unwrap();
    let saved = std::fs::read(tmp.path()).unwrap();
    let original = std::fs::read(MULTI_KEY_FIXTURE).unwrap();
    assert_eq!(
        saved, original,
        "multi-key fixture must round-trip byte-identical"
    );
}

#[test]
fn multi_key_fixture_exposes_both_entries() {
    let k = Keyring::load_from_file(Path::new(MULTI_KEY_FIXTURE)).unwrap();
    let entries = k.entries(&alice());
    assert_eq!(entries.len(), 2, "rotated principal must have 2 entries");
    let active_count = entries
        .iter()
        .filter(|e| e.state() == KeyState::Active)
        .count();
    assert_eq!(active_count, 1, "exactly one entry must be Active");
}

#[test]
fn two_active_entries_for_one_principal_rejected() {
    let content = "\
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w  active  2027-01-01T00:00:00Z  -  -
agent:local/alice  p_bfr484uJuozmSbWU-R5NAf3Ff5yUk99DteUKmYc2c  active  2027-01-01T00:00:00Z  -  -
";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let err = Keyring::load_from_file(tmp.path()).unwrap_err();
    match err {
        KeyringError::DuplicatePrincipal { principal, line } => {
            assert_eq!(principal, alice());
            assert_eq!(line, 2);
        }
        other => panic!("expected DuplicatePrincipal, got {other:?}"),
    }
}

#[test]
fn same_pubkey_twice_under_one_principal_rejected() {
    let content = "\
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w  active  2027-01-01T00:00:00Z  -  -
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w  retired  -  -  2026-01-01T00:00:00Z
";
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let err = Keyring::load_from_file(tmp.path()).unwrap_err();
    match err {
        KeyringError::DuplicateKeyEntry { principal, line } => {
            assert_eq!(principal, alice());
            assert_eq!(line, 2);
        }
        other => panic!("expected DuplicateKeyEntry, got {other:?}"),
    }
}

#[test]
fn non_canonical_timestamp_in_file_rejected_at_load() {
    for bad_line in [
        "agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w  active  2027-01-01T00:00:00+00:00  -  -\n",
        "agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w  active  2027-01-01T00:00:00.5Z  -  -\n",
    ] {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(bad_line.as_bytes()).unwrap();
        let err = Keyring::load_from_file(tmp.path()).unwrap_err();
        assert!(
            matches!(err, KeyringError::MalformedEntry { line: 1, .. }),
            "expected MalformedEntry at line 1, got {err:?}"
        );
    }
}

#[test]
fn wrong_field_count_rejected() {
    let pubkey = "iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w";
    for field_count in [3, 4, 5, 7] {
        let mut fields = vec!["agent:local/alice".to_string(), pubkey.to_string()];
        for i in 0..(field_count - 2) {
            fields.push(format!("extra{i}"));
        }
        let line = format!("{}\n", fields.join("  "));
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().write_all(line.as_bytes()).unwrap();
        let err = Keyring::load_from_file(tmp.path()).unwrap_err();
        assert!(
            matches!(err, KeyringError::MalformedEntry { line: 1, .. }),
            "field_count={field_count}: expected MalformedEntry, got {err:?}"
        );
    }
}
