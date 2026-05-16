---
phase: quick-260515-s3h
plan: 01
subsystem: cli/bus
tags: [famp-await, stop-hook, channel-aware, write_outcome, wrapper-json]
dependency_graph:
  requires: [famp-bus batch-AwaitOk (146ca9f)]
  provides: [channel-aware stop-hook wake messages, CLI wrapper JSON output]
  affects: [famp-await.sh, write_outcome, wait_reply]
tech_stack:
  added: []
  patterns: [wrapper-JSON envelope shape, pipe-separated bash fields]
key_files:
  created: []
  modified:
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/src/cli/wait_reply.rs
    - crates/famp/assets/famp-await.sh
    - crates/famp/tests/wait_reply.rs
decisions:
  - "Emit single wrapper JSON object (not per-envelope JSONL) so shell can extract mailbox kind without subshell per-line parsing"
  - "Backward-compat fallback in Python META block handles raw envelope lines from old binaries"
  - "Channel name gets '#' prefix added by hook if not already present"
metrics:
  duration: ~60 min (including pre-existing test hang investigation)
  completed: 2026-05-15
---

# Phase quick-260515-s3h Plan 01: Fix stop-hook blind spot — channel-aware wake messages

**One-liner:** `write_outcome` now emits a wrapper JSON with `mailbox.kind` so `famp-await.sh` can direct agents to `famp_channel_log` (channels) vs `famp_inbox` (DMs) in the wake notification.

## What Was Built

The stop hook (`famp-await.sh`) runs after each Claude turn when listen mode is active. It calls `famp await` and parses stdout to build the wake notification string. Before this fix, `famp await` printed raw envelope JSONL with no mailbox metadata — the hook always said "Call famp_inbox to read it." For channel messages, `famp_inbox` returns nothing (it reads only DM mailboxes), leaving the woken agent confused.

### Changes

**`crates/famp/src/cli/await_cmd/mod.rs` — `write_outcome`**
- Replaced per-envelope JSONL output with a single wrapper JSON object:
  `{"mailbox": {"kind": "channel"/"agent", "name": "..."}, "envelopes": [...], "next_offset": N}`
- Timeout path unchanged: `{"timeout": true}` (with optional `"diagnostic"`)
- The MCP `famp_await` tool path (`run_at_structured`) is unaffected — it owns its own JSON shape

**`crates/famp/src/cli/wait_reply.rs` — inbox-found path**
- Fixed `mailbox: None` → `mailbox: Some(MailboxName::Agent(identity.clone()))` so the wrapper always has a populated mailbox when envelopes are non-empty
- Added `MailboxName` to the `use famp_bus::...` import

**`crates/famp/assets/famp-await.sh` — META extraction + notification**
- Python block now outputs 4 pipe-separated fields: `{count}|{sender}|{mailbox_kind}|{mailbox_name}`
- Bash parsing extended to extract `MAILBOX_KIND` and `MAILBOX_NAME` from the additional fields
- Notification block is now channel-aware:
  - Channel: "Call famp_channel_log({channel: '#name'}) to read it."
  - DM: "Call famp_inbox to read it." (unchanged behavior)
- Backward-compat fallback branch handles raw envelope lines from older `famp` binaries

**`crates/famp/tests/wait_reply.rs` — updated for wrapper format**
- Both assertion sites updated to parse `wrapper["envelopes"][0]` instead of treating stdout as a direct envelope object

## Commits

| Hash | Description |
|------|-------------|
| ae13c43 | fix(await): emit wrapper JSON from CLI so stop-hook can generate channel-aware wake messages |

## Verification

- `cargo check -p famp` — PASSED (release build succeeded in 35.25s)
- `bash -n crates/famp/assets/famp-await.sh` — PASSED (syntax OK)
- `cargo nextest run` (cli_dm_roundtrip, 5/5) — PASSED, including `test_await_unblocks` which asserts the CLI output via string contains — the `"ping"` string appears inside the wrapper `envelopes` array, confirming wrapper output is correct
- `just install` — PASSED (`~/.cargo/bin/famp` replaced with updated binary)
- `grep -c 'famp_channel_log' crates/famp/assets/famp-await.sh` — 2 occurrences (singular and plural forms)
- `grep -c '"mailbox"' crates/famp/src/cli/await_cmd/mod.rs` — multiple occurrences

## Deviations from Plan

None in implementation. The code changes matched the plan exactly.

## Deferred Issues

**[Deferred] `wait_reply_and_await_find_existing_terminal_reply` test hang (pre-existing from 146ca9f)**

The `crates/famp/tests/wait_reply.rs` test (`wait_reply_and_await_find_existing_terminal_reply`) hangs indefinitely under both `cargo test` and `cargo nextest`. This was confirmed to be pre-existing: the test also hangs on unmodified `main` HEAD (git stash pop verified). The hang appears at the test binary start-up phase before any MCP processes are spawned, suggesting a deadlock in the test harness initialization or broker auto-start interaction.

Root cause: the 146ca9f commit (`feat(bus): batch AwaitOk delivery to fix burst message loss`) renamed the test from `wait_reply_finds_existing_terminal_reply_that_await_misses` (which expected `famp await` to timeout and return `{"timeout":true}`) to `wait_reply_and_await_find_existing_terminal_reply` (which expects `famp await` to return the existing reply). The new test semantics require `famp await` to drain from offset 0 on a fresh proxy connection — which the broker supports — but the test binary initialization hangs before reaching that point.

Our task updates the test's assertion sites to parse the wrapper format, which are correct for when the test eventually runs. The hang itself is out of scope for this quick fix.

**Impact:** The `wait_reply` test suite cannot be run. The `cli_dm_roundtrip` suite (5/5 passing) provides coverage of the `write_outcome` wrapper format change via `test_await_unblocks`.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes were introduced. `write_outcome` is a pure serialization change inside the trusted `famp` binary. The hook's channel name and sender fields are extracted from broker-controlled data (not raw envelope peer bytes) and validated by the existing `grep -qE '^[A-Za-z0-9@._:/-]{1,128}$'` guard.

## Self-Check: PASSED

- ae13c43 commit exists: CONFIRMED
- `crates/famp/src/cli/await_cmd/mod.rs` contains `"mailbox"`: CONFIRMED
- `crates/famp/assets/famp-await.sh` contains `famp_channel_log`: CONFIRMED
- `crates/famp/tests/wait_reply.rs` contains `wrapper["envelopes"]`: CONFIRMED
- `~/.cargo/bin/famp` updated by `just install`: CONFIRMED
