---
id: 260420-viu
title: Fail-open on InvalidUuid in inbox list filter (follow-up to 974cc4b)
date: 2026-04-21
status: complete
commit: 42327a1
---

# Summary

Follow-up patch to commit `974cc4b`. Fixes a gap where
`TaskDirError::InvalidUuid` from `TaskDir.read` was routed through the
fail-closed branch of `is_terminal_cached`, hiding inbox entries whose
`causality.ref` (or `id`, for `request`) wasn't a valid UUID. The spec's
edge-case table is explicit: unparseable task_id is a property of the
entry, not a terminal signal — surface it.

## What changed

- `crates/famp/src/cli/inbox/list.rs`
  - `is_terminal_cached` match now routes `NotFound` and `InvalidUuid`
    through the fail-open branch via a nested or-pattern:
    `Err(TaskDirError::NotFound { .. } | TaskDirError::InvalidUuid { .. }) => false`
  - Doc comment expanded to note the cache also de-duplicates the
    fail-closed `eprintln!` (one warning per run_list call, not per
    affected inbox entry).
  - Inline comment cites the spec edge-case table at the match arm.
- `crates/famp/tests/inbox_list_filters_terminal.rs`
  - New regression test `list_fail_open_on_malformed_task_id` writes a
    single entry with `causality.ref = "not-a-valid-uuid"` and asserts
    it surfaces (1 line of output).
  - Test file now has 7 tests total (was 6).

## Verification

- `cargo test -p famp --test inbox_list_filters_terminal` —
  **7 passed** (new test included).
- `cargo test -p famp --test inbox_list_respects_cursor` —
  **1 passed** (unchanged).
- `cargo clippy --workspace --all-targets -- -D warnings` — **clean**.
  (Initial attempt used `| Err(...)` which tripped
  `clippy::unnested_or_patterns`; rewrote to nested or-pattern per
  clippy's suggestion — no `#[allow]` added.)
- `cargo nextest run` — **384 passed, 1 skipped**.

## Out of scope (honored)

- Did not refactor duplicated `class` extraction.
- Did not retouch rustdoc beyond the cache-dedup note.
- Did not add `#[allow]` attributes.
- Did not modify any other task's files.

## Commit

`42327a1` — fix(inbox): fail-open on InvalidUuid, matching spec edge-case table

## Refs

- `docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`
- Parent: `974cc4b`
