//! FAMP top-level crate — runtime composition of envelope, crypto,
//! canonical JSON, FSM, transport, and keyring. Examples live under
//! `examples/`; integration tests under `tests/`.
//!
//! # Public API
//!
//! This crate re-exports the minimal protocol surface from `famp-core`,
//! `famp-envelope`, `famp-crypto`, and `famp-canonical` so callers can
//! write `famp::Principal`, `famp::SignedEnvelope`, `famp::sign_value`
//! without tracking which member crate owns each type. Rustdoc is
//! preserved automatically by Rust's `pub use`.
//!
//! For advanced usage (body schemas, FSM, transport, keyring, raw
//! canonical-JSON primitives beyond `canonicalize` / strict parse),
//! import the member crates directly.

#![forbid(unsafe_code)]

// These crates are used by Task 2 (loop_fn), examples, and integration tests.
// Silence the workspace `unused_crate_dependencies` lint for the lib compile
// unit (examples and tests are separate compile units).
#[cfg(test)]
use axum as _;
use base64 as _;
// `dirs`, `famp_bus`, and `nix` are wired up by the bus_client/identity
// modules in plan 02-01 task 2; the task-1 placeholders do not yet
// reference them, so silence the workspace `unused_crate_dependencies`
// lint until task 2 lands.
use dirs as _;
use ed25519_dalek as _;
use famp_bus as _;
use famp_transport as _;
use famp_transport_http as _;
use nix as _;
use rand as _;
#[cfg(test)]
use reqwest as _;
use tempfile as _;
use tokio as _;
use url as _;
// `assert_cmd` is a dev-dependency consumed by integration tests
// (`crates/famp/tests/*`); silence it in the library test compile unit.
#[cfg(test)]
use assert_cmd as _;

pub use famp_canonical::{canonicalize, from_slice_strict, from_str_strict, CanonicalError};
pub use famp_core::{
    ArtifactId, AuthorityScope, Instance, MessageId, Principal, ProtocolError, ProtocolErrorKind,
    TerminalStatus,
};
pub use famp_crypto::{
    sign_canonical_bytes, sign_value, verify_canonical_bytes, verify_value, CryptoError,
    FampSignature, FampSigningKey, TrustedVerifyingKey, DOMAIN_PREFIX,
};
pub use famp_envelope::{
    AnySignedEnvelope, EnvelopeDecodeError, EnvelopeScope, MessageClass, SignedEnvelope, Timestamp,
    UnsignedEnvelope, FAMP_SPEC_VERSION,
};

pub mod bus_client;
pub mod cli;
pub mod runtime;
