//! Shared test helpers for Phase 2 Plan 02-03 integration tests (and the
//! older Phase 1 cycle-driver consumers).
//!
//! Two coexisting access patterns:
//!
//! 1. Plan 02-03's listen integration tests (`listen_smoke`, `listen_durability`,
//!    `listen_bind_collision`, `listen_shutdown`, `listen_truncated_tail`)
//!    declare `mod common;` at the top of the file so Cargo builds this file
//!    as a shared test submodule. They consume the `listen_harness::*`
//!    helpers re-exported below.
//!
//! 2. Pre-existing tests (`http_happy_path.rs`, `cross_machine_happy_path.rs`,
//!    `example_happy_path.rs`) and the `cross_machine_two_agents` example use
//!    `#[path = "common/cycle_driver.rs"] mod cycle_driver;` to pull in the
//!    cycle driver directly as a sibling file. They are unaffected by this
//!    module's contents.
//!
//! Per-binary `dead_code` allowances: each Plan 02-03 test binary consumes a
//! different subset of these helpers, so we silence unused-per-binary warnings
//! at the module root.

#![allow(
    dead_code,
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub mod conversation_harness;
pub mod listen_harness;
pub mod two_daemon_harness;

pub use listen_harness::{
    build_signed_ack_bytes, build_trusting_reqwest_client, init_home_in_process, post_bytes,
    read_inbox_lines, read_stderr_bound_addr, self_principal, spawn_listen, wait_for_bind,
    ChildGuard,
};
