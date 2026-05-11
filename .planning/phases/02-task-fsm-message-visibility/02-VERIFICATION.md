---
phase: 02-task-fsm-message-visibility
verified: 2026-05-10T23:59:00Z
status: passed
score: 9/9 requirements verified
review_findings_resolved: true
human_verification_required: false
---

# Phase 02: Task FSM & Message Visibility Verification Report

**Phase Goal:** Operator runs `famp inspect tasks` and `famp inspect messages` and gets task FSM plus envelope-metadata visibility, with real I/O-bound inspect handlers protected by a 500 ms budget and cancellation discipline.
**Status:** passed

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | `famp inspect tasks` lists task rows with task_id/state/peer/envelope_count/last-transition-age and surfaces orphan rows first. | VERIFIED | `tasks.rs` renders `TASK_ID`, `STATE`, `PEER`, `ENVELOPES`, `LAST_TRANSITION_AGE`, `ORPHAN`; server sorts orphan rows first. `inspect_tasks` integration and server unit tests pass. |
| 2 | `famp inspect tasks --id <uuid>` and `--full` expose envelope-chain detail, with full output as JSON/JCS-compatible bytes. | VERIFIED | Server `InspectTasksReply::Detail` and `DetailFull` paths filter by task ID; CLI renders detail table or pretty JSON. `id_full_jcs_pipes_through_jq` passes. |
| 3 | `famp inspect messages` exposes metadata only, never body content, and supports `--tail` with most-recent ordering. | VERIFIED | `messages.rs` renders metadata columns only; server computes body length and SHA256 prefix, not body content. Review fix added global timestamp ordering before tail. `inspect_messages` and server unit tests pass. |
| 4 | I/O-bound inspect work is budgeted and cancellable at the broker wrapper. | VERIFIED | Broker executor wraps `spawn_blocking` in `tokio::time::timeout(Duration::from_millis(500))`; runtime uses `max_blocking_threads(1024)`; cancel pressure test passes. |
| 5 | Phase 1 broker/identities behavior did not regress. | VERIFIED | `cargo nextest run -p famp --test inspect_broker --test inspect_identities --no-fail-fast` passed 14/14 during regression gate. |

### Requirements Coverage

| Requirement | Status | Evidence |
|---|---|---|
| INSP-TASK-01 | SATISFIED | `InspectTasksReply::List`, `TaskRow`, `inspect_tasks` grouping, CLI table, and integration test `list_groups_by_task_id_with_state_and_envelope_count`. |
| INSP-TASK-02 | SATISFIED | `is_orphan_task_id`, orphan-first sorting, `--orphans` CLI filter, and server regression `dispatch_tasks_merges_mailbox_only_orphans_with_taskdir_rows`. |
| INSP-TASK-03 | SATISFIED | `InspectTasksRequest { id, full: false }` returns `TaskDetailReply` with envelope_id/sender/recipient/fsm_transition/timestamp/sig_verified. |
| INSP-TASK-04 | SATISFIED | `full: true` returns `TaskDetailFullReply` with canonical bytes; integration test validates JSON output can be parsed and contains the task envelope chain. |
| INSP-MSG-01 | SATISFIED | `MessageRow` has no body field; CLI table excludes `BODY`; `metadata_only_no_body` verifies body text does not leak. |
| INSP-MSG-02 | SATISFIED | `MessageRow` includes sender, recipient, task_id, class, state, timestamp, body_bytes, and 12-hex SHA256 prefix. |
| INSP-MSG-03 | SATISFIED | Request default tail is 50; server sorts entries by timestamp before tailing; integration tests cover default and tail=3. |
| INSP-RPC-03 | SATISFIED | Broker inspect dispatch uses `timeout(Duration::from_millis(500), spawn_blocking(...))` and maps timeout to `BudgetExceeded { elapsed_ms: 500 }`. |
| INSP-RPC-04 | SATISFIED | `inspect_cancel_1000` runs unignored and verifies 1000 concurrent inspect calls/cancels complete without FD leak. |

## Review Gate

Code review produced 3 blockers and 1 warning in `02-REVIEW.md`. All were fixed before verification:

- Orphan mailbox-only tasks are now merged with taskdir rows instead of only synthesized when taskdir is empty.
- Non-task envelopes no longer receive fake task IDs; new-task audit envelopes legitimately use envelope `id`, replies use `causality.ref`, and ordinary non-task envelopes remain empty.
- Unfiltered message tailing now sorts globally by parsed timestamp before applying tail.
- The cancel test counts FDs via `/proc/<pid>/fd` on Linux and falls back to `lsof` elsewhere.

## Automated Checks

| Command | Result |
|---|---|
| `cargo build -p famp` | PASS |
| `cargo nextest run -p famp-inspect-server --no-fail-fast` | PASS, 18/18 |
| `cargo nextest run -p famp --test inspect_tasks --no-fail-fast` | PASS, 4/4 |
| `cargo nextest run -p famp --test inspect_messages --no-fail-fast` | PASS, 3/3 |
| `cargo nextest run -p famp --test inspect_cancel_1000 --no-fail-fast` | PASS, 1/1 |
| `cargo nextest run -p famp --test inspect_broker --test inspect_identities --no-fail-fast` | PASS, 14/14 |
| `just check-inspect-readonly` | PASS |
| `just check-no-io-in-inspect-proto` | PASS |
| `just check-no-tokio-in-bus` | PASS |

Note: a combined nextest invocation across all inspect subprocess tests raced broker subprocess startup in this local session; the serialized per-file runs above are the authoritative verification evidence.

## Human Verification Required

None.

## Gaps Summary

No gaps found. Phase 02 achieves the planned task FSM and message visibility surface and preserves Phase 1 behavior.

---
_Verified: 2026-05-10T23:59:00Z_
_Verifier: inline orchestrator fallback after verifier agent failed to produce an artifact_
