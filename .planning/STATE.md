---
gsd_state_version: 1.0
milestone: v0.9
milestone_name: Local-First Bus
status: executing
stopped_at: Phase 2 context gathered
last_updated: "2026-04-28T22:52:36.515Z"
last_activity: 2026-04-28 -- Phase 02 execution started
progress:
  total_phases: 7
  completed_phases: 1
  total_plans: 3
  completed_plans: 3
  percent: 100
---

# STATE: FAMP — v0.9 Local-First Bus

**Last Updated:** 2026-04-28 — Phase 01 complete; verification PASS.

## Project Reference

See: .planning/PROJECT.md — v0.9 Local-First Bus is the active milestone; v1.0 Federation Profile remains trigger-gated on Sofer (or named equivalent) interop from a different machine.

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 02 — uds-wire-cli-mv-mcp-rewire-hook-subcommand

## Current Position

Phase: 02 (uds-wire-cli-mv-mcp-rewire-hook-subcommand) — EXECUTING
Plan: 1 of 14
Status: Executing Phase 02
Last activity: 2026-04-28 -- Phase 02 execution started

## Last Shipped

- **Plan 01-01: famp-bus scaffold and primitives** (2026-04-27) — `famp-bus` workspace crate, tokio-free dependency gate, bus protocol types, canonical length-prefixed codec, in-memory mailbox, liveness fakes, BusErrorKind exhaustive consumer stub, TDD-01 green codec fuzz tests, and deliberate TDD-02/03/04 compile-red broker scaffolds. Commits: `0a116f5`, `c604f03`, `235c752`.
- **Plan 01-02: pure broker actor and property suite** (2026-04-27) — tokio-free `Broker::handle(BrokerInput, Instant) -> Vec<Out>` actor, exhaustive dispatch for all nine `BusMessage` variants plus `Disconnect`/`Tick`, ordered `Out` intents, TDD-02/03/04 GREEN, and PROP-01..05 GREEN against temporary `Vec<serde_json::Value>` drained payloads. Commits: `86599aa`, `093c8f9`, `ae905ed`.
- **Plan 01-03: atomic v0.5.1→v0.5.2 bump + audit_log MessageClass + BusEnvelope (BUS-11) + broker drain typed-decoder (D-09)** (2026-04-28) — Single atomic commit landing `MessageClass::AuditLog`, `AuditLogBody`, `Relation::Audits`, `AnySignedEnvelope::AuditLog` dispatch, `BusEnvelope<B>` sibling type with private inner + 2 `compile_fail` doctests, `AnyBusEnvelope` 6-arm dispatch, `EnvelopeDecodeError::UnexpectedSignature`, `FAMP_SPEC_VERSION = "0.5.2"` flip + T5 lag block deletion, vector_1 worked example, broker drain typed-decoder gate (D-09 type-validation-only implementation), PROP-04 re-asserted with malformed-line negative case, `just check-spec-version-coherence` recipe wired into `ci:`. Commit: `9ca6e13`.
- **Phase 01 verification** (2026-04-28) — Goal-backward audit PASS; 28/28 in-scope requirements satisfied or formally deferred per policy. See `.planning/phases/01-famp-bus-library-and-audit-log/01-VERIFICATION.md`.

## Accumulated Context

- `famp-bus` is Layer 1 only: no UDS listener, no tokio runtime, no on-disk I/O, no CLI surface.
- All four TDD gates and all five PROP-01..05 properties GREEN.
- `FAMP_SPEC_VERSION = "0.5.2"`; `MessageClass::AuditLog` is the 6th wire variant; `Relation::Audits` is the 6th causality variant.
- `BusEnvelope<B>` (private-inner sibling type) and `AnyBusEnvelope` 6-arm dispatch enforce BUS-11 at compile time and at runtime.
- Broker `decode_lines` calls `AnyBusEnvelope::decode` against each drain line; failure short-circuits to `BusReply::Err{EnvelopeInvalid}` and aborts cursor advance. `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire to preserve BUS-02/03 round-trip — the swap to `Vec<AnyBusEnvelope>` was abandoned by design (D-09 type-validation-only); documented in 01-03-SUMMARY.md.
- `just check-spec-version-coherence` and `just check-no-tokio-in-bus` are now permanent CI gates.
- Pre-existing 8 listener/E2E TLS-loopback timeouts on macOS reproduce on Wave 2 commit `ae905ed`; not a Wave 3 regression. Deferred as a hygiene task.
- HTTP transport URL path `/famp/v0.5.1/inbox/{principal}` intentionally NOT bumped — transport URL versioning is out of Phase 1 scope.

## Decisions

- [Phase 01]: Plan 01-01 keeps TDD-02/03/04 as compile-red gates until Plan 01-02 adds Broker.
- [Phase 01]: `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire — D-09 implemented as type-validation gate (decode + accept), not type swap. Preserves BUS-02/03 round-trip; consumers wanting typed access call `AnyBusEnvelope::decode` per line.
- [Phase 01]: `famp-bus` no-tokio gate fails closed when `cargo tree` cannot run.
- [Phase 01]: Plan 01-02 tests apply `Out::AppendMailbox` intents to `TestEnv` explicitly, matching the future wire-layer side-effect executor.
- [Phase 01]: Exact all-target clippy remains blocked by pre-existing `famp-envelope` doc markdown; `famp-bus` all-target clippy passes with `--no-deps`.
- [Phase 01]: AUDIT-05 atomic-bump invariant honored — constant flip + impl + dispatch + body + doc-comment removal + Justfile recipe in ONE commit (`9ca6e13`). Necessary exhaustive-match fallout in `crates/famp/src/runtime/adapter.rs` and `crates/famp-transport-http/src/server.rs` rode the same commit.
- [Phase 01]: `audit_log` is non-FSM-firing per Δ31 / D-15. `git diff HEAD~1 HEAD -- crates/famp-fsm/` is empty; `fsm_input_from_envelope` returns `None` for `AuditLog` (joining `Ack` precedent).

## Issues / Blockers

- **8 pre-existing listener/E2E TLS-loopback timeouts** (`reqwest::Error { kind: Request, source: TimedOut }` against `https://127.0.0.1:.../famp/v0.5.1/inbox/...`). Reproduces on Wave 2 commit `ae905ed`. Not a Phase 1 regression. Documented in `01-03-SUMMARY.md` and `01-VERIFICATION.md`. Triage as a separate hygiene task before Phase 4.

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 23min | 2 tasks | 17 files |
| Phase 01 P02 | 15min | 2 tasks | 15 files |
| Phase 01 P03 | atomic | 1 task | 28 files |

## Session

**Last session:** 2026-04-28T20:27:25.219Z
**Stopped At:** Phase 2 context gathered
**Resume File:** .planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-CONTEXT.md
