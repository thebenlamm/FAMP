//! Narrow decoded input for `TaskFsm::step`.
//!
//! Derived from the §7.3a FSM-observable whitelist. famp-fsm never parses
//! JSON, never verifies signatures, never touches wire bytes — callers
//! (Phase 3 transport glue) extract these fields from a decoded envelope.

use famp_core::{MessageClass, TerminalStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskTransitionInput {
    pub class: MessageClass,
    pub terminal_status: Option<TerminalStatus>,
}
