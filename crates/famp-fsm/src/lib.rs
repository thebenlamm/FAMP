#![forbid(unsafe_code)]
//! famp-fsm — 5-state task lifecycle FSM for FAMP v0.7 Personal Runtime.
//!
//! See `.planning/phases/02-minimal-task-lifecycle/02-CONTEXT.md` for
//! the authoritative decision log. v0.7 is single-instance; competing-
//! instance commit races (§11.5a) defer to Federation Profile v0.8+.

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib compile unit.
#[cfg(test)]
use proptest as _;

pub mod engine;
pub mod error;
pub mod input;
pub mod state;

pub use engine::TaskFsm;
pub use error::TaskFsmError;
pub use input::TaskTransitionInput;
pub use state::TaskState;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles_and_links() {
        // Smoke test per D-25: ensures nextest reports >0 tests per crate
        // so a broken runner fails loudly instead of silently passing.
    }
}
