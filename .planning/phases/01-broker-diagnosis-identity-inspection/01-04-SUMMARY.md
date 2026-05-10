---
phase: 01-broker-diagnosis-identity-inspection
plan: 01-04
subsystem: inspect-cli
tags: [rust, cli, inspector, ci, diagnostics]

requires:
  - "01-01"
  - "01-02"
  - "01-03"
provides:
  - "famp inspect broker HEALTHY/down-state human and JSON rendering"
  - "famp inspect identities table/JSON rendering and dead-broker fast-fail"
  - "Inspector invariant Justfile gates wired into ci"
affects: [famp, famp-inspect-client, famp-inspect-server, famp-inspect-proto, justfile]

tech-stack:
  added: []
  patterns:
    - "Inspect CLI uses raw_connect_probe before RPC so dead-broker diagnosis never auto-spawns the broker"
    - "Invariant gates are cheap cargo-tree/source-grep checks wired into local CI"

key-files:
  created: []
  modified:
    - crates/famp/src/cli/error.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/cli/inspect/broker.rs
    - crates/famp/src/cli/inspect/identities.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/tests/inspect_broker.rs
    - crates/famp/tests/inspect_identities.rs
    - Justfile

key-decisions:
  - "Broker down-state diagnosis prints to stdout and exits 1, while identities dead-broker fast-fail prints the required stderr line with empty stdout."
  - "Tasks/messages inspect subcommands remain absent in Phase 1."
  - "check-inspect-readonly uses dependency and source-shape checks rather than trying to prove full semantic immutability."

patterns-established:
  - "Broker render states map ProbeOutcome to stable human and JSON output shapes."
  - "Identity rows are rendered with fixed headers and width calculation over the returned row set."

requirements-completed: [INSP-BROKER-01, INSP-BROKER-02, INSP-BROKER-03, INSP-BROKER-04, INSP-IDENT-01, INSP-IDENT-02, INSP-IDENT-03, INSP-CLI-01, INSP-CLI-02, INSP-CLI-03, INSP-CLI-04, INSP-CRATE-01, INSP-CRATE-02, INSP-CRATE-03, INSP-RPC-02]

duration: recovered
completed: 2026-05-10
---

# Phase 1 Plan 01-04: Inspect CLI Rendering and Gates Summary

**Final user-visible inspector rendering, integration coverage, and CI invariant gates for Phase 1**

## Performance

- **Tasks:** 3
- **Files modified:** 18 including formatting cleanup
- **Summary recovery:** The executor completed the first two tasks, then stalled after editing the final Justfile gate task. The orchestrator validated and committed the final gate task, formatted the wave, and recovered this summary.

## Accomplishments

- Implemented `famp inspect broker` rendering for HEALTHY, DOWN_CLEAN, STALE_SOCKET, ORPHAN_HOLDER, and PERMISSION_DENIED states, including `--json`.
- Implemented `famp inspect identities` live-broker table rendering, JSON output, and dead-broker fast-fail behavior.
- Expanded integration tests for broker and identities inspect behavior.
- Added `just check-no-io-in-inspect-proto`, `just check-inspect-readonly`, and `just check-inspect-version-aligned`, then wired all three into `just ci`.

## Task Commits

1. **Task 1: Implement inspect broker rendering and tests** - `3ddf3d0` (feat)
2. **Task 2: Implement inspect identities rendering and tests** - `a22efc8` (feat)
3. **Task 3: Add inspect invariant Justfile gates** - `3e71c46` (ci)
4. **Formatting cleanup after final wave** - `b2a8621` (style)

**Plan metadata:** recovered by orchestrator in this summary commit.

## Files Created/Modified

- `crates/famp/src/cli/inspect/broker.rs` - Full broker inspect state rendering and JSON output.
- `crates/famp/src/cli/inspect/identities.rs` - Identity table/JSON rendering and dead-broker fast-fail.
- `crates/famp/tests/inspect_broker.rs` - Broker inspect integration coverage.
- `crates/famp/tests/inspect_identities.rs` - Identity inspect integration coverage.
- `Justfile` - Inspector invariant gates and `ci` wiring.
- `crates/famp/src/cli/error.rs`, `crates/famp/src/bin/famp.rs`, `crates/famp/src/cli/mcp/error_kind.rs` - CLI/error support updates for inspect behavior.

## Decisions Made

- Kept dead-broker identities behavior intentionally different from broker diagnosis: identities emits the required stderr error and empty stdout.
- Used cargo-tree checks for dependency invariants and source-grep checks for read-only inspector-server guardrails.
- Preserved Phase 1 CLI scope by not adding `tasks` or `messages` inspect subcommands.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Recovered final gate task after stale executor channel**
- **Found during:** Task 3
- **Issue:** The executor edited `Justfile` but did not commit the task or produce a summary before the agent channel stopped making progress.
- **Fix:** Validated all three new Just recipes, committed the Justfile task, ran formatting, and wrote this summary.
- **Files modified:** `Justfile`, formatting-only Rust files
- **Verification:** `just check-no-io-in-inspect-proto`; `just check-inspect-readonly`; `just check-inspect-version-aligned`; `cargo fmt --all -- --check`
- **Committed in:** `3e71c46`, `b2a8621`

---

**Total deviations:** 1 auto-fixed
**Impact on plan:** No requirement or architecture change.

## Known Stubs

None for Phase 1 broker/identities inspect surface. Tasks/messages inspect remain intentionally absent until Phase 2.

## Issues Encountered

- Codex sandbox blocks local Unix domain socket binding, so the full workspace test gate must run outside the sandbox for this repo.
- Pre-existing untracked `.claude/scheduled_tasks.lock` and `.claude/settings.json` remain unmodified and uncommitted.

## User Setup Required

None.

## Verification

- `just check-no-io-in-inspect-proto` - passed
- `just check-inspect-readonly` - passed
- `just check-inspect-version-aligned` - passed
- `cargo fmt --all -- --check` - passed
- Prior executor-reported checks passed for broker/identities inspect tests, workspace build, no-tokio gate, and workspace nextest.

## Next Phase Readiness

All Phase 1 implementation plans now have summaries. Phase-level verification can check requirement traceability, CLI behavior, and invariant gates.

## Self-Check: PASSED

- Commit checks passed for `3ddf3d0`, `a22efc8`, `3e71c46`, and `b2a8621`.
- Justfile gate commands passed after recovery.
- No tracked file deletions were present in the final wave commits.

---
*Phase: 01-broker-diagnosis-identity-inspection*
*Completed: 2026-05-10*
