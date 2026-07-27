# TEST-01 Triage Ledger — `_deferred_v1/` retirement

**Date:** 2026-07-27
**Phase:** 10-test-reactivation-setup-docs (TEST-01)
**Outcome:** 27/27 RETIRE, 0 REACTIVATE.

## Why retirement, not reactivation

Per D-01/D-02 (`.planning/phases/10-test-reactivation-setup-docs/10-CONTEXT.md`),
a parked test is only eligible for reactivation if the behavior it exercises
**still exists on a shipping surface today**. Every one of the 27 files in
this directory depends, directly or via the shared `common::` test harness,
on a v0.8 federation CLI symbol (`famp::cli::init`, `famp::cli::setup`,
`famp::cli::listen::run_on_listener`, `famp::cli::peer::add::run_add_at`, or
the pre-Phase-4 HTTPS-shaped `famp::cli::send::run_at` /
`run_at_structured`) that was **hard-deleted in v0.9 Phase 4**
(`feat!(04): remove federation CLI surface ...`) and is not coming back —
v1.0 replaced that CLI with `famp-gateway` + `famp peer export/import`, not
a resurrected `famp listen`/`init`/`setup`. The `v0.8.1-federation-preserved`
git tag is the deliberate escape hatch for anyone who still needs the old
CLI; this repository's `main` does not.

None of the 27 files compile against `main` today. Of the two files
`10-CONTEXT.md` flagged as candidate-salvage (`send_principal_fallback.rs`,
`conversation_restart_safety.rs`), both were confirmed on inspection to have
no rewrite target — their harness dependencies
(`common::conversation_harness::spawn_listener`,
`famp::cli::peer::add::run_add_at`) have no current-API equivalent to
retarget the assertions at.

12 of the 27 rows carry an `ALREADY-COVERED` disposition: the file's intent
is independently and currently proven by a named, active, currently-green
test elsewhere in the workspace. The remaining 15 carry a plain `RETIRE`
disposition naming the specific deleted symbol the file depended on.

Table transcribed verbatim from
`.planning/phases/10-test-reactivation-setup-docs/10-RESEARCH.md`
§"TEST-01 Triage Table (all 27 files read in full)".

## Triage table

| File | Disposition | Rationale |
|------|-------------|-----------|
| `init_refuses.rs` | RETIRE | `famp::cli::init::run_at` / `CliError::AlreadyInitialized` — `cli::init` module deleted (not in `Commands` enum or `cli/mod.rs`'s module list) |
| `init_home_env.rs` | RETIRE | `famp::cli::init::run` / `InitArgs` — same, `FAMP_HOME`-driven `init` gone |
| `init_identity_incomplete.rs` | RETIRE | `famp::cli::init::load_identity` / `CliError::IdentityIncomplete` — module gone |
| `init_no_leak.rs` | RETIRE | `famp::cli::init::run_at` — module gone |
| `init_force.rs` | RETIRE | `famp::cli::init::run_at(..., force=true)` — module gone |
| `init_happy_path.rs` | RETIRE | `famp::cli::init::run_at` + TLS PEM cross-check against `famp_transport_http` — module gone |
| `listen_truncated_tail.rs` | RETIRE — ALREADY-COVERED | Uses `common::init_home_in_process` (wraps dead `cli::init`), but its actual assertion target — `famp_inbox::read::read_all` tail-tolerance (INBOX-04/05) — is independently and currently proven by `crates/famp-inbox/tests/truncated_tail.rs` (`tail_tolerant_when_file_lacks_trailing_newline`, active, in the live glob) |
| `listen_durability.rs` | RETIRE | Spawns `famp listen` subprocess (`spawn_listen` helper) — `listen` subcommand deleted |
| `info_happy_path.rs` | RETIRE | Depends on `famp::cli::setup::{PeerCard,SetupArgs}` — `setup` module deleted (kills the whole file even though `cli::info` itself still exists) |
| `listen_bind_collision.rs` | RETIRE | Spawns `famp listen` subprocess — deleted subcommand |
| `listen_smoke.rs` | RETIRE | `famp::cli::listen::run_on_listener` in-process — module deleted |
| `send_more_coming_requires_new_task.rs` | RETIRE — ALREADY-COVERED | `famp::cli::send::run_at_structured` (pre-Phase-4 HTTPS-shaped `SendArgs`, taking `home: &Path`) — current `send/mod.rs` has a different `SendArgs` shape (`sock: &Path`, bus-routed) and does not define this function signature at all. File's own header states the semantic check is already covered by unit test `more_coming_without_new_task_errors_in_run_at_structured` in `crates/famp/src/cli/send/mod.rs::tests` (confirmed still present) |
| `listen_shutdown.rs` | RETIRE | Spawns `famp listen` subprocess, sends SIGINT — deleted subcommand |
| `conversation_restart_safety.rs` | RETIRE | Uses `common::conversation_harness::{spawn_listener, stop_listener, ...}`, which wraps `famp::cli::listen::run_on_listener` — deleted module. This was CONTEXT.md's #1 salvage candidate; confirmed on inspection to have no rewrite target — the harness it depends on has no current-API equivalent (no `famp-bus`/`famp-gateway` "restart a listener and resume a task on a new port" surface exists to test against directly; the closest current concept, gateway process restart, is untested territory but a new test, not a rewrite of this one) |
| `peer_import.rs` | RETIRE — ALREADY-COVERED | `famp::cli::peer::import::run_import_at` — old TOML `peers.toml` shape, deleted in Phase 4 and replaced in Phase 8 by Ed25519 TOFU-pin `cli::peer::import` (different function signature, different keyring file). Current shape's export/import/pin round-trip is proven by active `crates/famp/tests/peer_roundtrip.rs` (TRUST-01) |
| `setup_happy_path.rs` | RETIRE | `famp::cli::setup::{run_with_io, SetupArgs, PeerCard}` — module deleted entirely |
| `send_tofu_bootstrap_refused.rs` | RETIRE — ALREADY-COVERED | Old HTTPS-shaped `cli::send::run_at` TOFU refusal test — the equivalent current-surface guarantee (unpinned peer key rejected before any bus/local-mailbox write) is proven by `famp-gateway`'s active `verify::tests::rejects_unpinned_key` and `verify_inbound_any_rejects_unpinned_key_for_every_class` (TRUST-02) |
| `send_new_task_scope_instructions.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `famp::cli::listen::run_on_listener` in-process — both deleted |
| `send_principal_fallback.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `famp::cli::peer::add::run_add_at` (deleted `peer add`). CONTEXT.md's #2 salvage candidate; confirmed no rewrite target — the "silent principal fallback" concern was about the old `config.toml`-resident self-principal; the current bus-routed send path resolves identity through `famp register`'s canonical-holder mechanism, a structurally different code path with no equivalent function to retarget the assertions at |
| `send_new_task.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + in-process listener — deleted |
| `peer_add.rs` | RETIRE — ALREADY-COVERED | `famp::cli::peer::add::run_add_at` (deleted TOML `peer add`) — current peer-trust CRUD is `cli::peer::export`/`import` (Ed25519), proven by `peer_roundtrip.rs` |
| `send_deliver_sequence.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `peer::add` — both deleted |
| `e2e_two_daemons.rs.deferred` | RETIRE — ALREADY-COVERED | Uses `famp_transport_http::{build_router, tls_server, HttpTransport}` directly (not the deleted CLI), but its intent — a full two-daemon signed cross-host `request→commit→deliver→ack` cycle — is exactly what Phase 9's `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` now proves against the current `famp-gateway` architecture (this file predates the gateway and drives `famp-transport-http` bare, without the gateway/broker proxy layer Phase 7-9 built). README's `_deferred_v1/README.md` already flags it as flaky (0/7 passes, oversubscribed-scheduler root cause) — an additional reason not to reactivate as-is even setting aside the coverage overlap |
| `mcp_session_bound_e2e.rs` | RETIRE — ALREADY-COVERED | Uses `common::two_daemon_harness::spawn_two_daemons_under_local_root`, which spawns `famp::cli::listen::run_on_listener` — deleted. Intent (two-MCP-window session-bound identity, full request→commit→deliver→terminal cycle) is proven against the current bus by active `mcp_bus_e2e.rs` (TEST-05) and `mcp_register_whoami.rs` |
| `send_terminal_advance_error_surfaces.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `peer::add`. Also: the specific bug class it sentinel-tests (`persist_post_send`/`advance_terminal`/swallowed-error-on-`tasks.update`) has no equivalent code path in current `send/mod.rs` — grepped for `advance_terminal`/`try_update`/`persist_post_send`, zero hits; the current bus-routed send module doesn't touch `TaskDir` directly at all. Not salvageable as a rewrite; if the underlying lost-update-on-error concern is still live somewhere in the current write path, it needs a new test authored from scratch, out of this phase's retirement-only scope |
| `mcp_stdio_tool_calls.rs` | RETIRE — ALREADY-COVERED | Spawns `famp init` as a subprocess (`Command::new(...).args(["init"])`) — `init` subcommand deleted, so this file cannot even reach its MCP assertions. Its actual assertion surface (8-tool enumeration: `famp_send/await/inbox/peers/register/whoami/join/leave`; error-kind mapping; register handshake) is fully covered today by active `mcp_tool_schema_invariants.rs`, `mcp_error_kind_exhaustive.rs`, `mcp_register_whoami.rs`, and `mcp_malformed_input.rs` |
| `listen_multi_peer_keyring.rs` | RETIRE | `common::conversation_harness::setup_home` + `common::listen_harness::init_home_in_process` + `famp::cli::peer::add::run_add_at` — all bound to the deleted daemon/keyring shape |

**Totals: 27/27 RETIRE. 12 of the 27 carry an `ALREADY-COVERED` pointer to a
named, currently-green test. 0 REACTIVATE.**

## Closing note

`_deferred_v1/` is retained only as this ledger (`TRIAGE.md`) plus the
retirement banner in `README.md`. Reactivation is not applicable to this
corpus: per D-02, the bar for reactivation is that a retired test's behavior
still exists on a shipping surface today, and none does — every file's
subject CLI was hard-deleted in v0.9 Phase 4 with no live equivalent to
rewrite against. Any future federation-adjacent test need is a *new* test
authored from scratch against the current `famp-bus`/`famp-gateway` API, not
a resurrection of a file in this directory.
