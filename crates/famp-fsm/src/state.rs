//! Task lifecycle state — 5 variants, narrowed for v0.7 Personal Profile.
//!
//! `REJECTED`, `EXPIRED`, and `COMMITTED_PENDING_RESOLUTION` are intentionally
//! absent (not optional, not feature-gated). See FSM-02 (narrowed) and
//! `.planning/phases/02-minimal-task-lifecycle/02-CONTEXT.md` D-C1..C3.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Requested,
    Committed,
    Completed,
    Failed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn task_state_serializes_snake_case() {
        let s = serde_json::to_string(&TaskState::Requested).unwrap();
        assert_eq!(s, "\"requested\"");
        let s = serde_json::to_string(&TaskState::Cancelled).unwrap();
        assert_eq!(s, "\"cancelled\"");
    }
}
