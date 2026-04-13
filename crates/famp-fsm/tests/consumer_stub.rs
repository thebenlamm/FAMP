//! FSM-03: Downstream consumer stub. This file simulates an external crate
//! that exhaustively matches `TaskState`. The `#![deny(unreachable_patterns)]`
//! attribute plus the exhaustive `match` (zero catch-all arms) means that:
//!
//! - Adding a new `TaskState` variant → "non-exhaustive patterns" compile error
//! - Removing a `TaskState` variant → "pattern does not cover" compile error
//! - Adding an unreachable arm → denied by the lint
//!
//! This is the INV-5 compile-time gate for v0.7 Personal Profile.
//! See `famp-core/tests/exhaustive_consumer_stub.rs` for the precedent.

#![deny(unreachable_patterns)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies,
    clippy::match_same_arms,
)]

use famp_fsm::TaskState;

/// Exhaustive describe — zero catch-all arms. Must list all 5 variants explicitly.
const fn describe_state(s: TaskState) -> &'static str {
    match s {
        TaskState::Requested => "requested",
        TaskState::Committed => "committed",
        TaskState::Completed => "completed",
        TaskState::Failed    => "failed",
        TaskState::Cancelled => "cancelled",
    }
}

/// Exhaustive `is_terminal` classifier — zero catch-all arms.
const fn is_terminal(s: TaskState) -> bool {
    match s {
        TaskState::Requested => false,
        TaskState::Committed => false,
        TaskState::Completed => true,
        TaskState::Failed    => true,
        TaskState::Cancelled => true,
    }
}

#[test]
fn describe_every_variant() {
    assert_eq!(describe_state(TaskState::Requested), "requested");
    assert_eq!(describe_state(TaskState::Committed), "committed");
    assert_eq!(describe_state(TaskState::Completed), "completed");
    assert_eq!(describe_state(TaskState::Failed),    "failed");
    assert_eq!(describe_state(TaskState::Cancelled), "cancelled");
}

#[test]
fn terminal_classification_is_exhaustive() {
    assert!(!is_terminal(TaskState::Requested));
    assert!(!is_terminal(TaskState::Committed));
    assert!(is_terminal(TaskState::Completed));
    assert!(is_terminal(TaskState::Failed));
    assert!(is_terminal(TaskState::Cancelled));
}
