//! `famp-gateway` — FAMP v0.11 Layer 2 gateway skeleton.
//!
//! Backs each remote (cross-host) principal with its own plain-`Register`
//! UDS connection to the local broker, carrying the gateway's own
//! `std::process::id()` — Design A's resolution to the same-host
//! `kill(pid,0)` liveness fork (LIVE-01/LIVE-02). See `principal` and
//! `registry` for the mechanism; zero `famp-bus` source change.

#![forbid(unsafe_code)]

// Silencers for dependencies not yet wired by this task. `famp`/`famp-bus`/
// `tokio` land in Task 2 (principal.rs register flow + parking bin); remove
// each line as the matching module starts using it.
use famp as _;
use famp_bus as _;
use tokio as _;

// Silencer for the dev-only dependency: no test file in this crate uses
// it yet (lands in a later plan in this phase). Remove once wired.
// `#[cfg(test)]` because assert_cmd is a dev-dependency, unavailable to
// non-test builds.
#[cfg(test)]
use assert_cmd as _;

pub mod error;
pub mod principal;
pub mod registry;

pub use error::GatewayError;
pub use principal::ProxiedPrincipal;
pub use registry::GatewayRegistry;
