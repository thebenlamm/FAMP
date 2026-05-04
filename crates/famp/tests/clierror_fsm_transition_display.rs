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
use famp_core::MessageClass;
use famp_fsm::{TaskFsm, TaskTransitionInput};

/// Build an FSM already in `COMMITTED`. Applying a second `MessageClass::Commit`
/// asks the FSM to apply `MessageClass::Commit` from
/// `TaskState::Committed`, which is illegal — the FSM returns
/// `TaskFsmError::IllegalTransition`.
fn duplicate_commit_error() -> CliError {
    let mut fsm = TaskFsm::new();
    fsm.step(TaskTransitionInput {
        class: MessageClass::Commit,
        terminal_status: None,
    })
    .expect("first commit transition succeeds");
    let err = fsm
        .step(TaskTransitionInput {
            class: MessageClass::Commit,
            terminal_status: None,
        })
        .expect_err("second commit transition must fail");
    CliError::FsmTransition(err)
}

#[test]
fn fsm_transition_failure_surfaces_correct_top_line_display() {
    let err = duplicate_commit_error();

    let top_line = format!("{err}");
    assert!(
        top_line.starts_with("illegal task state transition"),
        "expected top-line Display to start with \"illegal task state transition\", \
         got: {top_line:?}"
    );
    // tey strengthening (MED-1): direct `eprintln!("{e}")` sites at
    // await_cmd/mod.rs:183 and send/mod.rs:564 print only the top-line; if
    // the inner TaskFsmError detail isn't interpolated here, those operator
    // logs lose class/from/terminal_status. Asserting `class=` proves the
    // inner Display ("illegal transition: cannot apply class=... ...") is
    // included via `{0}` interpolation, not just the bare category top-line.
    assert!(
        top_line.contains("class="),
        "expected top-line Display to interpolate the inner TaskFsmError \
         detail (look for `class=`), got: {top_line:?}"
    );
}

#[test]
fn clierror_invalid_task_state_debug_quotes_value() {
    // tey MED-2: a corrupted on-disk task state string can contain newlines,
    // ANSI escapes, or other control bytes. Raw `{value}` interpolation
    // injects them into stderr verbatim. `{value:?}` debug-quotes them
    // (matching the `PrincipalInvalid` precedent at error.rs:127), which is
    // the safe default for an on-disk corruption diagnostic.
    let err = CliError::InvalidTaskState {
        value: "BAD\nSTATE".to_string(),
    };
    let rendered = format!("{err}");
    assert!(
        rendered.contains("\\n"),
        "expected debug-escaped newline (`\\n`) in rendered output, got: {rendered:?}"
    );
    assert!(
        !rendered.contains('\n'),
        "expected NO raw newline in rendered output (would mean we're \
         interpolating raw `{{value}}`), got: {rendered:?}"
    );
}

#[test]
fn fsm_transition_failure_surfaces_correct_mcp_error_kind() {
    let err = duplicate_commit_error();

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
    let err = duplicate_commit_error();

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
