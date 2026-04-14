---
phase: 03-conversation-cli
plan: 03
subsystem: conversation-cli-inbound
tags: [famp-await, famp-inbox, cursor, read-from, humantime]

requires:
  - phase: 03-conversation-cli-plan-01
    provides: famp_inbox::InboxCursor, paths::inbox_jsonl_path / inbox_cursor_path
  - phase: 03-conversation-cli-plan-02
    provides: FAMP_HOME layout, listen daemon harness, send/peer_add for manual wiring

provides:
  - famp_inbox::read::read_from(path, start_offset) -> Vec<(Value, end_offset)>
  - famp await subcommand with 250ms poll + typed AwaitTimeout + --task filter
  - famp inbox list [--since] subcommand (non-mutating dump)
  - famp inbox ack <offset> subcommand (cursor advance, no output)
  - CliError::{AwaitTimeout, InvalidDuration} variants
  - famp::cli::error::parse_duration humantime helper
  - Locked JSON output shape for Phase 4 MCP consumers

affects: [03-04-peer-add-lock, 04-mcp-claude-integration]

tech-stack:
  added:
    - "humantime 2.3 — user-facing duration parser (30s / 5m / 1h / 250ms)"
  patterns:
    - "Per-entry end_offset in read_from: the API returns Vec<(Value, u64)> rather than (Vec<Value>, u64) so await can advance the cursor past exactly ONE entry per call (single-entry semantics), leaving the rest of the batch for subsequent polls."
    - "Missing-file tolerance: read_from treats ErrorKind::NotFound as an empty inbox. A fresh `famp init` has no inbox.jsonl yet — the daemon creates it lazily — so await/list must not error before the first envelope lands."
    - "Dispatcher async fan-out: Commands::{Await, Inbox} each build their own multi-thread tokio runtime in cli::run, matching the existing Listen/Send pattern. Simpler than a single runtime with a re-match block."
    - "Consume-and-discard filter: when --task is set and the batch contains only non-matching entries, the cursor is advanced past the whole batch so the next poll does not re-scan the same already-rejected bytes."

key-files:
  created:
    - crates/famp-inbox/tests/read_from.rs
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/src/cli/await_cmd/poll.rs
    - crates/famp/src/cli/inbox/mod.rs
    - crates/famp/src/cli/inbox/list.rs
    - crates/famp/src/cli/inbox/ack.rs
    - crates/famp/tests/await_blocks_until_message.rs
    - crates/famp/tests/await_timeout.rs
    - crates/famp/tests/inbox_list_respects_cursor.rs
    - .planning/milestones/v0.8-phases/03-conversation-cli/03-03-SUMMARY.md
  modified:
    - crates/famp-inbox/src/read.rs
    - crates/famp/Cargo.toml
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/_gen_fixture_certs.rs

key-decisions:
  - "read_from returns Vec<(Value, end_offset)> (per-entry offsets), not (Vec<Value>, batch_end). Necessary because `famp await` advances the cursor past exactly ONE entry per call — any filter-match helper needs the exact end-offset of the matched line, not just the batch end. Planner flagged this as a forced design correction mid-plan; implemented that way on the first pass."
  - "Missing inbox.jsonl is NOT an error. A freshly-initialized FAMP_HOME has no inbox.jsonl until the daemon appends its first envelope. Both `famp await` and `famp inbox list` have to work before the daemon runs, so read_from returns (vec![], 0) on ErrorKind::NotFound rather than Io-wrapping the error. Discovered by the await-timeout test and fixed inline (Rule 1)."
  - "run_at output sink is `&mut (dyn Write + Send)` (not plain `dyn Write`). Required because the integration test spawns `run_at` inside a `tokio::spawn` and Futures crossing the spawn boundary must be Send."
  - "JSON output shape locked here (see section below). `offset` is the byte offset AFTER the consumed line, matching cursor-advance semantics. Phase 4 MCP wrappers verify they consumed the entry they expected by checking this value against what they see on re-read."
  - "250ms poll interval hardcoded as a `const POLL_INTERVAL` — matches REQUIREMENTS.md INBOX-03 and CONTEXT D-Cursor. Not user-tunable until a real use case demands it."
  - "InboxCommand dispatcher is async because `ack` requires `InboxCursor::advance` (async). `list` does not need async at all but is invoked through the same entry point so the runtime is built once for either branch."

requirements-completed: [CLI-04, CLI-05, CONV-04, INBOX-03]

duration: ~20min
completed: 2026-04-14
---

# Phase 3 Plan 03: `famp await` + `famp inbox` Summary

**Closes the inbound half of the Phase 3 conversation CLI: `famp await` polls the inbox past the cursor and emits a single structured JSON line on receipt; `famp inbox list/ack` expose the cursor to external (Phase 4 MCP) consumers without advancing it.**

## Performance

- **Duration:** ~20 min
- **Tasks:** 2/2 (one commit per task, both TDD — RED tests landed before GREEN in Task 1)
- **Files created:** 10
- **Files modified:** 8
- **Workspace tests:** 333/333 pass, 1 skipped (up from 324; +9 new: 6 read_from unit + 3 integration)

## Accomplishments

### Task 1 — `read_from` + duration parser

- `famp_inbox::read::read_from(path, start_offset)` → `Vec<(serde_json::Value, u64 /* end_offset */)>`
- Snap-forward-on-mid-line: if `start_offset` falls mid-line it walks to the next `\n + 1` boundary (line-boundary invariant recovery)
- EOF clamping: `start_offset >= file_len` returns empty, not an error
- Missing-file tolerance: `ErrorKind::NotFound` → empty result (fresh-FAMP_HOME support)
- Tail tolerance: final partial line at EOF silently dropped (matches `read_all`)
- Corrupt non-terminal line → `InboxError::CorruptLine { line_no }` (matches `read_all`)
- Six unit tests in `crates/famp-inbox/tests/read_from.rs` lock all six behaviors
- `CliError::{AwaitTimeout, InvalidDuration}` variants added
- `famp::cli::error::parse_duration` helper — thin wrapper over `humantime::parse_duration` returning `CliError::InvalidDuration` on failure

### Task 2 — `famp await` + `famp inbox` subcommands

- `famp await --timeout <dur> [--task <id>]` implemented in `crates/famp/src/cli/await_cmd/`
  - Polls every 250 ms (`POLL_INTERVAL` const), matching REQUIREMENTS.md INBOX-03
  - First matching entry → prints one JSON line, advances cursor past that one entry, exits 0
  - Timeout → typed `CliError::AwaitTimeout { timeout }`, cursor untouched
  - `--task` filter → matched entry printed as above; non-matching batch entries are consume-and-discard (cursor advances past them so the next poll sees new bytes)
- `famp inbox list [--since <offset>]` — non-blocking dump via `read_from`, same JSON shape as await, does NOT touch the cursor
- `famp inbox ack <offset>` — advances `InboxCursor`, prints nothing
- Wired into `cli::mod::run` via new `Commands::{Await, Inbox}` variants; each builds its own multi-thread tokio runtime (same pattern as `Listen` / `Send`)
- `#[command(name = "await")]` on the variant so users type `famp await` even though the module is `await_cmd` (Rust reserved word)

### Integration tests (3 new binaries)

- `crates/famp/tests/await_blocks_until_message.rs`: spawns `run_on_listener` on ephemeral port, launches `await_run_at` in a concurrent task, POSTs a signed ack envelope after 400 ms, asserts await unblocks within 3 s, prints exactly one JSON line, the JSON object has all five locked keys (`offset`, `task_id`, `from`, `class`, `body`), and `InboxCursor::read()` equals the printed offset.
- `crates/famp/tests/await_timeout.rs`: fresh home, no daemon, `await --timeout 200ms` → `CliError::AwaitTimeout { timeout: "200ms" }` in < 1.5 s, no cursor file on disk, no stdout output.
- `crates/famp/tests/inbox_list_respects_cursor.rs`: handcrafted 3-line JSONL fixture → `list` returns all three with byte-exact offsets (`LINE1.len`, `LINE1+LINE2`, `body.len`), cursor stays at 0 after list, `ack(off2)` sets cursor to `off2`, `list --since off2` returns exactly the trailing entry, cursor still at `off2` afterwards.

## Locked JSON Output Shape

Every `famp await` and `famp inbox list` line prints an object with **exactly** these five keys:

```json
{
  "offset": 12345,
  "task_id": "01913000-0000-7000-8000-000000000001",
  "from": "agent:localhost/self",
  "class": "request",
  "body": { "text": "hi" }
}
```

- `offset` (u64): byte offset of the first byte AFTER the consumed line. For `await`, this is the cursor value after advance. For `list`, this is the end-offset of the line being printed. Phase 4 MCP wrappers can compare `offset` against a fresh `cursor.read()` to verify no races.
- `task_id`, `from`, `class` (string): verbatim from the envelope's top-level fields. Missing fields serialize as empty strings — the Phase 3 `famp await` reader is deliberately forgiving so a hand-written test envelope doesn't need to satisfy the full envelope schema.
- `body`: the raw inner body JSON (any shape) or `null` if the field is absent.

This shape is locked by the three integration tests and is the stable contract for Phase 4 MCP consumption.

## Task Commits

1. **feat(03-03): add read_from slice reader + duration parser** — `c2190da`
2. **feat(03-03): add famp await + famp inbox list/ack subcommands** — `acad630`

## Deviations from Plan

### Rule 1 — Bug: read_from errored on missing inbox.jsonl

- **Found during:** Task 2, first run of `await_timeout.rs`.
- **Issue:** A fresh `famp init` does NOT create `inbox.jsonl` — the daemon writes it lazily on first append. The original `read_from` forwarded `std::fs::read`'s `NotFound` as `InboxError::Io`, which `await` surfaced as `CliError::Inbox` instead of `CliError::AwaitTimeout`. The timeout test caught this immediately.
- **Fix:** Added an `ErrorKind::NotFound` short-circuit in `read_from` that returns `(vec![], 0)` — semantically "empty inbox", which matches how both `await` and `inbox list` treat a freshly-initialized home.
- **Files modified:** `crates/famp-inbox/src/read.rs`
- **Commit:** squashed into `acad630`

### Rule 3 — Blocking: `&mut dyn Write` across `tokio::spawn` boundary

- **Found during:** Task 2, first compile of `await_blocks_until_message.rs`.
- **Issue:** `run_at(home, args, &mut dyn Write)` made the returned future non-`Send` because `dyn Write` is not `Send`. The integration test spawns `run_at` inside `tokio::spawn`, which requires `Send + 'static`.
- **Fix:** Changed the signature to `&mut (dyn Write + Send)`. `std::io::Stdout` and `&mut Vec<u8>` both satisfy this.
- **Impact:** None on the plan's contract — the integration test pattern from the plan works unchanged.

### Rule 3 — Blocking: clippy lint batch

- `clippy::too_long_first_doc_paragraph` on `read_from` — first paragraph split into a title + expanded body.
- `clippy::cast_possible_truncation` on `start_offset as usize` — replaced with `usize::try_from` and an early-return on overflow.
- `clippy::doc_markdown` on `FAMP_HOME` in the two test-module docs — wrapped in backticks.
- `unused_crate_dependencies` on `humantime` in the bin target and three example targets — added `use humantime as _;` silencers (matches the existing silencer pattern for other transitive deps).

All lints fixed inline; no impact on plan scope.

### No Rule-4 architectural changes.

**Total deviations:** 1 Rule-1 bug (missing-file handling), 2 Rule-3 compile/lint items (Send bound + clippy batch). Zero architectural surprises.

## Verification Artifacts

- `cargo nextest run -p famp-inbox --test read_from` → **6/6 passed**
- `cargo nextest run -p famp --test await_blocks_until_message --test await_timeout --test inbox_list_respects_cursor` → **3/3 passed**
- `cargo nextest run --workspace` → **333 passed, 1 skipped** (+9 over Plan 03-02's 324)
- `cargo clippy --workspace --all-targets -- -D warnings` → **0 warnings**

## Threat Flags

None. `famp await` and `famp inbox` are pure local-disk readers (plus one blocking wait for a cursor advance). No new network surface, no trust-boundary changes, no new auth paths. The locked JSON output does not include envelope signatures or raw key material — only the fields the Phase 4 MCP layer needs for routing. The cursor itself remains 0600.

## Next Plan Readiness

- **Plan 03-04 (peer add advisory lock + full E2E)** can consume `famp await` and `famp inbox list/ack` for the two-home round-trip test. The locked JSON shape means the E2E can assert on stable string keys instead of positional inspection.
- **Phase 4 (MCP + Claude integration)** gets the full inbound CLI: `await` for blocking on replies, `inbox list` for scanning arrivals, `inbox ack` for marking handled. The `offset` field in the JSON output is the race-free acknowledgment token.

## Self-Check: PASSED

- `crates/famp-inbox/tests/read_from.rs` — FOUND
- `crates/famp/src/cli/await_cmd/mod.rs` — FOUND
- `crates/famp/src/cli/await_cmd/poll.rs` — FOUND
- `crates/famp/src/cli/inbox/mod.rs` — FOUND
- `crates/famp/src/cli/inbox/list.rs` — FOUND
- `crates/famp/src/cli/inbox/ack.rs` — FOUND
- `crates/famp/tests/await_blocks_until_message.rs` — FOUND
- `crates/famp/tests/await_timeout.rs` — FOUND
- `crates/famp/tests/inbox_list_respects_cursor.rs` — FOUND
- Commit `c2190da` — FOUND in git log
- Commit `acad630` — FOUND in git log

---
*Phase: 03-conversation-cli*
*Plan: 03*
*Completed: 2026-04-14*
