//! FAMP top-level crate — runtime composition of envelope, crypto,
//! canonical JSON, FSM, transport, and keyring. Examples live under
//! `examples/`; integration tests under `tests/`.

#![forbid(unsafe_code)]

// These crates are used by Task 2 (loop_fn) and integration tests. Silence
// the workspace `unused_crate_dependencies` lint until then.
use famp_crypto as _;
use famp_transport as _;
use tokio as _;

pub mod runtime;
