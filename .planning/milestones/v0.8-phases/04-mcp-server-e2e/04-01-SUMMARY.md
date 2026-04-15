---
phase: 04-mcp-server-e2e
plan: "01"
subsystem: famp-daemon-autocommit
tags: [keyring, auto-commit, fsm, tdd, envelope-dispatch]
dependency_graph:
  requires: []
  provides: [multi-entry-keyring, auto-commit-handler, natural-fsm-walk]
  affects: [famp-fsm, famp-envelope, famp-inbox, famp-listen, famp-send, famp-await]
tech_stack:
  added: []
  patterns:
    - "Multi-entry Keyring built from peers.toml at daemon startup (fatal on malformed entries)"
    - "Fire-and-forget auto-commit via tokio::spawn in router handler after inbox fsync"
    - "Causality.referenced serializes as JSON key 'ref' (serde rename), not 'referenced'"
    - "await --task X skips request-class entries (originator awaits replies, not their own request)"
    - "TaskFsm::resume(state) reconstructs FSM from disk state without test shortcuts"
key_files:
  created:
    - crates/famp/src/cli/listen/auto_commit.rs
    - crates/famp/tests/conversation_auto_commit.rs
    - crates/famp/tests/listen_multi_peer_keyring.rs
  modified:
    - crates/famp/src/cli/listen/mod.rs
    - crates/famp/src/cli/listen/router.rs
    - crates/famp/src/cli/send/fsm_glue.rs
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/src/cli/await_cmd/poll.rs
    - crates/famp/src/cli/error.rs
    - crates/famp-envelope/src/dispatch.rs
    - crates/famp-fsm/src/engine.rs
    - crates/famp/tests/conversation_full_lifecycle.rs
    - crates/famp/tests/conversation_restart_safety.rs
    - crates/famp/tests/send_deliver_sequence.rs
    - crates/famp/tests/send_terminal_blocks_resend.rs
decisions:
  - "Auto-commit is fire-and-forget (spawn detached task) so the 200 durability receipt is never delayed"
  - "Causality.ref JSON key discovered at debug time — serde rename was the root cause of AwaitTimeout"
  - "find_match skips request-class entries when task_filter is Some (originator never awaits their own request)"
  - "TaskFsm::resume() added as public constructor to replace __with_state_for_testing in production paths"
  - "Three Phase 3 tests updated for Phase 4 inbox counts (auto-commit adds one commit envelope per request)"
metrics:
  duration: "~29 minutes (across two sessions)"
  completed: "2026-04-15"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 13
---

# Phase 04 Plan 01: Multi-Entry Keyring + Auto-Commit Round-Trip Summary

One-liner: Ed25519 multi-peer keyring from peers.toml + daemon auto-commit handler driving REQUESTED→COMMITTED→COMPLETED via real signed envelope round-trips.

## What Was Built

### Task 1: Multi-Entry Keyring (GREEN — commit f1db1d2)

`build_keyring()` in `listen/mod.rs` reads `peers.toml`, decodes each peer's `pubkey_b64` (base64url → 32-byte Ed25519 key → `TrustedVerifyingKey`), parses the `principal` field, inserts each peer via `Keyring::with_peer()`, then adds the daemon's own self-entry last. Any malformed entry is fatal (T-04-01: daemon refuses to start with a silently narrowed trust set). A new `CliError::KeyringBuildFailed { alias, reason }` variant surfaces the exact failure.

Three integration tests in `listen_multi_peer_keyring.rs` verify:
- Accepts signed envelopes from a registered peer principal
- Accepts envelopes from self (self-addressed flow)
- Rejects envelopes from an unregistered principal (FampSigVerifyLayer fires before handler)

### Task 2: Auto-Commit Handler + Natural FSM Walk (GREEN — commit cbe5041)

**Auto-commit dispatch** (`listen/auto_commit.rs`): After the router's inbox append fsyncs and returns 200, the handler checks `envelope.class() == MessageClass::Request`. If true, it calls `spawn_reply()` which fires a detached `tokio::spawn` task. `send_reply()` reads `peers.toml` for the request's `from` principal (T-04-02: unknown principals are logged and dropped), builds a `CommitBody` with `Causality { rel: Commits, referenced: req_id }`, signs it with the daemon's own key (T-04-03), and POSTs it back via `post_envelope()`.

**AnySignedEnvelope::class()** added to `famp-envelope/src/dispatch.rs` so the router can inspect the class of any already-decoded envelope without re-parsing.

**TaskFsm::resume(state)** added to `famp-fsm` as a public `const fn` constructor that reconstructs the FSM from a disk-persisted state without test-only shortcuts.

**fsm_glue rewrite**: `advance_committed()` does REQUESTED→COMMITTED via `TaskFsm::resume(current).step(Commit, None)`. `advance_terminal()` does COMMITTED→COMPLETED via `TaskFsm::resume(current).step(Deliver, Some(Completed))`. Neither function seeds state with `__with_state_for_testing`.

**await_cmd integration**: When `find_match` returns a commit-class entry, `mod.rs` calls `advance_committed` on the matching local task record before printing output, driving the originator's REQUESTED→COMMITTED transition without any inbox re-read.

**poll.rs fixes** (two bugs found during debugging):
1. `extract_task_id` read `causality["referenced"]` but the `Causality` struct serializes the field as `"ref"` (serde rename). Fixed to read `causality["ref"]`.
2. `find_match` with a task filter was matching the originator's own outgoing request envelope (whose `id == task_id`) before the commit reply arrived. Fixed by skipping `class == "request"` entries when `task_filter` is `Some`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] causality["referenced"] → causality["ref"] in poll.rs**
- **Found during:** Task 2 debugging — `await_run_at --task X` timed out despite commit arriving
- **Issue:** `extract_task_id` looked for JSON key `"referenced"` but `Causality` serde-renames the field to `"ref"`, so no commit envelope ever matched the task filter
- **Fix:** Changed `c.get("referenced")` to `c.get("ref")` in `extract_task_id`
- **Files modified:** `crates/famp/src/cli/await_cmd/poll.rs`
- **Commit:** cbe5041

**2. [Rule 1 - Bug] find_match returned originator's own request instead of commit reply**
- **Found during:** Task 2 debugging — `await_run_at --task X` returned `class=request` instead of waiting for `class=commit`
- **Issue:** When `task_filter` is set, `find_match` matched the outgoing request (whose `id == task_id`) first, returning it before the commit reply arrived
- **Fix:** Added skip rule: when `task_filter` is `Some`, skip `class == "request"` entries entirely
- **Files modified:** `crates/famp/src/cli/await_cmd/poll.rs`
- **Commit:** cbe5041

**3. [Rule 2 - Missing Critical] fsm_glue doc comment contained the banned string**
- **Found during:** Task 2 test step 6 (grep gate) — `rg '__with_state_for_testing' crates/famp/src` returned a hit in a comment
- **Fix:** Replaced the doc comment phrase with neutral wording
- **Files modified:** `crates/famp/src/cli/send/fsm_glue.rs`
- **Commit:** cbe5041

**4. [Rule 1 - Bug] Three Phase 3 tests used inbox count / FSM assumptions invalid in Phase 4**
- **Found during:** Task 2 full test run — 3 tests failed after auto-commit landed
- **Issue:** `send_deliver_sequence`, `conversation_restart_safety`, and `send_terminal_blocks_resend` assumed inbox lines = request + delivers, not accounting for the auto-commit reply. `send_terminal_blocks_resend` also went REQUESTED→terminal without awaiting COMMITTED state.
- **Fix:** Updated inbox count assertions (+1 for commit reply per request); added `await_cmd::run_at --task X` before terminal deliver in `send_terminal_blocks_resend` to properly advance to COMMITTED
- **Files modified:** `crates/famp/tests/send_deliver_sequence.rs`, `crates/famp/tests/conversation_restart_safety.rs`, `crates/famp/tests/send_terminal_blocks_resend.rs`
- **Commit:** cbe5041

## Verification

- `cargo nextest run --workspace`: 347/347 pass, 1 skipped
- `cargo clippy --all-targets`: clean (no errors)
- `rg '__with_state_for_testing' crates/famp/src`: zero matches
- `cargo tree -i openssl`: empty

## Known Stubs

None — all data flows are wired end-to-end.

## Threat Flags

No new threat surface beyond what the plan's threat model covers. Auto-commit's T-04-02 (principal lookup gate) and T-04-03 (signed reply) mitigations are fully implemented.

## Self-Check: PASSED

Files created/modified verified present:
- `crates/famp/src/cli/listen/auto_commit.rs` — exists
- `crates/famp/tests/conversation_auto_commit.rs` — exists
- `crates/famp/tests/listen_multi_peer_keyring.rs` — exists

Commits verified:
- f1db1d2 (Task 1: multi-entry keyring) — exists
- cbe5041 (Task 2: auto-commit + natural FSM) — exists
