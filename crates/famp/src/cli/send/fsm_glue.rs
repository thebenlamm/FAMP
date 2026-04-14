//! Bridge between the on-disk `TaskRecord` (string state) and the in-memory
//! `famp_fsm::TaskFsm` (typed state).
//!
//! ## Phase 3 shortcut (TODO(phase4))
//!
//! v0.7's `TaskFsm` only allows `Committed → Completed/Failed` on a deliver
//! with `terminal_status`. A proper flow would be:
//! `REQUESTED →(commit reply)→ COMMITTED →(terminal deliver)→ COMPLETED`.
//!
//! Phase 3 does NOT round-trip a real `commit` reply yet (that lands in
//! Phase 4 alongside MCP + E2E). To keep `famp send --terminal` functional
//! against a Phase 2 listener, [`advance_terminal`] SEEDS a fresh FSM at
//! `Committed` and steps it with the terminal deliver, producing
//! `Completed`. The local record's `state` is rewritten accordingly.
//!
//! This is a deliberate known limitation documented in 03-02-SUMMARY.md.

use famp_core::{MessageClass, TerminalStatus};
use famp_fsm::{TaskFsm, TaskState, TaskTransitionInput};
use famp_taskdir::TaskRecord;

use crate::cli::error::CliError;

pub const fn state_to_str(s: TaskState) -> &'static str {
    match s {
        TaskState::Requested => "REQUESTED",
        TaskState::Committed => "COMMITTED",
        TaskState::Completed => "COMPLETED",
        TaskState::Failed => "FAILED",
        TaskState::Cancelled => "CANCELLED",
    }
}

pub const fn is_terminal(s: TaskState) -> bool {
    matches!(
        s,
        TaskState::Completed | TaskState::Failed | TaskState::Cancelled
    )
}

/// Advance a task record on a terminal deliver. Returns the new FSM state.
///
/// See module docs for the Phase 3 FSM seeding shortcut.
pub fn advance_terminal(record: &mut TaskRecord) -> Result<TaskState, CliError> {
    let mut fsm = TaskFsm::__with_state_for_testing(TaskState::Committed);
    let next = fsm
        .step(TaskTransitionInput {
            class: MessageClass::Deliver,
            terminal_status: Some(TerminalStatus::Completed),
        })
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    record.state = state_to_str(next).to_string();
    record.terminal = is_terminal(next);
    Ok(next)
}
