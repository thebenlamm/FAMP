# `famp_inbox list` — Filter Terminal Tasks

- **Date:** 2026-04-20
- **Status:** Design (awaiting approval)
- **Author:** Ben Lamm + Claude
- **Reviewed by:** `zed-velocity-engineer`
- **Scope:** v0.8 patch. No impact on v0.9 local-bus design; the same semantic carries forward.

## TL;DR

`famp_inbox list` (and `famp inbox list`) currently return every JSONL entry since a byte-offset cursor, including entries for tasks that have already reached a terminal FSM state (`COMPLETED`, `FAILED`, `CANCELLED`). Agents calling the tool to see "what's on my plate" see finished work too.

This spec adds a **read-time filter** that joins `famp_inbox::read::read_from` output against `famp-taskdir` and omits entries whose task is terminal. An opt-in `include_terminal` flag (default `false`) returns the unfiltered view. `famp_await` is untouched; it remains the canonical real-time signal including task completion.

No change to the JSONL log, the cursor, or ack semantics. No daemon behavior change. ~80 lines + tests in a single crate.

## Problem statement

Today:

```rust
// crates/famp/src/cli/inbox/list.rs
pub fn run_list(home: &Path, since: Option<u64>, out: &mut dyn Write) -> Result<(), CliError> {
    let entries = famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0))?;
    for (value, end_offset) in entries {
        // ... shape and write every entry, unconditionally
    }
}
```

Every `request`, `commit`, and `deliver` envelope ever written to the inbox is returned on every `list` call from `since=0`. A Claude Code agent running a session with ten prior conversations sees thirty+ envelopes for work that closed days ago.

`famp-taskdir` already tracks per-task FSM state in `<home>/tasks/<task_id>.toml`, with a denormalized `terminal: bool` field maintained by the daemon on every transition. The data needed to filter is present; the filter is not.

## Non-goals

- Physical archival or compaction of `inbox.jsonl` (remains append-only).
- Any change to `famp_await`, `famp_send`, `famp_peers`, or the daemon's envelope-routing path.
- A new `famp_tasks` MCP surface (noted as a potential v0.9 follow-up, not this spec).
- Retroactive taskdir backfill for pre-v0.8-Phase-3 inboxes.

## Design

### Semantic contract

For `famp_inbox` action=`list` and `famp inbox list`:

- **Default (`include_terminal = false`):** for each inbox entry read from `read_from`, look up its `task_id` in the taskdir. If the record exists and `terminal == true`, skip the entry. Otherwise emit it.
- **Override (`include_terminal = true`):** emit every entry unmodified. Matches today's behavior.
- **Filter is absolute.** Once a task is terminal, *all* its entries are hidden under the default — request, commit, and closing deliver alike. "Gone means gone."
- **`famp_await` is not filtered.** It remains the canonical real-time signal. Originators learn of task completion by awaiting, not by listing. This invariant MUST be documented in the rustdoc for both tools.

### Where the filter lives

`crates/famp/src/cli/inbox/list.rs::run_list`. Single call site, single function, pure read logic. No new module.

```rust
use std::collections::HashMap;

use famp_taskdir::TaskDir;

pub fn run_list(
    home: &Path,
    since: Option<u64>,
    include_terminal: bool,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries =
        famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0)).map_err(CliError::Inbox)?;

    let taskdir = (!include_terminal).then(|| TaskDir::open(paths::tasks_dir(home)));
    let mut terminal_cache: HashMap<String, bool> = HashMap::new();

    for (value, end_offset) in entries {
        let task_id = extract_task_id(&value); // existing logic, factored out
        if let Some(ref td) = taskdir {
            if is_terminal(td, task_id, &mut terminal_cache) {
                continue;
            }
        }
        // existing shape + writeln logic
    }
    Ok(())
}
```

**Note on `TaskDir::open`:** it `mkdir -p`s the root and sets 0700 on Unix. In practice `tasks/` is always created at setup, so this is a no-op on normal paths. On the off chance `run_list` runs against a fresh `FAMP_HOME` with no tasks, the directory creation is harmless — no records to read, filter passes everything through.

`is_terminal` rules:

- Cache hit → return cached bool.
- Cache miss → attempt `TaskDir::read(task_id)`:
  - `Ok(record)` → cache and return `record.terminal`.
  - `Err(NotFound)` → cache `false` and return `false` (**fail-open**: surface the entry).
  - `Err(TomlParse | Io)` → **fail-closed**: cache `true`, hide the entry, log at `warn` with `task_id` and error detail. Rationale: a corrupt record for a terminal task must not resurrect its history into `list` forever; operator visibility comes through the warn log, not a silent unfilter.

### Task-id extraction — audit required

Current `run_list` derives `task_id` by message class:

- `class == "request"` → envelope's `id` field.
- otherwise → `causality.ref`.

The filter relies on this routing covering every terminal-carrying class. Implementation task: audit `famp_core::MessageClass` and the daemon's inbox-write path to confirm that every class which can close a task (`deliver` with terminal status, and any future class) lands its task_id through the `causality.ref` branch. Extract the current inline match into a named `extract_task_id(&Value) -> &str` helper for unit testability, and add a test that enumerates every `MessageClass` variant. If a class leaks its task_id to a different field, either update the extraction helper or reject the message class at this layer with a hard error — silent filter misses are unacceptable.

### Interfaces

**MCP (`famp_inbox` tool):**

```json
{
  "action": "list",
  "since": 123,
  "include_terminal": false
}
```

- `include_terminal` is optional; absent is equivalent to `false`.
- Non-boolean `include_terminal` is a hard tool-call error with a clear message (`"famp_inbox: 'include_terminal' must be a boolean"`). No silent coercion.
- Output shape unchanged.

**CLI (`famp inbox list`):**

- New boolean flag `--include-terminal` (default `false`). `clap`-derived; no env-var knob.
- All existing output formatting unchanged.

**Library (`run_list`):** new `include_terminal: bool` positional argument after `since`. All in-repo call sites updated to pass `false` for the default-filtered view.

### Cursor and ack semantics

Unchanged. `run_list` does not advance the inbox cursor today and does not after this change. `ack` continues to be caller-driven and operates on the raw byte offset space of `inbox.jsonl`.

Consequences made explicit in the rustdoc and tool description:

- A caller running in the default filtered mode that acks the last *visible* offset effectively "passes over" any hidden terminal entries sitting between `since` and that offset. This is intended behavior under Semantic A ("gone means gone"). The entries remain in the JSONL and are recoverable via `include_terminal=true` with a fresh `since`.
- Callers MUST NOT assume `list` output offsets form a contiguous sequence against the raw log. This was already true (filtering is additive to any prior hypothetical future filter), but the rustdoc will now say so.

### Originator-completion visibility

When Alice sends a request to Bob and Bob replies with a terminal deliver:

1. Bob's daemon writes the outbound deliver; Bob's taskdir flips `terminal = true`.
2. Alice's daemon receives the deliver, appends to Alice's `inbox.jsonl`, and flips Alice's taskdir `terminal = true` for that task.
3. The order of (append-inbox, flip-taskdir) on Alice's side is a daemon concern not changed by this spec.

**Alice's canonical path to "my task completed":** `famp_await`. It blocks until a new envelope arrives, and it is unfiltered — the closing deliver is returned exactly once, in real time.

**Alice calling `list` after the fact:** under default filter, the closing deliver is hidden (task is terminal). Under `include_terminal=true`, it's visible. This is stated in the rustdoc of both `famp_await` and `famp_inbox` so agents and humans pick the right tool.

## Edge cases

| Case | Behavior |
|---|---|
| Entry with unparseable or missing `task_id` | Surface the entry (fail-open). Filter only hides on a positive "terminal" signal. |
| Taskdir record missing (`NotFound`) | Surface the entry. Typical for ancient pre-Phase-3 inboxes. |
| Taskdir record corrupt (parse/IO error) | Hide the entry + `warn`-log. Fail-closed per zed review. |
| Same task_id seen 5x in one list call | HashMap cache — single TOML read per task per call. |
| `include_terminal = true` with a corrupt taskdir file | Filter is bypassed entirely; taskdir isn't consulted. Corrupt record does not block debug access. |
| `include_terminal` sent as the string `"false"` | Hard schema error (`must be a boolean`). No coercion. |
| Inbox empty / `since` past EOF | Same as today — empty output. |

## Testing

### Unit tests (new file: `crates/famp/tests/inbox_list_filters_terminal.rs`)

1. `list_hides_entries_for_terminal_tasks` — seed two taskdir records (`A: terminal=false`, `B: terminal=true`), write four inbox entries (two per task), assert only A's two come out.
2. `list_include_terminal_returns_all` — same fixture, `include_terminal=true`, assert all four come out.
3. `list_fail_open_on_missing_taskdir_record` — inbox entry for a `task_id` with no taskdir file; assert entry is surfaced.
4. `list_fail_closed_on_corrupt_taskdir_record` — write an invalid TOML to `<taskdir>/<task_id>.toml`; assert entry is hidden; assert a warn-level log was emitted (captured via an in-test `tracing` subscriber matching the project's existing test style).
5. `list_caches_taskdir_reads` — use a wrapper that counts `TaskDir::read` calls; assert at most one read per distinct `task_id` per `run_list` invocation.
6. `extract_task_id_exhaustive` — enumerate every `MessageClass` variant and assert the helper returns a non-empty `task_id` (or a documented empty-string for classes that carry none). Fails the build when a new class is added without handling.

### MCP schema tests

Extend `crates/famp/tests/mcp_stdio_tool_calls.rs`:

- `famp_inbox_list_filters_terminal_by_default` — round-trip through stdio MCP.
- `famp_inbox_list_include_terminal_true_returns_all`.

Extend `crates/famp/tests/mcp_malformed_input.rs`:

- `famp_inbox_list_rejects_non_bool_include_terminal` — send `"include_terminal": "true"` (string), assert hard error with the documented message.

### E2E test

Extend `crates/famp/tests/e2e_two_daemons.rs`:

- Walk a task all the way to `COMPLETED` on the originator side.
- Call `famp_inbox list` (default); assert no entries for that task_id.
- Call `famp_inbox list` with `include_terminal=true`; assert the closing deliver comes back.
- Call `famp_await` during the lifecycle and assert the closing deliver is surfaced live.

### Pre-existing tests

`crates/famp/tests/inbox_list_respects_cursor.rs` must continue passing unmodified — the cursor contract is not changed by this spec. All existing `run_list` call sites get `false` passed for `include_terminal`; test expectations unchanged because no prior test seeded a terminal taskdir record.

## Rollout

Single PR, single commit series:

1. Extract `extract_task_id` helper + exhaustive unit test.
2. Add `include_terminal` param to `run_list`, wire taskdir lookup + cache.
3. Thread the flag through the `famp inbox list` CLI subcommand.
4. Thread the flag through `famp_inbox` MCP tool; add schema rejection for non-bool.
5. Add new unit/MCP/E2E tests.
6. Update rustdoc on `run_list`, `famp_await`, and the `famp_inbox` tool describing the filter and the "await is canonical for completion" rule.

No migration. No feature flag. No config. The default filter is on from the first deploy; agents that relied on the old "everything comes back" behavior get `include_terminal=true` and a one-line docs pointer.

## Open questions

None blocking. The taskdir-class audit (§ Task-id extraction) is implementation work, not a design question — the spec commits to "if a class leaks task_id to a different field, the extraction helper MUST be updated before merge."
