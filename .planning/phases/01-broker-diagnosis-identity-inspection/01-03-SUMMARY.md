---
phase: 01-broker-diagnosis-identity-inspection
plan: 01-03
subsystem: broker-cli
tags: [rust, famp-bus, inspector, clap, uds]

requires:
  - "01-01"
  - "01-02"
provides:
  - "Out::InspectRequest sentinel path from BusMessage::Inspect to executor-side dispatch"
  - "Broker executor BrokerCtx assembly with mailbox metadata pre-read"
  - "famp inspect broker and famp inspect identities CLI help surface"
affects: [famp-bus, famp, inspector, broker-diagnosis-identity-inspection]

tech-stack:
  added: [famp-inspect-server, famp-inspect-proto, famp-inspect-client]
  patterns:
    - "famp-bus emits inspect requests as pure actor sentinels; famp executor performs I/O and calls dispatch"
    - "Inspect CLI subcommands ship only broker and identities in Phase 1; tasks/messages remain absent"

key-files:
  created:
    - crates/famp/src/cli/inspect/mod.rs
    - crates/famp/src/cli/inspect/broker.rs
    - crates/famp/src/cli/inspect/identities.rs
  modified:
    - crates/famp-bus/src/broker/mod.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp/Cargo.toml
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/lib.rs
    - crates/famp/src/cli/broker/mod.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/tests/inspect_broker.rs
    - crates/famp/tests/inspect_identities.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/cross_machine_two_agents.rs

key-decisions:
  - "Kept famp-bus free of famp-inspect-server; BusMessage::Inspect returns an Out::InspectRequest sentinel."
  - "Executor-side mailbox unread counts use broker cursor offsets with famp_inbox::read::read_from, while totals and last-message metadata use read_all."
  - "D-06 honored by omitting tasks/messages CLI subcommands entirely in Phase 1."

patterns-established:
  - "Broker::cursor_offset is a read-only accessor for executor diagnostics without exposing mutable broker state."
  - "Wave 2 inspect CLI modules accept --json but keep rendering bodies as explicit Wave 3 hook points."

requirements-completed: [INSP-RPC-01, INSP-CLI-01, INSP-CRATE-03, INSP-RPC-02]

duration: 15min
completed: 2026-05-10
---

# Phase 1 Plan 01-03: Broker Dispatch and Inspect CLI Scaffolding Summary

**Inspect RPC now leaves the pure broker actor as a sentinel, is dispatched by the broker executor with mailbox metadata, and exposes Phase 1 `famp inspect` help for broker and identities only**

## Performance

- **Duration:** 15 min
- **Started:** 2026-05-10T16:50:57Z
- **Completed:** 2026-05-10T17:05:47Z
- **Tasks:** 2
- **Files modified:** 14

## Accomplishments

- Replaced the temporary `BusMessage::Inspect` internal-error arm with `Out::InspectRequest { client, kind }`, keeping `famp-bus` tokio-free and free of any `famp-inspect-server` dependency.
- Added executor-side inspect handling in `crates/famp/src/cli/broker/mod.rs`: builds `BrokerCtx`, pre-reads mailbox metadata, calls `famp_inspect_server::dispatch`, and sends `BusReply::InspectOk`.
- Added `famp inspect broker` and `famp inspect identities` clap scaffolding with `--json` flags and smoke tests; `tasks` and `messages` remain absent per D-06.

## Task Commits

1. **Task 1: Wire inspect dispatch sentinel and executor BrokerCtx assembly** - `1eb02ba` (feat)
2. **Task 2: Wire inspect CLI scaffolding and help tests** - `0f858f4` (feat)

**Plan metadata:** pending final summary commit

## Files Created/Modified

- `crates/famp-bus/src/broker/mod.rs` - Added `Out::InspectRequest` and read-only `Broker::cursor_offset`.
- `crates/famp-bus/src/broker/handle.rs` - `BusMessage::Inspect` now emits the sentinel after the existing pre-Hello gate.
- `crates/famp/src/cli/broker/mod.rs` - Executor handles `Out::InspectRequest`, builds `BrokerCtx`, reads mailbox metadata, calls `famp_inspect_server::dispatch`, and replies with `InspectOk`.
- `crates/famp/Cargo.toml` - Added inspector server/proto/client crate dependencies to `famp`; `famp-bus` intentionally has no server dependency.
- `crates/famp/src/cli/inspect/mod.rs` - New `InspectArgs`/subcommand dispatcher with only `broker` and `identities`.
- `crates/famp/src/cli/inspect/broker.rs` - Wave 2 broker inspect stub and `--json` arg; Wave 3 fills probe/rendering.
- `crates/famp/src/cli/inspect/identities.rs` - Wave 2 identities inspect stub and `--json` arg; Wave 3 fills RPC/rendering.
- `crates/famp/src/cli/mod.rs` - Top-level `Commands::Inspect` wiring.
- `crates/famp/tests/inspect_broker.rs` - Help smoke tests for `broker`, `identities`, and D-06 absence.
- `crates/famp/tests/inspect_identities.rs` - Help smoke test for identities `--json`.
- `crates/famp/src/bin/famp.rs`, `crates/famp/src/lib.rs`, `crates/famp/examples/*.rs` - Added unused-dependency silencing for the new inspector crates.

## Mailbox Metadata Helper

The helper lives at `crates/famp/src/cli/broker/mod.rs::read_mailbox_meta_for`.

Cursor semantics: the executor reads total and last-message metadata with `famp_inbox::read::read_all(&path)`. Unread count uses the actor-owned cursor from `Broker::cursor_offset(&MailboxName::Agent(name))` and counts `famp_inbox::read::read_from(&path, cursor_offset).len()`. Missing or unreadable mailbox files return `MailboxMeta::default()`.

## Decisions Made

- Preserved the dependency boundary explicitly called out in the plan correction: `famp-bus` depends only on `famp-inspect-proto`; executor crate `famp` depends on `famp-inspect-server`.
- Added a narrow read-only cursor accessor instead of exposing broker internals or moving disk I/O into the actor.
- Left CLI rendering bodies as Wave 3 hook points in `inspect/broker.rs` and `inspect/identities.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Silenced new inspector deps in example/test targets**
- **Found during:** Task 2 verification
- **Issue:** Adding inspector crates to `famp` made example and integration-test targets emit `unused_crate_dependencies` warnings for the new dependencies.
- **Fix:** Added the existing local silencing pattern to `crates/famp/src/bin/famp.rs`, `crates/famp/src/lib.rs`, both examples, and the new inspect test targets.
- **Files modified:** `crates/famp/src/bin/famp.rs`, `crates/famp/src/lib.rs`, `crates/famp/examples/personal_two_agents.rs`, `crates/famp/examples/cross_machine_two_agents.rs`, `crates/famp/tests/inspect_broker.rs`, `crates/famp/tests/inspect_identities.rs`
- **Verification:** `cargo build --workspace --all-targets`; `cargo nextest run --workspace`
- **Committed in:** `1eb02ba`, `0f858f4`

---

**Total deviations:** 1 auto-fixed (1 blocking/verification hygiene)
**Impact on plan:** No architecture change. The dependency boundary and CLI surface are unchanged.

## Known Stubs

- `crates/famp/src/cli/inspect/broker.rs` - `run()` prints a Wave 3 placeholder; Wave 3 must call `famp_inspect_client::raw_connect_probe`, render HEALTHY/down states, implement `--json`, and set exit codes.
- `crates/famp/src/cli/inspect/identities.rs` - `run()` prints a Wave 3 placeholder; Wave 3 must probe/connect, call `InspectKind::Identities`, render table/JSON, and fail fast when the broker is down.

## Issues Encountered

- First `cargo nextest run --workspace` attempt hung during nextest test discovery (`--list --format terse` child binaries). I stopped that hung verification process and reran the workspace gate cleanly; the rerun passed.
- Pre-existing untracked `.claude/scheduled_tasks.lock` and `.claude/settings.json` remain unmodified and uncommitted.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo build -p famp-bus` - passed
- `cargo nextest run -p famp-bus` - passed, 59 tests
- `cargo build -p famp` - passed
- `cargo nextest run -p famp --test inspect_broker` - passed, 2 tests
- `cargo nextest run -p famp --test inspect_identities` - passed, 1 test
- `cargo nextest run --workspace` - passed, 590 tests, 2 skipped
- `cargo build --workspace --all-targets` - passed
- `just check-no-tokio-in-bus` - passed
- `cargo run -p famp -- inspect --help` - passed; lists `broker` and `identities`
- `cargo run -p famp -- inspect broker --help` - passed; lists `--json`
- `cargo run -p famp -- inspect identities --help` - passed; lists `--json`
- `cargo run -p famp -- inspect tasks --help` - failed as expected with `unrecognized subcommand 'tasks'`

## Next Phase Readiness

Wave 3 can replace the two inspect CLI stub bodies with real `famp-inspect-client` calls and renderers. The broker-side RPC path is mounted and returns `InspectOk` payloads from `famp_inspect_server::dispatch`.

## Self-Check: PASSED

- Created-file checks passed for `crates/famp/src/cli/inspect/mod.rs`, `crates/famp/src/cli/inspect/broker.rs`, `crates/famp/src/cli/inspect/identities.rs`, and this summary.
- Commit checks passed for `1eb02ba` and `0f858f4`.
- No tracked file deletions were present in either task commit.

---
*Phase: 01-broker-diagnosis-identity-inspection*
*Completed: 2026-05-10*
