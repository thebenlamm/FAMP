//! FAMP top-level crate — runtime composition of envelope, crypto,
//! canonical JSON, FSM, transport, and keyring. Examples live under
//! `examples/`; integration tests under `tests/`.

#![forbid(unsafe_code)]

// These crates are used by Task 2 (loop_fn), examples, and integration tests.
// Silence the workspace `unused_crate_dependencies` lint for the lib compile
// unit (examples and tests are separate compile units).
#[cfg(test)]
use axum as _;
use base64 as _;
use ed25519_dalek as _;
use famp_crypto as _;
use famp_transport as _;
use famp_transport_http as _;
use rand as _;
#[cfg(test)]
use rcgen as _;
#[cfg(test)]
use reqwest as _;
#[cfg(test)]
use tempfile as _;
use tokio as _;
use url as _;

pub mod runtime;
