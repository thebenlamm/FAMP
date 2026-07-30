---
phase: quick-260730-d9h
plan: 01
subsystem: mcp-tool-registry
tags: [mcp, tool-descriptions, test-hygiene, ci]
dependency-graph:
  requires: []
  provides: [corrected-mcp-tool-descriptions, flake-free-famp-local-wire-migration-tests]
  affects: [crates/famp/src/cli/mcp/server.rs, crates/famp/tests/famp_local_wire_migration.rs]
tech-stack:
  added: []
  patterns: [static-string-correction, dead-test-deletion]
key-files:
  created: []
  modified:
    - crates/famp/src/cli/mcp/server.rs
    - crates/famp/tests/famp_local_wire_migration.rs
  deleted:
    - crates/famp/tests/fixtures/famp_local_wire/legacy.mcp.json
    - crates/famp/tests/fixtures/famp_local_wire/already_migrated.mcp.json
decisions:
  - "Also deleted crates/famp/tests/fixtures/famp_local_wire/already_migrated.mcp.json (not named in the plan's files_modified list) because it was equally orphaned — no test read it after the two behavioral tests were removed — and the plan's own <done> criterion required the fixtures/famp_local_wire/ directory to end up empty."
metrics:
  duration: "~25 minutes"
  completed: "2026-07-30"
status: complete
---

# Phase quick-260730-d9h Plan 01: Fix stale deferred-to-v1 MCP tool descriptions Summary

Rewrote three factually false strings in the MCP tool registry that named
a "v1" deferral for a filter that never shipped as of the now-tagged v1.0.0,
and deleted two chronically flaky tests that drove an archived, CI-unreachable
shell script.

## What changed

### Task 1 — `crates/famp/src/cli/mcp/server.rs` (commit `7bab8f7`)

**Edit 1 — `famp_await` description**, before:
> `...This is the canonical real-time signal — unlike famp_inbox list (which hides entries for tasks that have reached a terminal FSM state), famp_await delivers every message including the closing 'terminal' reply that finishes a task...`

after:
> `...This is the canonical real-time signal — famp_await delivers every message as it arrives, including the closing 'terminal' reply that finishes a task...`

**Edit 2 — `famp_inbox` description**, before:
> `...Note: include_terminal is accepted on the wire but currently a no-op — broker-side terminal-FSM filtering is deferred to v1; today every list returns all unread envelopes.`

after:
> `...Note: include_terminal is accepted for wire compatibility but is currently a no-op — broker-side terminal-FSM filtering has not shipped as of v1.0, so every list returns all unread envelopes.`

**Edit 3 — `include_terminal` property description**, before:
> `Accepted for wire compatibility but currently a no-op: broker-side terminal-FSM filtering (hide COMPLETED/FAILED/CANCELLED) is deferred to v1. Today, list returns every unread envelope regardless of this flag.`

after:
> `Accepted for wire compatibility but currently a no-op: broker-side terminal-FSM filtering (hide COMPLETED/FAILED/CANCELLED) has not shipped as of v1.0. Today, list returns every unread envelope regardless of this flag.`

Only string VALUES changed inside `fn tool_descriptors()`; no reordering, no
reformatting, no descriptor added/removed. `crates/famp/src/cli/peer/mod.rs`
was NOT touched (its "deferred to v1.1" is an unrelated per-peer-keypair doc
comment, confirmed out of scope by the plan's own grep).

### Task 2 — `crates/famp/tests/famp_local_wire_migration.rs` (commit `21dd6f4`)

Deleted `wire_rewrites_legacy_mcp_json_in_place` and
`wire_idempotent_on_already_migrated_file`, plus the now-dead helpers
`make_stub_famp`, `seed_agent`, `generate_self_signed_cert_pem`,
`cmd_with_env`, and the now-unused `use std::process::Command;` import.
Kept `mcp_add_does_not_emit_famp_home`, `workspace_root()`, `script_path()`,
`use std::path::{Path, PathBuf};`, and the crate-root `#![allow(...)]` block
unmodified. Rewrote the module `//!` doc to describe the surviving static
grep and record the verified root cause of the flake (ephemeral port range
collision on ubuntu runners).

Deleted `crates/famp/tests/fixtures/famp_local_wire/legacy.mcp.json` (named
in the plan) after confirming via `grep -rn "legacy.mcp.json\|famp_local_wire" crates/ .github/ justfile`
that it had zero remaining consumers. Also deleted the sibling
`already_migrated.mcp.json` in the same directory — **this is a deviation
from the plan's exact file list**: it was not named as a target, but it was
equally orphaned (never read by any test — `wire_idempotent_on_already_migrated_file`
built its migrated-file content inline rather than reading this fixture),
and leaving it in place would have left the `fixtures/famp_local_wire/`
directory non-empty, contradicting the plan's own `<done>` criterion ("...
and its now-empty `fixtures/famp_local_wire/` directory in this same
commit"). Removed both files and the now-empty directory in the Task 2
commit.

## Verification — literal output

**Task 1:**
```
$ cargo test -p famp --test slash_command_assets
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p famp --lib tool_descriptors
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 273 filtered out

$ grep -c "has not shipped as of v1.0" crates/famp/src/cli/mcp/server.rs
2

$ grep -rn "which hides entries for tasks" crates/
(no output — grep exit 1, success case)

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 31s
(exit 0, zero warnings)
```

Pre-commit grep for stale substrings:
```
$ grep -rn "deferred to v1" crates/
crates/famp/src/cli/peer/mod.rs:19://! each needs its own keypair; generalizing this is deferred to v1.1.
(exactly one hit, confirmed out-of-scope unrelated doc comment — left untouched)

$ grep -rn "which hides entries for tasks" crates/
(no output)
```

**Task 2:**
```
$ cargo test -p famp --test famp_local_wire_migration
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ grep -c "wire_rewrites_legacy_mcp_json_in_place\|wire_idempotent_on_already_migrated_file\|make_stub_famp\|seed_agent\|generate_self_signed_cert_pem\|cmd_with_env" crates/famp/tests/famp_local_wire_migration.rs
0

$ grep -c "mcp_add_does_not_emit_famp_home\|fn workspace_root\|fn script_path" crates/famp/tests/famp_local_wire_migration.rs
3

$ just lint
cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.38s
(exit 0, zero warnings)
```

**Task 3 — installed binary falsification (grep against `~/.cargo/bin/famp`, not source):**
```
$ ls -l ~/.cargo/bin/famp
-rwxr-xr-x  1 benlamm  staff  7675200 Jul 30 10:19 /Users/benlamm/.cargo/bin/famp
(newer than Task 1 commit 7bab8f7, timestamped 2026-07-30 10:17:04 -0400)

$ grep -ao "terminal-FSM filtering is deferred to v1" ~/.cargo/bin/famp | wc -l
0

$ grep -ao "which hides entries for tasks" ~/.cargo/bin/famp | wc -l
0

$ grep -ao "has not shipped as of v1.0" ~/.cargo/bin/famp | wc -l
3
```
(Plan required "2 or more" — 3 is a positive, expected count; the source-level
`grep -c` reported 2 lines because `include_terminal`'s two-clause description
sits mid-line alongside other content in the JSON, so the binary's `.rodata`
substring count and the source line count are not required to match exactly.
All three grep checks are positive together, satisfying the falsification.)

## Commits, push, and CI

- `7bab8f7` — `fix(quick-260730-d9h): drop stale deferred-to-v1 claims from MCP tool descriptions`
- `21dd6f4` — `test(quick-260730-d9h): delete flaky archived-script daemon tests`

Pushed to `main` (`d5f3c47..21dd6f4`). Exact pushed SHA:
`21dd6f4e2a00914ea9841d505a9fe92a993ce94a`.

CI check-runs for that exact SHA:
```
$ gh api "repos/thebenlamm/FAMP/commits/21dd6f4e2a00914ea9841d505a9fe92a993ce94a/check-runs" --jq '.total_count'
12
```
All 12 conclusions `success`: `test (macos-latest)`, `test (ubuntu-latest)`,
`doc-test`, `build (ubuntu-latest)`, `build (macos-latest)`, `fmt-check`,
`famp-canonical RFC 8785 conformance gate`, `clippy`, `audit`,
`smoke-test (Quick Start install path)`,
`famp-crypto §7.1c worked-example + RFC 8032 gate`, `asset-gate`.

(Note: the first `check-runs` query immediately after push returned
`total_count == 0` because CI had not yet registered check-runs for the
fresh SHA — `gh run list` showed `queued`/`in_progress` at that moment. This
was NOT treated as UNVERIFIED-and-stop; the runs were watched to completion
with `gh run watch` and the check-runs query was re-run against the same
exact SHA, which then returned the populated, all-`success` result quoted
above.)

## Deviations from Plan

### Auto-fixed / extended issues

**1. [Rule 1 — plan-assumption gap] Deleted `already_migrated.mcp.json` in addition to `legacy.mcp.json`**
- Found during: Task 2's orphan-fixture verification step.
- Issue: the plan named only `legacy.mcp.json` for deletion and asserted the
  directory would be "now-empty" afterward, but a second unreferenced
  fixture (`already_migrated.mcp.json`) also lived in that directory and was
  never named.
- Fix: confirmed `already_migrated.mcp.json` had zero consumers repo-wide,
  then deleted it alongside `legacy.mcp.json` so the directory removal the
  plan's `<done>` criterion describes actually holds.
- Files modified: `crates/famp/tests/fixtures/famp_local_wire/already_migrated.mcp.json` (deleted)
- Commit: `21dd6f4`

No other deviations. All other action items, wording, and constraints were
followed exactly as written.

## Known Stubs

None.

## Threat Flags

None — this plan corrects existing description text and deletes tests; it
introduces no new network endpoint, auth path, file-access pattern, or
schema change at a trust boundary.

## Self-Check: PASSED

- FOUND: crates/famp/src/cli/mcp/server.rs (edited, present)
- FOUND: crates/famp/tests/famp_local_wire_migration.rs (edited, present)
- MISSING (expected — deleted intentionally): crates/famp/tests/fixtures/famp_local_wire/legacy.mcp.json
- MISSING (expected — deleted intentionally): crates/famp/tests/fixtures/famp_local_wire/already_migrated.mcp.json
- FOUND commit 7bab8f7 in `git log --oneline --all`
- FOUND commit 21dd6f4 in `git log --oneline --all`
