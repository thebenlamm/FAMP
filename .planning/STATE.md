---
gsd_state_version: 1.0
milestone: v0.9
milestone_name: Local-First Bus
status: executing
last_updated: "2026-04-27T20:10:30.723Z"
progress:
  total_phases: 7
  completed_phases: 0
  total_plans: 3
  completed_plans: 1
  percent: 33
---

# STATE: FAMP — v0.9 Local-First Bus

**Last Updated:** 2026-04-27 — Plan 01-01 completed.

## Project Reference

See: .planning/PROJECT.md — v0.9 Local-First Bus is the active milestone; v1.0 Federation Profile remains trigger-gated on Sofer (or named equivalent) interop from a different machine.

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 01 — famp-bus-library-and-audit-log

## Current Position

Phase: 01 (famp-bus-library-and-audit-log) — EXECUTING
Plan: 2 of 3
Status: Plan 01-01 complete; ready for Plan 01-02
Last activity: 2026-04-27 -- Plan 01-01 completed

## Last Shipped

- **Plan 01-01: famp-bus scaffold and primitives** (2026-04-27) — `famp-bus` workspace crate, tokio-free dependency gate, bus protocol types, canonical length-prefixed codec, in-memory mailbox, liveness fakes, BusErrorKind exhaustive consumer stub, TDD-01 green codec fuzz tests, and deliberate TDD-02/03/04 compile-red broker scaffolds. Commits: `0a116f5`, `c604f03`, `235c752`.

## Accumulated Context

- `famp-bus` is Layer 1 only: no UDS listener, no tokio runtime, no on-disk I/O, no CLI surface.
- TDD-02/03/04 are compile-red by design until Plan 01-02 adds `Broker`, `BrokerInput`, and `Out`.
- Plan 01-03 owns the typed bus envelope decoder and the atomic v0.5.2 audit-log spec-version bump.
- CARRY-04 is formally deferred to the v0.9 milestone-close audit per D-18.

## Decisions

- [Phase 01]: Plan 01-01 keeps TDD-02/03/04 as compile-red gates until Plan 01-02 adds Broker.
- [Phase 01]: `RegisterOk.drained` and related bus reply fields remain `serde_json::Value` until Plan 01-03 introduces `AnyBusEnvelope`.
- [Phase 01]: `famp-bus` no-tokio gate fails closed when `cargo tree` cannot run.

## Issues / Blockers

None blocking.

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 | 23min | 2 tasks | 17 files |

## Session

**Last session:** 2026-04-27T20:10:30Z
**Stopped At:** Completed 01-01-PLAN.md
**Resume File:** None

