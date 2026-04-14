//! On-disk task record schema.
//!
//! Wire vocabulary for the `state` field (uppercase): `"REQUESTED" |
//! "COMMITTED" | "COMPLETED" | "FAILED" | "CANCELLED"`. The field is a
//! plain `String` rather than a `famp_fsm::TaskState` so the file format
//! survives future FSM refactors. Mapping to/from the FSM enum lives in
//! the consumer (see `crates/famp/src/cli/` — Plan 03-02).

use serde::{Deserialize, Serialize};

/// One task record, round-trip-stable via TOML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskRecord {
    /// `UUIDv7` string form, identical to the envelope `task_id` field.
    pub task_id: String,
    /// FSM state in uppercase string form (see module docs).
    pub state: String,
    /// Peer alias from `peers.toml`.
    pub peer: String,
    /// RFC 3339 timestamp.
    pub opened_at: String,
    /// RFC 3339 timestamp; `None` until first successful send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_send_at: Option<String>,
    /// RFC 3339 timestamp; `None` until first matching inbox entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_recv_at: Option<String>,
    /// Mirror of `state` being terminal — denormalized for cheap
    /// `famp send --task` rejection without parsing state strings.
    pub terminal: bool,
}

impl TaskRecord {
    /// Build a fresh record in the `REQUESTED` state.
    pub fn new_requested(task_id: String, peer: String, now_rfc3339: String) -> Self {
        Self {
            task_id,
            state: "REQUESTED".to_string(),
            peer,
            opened_at: now_rfc3339,
            last_send_at: None,
            last_recv_at: None,
            terminal: false,
        }
    }
}
