---
phase: 02-minimal-task-lifecycle
plan: 01
subsystem: api
tags: [rust, famp-core, famp-envelope, MessageClass, TerminalStatus, crate-layering]

# Dependency graph
requires:
  - phase: 01-minimal-signed-envelope
    provides: MessageClass and TerminalStatus type definitions; famp-envelope crate with body types
provides:
  - MessageClass canonically defined in famp-core::class (re-exported from famp-envelope::class)
  - TerminalStatus canonically defined in famp-core::terminal_status (re-exported from famp-envelope::body::deliver)
  - famp-fsm can now import both types from famp-core without depending on famp-envelope
affects: [02-02, 02-03, famp-fsm]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Re-export for backward compatibility: define in lower crate, pub use in former home"
    - "Crate layering: famp-core is the shared types crate; famp-envelope and famp-fsm both depend on it"

key-files:
  created:
    - crates/famp-core/src/class.rs
    - crates/famp-core/src/terminal_status.rs
  modified:
    - crates/famp-core/src/lib.rs
    - crates/famp-envelope/src/class.rs
    - crates/famp-envelope/src/body/deliver.rs

key-decisions:
  - "MessageClass lifted to famp-core (D-D1): famp-fsm depends only on famp-core, not famp-envelope"
  - "TerminalStatus lifted to famp-core (D-B5 + D-D1 resolution): Option A chosen per research"
  - "Backward-compatible re-export pattern: all existing famp-envelope consumers unchanged"

patterns-established:
  - "Shared type lift: copy enum verbatim to famp-core, replace source with pub use famp_core::TypeName"

requirements-completed: []

# Metrics
duration: 8min
completed: 2026-04-13
---

# Phase 2 Plan 01: Type Lift — MessageClass and TerminalStatus to famp-core

**MessageClass and TerminalStatus lifted from famp-envelope to famp-core via backward-compatible re-exports, unblocking famp-fsm from any famp-envelope dependency (D-D1)**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-13T18:41:00Z
- **Completed:** 2026-04-13T18:49:57Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- `MessageClass` (5 variants) now canonically lives in `crates/famp-core/src/class.rs` with an exact-copy definition; `famp-envelope/src/class.rs` reduced to a one-line `pub use famp_core::MessageClass`
- `TerminalStatus` (3 variants: `Completed`, `Failed`, `Cancelled`) now canonically lives in `crates/famp-core/src/terminal_status.rs`; `famp-envelope/src/body/deliver.rs` uses `pub use famp_core::TerminalStatus` in place of the local definition
- Zero behavioral change — 184/184 workspace tests pass, including all 73 famp-envelope tests that exercise both types end-to-end

## Task Commits

Each task was committed atomically:

1. **Task 1: Move MessageClass from famp-envelope to famp-core** - `65c8ac1` (feat)
2. **Task 2: Move TerminalStatus from famp-envelope to famp-core** - `012b807` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/famp-core/src/class.rs` — new: canonical MessageClass enum definition (5 variants, serde snake_case, Display impl)
- `crates/famp-core/src/terminal_status.rs` — new: canonical TerminalStatus enum definition (3 variants, serde snake_case)
- `crates/famp-core/src/lib.rs` — added `pub mod class`, `pub use class::MessageClass`, `pub mod terminal_status`, `pub use terminal_status::TerminalStatus`
- `crates/famp-envelope/src/class.rs` — replaced local enum with `pub use famp_core::MessageClass`
- `crates/famp-envelope/src/body/deliver.rs` — replaced local TerminalStatus enum with `pub use famp_core::TerminalStatus`

## Decisions Made

- **D-B5 + D-D1 resolved:** `TerminalStatus` lifted to `famp-core` (Option A from research). Enum definition copied verbatim including `Cancelled` variant, which is needed for the FSM's terminal state even though it arrives via a `control` message rather than a `deliver`.
- **Backward-compatible re-export:** Both former home modules (`famp-envelope/src/class.rs` and `famp-envelope/src/body/deliver.rs`) keep their public surface intact via `pub use`. All downstream callers using `famp_envelope::MessageClass` or `famp_envelope::body::deliver::TerminalStatus` continue to compile without changes.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. Both moves compiled and passed all tests on first attempt.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `famp-fsm` (Plan 02) can now add `famp-core` as its only dependency and import both `MessageClass` and `TerminalStatus` without touching `famp-envelope`
- The D-D1 crate layering constraint (`famp-core → famp-envelope`, `famp-core → famp-fsm`, no `famp-fsm → famp-envelope`) is now satisfiable
- Workspace is clean: `cargo build --workspace` and `cargo nextest run --workspace` both exit 0

---
*Phase: 02-minimal-task-lifecycle*
*Completed: 2026-04-13*
