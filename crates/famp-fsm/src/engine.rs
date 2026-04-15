//! Transition engine. One step function is the only place legality is decided.

use famp_core::{MessageClass, TerminalStatus};

use crate::{error::TaskFsmError, input::TaskTransitionInput, state::TaskState};

#[derive(Debug, Clone)]
pub struct TaskFsm {
    state: TaskState,
}

impl TaskFsm {
    /// Construct a fresh task FSM in the `Requested` state.
    pub const fn new() -> Self {
        Self {
            state: TaskState::Requested,
        }
    }

    /// Current FSM state (cheap copy).
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// Apply one transition. Illegal transitions return
    /// `TaskFsmError::IllegalTransition` without mutating state. Terminal
    /// states have no outgoing arms.
    pub const fn step(&mut self, input: TaskTransitionInput) -> Result<TaskState, TaskFsmError> {
        let next = match (self.state, input.class, input.terminal_status) {
            (TaskState::Requested, MessageClass::Commit, None) => TaskState::Committed,
            (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Completed)) => {
                TaskState::Completed
            }
            (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Failed)) => {
                TaskState::Failed
            }
            // Both Requested and Committed can be cancelled via Control
            (TaskState::Requested | TaskState::Committed, MessageClass::Control, None) => {
                TaskState::Cancelled
            }
            _ => {
                return Err(TaskFsmError::IllegalTransition {
                    from: self.state,
                    class: input.class,
                    terminal_status: input.terminal_status,
                });
            }
        };
        self.state = next;
        Ok(next)
    }

    /// Resume an FSM from a persisted state (e.g. a task record loaded from
    /// disk). This is the Phase 4 replacement for the Phase 3
    /// `__with_state_for_testing` shortcut: it is public, intentional, and
    /// represents a legitimate "I know the current state from durable storage"
    /// operation rather than test-only seeding.
    ///
    /// `__with_state_for_testing` remains available for existing test code in
    /// `famp-fsm`'s own test suite but MUST NOT be used by `crates/famp/src/`.
    pub const fn resume(state: TaskState) -> Self {
        Self { state }
    }

    /// Test-only constructor that seeds the FSM in an arbitrary state.
    /// Public consumers must use `new()` or `resume()`. Hidden from rustdoc.
    #[doc(hidden)]
    pub const fn __with_state_for_testing(state: TaskState) -> Self {
        Self { state }
    }
}

impl Default for TaskFsm {
    fn default() -> Self {
        Self::new()
    }
}
