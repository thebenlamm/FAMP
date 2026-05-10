---
phase: 01-broker-diagnosis-identity-inspection
plan: 01-02
subsystem: inspector
tags: [rust, inspector, broker-state, uds-client, serde]

requires:
  - "01-01"
provides:
  - "famp-inspect-server crate with read-only BrokerStateView dispatch handlers"
  - "famp-inspect-client crate with raw UDS probe, InspectClient call path, and peer_pid helpers"
  - "BrokerStateView and ClientStateView exported from famp-bus for inspector consumers"
affects: [famp-bus, famp-inspect-server, famp-inspect-client, inspector]

tech-stack:
  added: [famp-inspect-server, famp-inspect-client]
  patterns:
    - "Inspector server handlers consume immutable BrokerStateView snapshots instead of broker internals"
    - "Inspector client diagnosis probes connect directly and never auto-spawn the broker"

key-files:
  created:
    - crates/famp-inspect-server/Cargo.toml
    - crates/famp-inspect-server/src/lib.rs
    - crates/famp-inspect-client/Cargo.toml
    - crates/famp-inspect-client/src/lib.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/famp-bus/src/broker/mod.rs
    - crates/famp-bus/src/broker/state.rs
    - crates/famp-bus/src/lib.rs

key-decisions:
  - "Expose a serializable BrokerStateView from famp-bus so the inspector server can stay read-only across crate boundaries."
  - "Keep famp-inspect-server tokio-free and pass mailbox metadata through BrokerCtx instead of depending on famp-inbox."
  - "Keep famp-inspect-client clap-free and make raw_connect_probe diagnose socket state without broker auto-spawn."

patterns-established:
  - "Broker::state_view() builds the inspector-facing snapshot from private broker state."
  - "PidSource distinguishes peercred, lsof, and unknown provenance for broker PID diagnosis."

requirements-completed: [INSP-CRATE-01, INSP-CRATE-02, INSP-CRATE-03, INSP-RPC-02, INSP-BROKER-01, INSP-BROKER-02, INSP-BROKER-03, INSP-IDENT-01, INSP-IDENT-02]

duration: recovered
completed: 2026-05-10
---

# Phase 1 Plan 01-02: Inspector Server and Client Crates Summary

**Reusable inspector server/client crates and read-only broker state snapshots for the later broker and CLI wiring waves**

## Performance

- **Tasks:** 2
- **Files modified:** 11
- **Summary recovery:** The executor completed and committed task work but did not return a final agent signal or write this summary. The orchestrator recovered the handoff from committed changes and verification output.

## Accomplishments

- Added `famp-inspect-server` with synchronous dispatch handlers over `&BrokerStateView` plus `BrokerCtx` carrying socket path, PID, build version, and pre-read mailbox metadata.
- Added `BrokerStateView`, `ClientStateView`, and `Broker::state_view()` so inspector code can read a serializable snapshot without mutating broker state or exposing private internals.
- Added `famp-inspect-client` with `raw_connect_probe`, `InspectClient::call`, broker-down classification types, `peer_pid`, and `PidSource`.
- Preserved architectural dependency boundaries: server crate has no tokio or famp-inbox dependency; client crate has no clap dependency.

## Task Commits

1. **Task 1: Add BrokerStateView and famp-inspect-server** - `22e5398` (feat)
2. **Task 2: Add famp-inspect-client probe crate** - `3ade44a` (feat)

**Plan metadata:** recovered by orchestrator in this summary commit

## Files Created/Modified

- `crates/famp-inspect-server/src/lib.rs` - Dispatch API, broker/identities/tasks/messages handlers, mailbox metadata context, and handler tests.
- `crates/famp-inspect-server/Cargo.toml` - Tokio-free server crate manifest.
- `crates/famp-inspect-client/src/lib.rs` - Raw UDS probe, inspect call client, broker-down states, peer PID helpers, and tests.
- `crates/famp-inspect-client/Cargo.toml` - Clap-free async client crate manifest.
- `crates/famp-bus/src/broker/state.rs` - Public inspector snapshot types derived from private broker state.
- `crates/famp-bus/src/broker/mod.rs` - Broker snapshot accessor.
- `crates/famp-bus/src/lib.rs` - Inspector view exports.
- `Cargo.toml`, `Cargo.lock` - Workspace membership and dependency lock updates.

## Decisions Made

- Kept read-only inspection enforced by API shape: server dispatch accepts `&BrokerStateView`, not broker internals or mutable state.
- Kept identities mailbox counts as caller-supplied metadata so the server crate stays synchronous and dependency-light.
- Treated peer PID lookup as best-effort diagnostic data; missing sockets return `PidSource::Unknown` rather than failing diagnosis.

## Deviations from Plan

### Auto-fixed Issues

None recorded by the executor.

---

**Total deviations:** 0
**Impact on plan:** None.

## Known Stubs

- `famp-inspect-server` returns not-yet-implemented payloads for tasks/messages, matching the plan.
- `famp-inspect-client::peer_pid` is best-effort and can return `Unknown` when platform or socket conditions do not expose a PID.
- Wave 2 still needs to replace the temporary broker inspect error arm from 01-01 with real server dispatch.

## Issues Encountered

- The executor committed the implementation but did not emit a final completion signal or create `01-02-SUMMARY.md`; the orchestrator closed the stale agent channel and recovered this summary.
- Shared `.planning/STATE.md` and unrelated `.claude/*` working tree changes were left for the orchestrator/user and not included in the plan commits.

## User Setup Required

None.

## Verification

- `cargo build -p famp-inspect-server` - passed
- `cargo build -p famp-inspect-client` - passed
- `cargo nextest run -p famp-inspect-server -p famp-inspect-client` - passed, 8 tests

## Next Phase Readiness

Wave 2 can now wire the running broker to `famp-inspect-server` dispatch and wire the CLI to `famp-inspect-client`.

## Self-Check: PASSED

- Created-file checks passed for both inspector crate manifests and source files.
- Commit checks passed for `22e5398` and `3ade44a`.
- Targeted build/test checks passed.

---
*Phase: 01-broker-diagnosis-identity-inspection*
*Completed: 2026-05-10*
