---
phase: 02-minimal-task-lifecycle
plan: 03
subsystem: testing
tags: [rust, famp-fsm, state-machine, FSM, proptest, exhaustive-match, compile-time-gate]

# Dependency graph
requires:
  - phase: 02-minimal-task-lifecycle/02-02
    provides: 5-state TaskFsm engine with step() and __with_state_for_testing constructor
  - phase: 02-minimal-task-lifecycle/02-01
    provides: MessageClass and TerminalStatus in famp-core
provides:
  - FSM-03 compile-time exhaustiveness gate: consumer_stub.rs under #![deny(unreachable_patterns)]
  - FSM-08 proptest Cartesian product legality matrix: 5×5×4=100 tuples, 2048 cases, zero panics
  - All 5 Phase 2 FSM requirements satisfied (FSM-02, FSM-03, FSM-04, FSM-05, FSM-08)
affects: [famp-transport, famp-fsm-consumers, 03-transport-binding]

# Tech tracking
tech-stack:
  added:
    - proptest (workspace, dev-dep) added to famp-fsm
  patterns:
    - "#[cfg(test)] use proptest as _; in lib.rs silences unused_crate_dependencies on lib target (mirrors famp-envelope pattern)"
    - "Oracle const fn expected_next mirrors engine.rs transition table — independent legality authority for proptest"
    - "Or-pattern in proptest oracle: (Requested | Committed, Control, None) matches engine.rs nested or-pattern exactly"
    - "#![allow(clippy::match_same_arms)] on consumer stub preserves intentionally separate arms for FSM-03 exhaustiveness documentation"

key-files:
  created:
    - crates/famp-fsm/tests/consumer_stub.rs
    - crates/famp-fsm/tests/proptest_matrix.rs
  modified:
    - crates/famp-fsm/Cargo.toml
    - crates/famp-fsm/src/lib.rs

key-decisions:
  - "match_same_arms allowed in consumer_stub.rs: intentionally separate arms (Requested=>false, Committed=>false) document each variant explicitly — this is the point of the FSM-03 gate"
  - "expected_next oracle is const fn: all arms operate on Copy enums, mirrors engine.rs decision from Plan 02-02"
  - "proptest oracle uses or-pattern (Requested | Committed, Control, None): matches engine.rs transition table exactly, single source of truth for cancel behavior"

patterns-established:
  - "FSM compile-time gate: standalone integration test under #![deny(unreachable_patterns)] + zero catch-all arms; adding or removing a variant is a hard compile error"
  - "Proptest legality oracle: const fn expected_next that independently re-expresses the transition table; test asserts engine agrees with oracle for every sampled tuple"
  - "Proptest strategy composition for exhaustive tuple enumeration: prop_oneof![Just(...), ...] for small finite domains"

requirements-completed: [FSM-03, FSM-08]

# Metrics
duration: 4min
completed: 2026-04-13
---

# Phase 2 Plan 03: FSM-03 compile-time exhaustiveness gate and FSM-08 proptest 100-tuple legality matrix

**Consumer stub under `#![deny(unreachable_patterns)]` proves variant-change safety at compile time; proptest matrix runs 2048 cases over the full 5×5×4 Cartesian product with an independent oracle, zero panics, and exact error-field assertions**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-04-13T19:00:24Z
- **Completed:** 2026-04-13T19:04:02Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- FSM-03 compile-time gate: `crates/famp-fsm/tests/consumer_stub.rs` exhaustively matches all 5 `TaskState` variants with zero catch-all arms under `#![deny(unreachable_patterns)]`; sanity-check confirmed gate fires with `error[E0599]` when a bogus variant arm is added
- FSM-08 proptest matrix: `crates/famp-fsm/tests/proptest_matrix.rs` enumerates 5×5×4=100 tuples over 2048 proptest cases; `expected_next` oracle is a `const fn` that independently mirrors the engine's transition table; every legal arrow asserts `Ok(expected_next)` and matching post-step state; every illegal tuple asserts `TaskFsmError::IllegalTransition` with exact (from, class, terminal_status) fields; state never mutates on illegal transition
- All 5 Phase 2 FSM requirements now satisfied: FSM-02 (5-state enum), FSM-03 (consumer stub gate), FSM-04 (single transition function), FSM-05 (owned types), FSM-08 (proptest matrix)
- 200/200 workspace tests green; `cargo clippy --workspace -- -D warnings` clean

## Task Commits

Each task was committed atomically:

1. **Task 1: FSM-03 downstream consumer stub under deny(unreachable_patterns)** - `4460297` (feat)
2. **Task 2: FSM-08 proptest Cartesian product legality matrix** - `507c565` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `crates/famp-fsm/tests/consumer_stub.rs` — FSM-03 integration test: exhaustive match on 5 TaskState variants under `#![deny(unreachable_patterns)]`; describe_every_variant and terminal_classification_is_exhaustive tests
- `crates/famp-fsm/tests/proptest_matrix.rs` — FSM-08 proptest matrix: 3 strategy fns, expected_next oracle const fn, proptest with 2048 cases, plus 2 deterministic helper tests
- `crates/famp-fsm/Cargo.toml` — added `proptest = { workspace = true }` to dev-dependencies
- `crates/famp-fsm/src/lib.rs` — added `#[cfg(test)] use proptest as _;` after module-level doc comments to silence unused_crate_dependencies on the lib target

## Decisions Made

- **match_same_arms allowed in consumer stub:** The FSM-03 gate is specifically about exhaustiveness — each variant must be listed individually, and merging `Requested => false, Committed => false` into an or-pattern defeats the documentation purpose. Added `#![allow(clippy::match_same_arms)]` rather than merging; this is the only case in the codebase where this allow is semantically justified.
- **expected_next oracle as const fn:** Clippy `missing_const_for_fn` (pedantic/nursery) required it; all arms operate on `Copy` enums, so `const fn` is correct and free. Matches the engine.rs `step()` decision from Plan 02-02.
- **Or-pattern in oracle:** `(TaskState::Requested | TaskState::Committed, MessageClass::Control, None)` mirrors the engine.rs arm exactly — single authoritative shape for the cancel transition in both production code and test oracle.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy pedantic violations in consumer_stub.rs and proptest_matrix.rs**
- **Found during:** Task 2 verification (clippy -p famp-fsm --tests -- -D warnings)
- **Issue:** `doc_markdown` (is_terminal not backtick-wrapped), `match_same_arms` (separate false/true arms in is_terminal), `missing_const_for_fn` for expected_next, `match_same_arms` in proptest oracle, doc comment items not wrapped, unused_crate_dependencies for proptest in lib target
- **Fix:** Fixed doc comments (backtick-wrapped `is_terminal`, `Some(next_state)`); added `#![allow(clippy::match_same_arms)]` to consumer_stub.rs; made expected_next `const fn`; merged oracle Control arms to or-pattern; added `#[cfg(test)] use proptest as _;` to lib.rs
- **Files modified:** `crates/famp-fsm/tests/consumer_stub.rs`, `crates/famp-fsm/tests/proptest_matrix.rs`, `crates/famp-fsm/src/lib.rs`
- **Verification:** `cargo clippy -p famp-fsm --tests -- -D warnings` exits 0; `cargo clippy --workspace -- -D warnings` exits 0
- **Committed in:** `507c565` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - clippy violations)
**Impact on plan:** All fixes are correctness and style improvements consistent with the codebase's pedantic clippy stance. No scope creep. The or-pattern in the oracle is actually more correct as it matches the engine.rs arm shape exactly.

## Issues Encountered

- PATH environment requires `$HOME/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin` to be prepended for cargo/rustc to be found. Same issue documented in Plan 02-02 SUMMARY.

## Known Stubs

None — all test files are fully implemented. The proptest matrix covers all 100 tuples. No placeholder behavior.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Phase 2 is complete: all 5 FSM requirements satisfied (FSM-02, FSM-03, FSM-04, FSM-05, FSM-08)
- `famp-fsm` public API is stable and fully tested: 17 tests across 4 binaries, including 12 deterministic fixture tests from Plan 02-02 and 5 new tests from Plan 02-03
- Phase 3 transport binding can consume `TaskFsm::step()` + the ~20-line envelope-to-TaskTransitionInput adapter described in D-D3

---
*Phase: 02-minimal-task-lifecycle*
*Completed: 2026-04-13*
