---
phase: 01-broker-diagnosis-identity-inspection
plan: 01-01
subsystem: bus
tags: [rust, famp-bus, inspector, serde, canonical-json]

requires: []
provides:
  - "famp-inspect-proto RPC type crate with InspectKind and broker/identities/tasks/messages request-reply types"
  - "BusMessage::Inspect and BusReply::InspectOk wire variants"
  - "Register cwd/listen compatibility fields and broker state storage for inspect identities"
  - "Wave-0 inspect broker/identities integration test scaffolds"
affects: [famp-bus, famp, inspector, broker-diagnosis-identity-inspection]

tech-stack:
  added: [famp-inspect-proto]
  patterns:
    - "Inspector RPC types live in a no-I/O proto crate shared by future client/server crates"
    - "BrokerState uses explicit new() when construction needs wall-clock startup time"

key-files:
  created:
    - crates/famp-inspect-proto/Cargo.toml
    - crates/famp-inspect-proto/src/lib.rs
    - crates/famp/tests/inspect_broker.rs
    - crates/famp/tests/inspect_identities.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/famp-bus/Cargo.toml
    - crates/famp-bus/src/proto.rs
    - crates/famp-bus/src/broker/state.rs
    - crates/famp-bus/src/broker/mod.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp/src/cli/register.rs
    - crates/famp/src/cli/mcp/tools/register.rs

key-decisions:
  - "Use explicit BrokerState::new() instead of Default so started_at is SystemTime::now(), not UNIX_EPOCH."
  - "Register callers now populate cwd from current_dir() and listen from CLI tail / MCP listen input."

patterns-established:
  - "InspectKind is op-tagged, snake_case, deny_unknown_fields to match BusMessage wire strictness."
  - "Forward inspect dispatch returns a typed bus error until the server dispatch crate lands in a later wave."

requirements-completed: [INSP-RPC-01, INSP-IDENT-01, INSP-IDENT-03, INSP-CRATE-01, INSP-BROKER-01]

duration: 13min
completed: 2026-05-10
---

# Phase 1 Plan 01-01: Wave 0 Inspect Foundations Summary

**Inspector proto types, bus inspect wire variants, register cwd/listen state, and Wave-0 test scaffolds for later broker/identity inspection waves**

## Performance

- **Duration:** 13 min
- **Started:** 2026-05-10T15:58:16Z
- **Completed:** 2026-05-10T16:11:08Z
- **Tasks:** 3
- **Files modified:** 23

## Accomplishments

- Added `famp-inspect-proto` as a workspace crate with `InspectKind`, four request/reply pairs, `IdentityRow`, canonical round-trip tests, and the INSP-IDENT-03 forbidden-field schema test.
- Extended `famp-bus` with `BusMessage::Inspect { kind }`, `BusReply::InspectOk { payload }`, and `Register { cwd, listen }` compatibility fields.
- Replaced `BrokerState::default()` with `BrokerState::new()` and added client/broker timestamps plus stored cwd/listen state for future identity inspection.
- Added compiling Wave-0 integration test scaffolds for `inspect_broker` and `inspect_identities`.

## Task Commits

1. **Task 1: Create famp-inspect-proto crate** - `f3e01c3` (feat)
2. **Task 2: Extend famp-bus proto wire shape** - `fece87e` (feat)
3. **Task 3: Wire broker state foundations and scaffolds** - `0430b4f` (feat)

**Plan metadata:** pending final summary commit

## Files Created/Modified

- `crates/famp-inspect-proto/src/lib.rs` - Inspector RPC type system, identity row schema, and proto-level tests.
- `crates/famp-inspect-proto/Cargo.toml` - No-I/O proto crate manifest.
- `Cargo.toml`, `Cargo.lock` - Workspace membership and lockfile updates.
- `crates/famp-bus/src/proto.rs` - Inspect/Register/InspectOk wire-shape changes and codec tests.
- `crates/famp-bus/src/broker/state.rs` - `cwd`, `listen_mode`, `registered_at`, `last_activity`, and `started_at`.
- `crates/famp-bus/src/broker/mod.rs` - Broker construction now uses `BrokerState::new()`.
- `crates/famp-bus/src/broker/handle.rs` - Register destructuring/state assignment plus temporary inspect dispatch error.
- `crates/famp/src/cli/register.rs` - Sends cwd and listen mode on register.
- `crates/famp/src/cli/mcp/tools/register.rs` - Accepts MCP `listen` and sends cwd/listen on register.
- `crates/famp/tests/inspect_broker.rs`, `crates/famp/tests/inspect_identities.rs` - Wave-0 scaffold tests.

## Decisions Made

- Followed the plan's explicit `BrokerState::new()` direction to avoid a misleading `UNIX_EPOCH` default startup time.
- Used `std::env::current_dir().ok()` for register cwd capture so registration remains best-effort and does not fail if cwd is unavailable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added temporary Inspect match arm**
- **Found during:** Task 2
- **Issue:** Adding `BusMessage::Inspect` made `handle_wire()` non-exhaustive, so `cargo build -p famp-bus` could not pass before the later dispatch wave.
- **Fix:** Added a temporary `BusReply::Err { kind: Internal, ... }` path for Inspect frames until the server dispatch crate lands.
- **Files modified:** `crates/famp-bus/src/broker/handle.rs`
- **Verification:** `cargo build -p famp-bus`; `cargo nextest run -p famp-bus --lib`
- **Committed in:** `fece87e`

**2. [Rule 3 - Blocking] Updated Register constructors after adding cwd/listen**
- **Found during:** Task 3
- **Issue:** Existing CLI, MCP, and test constructors for `BusMessage::Register` no longer compiled after adding required Rust enum fields.
- **Fix:** Updated test constructors with `cwd: None, listen: false`, and updated CLI/MCP registration to populate cwd/listen from runtime inputs.
- **Files modified:** `crates/famp/src/cli/register.rs`, `crates/famp/src/cli/mcp/tools/register.rs`, `crates/famp-bus/tests/*.rs`, `crates/famp/tests/broker_lifecycle.rs`
- **Verification:** `cargo build --workspace --all-targets`; `cargo nextest run --workspace`
- **Committed in:** `0430b4f`

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes were required to keep atomic task commits buildable after the wire enum changed. No architecture change.

## Known Stubs

- `crates/famp-bus/src/broker/handle.rs` - `BusMessage::Inspect` currently returns an Internal error because the inspect server dispatch crate is scheduled for a later wave.
- `crates/famp/tests/inspect_broker.rs` - Intentional Wave-0 scaffold; Wave 3 will populate real broker inspection tests.
- `crates/famp/tests/inspect_identities.rs` - Intentional Wave-0 scaffold; Wave 3 will populate real identities inspection tests.

## Issues Encountered

- Avoided committing unrelated pre-existing `.planning/STATE.md` and `.claude/*` working tree changes; shared state updates are intentionally owned by the orchestrator.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo build -p famp-inspect-proto` - passed
- `cargo nextest run -p famp-inspect-proto --lib` - passed, 5 tests
- `cargo build -p famp-bus` - passed
- `cargo nextest run -p famp-bus --lib` - passed, 35 tests
- `cargo build -p famp` - passed
- `cargo nextest run -p famp --test inspect_broker` - passed, 1 test
- `cargo nextest run -p famp --test inspect_identities` - passed, 1 test
- `cargo nextest run --workspace` - passed, 581 tests, 2 skipped
- `cargo build --workspace --all-targets` - passed
- `just check-no-tokio-in-bus` - passed

## Next Phase Readiness

Wave 1 can consume `famp-inspect-proto` directly. Wave 2 must replace the temporary `BusMessage::Inspect` error arm with real `famp-inspect-server` dispatch.

## Self-Check: PASSED

- Created-file checks passed for `crates/famp-inspect-proto/Cargo.toml`, `crates/famp-inspect-proto/src/lib.rs`, `crates/famp/tests/inspect_broker.rs`, `crates/famp/tests/inspect_identities.rs`, and this summary.
- Commit checks passed for `f3e01c3`, `fece87e`, and `0430b4f`.

---
*Phase: 01-broker-diagnosis-identity-inspection*
*Completed: 2026-05-10*
