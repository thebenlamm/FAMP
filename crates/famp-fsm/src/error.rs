//! Phase-local narrow error enum. Matches v0.6 Phase 1/2 precedent.
//! Does NOT convert to `famp_core::ProtocolErrorKind` — that mapping
//! happens at the Phase 3 runtime/transport boundary.

use famp_core::{MessageClass, TerminalStatus};

use crate::state::TaskState;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskFsmError {
    #[error("illegal transition: cannot apply class={class:?} terminal_status={terminal_status:?} from state={from:?}")]
    IllegalTransition {
        from: TaskState,
        class: MessageClass,
        terminal_status: Option<TerminalStatus>,
    },
}
