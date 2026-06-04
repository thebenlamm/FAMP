---
phase: 04-broker-lifecycle-bootstrap-diagnostics
status: passed
score: 3/3 must-haves verified
verified: 2026-06-04T14:20:00Z
requirements_checked: [BLC-01, BLC-02, BOOT-01]
overrides_applied: 0
human_verification: []
---

# Phase 04 Verification: Broker Lifecycle & Bootstrap Diagnostics

**Phase Goal (ROADMAP.md):** Users can run a long-lived broker that never self-terminates on idle, and sandboxed clients receive an actionable error explaining the constraint and remedy rather than a generic broker-unreachable failure.

**Verified:** 2026-06-04
**Status:** passed

## Verdict

All three roadmap success criteria are backed by codebase and deployed-binary evidence:

1. `famp broker --no-idle-exit` exists, is visible in help, and keeps a no-client broker alive past the default idle window.
2. Default broker idle-exit behavior remains intact; existing BROKER-04 and BROKER-04b tests pass.
3. Sandbox bind EPERM/EACCES is surfaced as `SpawnError::SandboxEperm` with fixed cause-plus-remedy text, and both CLI and MCP surfaces expose that message while non-EPERM spawn I/O does not claim sandbox.

## Automated Checks

| Check | Status | Evidence |
|-------|--------|----------|
| Full library suite | PASS | `cargo test --lib -p famp` passed outside the sandbox: 157/157 tests. |
| Broker lifecycle suite | PASS | `cargo test --test broker_lifecycle -p famp` passed outside the sandbox: 6/6 tests. |
| Broker spawn race suite | PASS | `cargo test --test broker_spawn_race -p famp` passed outside the sandbox: 1/1 test. |
| Deployed help output | PASS | `famp broker --help | grep -q no-idle-exit` passed with PATH resolving to `/Users/benlamm/.cargo/bin/famp`. |
| Installed binary freshness | PASS | `just install` replaced `/Users/benlamm/.cargo/bin/famp`; mtime `Jun 4 10:12:39 2026`, newer than the deployment gate start. |
| Code review | PASS | `04-REVIEW.md` status `clean`, 0 findings. |

The first full-suite attempt inside the Codex sandbox failed on Unix socket `bind()` with EPERM. The same command passed outside the sandbox, which is the correct environment for UDS bind tests.

## Must-Have Verification

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A broker started with `--no-idle-exit` survives past `IDLE_TIMEOUT`. | VERIFIED | `crates/famp/tests/broker_lifecycle.rs::test_broker_no_idle_exit_stays_alive` calls `run_on_listener_with_opts(..., true)`, advances virtual time by 301s, and asserts the broker handle is not finished and the socket still exists. Full broker lifecycle suite passed. |
| 2 | A broker started without the flag still self-terminates after `IDLE_TIMEOUT`; existing BROKER-04/04b tests pass. | VERIFIED | Existing `test_broker_idle_exit` and `test_broker_idle_exit_with_no_clients_ever_connected` both passed. The original 4-arg `run_on_listener` wrapper still delegates to opts with `false`. |
| 3 | EPERM/EACCES bind failure is actionable and distinguished from non-EPERM spawn failures on CLI and MCP surfaces. | VERIFIED | `SpawnError::SandboxEperm` has fixed text containing `sandbox` and `famp daemon install`; `map_bus_client_err_sandbox_eperm_contains_remedy`, `bus_err_detail_sandbox_eperm_contains_remedy`, and both non-EPERM negative tests passed in the full library suite. |

**Score:** 3/3 truths verified.

## Required Artifacts

| Artifact | Expected | Exists | Substantive | Wired | Status |
|----------|----------|--------|-------------|-------|--------|
| `crates/famp/src/cli/broker/mod.rs` | `BrokerArgs.no_idle_exit`, production flag threading, `run_on_listener_with_opts`, preserved wrapper | YES | YES | YES | VERIFIED |
| `crates/famp/tests/broker_lifecycle.rs` | `test_broker_no_idle_exit_stays_alive` | YES | YES | YES | VERIFIED |
| `crates/famp/src/bus_client/spawn.rs` | `SpawnError::SandboxEperm`, parent-side bind probe, no swallowed `let _ =` on bind/probe path | YES | YES | YES | VERIFIED |
| `crates/famp/src/cli/register.rs` | CLI `SandboxEperm` mapping and non-EPERM negative test | YES | YES | YES | VERIFIED |
| `crates/famp/src/cli/mcp/session.rs` | MCP `SandboxEperm` mapping and non-EPERM negative test | YES | YES | YES | VERIFIED |
| `/Users/benlamm/.cargo/bin/famp` | Deployed binary refreshed by `just install` | YES | YES | YES | VERIFIED |

## Key Link Verification

| From | To | Via | Status | Detail |
|------|----|-----|--------|--------|
| `BrokerArgs.no_idle_exit` | broker run loop | `run(args)` passes `no_idle_exit` into `run_on_listener_with_opts` | WIRED | `crates/famp/src/cli/broker/mod.rs` lines 84, 89, 121-126. |
| `run_on_listener` wrapper | default idle-exit behavior | wrapper calls `run_on_listener_with_opts(..., false)` | WIRED | Existing tests use the wrapper and still pass. |
| `run_on_listener_with_opts` | idle timer arming | startup `idle` is `None` when `no_idle_exit`; disconnect re-arms only when `!no_idle_exit` | WIRED | `wait_or_never(&mut idle)` arm remains unchanged. |
| parent-side bind probe | `SpawnError::SandboxEperm` | EPERM/EACCES from temporary UDS bind returns the new variant before fork/spawn | WIRED | `preflight_bind_probe(bus_dir)?` runs after `create_dir_all` and before log/exe/spawn setup. |
| `SpawnError::SandboxEperm` | CLI/MCP user-facing text | explicit match arms in `register.rs` and `session.rs` call the fixed Display text | WIRED | Targeted CLI/MCP tests passed. |

## Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| BLC-01 | SATISFIED | New flag, opts runner, help check, and `test_broker_no_idle_exit_stays_alive` pass. |
| BLC-02 | SATISFIED | Existing idle-exit tests pass through the original wrapper with default behavior. |
| BOOT-01 | SATISFIED | `SandboxEperm` plus CLI/MCP remedy tests and non-EPERM negative tests pass; deployed binary refreshed via `just install`. |

No orphaned Phase 04 requirements: `REQUIREMENTS.md` marks BLC-01, BLC-02, and BOOT-01 complete.

## Issues Encountered

- Sandboxed full-suite run failed with EPERM on Unix socket bind. Rerun outside the sandbox passed and is the accepted verification evidence.

## Human Verification Required

None. All Phase 04 success criteria are mechanically verifiable with automated tests and deployed CLI checks.

## Result

`status: passed`

Phase 04 is ready to be marked complete in ROADMAP.md. Phase 05 can proceed with daemon service management and version safety.

---
*Verified: 2026-06-04*
*Verifier: Codex (inline gsd-verifier-equivalent pass)*
