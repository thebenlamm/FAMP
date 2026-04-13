//! `famp-envelope` — FAMP v0.5.1 signed envelope reference implementation.
//!
//! CRITICAL: do NOT refactor the envelope to use `#[serde(flatten)]` or
//! `#[serde(tag = ...)]` on a Body enum. See RESEARCH.md Pitfalls 1 and 2.
//! This composition is the only pattern that actually enforces
//! `deny_unknown_fields` on both envelope and body in serde 1.0.228.

#![forbid(unsafe_code)]

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib compile unit.
#[cfg(test)]
use hex as _;
#[cfg(test)]
use insta as _;
#[cfg(test)]
use proptest as _;
// `serde_json` is consumed by tests; silence the lib-compile-unit warning.
use serde_json as _;

pub mod class;
pub mod error;
pub mod scope;
pub mod timestamp;
pub mod version;
pub(crate) mod wire;

pub use class::MessageClass;
pub use error::EnvelopeDecodeError;
pub use scope::EnvelopeScope;
pub use timestamp::Timestamp;
pub use version::{FampVersion, FAMP_SPEC_VERSION};
