---
phase: 04-broker-lifecycle-bootstrap-diagnostics
plan: 02
subsystem: cli
tags: [broker, spawn, sandbox, eperm, mcp, diagnostics]

requires: []
provides:
  - `SpawnError::SandboxEperm` for parent-side sandbox bind failures
  - parent-side UDS bind probe before broker fork/spawn
  - CLI and MCP cause-plus-remedy messages for sandboxed broker creation
affects: [phase-05-daemon-service-version, mcp-session, broker-bootstrap]

tech-stack:
  added:
    - libc
  patterns:
    - parent-side pre-flight probe before process spawn
    - fixed-literal sandbox remediation message with no path interpolation
    - stage-aware spawn error formatting shared by CLI and MCP surfaces

key-files:
  created: []
  modified:
    - Cargo.lock
    - crates/famp/Cargo.toml
    - crates/famp/src/bus_client/spawn.rs
    - crates/famp/src/cli/register.rs
    - crates/famp/src/cli/mcp/session.rs

key-decisions:
  - "Use a parent-side temporary UDS bind probe under the bus directory to detect EPERM/EACCES before forking."
  - "Surface sandbox bind failures as fixed literal text: `can't create a broker inside a sandbox; run `famp daemon install` from a normal shell`."
  - "Keep non-EPERM `SpawnError::Io` messages generic while preserving the prior EPERM fork/setsid hint path."

patterns-established:
  - "Bootstrap diagnostics distinguish connect-stage errors, generic spawn I/O, spawn timeout, and sandbox bind refusal."

requirements-completed: [BOOT-01]

duration: 4 min
completed: 2026-06-04
---

# Phase 04 Plan 02: Sandbox EPERM Diagnostics Summary

**Parent-side sandbox bind detection with actionable CLI and MCP remediation text**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-04T14:03:29Z
- **Completed:** 2026-06-04T14:07:00Z
- **Tasks:** 4
- **Files modified:** 5

## Accomplishments

- Added `SpawnError::SandboxEperm` with fixed cause-plus-remedy text naming sandbox and `famp daemon install`.
- Added a parent-side temporary UDS bind probe before the fork/spawn path, returning `SandboxEperm` on EPERM/EACCES.
- Wired the new error through both `famp register` and MCP `bus_err_detail` surfaces.
- Added tests proving SandboxEperm includes the remedy and non-EPERM spawn I/O does not claim sandbox.
- Preserved the existing `returns_ok_when_socket_already_accepting` fast path.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add RED diagnostics tests** - `84fb452` (test)
2. **Task 2: Add SandboxEperm and parent-side probe** - `da83252` (feat)
3. **Task 3: Verify CLI SandboxEperm surface** - `3cd6a20` (test, empty verification commit)
4. **Task 4: Verify MCP SandboxEperm surface and full lib suite** - `560dce3` (test, empty verification commit)

**Plan metadata:** pending in docs commit

## Files Created/Modified

- `crates/famp/src/bus_client/spawn.rs` - Added `SandboxEperm`, parent-side bind probe, and display regression test.
- `crates/famp/src/cli/register.rs` - Added SandboxEperm mapping and non-EPERM spawn I/O regression coverage.
- `crates/famp/src/cli/mcp/session.rs` - Added MCP SandboxEperm mapping and non-EPERM spawn I/O regression coverage.
- `crates/famp/Cargo.toml` - Added direct `libc` dependency for errno constants.
- `Cargo.lock` - Reflected the direct dependency edge.

## Decisions Made

- The sandbox bind error message remains a fixed literal and does not interpolate `bus_dir`, probe path, or raw I/O payload.
- Non-EPERM spawn I/O messages stay generic. EPERM/EACCES fork/setsid errors retain the older sandbox hint, while bind-probe sandbox refusal uses the new explicit `SandboxEperm` path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Rust enum exhaustiveness required CLI/MCP arms during Task 2**
- **Found during:** Task 2 (add `SandboxEperm` variant)
- **Issue:** Adding an enum variant made the existing CLI and MCP `match` expressions non-exhaustive, so the spawn-focused Task 2 acceptance tests could not compile until those surfaces handled the variant.
- **Fix:** Added the CLI and MCP `SandboxEperm` arms with the enum implementation, then used Tasks 3 and 4 as explicit surface verification commits.
- **Files modified:** `crates/famp/src/cli/register.rs`, `crates/famp/src/cli/mcp/session.rs`
- **Verification:** CLI/MCP targeted tests and `cargo test --lib -p famp` passed.
- **Committed in:** `da83252`

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** No behavioral scope change. The planned CLI/MCP arms landed earlier than their verification tasks because Rust required exhaustive matches.

## Issues Encountered

None beyond the compile-order deviation documented above.

## Verification

- `rg -n "map_bus_client_err_sandbox_eperm_contains_remedy|map_bus_client_err_non_eperm_spawn_io_does_not_claim_sandbox|bus_err_detail_sandbox_eperm_contains_remedy|bus_err_detail_non_eperm_spawn_io_does_not_claim_sandbox|sandbox_eperm_display_names_cause_and_remedy" crates/famp/src` - all five tests present.
- `cargo test --lib -p famp sandbox_eperm_display_names_cause_and_remedy` - passed.
- `cargo test --lib -p famp returns_ok_when_socket_already_accepting` - passed.
- `rg -n "let _ =" crates/famp/src/bus_client/spawn.rs` - no matches.
- `cargo test --lib -p famp map_bus_client_err_sandbox_eperm_contains_remedy` - passed.
- `cargo test --lib -p famp map_bus_client_err_non_eperm_spawn_io_does_not_claim_sandbox` - passed.
- `cargo test --lib -p famp bus_err_detail_sandbox_eperm_contains_remedy` - passed.
- `cargo test --lib -p famp bus_err_detail_non_eperm_spawn_io_does_not_claim_sandbox` - passed.
- `cargo test --lib -p famp` - passed, 157 tests.
- `cargo fmt --check -p famp` - passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

04-03 can deploy the freshly built binary so the MCP process surface used by agent sessions reflects the new SandboxEperm diagnostics.

---
*Phase: 04-broker-lifecycle-bootstrap-diagnostics*
*Completed: 2026-06-04*
