# Phase 02 — Deferred Items

Items discovered during execution that are out of scope for the current plan.

## From plan 02-01 execution (2026-04-28)

### Pre-existing fmt violations in Wave-0 stub files (RESOLVED in 02-01)
**Discovered during:** plan 02-01 task 1 (`cargo fmt --all -- --check`)
**Files:**
- `crates/famp/tests/broker_lifecycle.rs` (4 single-line stub bodies)
- `crates/famp/tests/cli_dm_roundtrip.rs` (5 single-line stub bodies)
- `crates/famp/tests/hook_subcommand.rs` (3 single-line stub bodies)

The single-line stub bodies (`fn test_x() { unimplemented!(...); }`) tripped `cargo fmt --check` in CI. They came in via the Wave-0 merge (02-00 plan). Plan 02-01 task 2 ran `cargo fmt --all` to write back the multi-line form so the new BusClient/identity sources could co-exist on a green `fmt-check` gate. Test bodies are unchanged (still `unimplemented!(...)` under `#[ignore]`); only the brace style was reformatted. Wave-0 stub ownership and `#[ignore]` discipline (un-ignoring is the exclusive right of the owning plan) are unaffected.

### Pre-existing failing test
**Test:** `famp::listen_bind_collision second_listen_on_same_port_errors_port_in_use`
**Verified pre-existing:** Reproduced on the merge base before any plan 02-01 changes were applied (`git stash` clean state).
**Match:** Aligns with the 8 pre-existing listener/E2E TLS-loopback timeouts noted in `STATE.md` issues section.
