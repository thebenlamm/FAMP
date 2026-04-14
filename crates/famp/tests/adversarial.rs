//! Adversarial matrix: CONF-05/06/07 × {MemoryTransport, HttpTransport}.
//!
//! Phase 4 Plan 04-05 promoted this from a monolithic file to a directory
//! module with shared case definitions + one adapter per transport.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

// Silence dev-deps consumed only by child modules.
use ed25519_dalek as _;
use rand as _;
use thiserror as _;

#[path = "adversarial/harness.rs"]
mod harness;
#[path = "adversarial/fixtures.rs"]
mod fixtures;
#[path = "adversarial/memory.rs"]
mod memory;
#[path = "adversarial/http.rs"]
mod http;
