# Phase 02 — Deferred Items

Items discovered during execution that are out of scope for the current plan.

## From plan 02-01 execution (2026-04-28)

### Pre-existing fmt violations in Wave-0 stub files
**Discovered during:** plan 02-01 task 1 (`cargo fmt --all -- --check`)
**Files:**
- `crates/famp/tests/broker_lifecycle.rs` (4 single-line stub bodies)
- `crates/famp/tests/cli_dm_roundtrip.rs` (5 single-line stub bodies)
- `crates/famp/tests/hook_subcommand.rs` (3 single-line stub bodies)

The single-line stub bodies (`fn test_x() { unimplemented!(...); }`) need to be expanded to multi-line. These came in via the Wave-0 merge (02-00 plan). Out of scope for plan 02-01 which doesn't touch any test files.

**Recommended fix:** Wave-0 ownership plans (02-10/02-11/02-12) will replace these stubs with full bodies and the fmt issue will resolve itself. Alternatively, a small follow-up commit could `cargo fmt` the stub files in place.

### Pre-existing failing test
**Test:** `famp::listen_bind_collision second_listen_on_same_port_errors_port_in_use`
**Verified pre-existing:** Reproduced on the merge base before any plan 02-01 changes were applied (`git stash` clean state).
**Match:** Aligns with the 8 pre-existing listener/E2E TLS-loopback timeouts noted in `STATE.md` issues section.
