//! FSM-08: Proptest transition-legality matrix.
//!
//! Enumerates the full Cartesian product of (`TaskState` × `MessageClass` ×
//! `Option<TerminalStatus>`) = 5 × 5 × 4 = 100 tuples. For each tuple:
//! - If the tuple matches the authoritative transition table (5 legal arrows),
//!   assert `step` returns `Ok(expected_next)`.
//! - Otherwise assert `step` returns `Err(TaskFsmError::IllegalTransition {..})`
//!   with the EXACT offending tuple.
//! - Zero panics across the entire matrix.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies,
    clippy::match_same_arms
)]

use famp_core::{MessageClass, TerminalStatus};
use famp_fsm::{TaskFsm, TaskFsmError, TaskState, TaskTransitionInput};
use proptest::prelude::*;

fn arb_task_state() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Requested),
        Just(TaskState::Committed),
        Just(TaskState::Completed),
        Just(TaskState::Failed),
        Just(TaskState::Cancelled),
    ]
}

fn arb_message_class() -> impl Strategy<Value = MessageClass> {
    prop_oneof![
        Just(MessageClass::Request),
        Just(MessageClass::Commit),
        Just(MessageClass::Deliver),
        Just(MessageClass::Ack),
        Just(MessageClass::Control),
    ]
}

fn arb_terminal_status() -> impl Strategy<Value = Option<TerminalStatus>> {
    prop_oneof![
        Just(None),
        Just(Some(TerminalStatus::Completed)),
        Just(Some(TerminalStatus::Failed)),
        Just(Some(TerminalStatus::Cancelled)),
    ]
}

/// Authoritative legality oracle — MUST match engine.rs transition table exactly.
/// Returns `Some(next_state)` for legal arrows, None for illegal.
const fn expected_next(
    state: TaskState,
    class: MessageClass,
    ts: Option<TerminalStatus>,
) -> Option<TaskState> {
    match (state, class, ts) {
        (TaskState::Requested, MessageClass::Commit, None) => Some(TaskState::Committed),
        (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Completed)) => {
            Some(TaskState::Completed)
        }
        (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Failed)) => {
            Some(TaskState::Failed)
        }
        (TaskState::Requested | TaskState::Committed, MessageClass::Control, None) => {
            Some(TaskState::Cancelled)
        }
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn fsm_transition_legality(
        state in arb_task_state(),
        class in arb_message_class(),
        ts    in arb_terminal_status(),
    ) {
        let input = TaskTransitionInput { class, terminal_status: ts };
        let mut fsm = TaskFsm::__with_state_for_testing(state);
        let result = fsm.step(input);

        match (expected_next(state, class, ts), result) {
            (Some(expected), Ok(got)) => {
                prop_assert_eq!(got, expected);
                prop_assert_eq!(fsm.state(), expected);
            }
            (None, Err(TaskFsmError::IllegalTransition { from, class: c, terminal_status })) => {
                prop_assert_eq!(from, state);
                prop_assert_eq!(c, class);
                prop_assert_eq!(terminal_status, ts);
                prop_assert_eq!(fsm.state(), state, "state must not mutate on illegal transition");
            }
            (Some(expected), Err(e)) => {
                prop_assert!(false, "expected Ok({:?}), got Err({:?}) for ({:?}, {:?}, {:?})", expected, e, state, class, ts);
            }
            (None, Ok(got)) => {
                prop_assert!(false, "expected IllegalTransition, got Ok({:?}) for ({:?}, {:?}, {:?})", got, state, class, ts);
            }
        }
    }
}

/// Explicit coverage check: assert the 5 legal arrows are hit at least once
/// in a deterministic pass. Proptest random sampling eventually covers all
/// 100 combinations, but this test documents the legal arrows directly.
#[test]
fn all_five_legal_arrows_covered_by_oracle() {
    assert_eq!(
        expected_next(TaskState::Requested, MessageClass::Commit, None),
        Some(TaskState::Committed)
    );
    assert_eq!(
        expected_next(
            TaskState::Committed,
            MessageClass::Deliver,
            Some(TerminalStatus::Completed)
        ),
        Some(TaskState::Completed)
    );
    assert_eq!(
        expected_next(
            TaskState::Committed,
            MessageClass::Deliver,
            Some(TerminalStatus::Failed)
        ),
        Some(TaskState::Failed)
    );
    assert_eq!(
        expected_next(TaskState::Requested, MessageClass::Control, None),
        Some(TaskState::Cancelled)
    );
    assert_eq!(
        expected_next(TaskState::Committed, MessageClass::Control, None),
        Some(TaskState::Cancelled)
    );
}

/// Spot-check: oracle rejects a representative sample of illegal tuples.
#[test]
fn oracle_rejects_known_illegal_tuples() {
    assert_eq!(
        expected_next(
            TaskState::Requested,
            MessageClass::Deliver,
            Some(TerminalStatus::Completed)
        ),
        None
    );
    assert_eq!(
        expected_next(TaskState::Committed, MessageClass::Request, None),
        None
    );
    assert_eq!(
        expected_next(TaskState::Completed, MessageClass::Commit, None),
        None
    );
    assert_eq!(
        expected_next(
            TaskState::Failed,
            MessageClass::Deliver,
            Some(TerminalStatus::Completed)
        ),
        None
    );
    assert_eq!(
        expected_next(TaskState::Cancelled, MessageClass::Control, None),
        None
    );
    assert_eq!(
        expected_next(TaskState::Requested, MessageClass::Ack, None),
        None
    );
    assert_eq!(
        expected_next(
            TaskState::Committed,
            MessageClass::Deliver,
            Some(TerminalStatus::Cancelled)
        ),
        None
    );
}
