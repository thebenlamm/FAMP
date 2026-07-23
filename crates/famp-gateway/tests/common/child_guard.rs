//! Shared RAII guard for spawned child processes in integration tests.
//!
//! Holds a `std::process::Child` and kills + waits it on `Drop`, so a test
//! that panics (or returns early) before its explicit teardown still reaps
//! the child during unwind. Promoted out of `listen_harness.rs` so the
//! standalone test binaries (inspect_*, broker_*) can include it via
//! `#[path = "common/child_guard.rs"] mod child_guard;` without depending on
//! the (currently dormant) `common` listen helpers.
//!
//! Copied verbatim from `crates/famp/tests/common/child_guard.rs` per the
//! project's documented ChildGuard test convention (07-RESEARCH.md
//! Pitfall 3) — every test that spawns a `famp broker`/`famp register`/
//! `famp-gateway` child MUST wrap it in this guard.

#![allow(dead_code)]

use std::process::Child;

/// RAII guard that kills + waits the child on drop. Hold it for the
/// duration of the test body; let scope end (or `drop` it) to clean up
/// even on panic unwind.
pub struct ChildGuard(pub Option<Child>);

impl ChildGuard {
    #[must_use]
    pub const fn new(child: Child) -> Self {
        Self(Some(child))
    }

    pub const fn as_mut(&mut self) -> Option<&mut Child> {
        self.0.as_mut()
    }

    pub const fn take(&mut self) -> Option<Child> {
        self.0.take()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
