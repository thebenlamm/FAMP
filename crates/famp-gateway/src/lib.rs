//! `famp-gateway` — FAMP v0.11 Layer 2 gateway skeleton.
//!
//! Backs each remote (cross-host) principal with its own plain-`Register`
//! UDS connection to the local broker, carrying the gateway's own
//! `std::process::id()` — Design A's resolution to the same-host
//! `kill(pid,0)` liveness fork (LIVE-01/LIVE-02). See `principal` and
//! `registry` for the mechanism; zero `famp-bus` source change.

#![forbid(unsafe_code)]

// Silencer: `tokio` is only used by the `[[bin]]` (main.rs is a separate
// compilation unit); the lib target itself has no direct tokio reference
// (async/await here is plain language syntax, not a tokio API call).
use tokio as _;

// Silencer for dev-only dependencies: these are used exclusively by the
// `tests/liveness.rs` / `tests/no_cross_talk.rs` integration test
// binaries (07-03), which are separate compilation units from this lib
// target. `#[cfg(test)]` because they are dev-dependencies, unavailable
// to non-test builds.
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use famp_inspect_proto as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tempfile as _;
#[cfg(test)]
use uuid as _;

pub mod error;
pub mod principal;
pub mod registry;
pub mod verify;

pub use error::{GatewayError, RejectReason};
pub use principal::ProxiedPrincipal;
pub use registry::GatewayRegistry;
pub use verify::verify_inbound;
