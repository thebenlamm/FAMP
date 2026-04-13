#![forbid(unsafe_code)]

//! `famp-envelope` — FAMP v0.5.1 signed envelope reference implementation.
//!
//! CRITICAL: do NOT refactor the envelope to use `#[serde(flatten)]` or
//! `#[serde(tag = ...)]` on a Body enum. See RESEARCH.md Pitfalls 1 and 2.
//! This composition is the only pattern that actually enforces
//! `deny_unknown_fields` on both envelope and body in serde 1.0.228.

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
