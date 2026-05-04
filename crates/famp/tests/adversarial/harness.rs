//! Shared adversarial harness — Phase 4 D-D2/D-D3.
//!
//! ONE definition of the three cases, ONE expected-error map, ONE assertion
//! shape. Each transport adapter in sibling modules calls
//! `assert_expected_error`; HttpTransport synthesizes a `RuntimeError` from
//! the observed HTTP status+slug before calling this function.

#![allow(dead_code, clippy::match_same_arms)]

use famp_envelope::EnvelopeDecodeError;

#[path = "../common/cycle_driver.rs"]
pub mod cycle_driver;
pub use cycle_driver::RuntimeError;

#[derive(Debug, Clone, Copy)]
pub enum Case {
    Unsigned,
    WrongKey,
    CanonicalDivergence,
}

/// D-D6 expected-error mapping (RuntimeError side).
pub fn assert_expected_error(case: Case, err: &RuntimeError) {
    match (case, err) {
        (Case::Unsigned, RuntimeError::Decode(EnvelopeDecodeError::MissingSignature)) => {}
        (Case::WrongKey, RuntimeError::Decode(EnvelopeDecodeError::SignatureInvalid)) => {}
        (Case::CanonicalDivergence, RuntimeError::CanonicalDivergence) => {}
        _ => panic!("case {case:?} produced unexpected error: {err:?}"),
    }
}
