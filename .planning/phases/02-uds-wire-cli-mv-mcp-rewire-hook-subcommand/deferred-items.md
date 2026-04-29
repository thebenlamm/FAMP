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

## From plan 02-12 execution (2026-04-29)

### Pre-existing MCP malformed-input timeouts (3 tests)
**Discovered during:** plan 02-12 final test sweep (`cargo nextest run -p famp`).
**Tests:**
- `famp::mcp_pre_registration_gating::messaging_tools_refuse_before_register`
- `famp::mcp_malformed_input::famp_inbox_list_rejects_non_bool_include_terminal`
- `famp::mcp_malformed_input::mcp_famp_send_rejects_non_bool_more_coming`

All three FAIL on the wave-6 merge base (`67fe1b2`) BEFORE any 02-12 changes are
applied — verified via `git stash && cargo nextest run`. They time out waiting
for the MCP stdio reply, suggesting the MCP server's malformed-input + pre-
registration error paths regressed during the wave 5 MCP rewrite (plan 02-09).
Out of scope for plan 02-12 (CLI integration tests + CARRY-02 + D-12 split);
should be triaged by the plan 02-09 owner or rolled into the eventual 02-13
MCP-bus E2E plan.

### Phase-1 D-09 typed-decoder vs Phase-2 minimal-envelope mismatch (FIXED in 02-12)
**Discovered during:** plan 02-12 task 1 first run of `test_dm_roundtrip`.
**Symptom:** `famp inbox list --as bob` after `famp send --as alice --to bob
--new-task hi` returned `bus error: EnvelopeInvalid: drain line rejected by
AnyBusEnvelope::decode: missing required envelope field: class`.
**Root cause:** Commit `9ca6e13` (Phase 1 atomic v0.5.1→v0.5.2 bump) added a
typed-decoder to the broker's drain path (`fn decode_lines` in
`famp-bus/src/broker/handle.rs` calling `AnyBusEnvelope::decode`) requiring
every drained line to have a valid `class` field per `AnyBusEnvelope` dispatch.
Plan 02-04 (`famp send` UDS rewire) shipped a minimal mode-tagged envelope
shape (`{"mode":"new_task","summary":"hi"}`) with no `class` field — the
two contracts diverged at wave merge.
**Fix (Rule 1):** `crates/famp/src/cli/send/mod.rs::build_envelope_value` now
wraps the existing mode-tagged payload in an unsigned `audit_log` `BusEnvelope`
shape with `body.details = <inner payload>`. The `audit_log` class is chosen
because BUS-11 forbids signatures on the bus path, `AuditLogBody`'s only
required field is `event` (a fixed sentinel like `"famp.send.new_task"`), and
audit_log is fire-and-forget (no FSM-firing on receipt — preserves the v0.8
send semantics on the local bus). The mode-tagged payload (mode, summary,
task, body, terminal, more_coming) lives verbatim under `body.details`, so
downstream readers continue to read those fields by name (just one level
deeper). Phase 4 federation will replace this with full signed envelope
construction.
**Tests:** `cli_dm_roundtrip::test_dm_roundtrip` + `cli_channel_fanout::
test_channel_fanout` GREEN; `build_envelope_value_decodes_as_audit_log` unit
test locks the round-trip through `AnyBusEnvelope::decode`.

### `famp inbox list` does NOT read channel mailboxes
**Discovered during:** plan 02-12 task 2 design review (BEFORE writing test).
**Symptom:** Plan 02-12 prescribes `famp inbox list --as bob` reading channel
posts after alice sends `--channel #planning`; broker `fn inbox` only drains
`MailboxName::Agent(name)` and never `MailboxName::Channel(name)`.
**Decision (Rule 1 deviation):** `cli_channel_fanout::test_channel_fanout`
asserts fan-out via PARKED `famp await --as <name>`s on bob and charlie
instead of via `famp inbox list`. The broker's `send_channel` iterates
channel members and unparks each member's `pending_awaits` entry with the
envelope (`waiting_client_for_name`); this is the in-broker fan-out signal
and faithfully tests TEST-02 + CLI-06 against the implementation.
**Future work:** A separate plan should decide whether `inbox list` should
also drain a member's per-channel mailbox slice (or a unified inbox view
that merges agent + joined channels). Tracked here for the v0.9 retro;
NOT a blocker for Phase 2 closure.
