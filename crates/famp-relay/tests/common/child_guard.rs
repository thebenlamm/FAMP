//! Shared RAII guard for spawned child processes in integration tests.
//!
//! Holds a `std::process::Child` and kills + waits it on `Drop`, so a
//! test that panics (or returns early) before its explicit teardown
//! still reaps the child during unwind.
//!
//! Copied verbatim from `crates/famp-gateway/tests/common/child_guard.rs`
//! per the project's documented ChildGuard test convention — every test
//! that spawns a `famp-relay` (or `famp`/`famp-gateway`) child MUST wrap
//! it in this guard.

#![allow(dead_code)]

use std::process::Child;

/// RAII guard that kills + waits the child on drop.
pub struct ChildGuard(pub Option<Child>);

impl ChildGuard {
    #[must_use]
    pub const fn new(child: Child) -> Self {
        Self(Some(child))
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
