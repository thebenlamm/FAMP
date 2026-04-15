//! Bridge between the on-disk `TaskRecord` (string state) and the in-memory
//! `famp_fsm::TaskFsm` (typed state).
//!
//! ## Phase 4: FSM walks naturally
//!
//! The full commit-reply handshake (Phase 4 Plan 04-01) drives the local
//! task record through the real three-state path:
//!
//! ```text
//! REQUESTED →(commit reply via famp await)→ COMMITTED
//!           →(terminal deliver)→ COMPLETED
//! ```
//!
//! Both transitions use `TaskFsm::resume(current_state)` to reconstruct the
//! FSM from the persisted state, then call `step()` with the appropriate
//! input. This produces a verifiable audit trail on disk — only real signed
//! envelope round-trips can advance the state.
//!
//! The FSM shortcut that was used in Phase 3 tests is absent from this file.
//! The grep gate in `conversation_auto_commit` confirms this at CI time.

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

/// Parse the on-disk state string into a typed `TaskState`.
fn parse_state(s: &str) -> Result<TaskState, CliError> {
    match s {
        "REQUESTED" => Ok(TaskState::Requested),
        "COMMITTED" => Ok(TaskState::Committed),
        "COMPLETED" => Ok(TaskState::Completed),
        "FAILED" => Ok(TaskState::Failed),
        "CANCELLED" => Ok(TaskState::Cancelled),
        other => Err(CliError::Envelope(Box::new(std::io::Error::other(
            format!("unknown task state: {other}"),
        )))),
    }
}

/// Advance a task record from REQUESTED → COMMITTED on receiving a
/// `MessageClass::Commit` envelope. Returns the new `TaskState`.
///
/// Precondition: `record.state == "REQUESTED"`. An FSM in any other state
/// will return `TaskFsmError::IllegalTransition` mapped to `CliError::Envelope`.
///
/// Called by `await_cmd` when a commit-class inbox entry matches a local task.
pub fn advance_committed(record: &mut TaskRecord) -> Result<TaskState, CliError> {
    let current = parse_state(&record.state)?;
    let mut fsm = TaskFsm::resume(current);
    let next = fsm
        .step(TaskTransitionInput {
            class: MessageClass::Commit,
            terminal_status: None,
        })
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    record.state = state_to_str(next).to_string();
    record.terminal = is_terminal(next);
    Ok(next)
}

/// Advance a task record from COMMITTED → COMPLETED via a terminal deliver.
/// Returns the new `TaskState`.
///
/// Precondition: `record.state == "COMMITTED"`. An FSM in any other state
/// returns `TaskFsmError::IllegalTransition` mapped to `CliError::Envelope`.
/// In particular, calling this when the record is still in REQUESTED will
/// now correctly error — the caller must wait for the commit reply first
/// (which `famp await` handles via `advance_committed`).
pub fn advance_terminal(record: &mut TaskRecord) -> Result<TaskState, CliError> {
    let current = parse_state(&record.state)?;
    let mut fsm = TaskFsm::resume(current);
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
