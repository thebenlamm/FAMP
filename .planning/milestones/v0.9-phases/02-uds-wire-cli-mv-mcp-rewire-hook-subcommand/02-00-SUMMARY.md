---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 00
subsystem: testing
tags: [test-stubs, wave-0, infrastructure, ignore-gating, integration-tests]

# Dependency graph
requires:
  - phase: 01-famp-bus-library-and-audit-log
    provides: famp-bus crate (BUS-* types), unblocks wire-layer plans that consume the bus
provides:
  - 9 #[ignore]-gated test stub files at canonical paths under crates/famp/tests/
  - Pinned canonical 3-line stub header (#![cfg(unix)] + 2 allows) reused by every owning plan
  - Locked test-name → file → owner map (18 stub functions, 4 owners: 02-10/11/12/13)
affects: [02-01, 02-10, 02-11, 02-12, 02-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "#[ignore]-gated stub: panics with unimplemented!() carrying owner plan ID — accidental un-ignoring fails LOUDLY"
    - "Canonical 3-line test-file header pinned at 02-00 so downstream plans inherit it byte-for-byte"

key-files:
  created:
    - crates/famp/tests/broker_lifecycle.rs
    - crates/famp/tests/broker_spawn_race.rs
    - crates/famp/tests/broker_crash_recovery.rs
    - crates/famp/tests/cli_dm_roundtrip.rs
    - crates/famp/tests/cli_channel_fanout.rs
    - crates/famp/tests/cli_inbox.rs
    - crates/famp/tests/cli_sessions.rs
    - crates/famp/tests/mcp_bus_e2e.rs
    - crates/famp/tests/hook_subcommand.rs
  modified: []

key-decisions:
  - "Compact one-line bodies in multi-stub files (cli_dm_roundtrip.rs 5 stubs, broker_lifecycle.rs 4 stubs, hook_subcommand.rs 3 stubs) to stay under the 30-line cap"
  - "No use-imports, no Cargo.toml change, no shared module — every stub is self-contained against std + unimplemented!() only"

patterns-established:
  - "Wave-0 stub authority chain: 02-00 lays the file, the OWNER plan replaces the body — un-ignoring is the EXCLUSIVE right of the OWNER"
  - "Stub message format: #[ignore = \"stub: implementation lands in plan 02-OWNER\"] paired with unimplemented!(\"filled in by plan 02-OWNER\") — both carry the owner number"

requirements-completed: []

# Metrics
duration: ~4min
completed: 2026-04-28
---

# Phase 02 Plan 00: Wave-0 Test Stub Files Summary

**9 #[ignore]-gated test stub files (18 stub functions, 138 LOC) pre-created so plan 02-01 stays below the 15-file blocker threshold and plans 02-10/11/12/13 have a known-good landing pad to overwrite.**

## Performance

- **Duration:** ~4 minutes
- **Started:** 2026-04-28T22:53:30Z
- **Completed:** 2026-04-28T22:57:20Z
- **Tasks:** 1 (single auto task)
- **Files created:** 9
- **Files modified:** 0
- **Total LOC:** 138 lines (target: ~150)

## Accomplishments

- All 9 Wave-0 stub files exist at canonical paths under `crates/famp/tests/`
- 18 #[ignore]-gated stub functions, names matching `02-VALIDATION.md` Per-Task Verification Map exactly
- Canonical 3-line header (`#![cfg(unix)]` + 2 allows) pinned at byte level
- Every stub carries owner-plan ID in both ignore reason and `unimplemented!()` payload — accidental un-ignoring fails LOUDLY
- `cargo build --workspace --tests` exits 0
- `cargo nextest run -p famp` reports all 18 stubs as SKIP, never FAIL

## Task Commits

1. **Task 1: Create the 9 Wave-0 test stub files** — `dd73883` (test)

## Files Created/Modified

| File | Stub functions | Owner |
|------|----------------|-------|
| `crates/famp/tests/broker_lifecycle.rs` | 4 (`test_broker_accepts_connection`, `test_broker_idle_exit`, `test_sessions_jsonl_diagnostic_only`, `test_nfs_warning`) | 02-11 |
| `crates/famp/tests/broker_spawn_race.rs` | 1 (`test_broker_spawn_race`) | 02-11 |
| `crates/famp/tests/broker_crash_recovery.rs` | 1 (`test_kill9_recovery`) | 02-11 |
| `crates/famp/tests/cli_dm_roundtrip.rs` | 5 (`test_register_blocks`, `test_dm_roundtrip`, `test_inbox_list`, `test_await_unblocks`, `test_whoami`) | 02-12 |
| `crates/famp/tests/cli_channel_fanout.rs` | 1 (`test_channel_fanout`) | 02-12 |
| `crates/famp/tests/cli_inbox.rs` | 1 (`test_inbox_ack_cursor`) | 02-12 |
| `crates/famp/tests/cli_sessions.rs` | 1 (`test_sessions_list`) | 02-12 |
| `crates/famp/tests/mcp_bus_e2e.rs` | 1 (`test_mcp_bus_e2e`) | 02-13 |
| `crates/famp/tests/hook_subcommand.rs` | 3 (`test_hook_add`, `test_hook_list`, `test_hook_remove`) | 02-10 |
| **Total** | **18 stubs** | **4 owners** |

## Cargo.toml Confirmation

**No Cargo.toml change was needed.** Every stub uses only `std` + the `unimplemented!()` macro from the prelude. Zero `use` import statements across all 9 files (verified: `for f in ...; do grep -c '^use ' $f; done` returns `0` per file). Future plans add deps when they fill stubs.

## IGNORED-Count Verification

`cargo nextest run -p famp --no-fail-fast --status-level skip` reports **19 tests skipped** in the famp test crate:
- **18 SKIP** lines map exactly 1-to-1 onto the stub functions created by this plan.
- **1 SKIP** is pre-existing (`cross_machine_happy_path::cross_machine_request_commit_deliver_ack`) and predates 02-00.

Total of 19 skipped satisfies the plan's `awk '$1 >= 19'` acceptance gate. Zero stub function reports as FAIL.

## Decisions Made

- **Compact body style for multi-stub files.** Files with ≥3 stubs (`cli_dm_roundtrip.rs`, `broker_lifecycle.rs`, `hook_subcommand.rs`) use a single-line `fn ... { unimplemented!(...); }` body instead of the 5-line block in the plan's byte template, to stay under the ≤30-line cap. The pin-points (header, attrs, ignore reason, unimplemented payload) are preserved byte-for-byte; only whitespace within function bodies was compacted.
- **No `cargo nextest` fix-up of pre-existing flake.** The single FAIL during nextest (`listen_bind_collision::second_listen_on_same_port_errors_port_in_use`) is a known macOS port-bind timing flake — it pre-dates 02-00 and is documented in STATE.md as part of the "8 pre-existing listener/E2E TLS-loopback timeouts" hygiene-task backlog. Out of scope per execute-plan SCOPE BOUNDARY.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Plan acceptance criterion arithmetic mismatch**
- **Found during:** Task 1 (post-write static check)
- **Issue:** Plan acceptance criteria stated "the other 6 return 1 each — total 19 across 9 files" but the locked test-name map enumerates 4+1+1+5+1+1+1+1+3 = **18** test functions, not 19. The plan's awk gate `awk '$1 >= 19 { exit 0 }` would superficially require 19 IGNORED but the table itself only specifies 18.
- **Fix:** Wrote exactly 18 stubs (1 per row in the locked test-name map — the table is the source of truth). The plan's `awk '$1 >= 19'` gate is satisfied incidentally because nextest reports 19 SKIPs total (18 new + 1 pre-existing `cross_machine_happy_path`). No change to plan or files needed; documented here so 02-01 / 02-VERIFICATION knows the 18-vs-19 discrepancy is a doc-arithmetic bug, not a missing stub.
- **Files modified:** None (only documenting)
- **Verification:** `nextest summary: 19 skipped`, of which `grep -E 'broker_|cli_|mcp_|hook_' SKIP lines | wc -l == 18` (exact match against locked map)
- **Committed in:** N/A — no code change required

---

**Total deviations:** 1 auto-fix (1 documentation/arithmetic Rule 1 bug, no code impact)
**Impact on plan:** Zero. The locked `<interfaces>` map and the `02-VALIDATION.md` Per-Task Verification Map both enumerate 18 stub functions; the prose "total 19" was a tally error. The awk acceptance gate is incidentally satisfied because of a 1 pre-existing ignored test in the famp crate. No scope creep.

## Issues Encountered

- **`cargo` not on `PATH` in fresh Bash shell.** Resolved by exporting `PATH="$HOME/.cargo/bin:$PATH"` for verification commands. Not a deviation — purely a worktree shell-environment quirk.
- **Initial `cli_dm_roundtrip.rs` was 36 lines (>30 cap)** with 5 multi-line stub bodies. Compacted to single-line bodies; final size 26 lines. Caught by line-count static check before commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Plan 02-01 unblocked:** Can drop 9 stub paths from its `files_modified` and stay below the 15-file blocker threshold.
- **Plans 02-10 / 02-11 / 02-12 / 02-13 unblocked:** Each can declare `depends_on: [00]` for stub-file presence without coupling to 02-01's substantive Cargo.toml/bus_client work. When each owner plan replaces its assigned stub file's body, it inherits the canonical 3-line header byte-for-byte.
- **Wave-0 sign-off in `02-VALIDATION.md` remains `wave_0_complete: false`** — gets flipped by 02-01 once Cargo.toml + bus_client land. This plan only handled the stub-creation half of Wave-0.
- **Pre-existing `listen_bind_collision` flake** persists; orthogonal to Phase 02 work; tracked in STATE.md hygiene backlog.

## Self-Check: PASSED

**Files verified on disk (9/9):**
- `crates/famp/tests/broker_lifecycle.rs` — FOUND
- `crates/famp/tests/broker_spawn_race.rs` — FOUND
- `crates/famp/tests/broker_crash_recovery.rs` — FOUND
- `crates/famp/tests/cli_dm_roundtrip.rs` — FOUND
- `crates/famp/tests/cli_channel_fanout.rs` — FOUND
- `crates/famp/tests/cli_inbox.rs` — FOUND
- `crates/famp/tests/cli_sessions.rs` — FOUND
- `crates/famp/tests/mcp_bus_e2e.rs` — FOUND
- `crates/famp/tests/hook_subcommand.rs` — FOUND

**Commits verified in git log:**
- `dd73883` — FOUND (test(02-00): pre-create 9 Wave-0 test stub files)

---
*Phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand*
*Plan: 00*
*Completed: 2026-04-28*
