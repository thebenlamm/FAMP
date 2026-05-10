---
phase: 02-task-fsm-message-visibility
plan: 01
subsystem: inspector
tags: [inspector, observability, proto, jcs, fsm]

requires:
  - phase: 01-broker-diagnosis-identity-inspection
    provides: Inspector RPC namespace, proto/client/server crates, broker context pattern
provides:
  - Kind-tagged task and message inspector reply enums
  - TaskSnapshot and MessageSnapshot context types for sync server handlers
  - Task and message inspect dispatch logic with FSM derivation and body hash prefixes
affects: [02-task-fsm-message-visibility, famp-inspect-proto, famp-inspect-server, broker]

tech-stack:
  added: [sha2, hex, time]
  patterns:
    - Internally tagged `kind` reply enums for inspector wire output
    - Lazy pre-read context fields on `BrokerCtx`
    - TDD RED/GREEN commits for protocol and server handler behavior

key-files:
  created: []
  modified:
    - crates/famp-inspect-proto/src/lib.rs
    - crates/famp-inspect-server/src/lib.rs
    - crates/famp-inspect-server/Cargo.toml
    - crates/famp/src/cli/broker/mod.rs
    - Cargo.lock

key-decisions:
  - "Inspector task/message replies use `#[serde(tag = \"kind\", rename_all = \"snake_case\")]` as the v0.10 wire commitment."
  - "famp-inspect-server remains sync/tokio-free by accepting pre-read TaskSnapshot and MessageSnapshot data through BrokerCtx."
  - "The INSP-TASK-04 A1 proof uses the existing Phase 1 vector_0 canonical.hex fixture for byte-for-byte JCS roundtrip validation."

patterns-established:
  - "Wire replies are enums when variants have different shapes."
  - "Message bodies stay private: handlers expose only canonical body byte length and a 12-hex sha256 prefix."
  - "FSM state derivation distinguishes completed, failed, and cancelled terminal modes."

requirements-completed: [INSP-TASK-01, INSP-TASK-02, INSP-TASK-03, INSP-TASK-04, INSP-MSG-01, INSP-MSG-02, INSP-MSG-03, INSP-RPC-03]

duration: 12 min
completed: 2026-05-10
---

# Phase 02 Plan 01: Proto and Sync Handler Wire Shape Summary

**Task and message inspector RPC wire types plus tokio-free sync handlers for task snapshots and mailbox metadata.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-10T21:58:49Z
- **Completed:** 2026-05-10T22:10:33Z
- **Tasks:** 2
- **Files modified:** 5 code files plus this summary

## Accomplishments

- Replaced Phase 1 `not_yet_implemented` task/message reply stubs with `kind`-tagged enum replies and supporting task/message row structs.
- Added TDD coverage for reply codec roundtrips, orphan task ID classification, and byte-for-byte canonical JCS roundtrip against the Phase 1 vector fixture.
- Added `TaskSnapshot`, `MessageSnapshot`, and sync server handlers for task list/detail/full and message list output.
- Computed message body metadata without exposing bodies: canonical body byte length plus 12-character sha256 prefix.
- Fixed FSM derivation so failed and cancelled terminal states no longer collapse to completed.

## Task Commits

1. **Task 1 RED: Proto tests** - `5dd1817` (test)
2. **Task 1 GREEN: Proto wire types** - `6a937d7` (feat)
3. **Task 2 RED: Server handler tests** - `cdaea31` (test)
4. **Task 2 GREEN: Server handlers** - `44da85e` (feat)

## Files Created/Modified

- `crates/famp-inspect-proto/src/lib.rs` - Added task/message reply enums, row/detail/full structs, orphan helper, and codec/JCS tests.
- `crates/famp-inspect-server/src/lib.rs` - Added snapshot types, extended `BrokerCtx`, implemented task/message dispatch handlers, helper extraction, FSM derivation, and unit tests.
- `crates/famp-inspect-server/Cargo.toml` - Added `sha2`, `hex`, and `time` dependencies for message hashes and RFC3339 parsing.
- `crates/famp/src/cli/broker/mod.rs` - Initialized new `BrokerCtx` fields to `None` until Plan 02 wires lazy pre-read data.
- `Cargo.lock` - Locked the added `time` transitive dependency.

## Verification

- `cargo build -p famp-inspect-proto -p famp-inspect-server` - passed
- `cargo nextest run -p famp-inspect-proto -p famp-inspect-server` - passed, 27/27 tests
- `cargo nextest run -p famp-inspect-proto -E 'test(=canonicalize_roundtrip)'` - passed
- `cargo nextest run -p famp-inspect-server -E 'test(/derive_fsm_state_/)'` - passed, 4/4 tests
- `cargo check -p famp` - passed after the broker context compatibility fix
- `just check-inspect-readonly` - passed
- `just check-no-io-in-inspect-proto` - passed
- `grep -c 'not_yet_implemented' crates/famp-inspect-proto/src/lib.rs` - 0
- `grep -c 'not_yet_implemented' crates/famp-inspect-server/src/lib.rs` - 0

## Decisions Made

- Used `canonical.hex` instead of `envelope.json` for `canonicalize_roundtrip`, because the envelope JSON fixture is pretty-printed and includes the signature while the plan requires canonical bytes.
- Added the minimal broker constructor compatibility update in this plan so the workspace-facing `famp` crate still type-checks after `BrokerCtx` gained required fields.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated broker context constructor for new BrokerCtx fields**
- **Found during:** Task 2 (server handler implementation)
- **Issue:** Adding required `task_data` and `message_data` fields to `BrokerCtx` broke `crates/famp/src/cli/broker/mod.rs`.
- **Fix:** Initialized both fields to `None` in the existing Phase 1 context builder; Plan 02 will replace this with lazy pre-read data for tasks/messages.
- **Files modified:** `crates/famp/src/cli/broker/mod.rs`
- **Verification:** `cargo check -p famp` passed.
- **Committed in:** `44da85e`

---

**Total deviations:** 1 auto-fixed (Rule 3)
**Impact on plan:** Compatibility-only fix required by the `BrokerCtx` API change; no mutation or I/O was added to `famp-inspect-server`.

## Issues Encountered

None.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 02 to wire the broker executor pre-read path, spawn_blocking/timeout budget, and real `TaskSnapshot`/`MessageSnapshot` population.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/02-task-fsm-message-visibility/02-01-SUMMARY.md`
- Key code files exist: `crates/famp-inspect-proto/src/lib.rs`, `crates/famp-inspect-server/src/lib.rs`
- Task commits found: `5dd1817`, `6a937d7`, `cdaea31`, `44da85e`

---
*Phase: 02-task-fsm-message-visibility*
*Completed: 2026-05-10*
