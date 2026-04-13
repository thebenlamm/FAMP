//! Deterministic fixture tests — documents the 5 legal v0.7 arrows and
//! terminal immutability as code. (D-F2, D-F3)
#![allow(
    clippy::unwrap_used,
    clippy::missing_const_for_fn,
    unused_crate_dependencies
)]

use famp_core::{MessageClass, TerminalStatus};
use famp_fsm::{TaskFsm, TaskFsmError, TaskState, TaskTransitionInput};

fn input(class: MessageClass, ts: Option<TerminalStatus>) -> TaskTransitionInput {
    TaskTransitionInput { class, terminal_status: ts }
}

#[test]
fn new_starts_in_requested() {
    let fsm = TaskFsm::new();
    assert_eq!(fsm.state(), TaskState::Requested);
}

#[test]
fn requested_commit_to_committed() {
    let mut fsm = TaskFsm::new();
    let next = fsm.step(input(MessageClass::Commit, None)).unwrap();
    assert_eq!(next, TaskState::Committed);
    assert_eq!(fsm.state(), TaskState::Committed);
}

#[test]
fn committed_deliver_completed_to_completed() {
    let mut fsm = TaskFsm::__with_state_for_testing(TaskState::Committed);
    let next = fsm.step(input(MessageClass::Deliver, Some(TerminalStatus::Completed))).unwrap();
    assert_eq!(next, TaskState::Completed);
}

#[test]
fn committed_deliver_failed_to_failed() {
    let mut fsm = TaskFsm::__with_state_for_testing(TaskState::Committed);
    let next = fsm.step(input(MessageClass::Deliver, Some(TerminalStatus::Failed))).unwrap();
    assert_eq!(next, TaskState::Failed);
}

#[test]
fn requested_control_to_cancelled() {
    let mut fsm = TaskFsm::new();
    let next = fsm.step(input(MessageClass::Control, None)).unwrap();
    assert_eq!(next, TaskState::Cancelled);
}

#[test]
fn committed_control_to_cancelled() {
    let mut fsm = TaskFsm::__with_state_for_testing(TaskState::Committed);
    let next = fsm.step(input(MessageClass::Control, None)).unwrap();
    assert_eq!(next, TaskState::Cancelled);
}

#[test]
fn terminal_completed_rejects_every_input() {
    check_terminal_is_stuck(TaskState::Completed);
}

#[test]
fn terminal_failed_rejects_every_input() {
    check_terminal_is_stuck(TaskState::Failed);
}

#[test]
fn terminal_cancelled_rejects_every_input() {
    check_terminal_is_stuck(TaskState::Cancelled);
}

fn check_terminal_is_stuck(terminal: TaskState) {
    let classes = [
        MessageClass::Request,
        MessageClass::Commit,
        MessageClass::Deliver,
        MessageClass::Ack,
        MessageClass::Control,
    ];
    let ts_values = [
        None,
        Some(TerminalStatus::Completed),
        Some(TerminalStatus::Failed),
        Some(TerminalStatus::Cancelled),
    ];
    for class in classes {
        for ts in ts_values {
            let mut fsm = TaskFsm::__with_state_for_testing(terminal);
            let result = fsm.step(input(class, ts));
            match result {
                Err(TaskFsmError::IllegalTransition { from, class: c, terminal_status }) => {
                    assert_eq!(from, terminal);
                    assert_eq!(c, class);
                    assert_eq!(terminal_status, ts);
                }
                other => panic!("expected IllegalTransition from {terminal:?} for ({class:?}, {ts:?}), got {other:?}"),
            }
            assert_eq!(fsm.state(), terminal, "state must not mutate on illegal transition");
        }
    }
}

#[test]
fn illegal_at_requested_rejected() {
    let mut fsm = TaskFsm::new();
    let err = fsm.step(input(MessageClass::Deliver, Some(TerminalStatus::Completed))).unwrap_err();
    assert!(matches!(err, TaskFsmError::IllegalTransition { .. }));
    assert_eq!(fsm.state(), TaskState::Requested);
}
