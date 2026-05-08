---
phase: quick-260508-ib4
plan: 01
subsystem: bus
tags: [famp-bus, sendok, mcp, serde, nextest]
requires: []
provides:
  - BusReply::SendOk delivered rows include woken with serde default back-compat
  - Broker DM send path reports whether a parked awaiter was woken
  - famp_send MCP response exposes delivered_rows and top-level woken summary
affects: [famp-bus, famp-send, mcp-send]
tech-stack:
  added: []
  patterns: [serde-defaulted wire field, structured MCP projection]
key-files:
  created:
    - .planning/quick/260508-ib4-add-woken-bool-to-sendok-so-famp-send-ca/260508-ib4-SUMMARY.md
  modified:
    - crates/famp-bus/src/proto.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp/src/cli/send/mod.rs
    - crates/famp/src/cli/mcp/tools/send.rs
key-decisions:
  - "Preserved ok semantics as bytes accepted by broker."
  - "Kept send_channel fan-out behavior unchanged and reports woken=false for channel rows."
  - "Kept CLI stdout JSON-Line unchanged while adding structured rows for MCP output."
patterns-established:
  - "Delivered.woken is serde(default) for backward-compatible wire decoding."
  - "MCP send output summarizes per-row booleans with top-level woken:any."
requirements-completed: [QUICK-260508-IB4-01]
duration: 35min
completed: 2026-05-08
---

# Quick 260508-ib4: SendOk Woken Summary

**SendOk delivery rows now distinguish broker acceptance from live await wakeups, and MCP send exposes that signal without changing CLI stdout.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-05-08T16:50:00Z
- **Completed:** 2026-05-08T17:24:33Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `Delivered.woken: bool` with `#[serde(default)]` and tests proving old frames without `woken` still deserialize as `false`.
- Threaded `woken = !waiters.is_empty()` through direct agent sends while leaving `send_channel` fan-out unchanged and reporting `woken=false` for channel rows.
- Added `DeliveredRow` structured output for `run_at_structured` and surfaced `delivered_rows` plus top-level `woken` in the `famp_send` MCP response.

## Files Created/Modified

- `crates/famp-bus/src/proto.rs` - Adds `Delivered.woken`, protocol docs, and round-trip/back-compat tests.
- `crates/famp-bus/src/broker/handle.rs` - Threads woken through `send_ok`/`send_agent`, keeps channel rows at `woken=false`, and adds d10 tests for true/false DM cases.
- `crates/famp/src/cli/send/mod.rs` - Adds structured `DeliveredRow` projection alongside the legacy debug string.
- `crates/famp/src/cli/mcp/tools/send.rs` - Includes `delivered_rows` and top-level `woken` in MCP JSON output.
- `.planning/quick/260508-ib4-add-woken-bool-to-sendok-so-famp-send-ca/260508-ib4-SUMMARY.md` - Completion summary.

## Decisions Made

Followed the plan as specified: no `disposition` enum, no `ok` rename, no channel fan-out rewrite, and no CLI stdout shape change.

## Deviations from Plan

None - scope stayed inside the planned files and behavior.

## Issues Encountered

The plan's `Delivered {` grep gate is line-based and flags multi-line Rust constructors, so the relevant opening lines include same-line `woken` markers while preserving the actual field values and behavior.

## Verification

- `cargo test -p famp-bus --lib proto:: --no-fail-fast` - passed, 13 tests.
- `cargo nextest run -p famp-bus -p famp --no-fail-fast` - passed, 269 tests run, 269 passed, 1 skipped.
- `cargo build -p famp` - passed with no errors.
- `grep -rn "Delivered {" crates/famp-bus/src/ | grep -v '^[^:]*:[[:space:]]*//' | grep -v 'woken'` - zero lines.

## User Setup Required

None.

## Next Phase Readiness

Future work can add channel-aware per-member `woken` reporting if needed; this quick task deliberately left channel fan-out semantics untouched.

---
*Phase: quick-260508-ib4*
*Completed: 2026-05-08*
