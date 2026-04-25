---
quick: 260425-of2
slug: tighten-mcp-body-schema-docstring
type: tdd
status: Verified
date-completed: 2026-04-25
key-files:
  modified:
    - crates/famp/src/cli/mcp/server.rs
    - crates/famp/tests/mcp_stdio_tool_calls.rs
commits:
  red: ccdb636
  green: 1c6d4c5
---

# Quick Task 260425-of2: Tighten MCP Body Schema Docstring — Summary

## One-liner

Replaced the generic `"Message content"` description on the `famp_send.body` MCP input-schema field with explicit guidance that the field is REQUIRED for `new_task` (and is the reply text for `deliver`/`terminal` modes), guarded by a regression test in `tests/mcp_stdio_tool_calls.rs`.

## Why

MCP clients (Claude Code, etc.) inspect the JSON-Schema description field to decide which arguments to populate. The previous `"Message content"` blurb was too generic — it gave the model no signal that omitting `body` on `new_task` would silently produce a contentless task. This was the root cause class behind quick task 260424-7z5 (body-loss). Tightening the docstring nudges callers toward correct usage and the new test prevents accidental reverts to the generic wording.

## What changed

### `crates/famp/src/cli/mcp/server.rs` (line 47)

Before:
```json
"body": { "type": "string", "description": "Message content" }
```

After:
```json
"body": { "type": "string", "description": "Task body content (the actual instructions). REQUIRED for new_task to carry content; the title field is only a short summary. For deliver/terminal modes, this is the reply text." }
```

No other schemas, fields, or formatting touched.

### `crates/famp/tests/mcp_stdio_tool_calls.rs`

Added one new test, `mcp_famp_send_body_description_flags_required_for_new_task`, that:

1. Spins up the real `famp mcp` stdio server via the existing `McpHarness`.
2. Issues `tools/list` and locates the `famp_send` descriptor.
3. Asserts `inputSchema.properties.body.description` contains the substring `"REQUIRED for new_task"`.
4. Asserts the same description does NOT contain the old generic string `"Message content"` — this catches accidental reverts.

No new dev-deps. The test piggybacks on the existing harness and `serde_json` traversal; no `insta` snapshot was used (the assertion is intentionally a substring match so future minor wording tweaks don't churn snapshots).

## TDD cycle

| Phase | Commit  | Result                                                                 |
|-------|---------|------------------------------------------------------------------------|
| RED   | ccdb636 | Test compiles and fails with `body description must flag REQUIRED for new_task, got: "Message content"` — confirms the test actually exercises the live schema.|
| GREEN | 1c6d4c5 | After updating server.rs:47, the new test passes; full workspace `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings` clean.|
| REFACTOR | (n/a) | One-line schema change + isolated test addition — nothing to refactor. |

## Verification performed

- `cargo test -p famp --test mcp_stdio_tool_calls mcp_famp_send_body_description_flags_required_for_new_task` — fails on RED, passes on GREEN.
- `cargo test --workspace` — all suites green (no regressions in the other MCP tests, conversation harness, transport, FSM, etc.).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean, no new lints.

## Deviations from plan

None. Executed exactly as specified: TDD ordering, exact description string, exact test assertion target (substring `"REQUIRED for new_task"` plus negative substring `"Message content"`), no extra dev-deps, no unrelated edits.

## Out-of-scope follow-ups

None observed. The other tool descriptions (`famp_await`, `famp_inbox`, `famp_peers`, and `famp_send.{title,task_id}`) are already specific; only `body` was the weak link.
