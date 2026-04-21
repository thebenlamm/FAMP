---
id: 260420-viu
title: Fail-open on InvalidUuid in inbox list filter (follow-up to 974cc4b)
mode: quick
date: 2026-04-21
---

# Plan: Fail-open on InvalidUuid

## Context

Follow-up to commit `974cc4b` (Task 2). The spec at
`docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`
edge-case table requires: "Entry with unparseable or missing `task_id` →
Surface the entry (fail-open). Filter only hides on a positive 'terminal'
signal."

Current `is_terminal_cached` in `crates/famp/src/cli/inbox/list.rs` routes
`TaskDirError::InvalidUuid` through the "any other error → fail-closed"
branch — so an inbox entry with a malformed `causality.ref` (e.g.
`"not-a-uuid"`) would be hidden forever, not surfaced.

`TaskDirError::InvalidUuid` is fired by `crates/famp-taskdir/src/store.rs:42`
when `uuid::Uuid::parse_str` on the caller-supplied `task_id` fails. That's
exactly "unparseable task_id" — spec says fail-open.

## Tasks

### Task 1: Patch `is_terminal_cached` + add regression test

**Files:**
- `crates/famp/src/cli/inbox/list.rs` — extend `NotFound` arm to match
  `InvalidUuid`; add cache-dedup note to doc comment
- `crates/famp/tests/inbox_list_filters_terminal.rs` — append
  `list_fail_open_on_malformed_task_id` test

**Action:**
1. In `is_terminal_cached`, change:
   ```rust
   Err(TaskDirError::NotFound { .. }) => false,
   ```
   to:
   ```rust
   Err(TaskDirError::NotFound { .. }) | Err(TaskDirError::InvalidUuid { .. }) => false,
   ```
   with an inline comment citing the spec edge-case table.
2. Update the rustdoc on `is_terminal_cached` to note:
   - `NotFound` / `InvalidUuid` → `false` (fail-open)
   - `Ok(rec)` → `rec.terminal`
   - any other error → `true` (fail-closed + eprintln)
   - caching de-duplicates the fail-closed `eprintln!`
3. Append regression test `list_fail_open_on_malformed_task_id` to
   `crates/famp/tests/inbox_list_filters_terminal.rs`. Test writes a
   single inbox entry with `causality.ref = "not-a-valid-uuid"` and
   asserts `run_list` surfaces it (1 line of output) under default
   (non-terminal) filter.

**Verify:**
- `cargo test -p famp --test inbox_list_filters_terminal` — new test
  passes; total 7 tests in file
- `cargo test -p famp --test inbox_list_respects_cursor` — unchanged
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo nextest run` — workspace green

**Done:** All four verify commands pass; commit with conventional message
citing `974cc4b` and the spec file.

## must_haves

- **truths:**
  - InvalidUuid errors from TaskDir.read must not hide inbox entries
  - Spec edge-case table (2026-04-20 design) governs fail-open semantics
- **artifacts:**
  - `crates/famp/src/cli/inbox/list.rs` — patched match arm
  - `crates/famp/tests/inbox_list_filters_terminal.rs` — new test
- **key_links:**
  - `docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`
  - Parent commit `974cc4b`

## Out of Scope

- Refactor of duplicated `class` extraction (pre-existing)
- Any rustdoc beyond the cache-dedup note
- `#[allow]` attributes
- Any other task's files
