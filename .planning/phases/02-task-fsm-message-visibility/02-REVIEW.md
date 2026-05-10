---
phase: 02-task-fsm-message-visibility
reviewed: 2026-05-10T23:37:02Z
depth: standard
files_reviewed: 13
files_reviewed_list:
  - .config/nextest.toml
  - crates/famp-inspect-proto/src/lib.rs
  - crates/famp-inspect-server/Cargo.toml
  - crates/famp-inspect-server/src/lib.rs
  - crates/famp/src/cli/mod.rs
  - crates/famp/src/cli/broker/mod.rs
  - crates/famp/src/cli/inspect/mod.rs
  - crates/famp/src/cli/inspect/tasks.rs
  - crates/famp/src/cli/inspect/messages.rs
  - crates/famp/tests/inspect_broker.rs
  - crates/famp/tests/inspect_tasks.rs
  - crates/famp/tests/inspect_messages.rs
  - crates/famp/tests/inspect_cancel_1000.rs
findings:
  critical: 3
  warning: 1
  info: 0
  total: 4
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-05-10T23:37:02Z
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

Reviewed the inspector protocol types, server dispatch, broker-side snapshot construction, CLI renderers, and integration tests. The main defects are in task/message derivation: orphan tasks are only synthesized when the taskdir is otherwise empty, non-task envelopes are reported as task envelopes, and unfiltered message tailing is not actually most-recent across recipients.

## Critical Issues

### CR-01: BLOCKER - Orphan Envelope-Only Tasks Disappear When Any Taskdir Record Exists

**File:** `crates/famp-inspect-server/src/lib.rs:227`
**Issue:** `inspect_tasks` first builds rows only from `snapshot.records` and only synthesizes rows from mailbox envelopes inside `if rows.is_empty()` at lines 265-310. As soon as the taskdir contains one valid record, any envelope-only orphan task IDs in the mailbox snapshot are omitted entirely, so `famp inspect tasks --orphans` can return no orphan row even though orphan envelopes exist. This violates the task list contract that orphan rows are surfaced, and it hides exactly the degraded state this command is meant to diagnose.
**Fix:**
```rust
let mut seen = BTreeSet::new();
let mut rows: Vec<TaskRow> = snapshot.records.iter().map(|record| {
    seen.insert(record.task_id.clone());
    // existing TaskRecord -> TaskRow mapping
}).collect();

let mut by_task: BTreeMap<String, Vec<&serde_json::Value>> = BTreeMap::new();
for env in &all_envs {
    if let Some(task_id) = envelope_task_id(env) {
        if !seen.contains(&task_id) {
            by_task.entry(task_id).or_default().push(env);
        }
    }
}
rows.extend(by_task.into_iter().map(|(task_id, envelopes)| {
    // existing synthesized-row mapping
}));
```

### CR-02: BLOCKER - Non-Task Envelopes Are Misreported As Task Envelopes

**File:** `crates/famp-inspect-server/src/lib.rs:383`
**Issue:** `envelope_task_id` falls back from `causality.ref` and `body.details.task` to the envelope `id` at lines 393-398. The wire type documents `task_id` as "`causality.ref` if present, else `body.details.task`, else empty" in `crates/famp-inspect-proto/src/lib.rs:235`. With the current fallback, any envelope without task metadata gets a fake task ID equal to its envelope ID. That pollutes `inspect messages` output and can cause `inspect tasks` to synthesize bogus orphan task rows from ordinary non-task messages.
**Fix:**
```rust
fn envelope_task_id(env: &serde_json::Value) -> Option<String> {
    if let Some(task_id) = env
        .get("causality")
        .and_then(|c| c.get("ref"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(task_id.to_string());
    }
    env.get("body")
        .and_then(|b| b.get("details"))
        .and_then(|d| d.get("task"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}
```

### CR-03: BLOCKER - Global `inspect messages --tail` Is Not Most-Recent

**File:** `crates/famp-inspect-server/src/lib.rs:333`
**Issue:** For `--to` omitted, entries are collected with `snapshot.by_recipient.values().flatten()` from a `BTreeMap`, then `tail` slices the last N entries in recipient-key order at lines 342-344. The CLI promises "Limit to N most-recent envelopes" in `crates/famp/src/cli/inspect/messages.rs:18`, but unfiltered tailing returns the last N entries from the lexicographically last recipient, not the newest N envelopes across all recipients. Operators using `famp inspect messages --tail 50` can miss newer messages for earlier-sorted identities.
**Fix:** Gather all entries, sort by parsed `ts` before applying `tail`, and preserve per-recipient file order only as a tie-breaker.
```rust
let mut entries: Vec<&serde_json::Value> = match req.to.as_deref() {
    Some(name) => snapshot.by_recipient.get(name).map(|v| v.iter().collect()).unwrap_or_default(),
    None => snapshot.by_recipient.values().flatten().collect(),
};
entries.sort_by_key(|env| {
    env.get("ts")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_rfc3339_to_epoch)
        .unwrap_or(0)
});
let start = entries.len().saturating_sub(tail);
```

## Warnings

### WR-01: WARNING - FD-Leak Test Hard-Requires `lsof`

**File:** `crates/famp/tests/inspect_cancel_1000.rs:73`
**Issue:** `count_broker_fds` panics if `lsof` is unavailable or fails. The file is only `cfg(unix)`, and nextest includes this test in the inspect subprocess group, so a Linux CI image or minimal developer environment without `lsof` will fail the test suite for an environment prerequisite rather than a product regression.
**Fix:** Either gate the test on `lsof` availability and skip with a clear message, or implement platform-specific FD counting with `/proc/{pid}/fd` on Linux and `lsof` only where needed.

---

_Reviewed: 2026-05-10T23:37:02Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
