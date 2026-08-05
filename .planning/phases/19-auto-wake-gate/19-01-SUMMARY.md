---
phase: 19-auto-wake-gate
plan: 01
subsystem: broker
tags: [await, origin, quarantine, mailbox]
requires:
  - phase: 14-inbound-content-is-data-quarantine
    provides: origin-stamped mailbox records
provides:
  - broker-enforced Local-only Await eligibility
  - skip-and-advance semantics for non-Local Await records
  - production-faithful actor and property fixtures
affects: [auto-wake, inbox, gateway]
tech-stack:
  added: []
  patterns: [positive-trust origin gate, independent Await and Inbox cursors]
key-files:
  created: []
  modified: [crates/famp-bus/src/broker/drain_walk.rs, crates/famp-bus/src/broker/awaiting.rs, crates/famp-bus/src/broker/handle.rs, crates/famp-bus/src/broker/handle/tests.rs]
key-decisions:
  - "Only exact Origin::Local may satisfy Await; Gateway and Unknown remain durable but ineligible."
  - "Non-Local records advance only the Await cursor, preserving explicit Inbox visibility."
patterns-established:
  - "Await eligibility is decided inside the broker before any CLI receives AwaitOk."
requirements-completed: [QUAR-12, QUAR-13, QUAR-14]
coverage:
  - id: D1
    description: Gateway and Unknown traffic cannot wake DM or channel waiters while Local traffic still can.
    requirement: QUAR-12
    verification:
      - kind: unit
        ref: "cargo test -p famp-bus --lib auto_wake"
        status: pass
    human_judgment: false
  - id: D2
    description: Await skips remote records without hiding them from explicit Inbox reads or blocking later Local records.
    requirement: QUAR-14
    verification:
      - kind: unit
        ref: "crates/famp-bus/src/broker/handle/tests.rs#auto_wake_initial_drain_skips_remote_and_preserves_inbox"
        status: pass
    human_judgment: false
duration: 13min
completed: 2026-08-04
status: complete
---

# Phase 19 Plan 01: Broker Auto-Wake Gate Summary

**The broker now permits only Local-origin records to satisfy Await while retaining remote records for explicit Inbox reads.**

## Accomplishments

- Enforced the positive-trust origin gate in both parked-waiter selection and initial Await drain.
- Preserved Local DM/channel wake behavior, independent cursor semantics, and remote Inbox visibility.
- Made actor/property fixtures reproduce production origin stamping.

## Task Commits

1. **Task 19-01-01:** RED actor tests and Local-only broker gate — `483ef9c`, `aafb935`
2. **Task 19-01-02:** Production-faithful mailbox/property fixtures — `40ef87f`, `699eef2`

## Deviations from Plan

- Extended the fixture correction to `tdd04_eof_cleanup.rs` because its ordinary Local registrations otherwise fail-closed to Unknown under the new gate.

## Verification

- Full workspace suite passed on 2026-08-05.
- `cargo fmt --all -- --check`, clippy with warnings denied, and the no-Tokio dependency gate passed.

## User Setup Required

None.

---
*Phase: 19-auto-wake-gate · Plan: 01 · Completed: 2026-08-04*
