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

// `unsafe_code` is `deny` (not `forbid`) at the crate level so
// `bus_client::spawn` can opt in via a single narrowly-scoped
// `#[allow(unsafe_code)]` for the locked Q1 portable broker spawn
// pattern (`Command::new` + child-side `pre_exec(setsid)` per RESEARCH).
// Every other module keeps the `deny` posture.
#![deny(unsafe_code)]

// These crates are used by examples and integration tests, not by the
// library compile unit directly. Silence the workspace
// `unused_crate_dependencies` lint here.
#[cfg(test)]
use axum as _;
use base64 as _;
use ed25519_dalek as _;
use famp_transport as _;
use famp_transport_http as _;
use rand as _;
#[cfg(test)]
use reqwest as _;
use tempfile as _;
use url as _;
// `uuid` is consumed transitively by `famp-bus` types we re-thread (the
// CLI used to construct `MessageId::new_v7()` directly; Phase 02 Plan
// 02-04 swapped that for the broker-assigned `task_id`). Keep the
// dependency declared so workspace consumers and integration tests can
// reach `uuid::Uuid` parsing helpers without re-adding the dep.
use uuid as _;
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
