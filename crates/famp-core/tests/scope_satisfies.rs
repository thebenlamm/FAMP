//! Hand-written 5×5 truth table for `AuthorityScope::satisfies` (spec §5.3).
//!
//! Ladder (strict): `advisory` < `negotiate` < `commit_local` < `commit_delegate` < `transfer`.
//! `provided.satisfies(required)` iff `provided` rank ≥ `required` rank.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

use famp_core::AuthorityScope::{self, Advisory, CommitDelegate, CommitLocal, Negotiate, Transfer};

// provided, required, expected — 25 entries, hand-written per D-32.
const TABLE: &[(AuthorityScope, AuthorityScope, bool); 25] = &[
    // Advisory row
    (Advisory, Advisory, true),
    (Advisory, Negotiate, false),
    (Advisory, CommitLocal, false),
    (Advisory, CommitDelegate, false),
    (Advisory, Transfer, false),
    // Negotiate row
    (Negotiate, Advisory, true),
    (Negotiate, Negotiate, true),
    (Negotiate, CommitLocal, false),
    (Negotiate, CommitDelegate, false),
    (Negotiate, Transfer, false),
    // CommitLocal row
    (CommitLocal, Advisory, true),
    (CommitLocal, Negotiate, true),
    (CommitLocal, CommitLocal, true),
    (CommitLocal, CommitDelegate, false),
    (CommitLocal, Transfer, false),
    // CommitDelegate row
    (CommitDelegate, Advisory, true),
    (CommitDelegate, Negotiate, true),
    (CommitDelegate, CommitLocal, true),
    (CommitDelegate, CommitDelegate, true),
    (CommitDelegate, Transfer, false),
    // Transfer row
    (Transfer, Advisory, true),
    (Transfer, Negotiate, true),
    (Transfer, CommitLocal, true),
    (Transfer, CommitDelegate, true),
    (Transfer, Transfer, true),
];

#[test]
fn table_has_25_entries() {
    assert_eq!(TABLE.len(), 25);
}

#[test]
fn truth_table_matches() {
    for (provided, required, expected) in TABLE {
        assert_eq!(
            provided.satisfies(*required),
            *expected,
            "{provided:?}.satisfies({required:?})"
        );
    }
}

#[test]
fn reflexivity() {
    for scope in [Advisory, Negotiate, CommitLocal, CommitDelegate, Transfer] {
        assert!(scope.satisfies(scope));
    }
}

#[test]
fn transfer_satisfies_advisory() {
    assert!(Transfer.satisfies(Advisory));
}

#[test]
fn advisory_does_not_satisfy_negotiate() {
    assert!(!Advisory.satisfies(Negotiate));
}
