---
gsd_state_version: 1.0
milestone: v0.9
milestone_name: Local-First Bus
status: milestone_complete
stopped_at: Completed 04-05-PLAN.md
last_updated: "2026-05-04T01:23:57.243Z"
last_activity: 2026-05-04
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 31
  completed_plans: 29
  percent: 100
---

# STATE: FAMP — v0.9 Local-First Bus

**Last Updated:** 2026-04-30 — Phase 02 complete; v0.9 tech-debt sweep landed (7 commits, 492/22 still green); audit_log-wrapper protocol-shape question parked for architect counsel.

## Project Reference

See: .planning/PROJECT.md — v0.9 Local-First Bus is the active milestone; v1.0 Federation Profile remains trigger-gated on Sofer (or named equivalent) interop from a different machine.

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 04 — federation-cli-unwire-federation-ci-preservation

## Current Position

Phase: 04
Plan: Not started
Plans: 14 of 14 complete
Status: Milestone complete
Last activity: 2026-05-04

## Last Shipped

- **Plan 01-01: famp-bus scaffold and primitives** (2026-04-27) — `famp-bus` workspace crate, tokio-free dependency gate, bus protocol types, canonical length-prefixed codec, in-memory mailbox, liveness fakes, BusErrorKind exhaustive consumer stub, TDD-01 green codec fuzz tests, and deliberate TDD-02/03/04 compile-red broker scaffolds. Commits: `0a116f5`, `c604f03`, `235c752`.
- **Plan 01-02: pure broker actor and property suite** (2026-04-27) — tokio-free `Broker::handle(BrokerInput, Instant) -> Vec<Out>` actor, exhaustive dispatch for all nine `BusMessage` variants plus `Disconnect`/`Tick`, ordered `Out` intents, TDD-02/03/04 GREEN, and PROP-01..05 GREEN against temporary `Vec<serde_json::Value>` drained payloads. Commits: `86599aa`, `093c8f9`, `ae905ed`.
- **Plan 01-03: atomic v0.5.1→v0.5.2 bump + audit_log MessageClass + BusEnvelope (BUS-11) + broker drain typed-decoder (D-09)** (2026-04-28) — Single atomic commit landing `MessageClass::AuditLog`, `AuditLogBody`, `Relation::Audits`, `AnySignedEnvelope::AuditLog` dispatch, `BusEnvelope<B>` sibling type with private inner + 2 `compile_fail` doctests, `AnyBusEnvelope` 6-arm dispatch, `EnvelopeDecodeError::UnexpectedSignature`, `FAMP_SPEC_VERSION = "0.5.2"` flip + T5 lag block deletion, vector_1 worked example, broker drain typed-decoder gate (D-09 type-validation-only implementation), PROP-04 re-asserted with malformed-line negative case, `just check-spec-version-coherence` recipe wired into `ci:`. Commit: `9ca6e13`.
- **Phase 01 verification** (2026-04-28) — Goal-backward audit PASS; 28/28 in-scope requirements satisfied or formally deferred per policy. See `.planning/phases/01-famp-bus-library-and-audit-log/01-VERIFICATION.md`.
- **Phase 02: UDS wire + CLI + MCP rewire + hook subcommand** (2026-04-28..30) — 14 plans across 7 waves; `famp broker` UDS daemon, `famp register/send/inbox/await/join/leave/sessions/whoami` CLI surface, MCP rewired to bus (8-tool surface), `famp-local hook add/list/remove`. 492 tests green, 22 skipped. Code review: 22 findings, 15 fixed across 14 atomic `fix(02)` commits (WR-06 deferred — env-var test races, currently safe under nextest). Verification PASS 36/36 (2 manual UATs resolved 2026-04-30: BROKER-02 broker-survives-SIGINT-to-holder confirmed; BROKER-05 negative path passed, positive path waived absent NFS environment).

## Accumulated Context

- `famp-bus` is Layer 1 only: no UDS listener, no tokio runtime, no on-disk I/O, no CLI surface.
- All four TDD gates and all five PROP-01..05 properties GREEN.
- `FAMP_SPEC_VERSION = "0.5.2"`; `MessageClass::AuditLog` is the 6th wire variant; `Relation::Audits` is the 6th causality variant.
- `BusEnvelope<B>` (private-inner sibling type) and `AnyBusEnvelope` 6-arm dispatch enforce BUS-11 at compile time and at runtime.
- Broker `decode_lines` calls `AnyBusEnvelope::decode` against each drain line; failure short-circuits to `BusReply::Err{EnvelopeInvalid}` and aborts cursor advance. `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire to preserve BUS-02/03 round-trip — the swap to `Vec<AnyBusEnvelope>` was abandoned by design (D-09 type-validation-only); documented in 01-03-SUMMARY.md.
- `just check-spec-version-coherence` and `just check-no-tokio-in-bus` are now permanent CI gates.
- The 8 listener/E2E TLS-loopback timeout note from Phase 01 is moot at HEAD: those tests are now `#[ignore]`'d as v0.8-federation tests parked for Phase 04. The test surface is 492 passed / 22 skipped / 0 failed at HEAD on macOS.
- HTTP transport URL path `/famp/v0.5.1/inbox/{principal}` intentionally NOT bumped — transport URL versioning is out of Phase 1 scope.
- `[[profile.default.test-groups]]` `listen-subprocess = max-threads = 4` is now pinned in `.config/nextest.toml` (TD-1 carry-forward closed in 2026-04-30 sweep). Listen-subprocess parallelism flake on macOS is no longer latent.
- v0.8 federation `#[ignore]` reasons across 14 test files are now uniformly anchored at "Phase 04 (v0.9 federation deletion)"; Phase 04 will delete or migrate them with the v0.8 CLI surface. Two `#[ignore]`'d tests are NOT in this anchor (`cross_machine_happy_path` is v0.7 chicken-and-egg; `provisional_scope_instructions_vector` is a fixture regenerator).
- Env-var tests in `cli/identity.rs`, `bus_client/mod.rs`, `tests/mcp_register_whoami.rs` migrated to `temp-env` scoped helpers (WR-06 closed 2026-04-30 sweep). Edition 2024 toolchain bump no longer requires test-file changes.

## Open question — pending architect counsel before Phase 03 plan

- **`famp send` audit_log wrapper.** `crates/famp/src/cli/send/mod.rs::build_envelope_value` wraps every local DM, deliver, and channel post payload as an unsigned `audit_log` `BusEnvelope` with the mode-tagged payload (mode/summary/task/body/terminal/more_coming) under `body.details`. Class is hardcoded `"audit_log"`; `event` is `famp.send.{new_task,deliver,deliver_terminal,channel_post}`; from/to are synthetic `agent:local.bus/<name>` Principals. The wrapper exists because Phase 1 D-09 added a typed-decoder gate on the broker's drain path (`AnyBusEnvelope::decode` per drained line) and Phase 2 02-04's mode-tagged envelope had no `class` field. Three options on the table: (1) accept as v0.9 convention and let v1.0 federation gateway translate; (2) add a bus-internal `MessageClass::BusDm`/`LocalRequest` (v0.5.3 spec amendment, AUDIT-05 atomic-bump); (3) loosen D-09 to accept untyped local payloads. Lean is option 1 but the user wants architect counsel before Phase 03 scope locks. Full briefing drafted at .planning/STATE.md Q1 (this entry); architect MCP session was not running at sweep close (2026-04-30), so the question is parked for the next architect session.

## Decisions

- [Phase 01]: Plan 01-01 keeps TDD-02/03/04 as compile-red gates until Plan 01-02 adds Broker.
- [Phase 01]: `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire — D-09 implemented as type-validation gate (decode + accept), not type swap. Preserves BUS-02/03 round-trip; consumers wanting typed access call `AnyBusEnvelope::decode` per line.
- [Phase 01]: `famp-bus` no-tokio gate fails closed when `cargo tree` cannot run.
- [Phase 01]: Plan 01-02 tests apply `Out::AppendMailbox` intents to `TestEnv` explicitly, matching the future wire-layer side-effect executor.
- [Phase 01]: Exact all-target clippy remains blocked by pre-existing `famp-envelope` doc markdown; `famp-bus` all-target clippy passes with `--no-deps`.
- [Phase 01]: AUDIT-05 atomic-bump invariant honored — constant flip + impl + dispatch + body + doc-comment removal + Justfile recipe in ONE commit (`9ca6e13`). Necessary exhaustive-match fallout in `crates/famp/src/runtime/adapter.rs` and `crates/famp-transport-http/src/server.rs` rode the same commit.
- [Phase 01]: `audit_log` is non-FSM-firing per Δ31 / D-15. `git diff HEAD~1 HEAD -- crates/famp-fsm/` is empty; `fsm_input_from_envelope` returns `None` for `AuditLog` (joining `Ack` precedent).
- [Phase 04]: Plan 04-01 copied http_happy_path.rs library-API body into e2e_two_daemons.rs, changing only the Phase 4 doc comment and test function name.
- [Phase 04]: Plan 04-01 kept e2e_two_daemons_adversarial.rs independent of famp::runtime because runtime is removed later in Phase 4.
- [Phase 04]: Plan 04-02 moved info_happy_path.rs into _deferred_v1 because the live tree still imported famp::cli::setup; the planned keep condition had not landed.
- [Phase 04]: Plan 04-02 resolved D-03 row 7 as MOVE via active send unit coverage and row 13 as MOVE via active TaskNotFound error-surface mapping; full stale-task broker validation remains out of scope.
- [Phase 04]: Plan 04-05 uses staged framing rather than identity rewrite: FAMP today is local-first; FAMP at v1.0 is federated.

## Issues / Blockers

- **8 pre-existing listener/E2E TLS-loopback timeouts** (`reqwest::Error { kind: Request, source: TimedOut }` against `https://127.0.0.1:.../famp/v0.5.1/inbox/...`). Reproduces on Wave 2 commit `ae905ed`. Not a Phase 1 regression. Documented in `01-03-SUMMARY.md` and `01-VERIFICATION.md`. Triage as a separate hygiene task before Phase 4.
- **Plan 04-06 D-20 gate resolved:** pre-tag `just ci` blockers were fixed in `debed78`; lightweight tag `v0.8.1-federation-preserved` now points at `debed78f1b55df44fb2ca18687c5794147226a40`.

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 23min | 2 tasks | 17 files |
| Phase 01 P02 | 15min | 2 tasks | 15 files |
| Phase 01 P03 | atomic | 1 task | 28 files |
| Phase 04 P05 | 8min | 1 tasks | 6 files |

## Session

**Last session:** 2026-05-04T01:23:57.240Z
**Stopped At:** Completed 04-05-PLAN.md
**Resume File:** None
