//! Quick task 260425-rz6 — RED test.
//!
//! Asserts that an FSM transition failure surfaced via `advance_committed`
//! produces the correct top-line `Display` and MCP `famp_error_kind`:
//!
//! - top-line `format!("{e}")` starts with `"illegal task state transition"`
//!   (NOT `"envelope encode/sign failed"`).
//! - `e.mcp_error_kind() == "fsm_transition_illegal"` (NOT `"envelope_error"`).
//!
//! On current main both assertions FAIL because `fsm_glue::advance_committed`
//! maps every `TaskFsmError` to `CliError::Envelope`. After the GREEN fix in
//! T2 (new `CliError::FsmTransition` + `InvalidTaskState` variants and
//! rewired `fsm_glue`), both assertions PASS.

// Integration test binaries inherit all of famp's transitive deps; silence
// "unused crate" warnings for crates we don't explicitly reference here.
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use famp::cli::error::CliError;
use famp::cli::send::fsm_glue::advance_committed;
use famp_taskdir::TaskRecord;

/// Build a `TaskRecord` already in `COMMITTED`. Calling `advance_committed`
/// on this record asks the FSM to apply `MessageClass::Commit` from
/// `TaskState::Committed`, which is illegal — the FSM returns
/// `TaskFsmError::IllegalTransition`.
fn committed_record() -> TaskRecord {
    TaskRecord::new_committed(
        "0192f000-0000-7000-8000-000000000000".to_string(),
        "alice".to_string(),
        "2026-04-25T00:00:00Z".to_string(),
    )
}

#[test]
fn fsm_transition_failure_surfaces_correct_top_line_display() {
    let mut record = committed_record();
    let err = advance_committed(&mut record)
        .expect_err("advance_committed on a COMMITTED record must return IllegalTransition");

    let top_line = format!("{err}");
    assert!(
        top_line.starts_with("illegal task state transition"),
        "expected top-line Display to start with \"illegal task state transition\", \
         got: {top_line:?}"
    );
}

#[test]
fn fsm_transition_failure_surfaces_correct_mcp_error_kind() {
    let mut record = committed_record();
    let err = advance_committed(&mut record)
        .expect_err("advance_committed on a COMMITTED record must return IllegalTransition");

    assert_eq!(
        err.mcp_error_kind(),
        "fsm_transition_illegal",
        "expected mcp_error_kind() to discriminate FSM failures from envelope failures; \
         got: {:?}",
        err.mcp_error_kind()
    );
}

#[test]
fn fsm_transition_failure_is_not_classified_as_envelope() {
    // Belt-and-braces: explicitly assert the mis-classification we are
    // fixing. If this ever passes again, we have regressed.
    let mut record = committed_record();
    let err = advance_committed(&mut record)
        .expect_err("advance_committed on a COMMITTED record must return IllegalTransition");

    let kind = err.mcp_error_kind();
    assert_ne!(
        kind, "envelope_error",
        "FSM transition failures must NOT be mapped to envelope_error"
    );
    assert!(
        !matches!(err, CliError::Envelope(_)),
        "FSM transition failures must NOT be a CliError::Envelope variant"
    );
}
