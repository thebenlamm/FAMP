//! Envelope-field and timestamp parsers shared across the inspect handlers.
//!
//! Pure functions over already-parsed `serde_json::Value` envelopes plus the
//! RFC3339 / `SystemTime` epoch helpers. Extracted from `lib.rs` so each
//! inspect-kind handler (`tasks`, `messages`, …) depends on one parsing module
//! instead of the dispatcher carrying parser utilities.

use std::time::{SystemTime, UNIX_EPOCH};

use famp_envelope::{body::TerminalStatus, MessageClass};
use famp_fsm::{TaskFsm, TaskState, TaskTransitionInput};

/// Parse the top-level `class` field as a typed `MessageClass`.
fn parse_class(env: &serde_json::Value) -> Option<MessageClass> {
    env.get("class")
        .and_then(|v| serde_json::from_value::<MessageClass>(v.clone()).ok())
}

/// Parse the top-level `terminal_status` header field as a `TerminalStatus`.
fn parse_terminal_status(env: &serde_json::Value) -> Option<TerminalStatus> {
    env.get("terminal_status")
        .and_then(|v| serde_json::from_value::<TerminalStatus>(v.clone()).ok())
}

/// Map FSM state to uppercase state label.
const fn state_label(s: TaskState) -> &'static str {
    match s {
        TaskState::Requested => "REQUESTED",
        TaskState::Committed => "COMMITTED",
        TaskState::Completed => "COMPLETED",
        TaskState::Failed => "FAILED",
        TaskState::Cancelled => "CANCELLED",
    }
}

/// Engine-backed FSM fold: applies typed envelopes in sequence,
/// mirroring `famp-fsm::TaskFsm::step` exactly. Single source of truth
/// is the engine (crates/famp-fsm/src/engine.rs lines 29–48), not a
/// hand-written truth table.
#[derive(Debug, Clone, Default)]
pub struct FsmFold {
    fsm: TaskFsm,
    saw_fsm_class: bool,
}

impl FsmFold {
    /// Create a fresh fold in `Requested` state (the request envelope
    /// is the FSM's birth event, not a transition).
    pub const fn new() -> Self {
        Self {
            fsm: TaskFsm::new(),
            saw_fsm_class: false,
        }
    }

    /// Apply one envelope to the fold, returning the state label AFTER
    /// this envelope. Illegal transitions are absorbed (engine leaves
    /// state unchanged; see engine.rs lines 41–47).
    pub fn apply(&mut self, env: &serde_json::Value) -> String {
        // Parse class and terminal_status; fall through to "UNKNOWN" if unparseable.
        let class = match parse_class(env) {
            None | Some(MessageClass::AuditLog) => return "UNKNOWN".to_string(),
            Some(c) => c,
        };

        match class {
            MessageClass::Request => {
                self.saw_fsm_class = true;
                state_label(self.fsm.state()).to_string()
            }
            MessageClass::Ack => {
                // Ack drives no transition (engine has no Ack arm).
                // If we've seen an FSM-class envelope, report the absorbed state;
                // otherwise "UNKNOWN" (a lone ack determines no state).
                if self.saw_fsm_class {
                    state_label(self.fsm.state()).to_string()
                } else {
                    "UNKNOWN".to_string()
                }
            }
            MessageClass::Commit | MessageClass::Deliver | MessageClass::Control => {
                self.saw_fsm_class = true;
                let terminal_status = parse_terminal_status(env);
                let input = TaskTransitionInput {
                    class,
                    terminal_status,
                };
                // Discard the step result: Err = illegal = state NOT mutated.
                // The engine guarantees this (engine.rs lines 41–47).
                if self.fsm.step(input).is_err() {
                    // Illegal transition: engine leaves state unchanged.
                }
                state_label(self.fsm.state()).to_string()
            }
            MessageClass::AuditLog => unreachable!("filtered out above"),
        }
    }

    /// Current state label after folding all envelopes.
    pub fn final_label(&self) -> String {
        if self.saw_fsm_class {
            state_label(self.fsm.state()).to_string()
        } else {
            "UNKNOWN".to_string()
        }
    }
}

/// Derive FSM state from envelope fields using canonical class strings
/// and `famp_core::TerminalStatus` `snake_case` mode strings.
/// Context-free single-envelope view (still used by `messages.rs`).
pub fn derive_fsm_state(env: &serde_json::Value) -> String {
    match parse_class(env) {
        None | Some(MessageClass::AuditLog) => "UNKNOWN".to_string(),
        Some(MessageClass::Request) => "REQUESTED".to_string(),
        Some(MessageClass::Commit) => "COMMITTED".to_string(),
        Some(MessageClass::Deliver) => {
            match parse_terminal_status(env) {
                Some(TerminalStatus::Completed) => "COMPLETED".to_string(),
                Some(TerminalStatus::Failed) => "FAILED".to_string(),
                // Deliver + terminal_status: "cancelled" is illegal wire combo
                // (engine.rs: Cancelled arrives via Control, not Deliver).
                Some(TerminalStatus::Cancelled) => "UNKNOWN".to_string(),
                // Interim deliver (no terminal_status): task remains Committed.
                None => "COMMITTED".to_string(),
            }
        }
        Some(MessageClass::Control) => "CANCELLED".to_string(),
        Some(MessageClass::Ack) => {
            // Ack drives no FSM transition (famp-fsm has no Ack arm;
            // disposition is not FSM-observable — see §9.6 discipline).
            // A lone ack determines no state; task-level views use FsmFold instead.
            "UNKNOWN".to_string()
        }
    }
}

/// Best-effort RFC3339 -> epoch seconds.
pub fn parse_rfc3339_to_epoch(s: &str) -> Option<u64> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .ok()
        .and_then(|dt| u64::try_from(dt.unix_timestamp()).ok())
}

pub fn to_epoch_seconds(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
