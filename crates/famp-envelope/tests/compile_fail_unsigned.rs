//! INV-10 `compile_fail` gates live as doctests on `SignedEnvelope` in
//! `src/envelope.rs`. Run `cargo test -p famp-envelope --doc` to exercise
//! them. This file exists as a grep-discoverable marker for reviewers
//! looking for "where is INV-10 enforced at the type level".

#![allow(clippy::unwrap_used)]

// Dev-deps that have to be acknowledged by every integration test crate
// or the workspace `unused_crate_dependencies = "warn"` lint fires.
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use serde_json as _;
use thiserror as _;

#[test]
fn inv10_doctest_marker_exists() {
    // The real assertion runs in `cargo test -p famp-envelope --doc` on the
    // two `compile_fail` blocks attached to `SignedEnvelope` in envelope.rs.
}
