---
phase: 01-famp-bus-library-and-audit-log
type: phase-verification
status: PASS
created: 2026-04-28
verifier: gsd-execute-phase post-execution gate
plans_verified:
  - 01-01-PLAN.md (Wave 1) — commits 0a116f5, c604f03, 235c752
  - 01-02-PLAN.md (Wave 2) — commits 86599aa, 093c8f9, ae905ed
  - 01-03-PLAN.md (Wave 3) — commit 9ca6e131b9db6b07afa414a423179793945b0aac
requirements_in_scope:
  - BUS-01..11 (11)
  - TDD-01..04 (4)
  - PROP-01..05 (5)
  - AUDIT-01..06 (6)
  - CARRY-03, CARRY-04 (2)
total_in_scope: 28
---

# Phase 01 Verification — `famp-bus` library + `audit_log` MessageClass

## Goal (from `.planning/ROADMAP.md`)

> Ship the protocol-primitive substrate for the local bus — pure state machine, types, codec, in-memory mailbox, four RED-first TDD gates, full proptest coverage — and atomically close the v0.5.1→v0.5.2 spec-vs-constant lag T5 intentionally introduced. Library only: no UDS, no tokio in broker core, no I/O.

**Verdict: PASS.** All 28 in-scope requirements satisfied; goal achieved.

---

## Goal-Backward Audit

### Phase success criterion 1 — `famp-bus` test surface fully green

Plan 01-02 SUMMARY records all four TDD gates GREEN and all five PROP-01..05 GREEN. Plan 01-03 SUMMARY records `prop04_drain_completeness` re-asserted GREEN against the typed-decoder gate, plus a new negative case (`malformed_drain_line_returns_error_and_does_not_advance_cursor`).

Evidence (from `01-03-SUMMARY.md`):
- `cargo nextest run -p famp-bus --test prop04_drain_completeness` — 2 passed
- `cargo nextest run -p famp-bus --test audit_log_dispatch` — 2 passed
- TDD-02/03/04 transitioned RED → GREEN on Wave 2 commits (86599aa, 093c8f9)
- PROP-01..05 GREEN per Wave 2 SUMMARY

**Status: PASS.**

### Phase success criterion 2 — Pure broker actor, tokio-free

`Broker::handle(BrokerInput, Instant) -> Vec<Out>` is implemented in `crates/famp-bus/src/broker/handle.rs` with zero `.await` and zero `RwLock`/`Mutex` on broker state.

Live evidence (run on this verification turn):
```
$ just check-no-tokio-in-bus
OK - famp-bus is tokio-free.
```

`Justfile` recipe `check-no-tokio-in-bus` is wired into `ci:` (BUS-01 contract).

**Status: PASS.**

### Phase success criterion 3 — Atomic v0.5.2 bump (AUDIT-05 invariant)

The atomic-commit invariant states: `MessageClass::AuditLog` enum variant + dispatch + body validation MUST land in the same commit as the `FAMP_SPEC_VERSION` flip from `"0.5.1"` → `"0.5.2"`, and the T5 doc-comment lag note must be removed in the same commit.

Live evidence (run on this verification turn):
```
$ just check-spec-version-coherence
(exit 0 — both `MessageClass::AuditLog` and `AuditLogBody` symbols present
when constant declares 0.5.2; recipe wired into `ci:`)
```

Commit `9ca6e131b9db6b07afa414a423179793945b0aac` contains all of:
- `crates/famp-envelope/src/version.rs`: `FAMP_SPEC_VERSION = "0.5.2"` + T5 lag block deleted
- `crates/famp-core/src/class.rs`: `MessageClass::AuditLog` 6th variant + Display arm
- `crates/famp-envelope/src/body/audit_log.rs` (NEW): `AuditLogBody` schema with `deny_unknown_fields` and `post_decode_validate`
- `crates/famp-envelope/src/dispatch.rs`: `AnySignedEnvelope::AuditLog` arm + decode dispatch
- `crates/famp-envelope/src/causality.rs`: `Relation::Audits` 6th variant
- `crates/famp-envelope/src/bus.rs` (NEW): `BusEnvelope<B>` sibling type + `AnyBusEnvelope` 6-arm dispatch + `UnexpectedSignature` (BUS-11)
- `Justfile`: `check-spec-version-coherence` recipe added and wired into `ci:`

Re-verifying that the FSM crate is untouched (Δ31 / D-15: audit_log is non-FSM-firing):
```
$ git diff HEAD~1 HEAD -- crates/famp-fsm/
(empty)
```

**Status: PASS.**

### Phase success criterion 4 — `just ci` conformance gates unaffected

The four conformance gates that were green at v0.8 close (RFC 8785 byte-exact, §7.1c worked example, RFC 8032 KATs, NIST FIPS 180-2 KATs) are not touched by Wave 3. Targeted runs in `01-03-SUMMARY.md` show `cargo build --workspace --all-targets` clean and `cargo clippy ... -- -D warnings` clean across the changed crates.

**Caveat — full `just ci` blocked by pre-existing TLS-loopback flake:** 8 tests in `crates/famp/tests/` and `crates/famp-transport-http/tests/` fail with `reqwest::Error { kind: Request, source: TimedOut }` against `https://127.0.0.1:.../famp/v0.5.1/inbox/...`. The same 8 tests fail identically when stashing all Wave 3 changes and re-running on Wave 2 commit `ae905ed`, confirming **pre-existing local-environment issue, not a Wave 3 regression**. Reproduction protocol documented in `01-03-SUMMARY.md` §verification.blocked. Listed as a deferred hygiene task; does not gate Phase 1 closure.

**Status: PASS (with documented pre-existing flake outside phase scope).**

### Phase success criterion 5 — Carry-forward debt addressed

- **CARRY-03 (TD-4):** Broker auto-creates REQUESTED task record on inbound request. Naturally absorbed by Phase 1 broker state-machine design per Wave 2 SUMMARY. **Status: SATISFIED.**
- **CARRY-04 (TD-7):** Backfill Nyquist `VALIDATION.md` for v0.8 phases 02-04 + bridge phase, OR formally defer. Wave 1 PLAN.md lists CARRY-04 in `requirements:`; Wave 2 STATE.md notes "CARRY-04 is formally deferred to the v0.9 milestone-close audit per D-18." **Status: FORMALLY DEFERRED per D-18 — counts as satisfied.**

**Status: PASS.**

---

## Per-Requirement Coverage

### BUS — `famp-bus` library (11/11 satisfied)

| REQ | Status | Evidence |
|---|---|---|
| BUS-01 | PASS | Crate in workspace; `just check-no-tokio-in-bus` exits 0 (this verification run); pure state machine has no I/O. |
| BUS-02 | PASS | `BusMessage` enum byte-exact round-trip via `famp-canonical` per Wave 1 SUMMARY codec_fuzz. |
| BUS-03 | PASS | `BusReply` enum byte-exact round-trip per Wave 1 SUMMARY. |
| BUS-04 | PASS | `Target::Channel` regex `^#[a-z0-9][a-z0-9_-]{0,31}$` enforced via `deserialize_with` in `proto.rs`. |
| BUS-05 | PASS | `BusErrorKind` exhaustive flat enum; consumer stub `tests/buserror_consumer_stub.rs` compiles under `#![deny(unreachable_patterns)]`. |
| BUS-06 | PASS | 4-byte big-endian length-prefixed canonical-JSON codec; max 16 MiB; sync. TDD-01 GREEN. |
| BUS-07 | PASS | `Broker::handle(BrokerInput, Instant) -> Vec<Out>` total function; tested without UDS/runtime. |
| BUS-08 | PASS | Hello handshake required first; pre-Hello messages return `BrokerProtoMismatch`. |
| BUS-09 | PASS | Single-threaded actor; zero RwLock/Mutex on broker state per Wave 2 truths and Wave 1 BrokerEnv design. |
| BUS-10 | PASS | `InMemoryMailbox` in `crates/famp-bus/src/mailbox.rs`. |
| BUS-11 | PASS | `BusEnvelope<B>` sibling type with private `inner` field + two `compile_fail` doctests + runtime `UnexpectedSignature` rejection (`crates/famp-envelope/src/bus.rs:32`, `crates/famp-envelope/src/bus.rs:56`). |

### TDD — RED-first gates (4/4 satisfied)

| REQ | Status | Evidence |
|---|---|---|
| TDD-01 | PASS | `tests/codec_fuzz.rs` GREEN at end of Wave 1 (TDD-01 was the only gate green-by-wave-1 design). |
| TDD-02 | PASS | `tests/tdd02_drain_cursor_order.rs` RED→GREEN at Wave 2. |
| TDD-03 | PASS | `tests/tdd03_pid_reuse.rs` RED→GREEN at Wave 2 with D-08 dual liveness + `clients` map cross-check. |
| TDD-04 | PASS | `tests/tdd04_eof_cleanup.rs` RED→GREEN at Wave 2; pending_awaits cleanup on Disconnect. |

### PROP — Property-test coverage (5/5 satisfied)

| REQ | Status | Evidence |
|---|---|---|
| PROP-01 | PASS | `tests/prop01_dm_fanin_order.rs` GREEN (Wave 2). |
| PROP-02 | PASS | `tests/prop02_channel_fanout.rs` GREEN (Wave 2). |
| PROP-03 | PASS | `tests/prop03_join_leave_idempotent.rs` GREEN (Wave 2). |
| PROP-04 | PASS | `tests/prop04_drain_completeness.rs` GREEN twice — first against `Vec<serde_json::Value>` in Wave 2, then re-asserted against the typed-decoder gate in Wave 3 with a new malformed-line negative case. |
| PROP-05 | PASS | `tests/prop05_pid_unique.rs` GREEN (Wave 2). |

### AUDIT — `audit_log` MessageClass v0.5.2 amendment (6/6 satisfied)

| REQ | Status | Evidence |
|---|---|---|
| AUDIT-01 | PASS | `MessageClass::AuditLog` 6th variant + Display arm in `crates/famp-core/src/class.rs:20`, `:31`. |
| AUDIT-02 | PASS | `AuditLogBody { event, subject?, details? }` with `#[serde(deny_unknown_fields)]` and `post_decode_validate` rejecting empty event in `crates/famp-envelope/src/body/audit_log.rs:15-29`. |
| AUDIT-03 | PASS | Receiver MUST store, MUST NOT emit `ack`. `git diff HEAD~1 HEAD -- crates/famp-fsm/` is empty (no transitions added). `fsm_input_from_envelope` returns `None` for `AuditLog` (joining `Ack` precedent) per Wave 3 SUMMARY key-decisions. |
| AUDIT-04 | PASS | `Relation::Audits` 6th variant in `crates/famp-envelope/src/causality.rs:17`. |
| AUDIT-05 | PASS | All changes in single commit `9ca6e131b9db6b07afa414a423179793945b0aac`. `just check-spec-version-coherence` passes (this verification run). Recipe wired into `ci:`. Commit message cites `AUDIT-05` token. |
| AUDIT-06 | PASS | T5 doc-comment lag block deleted in version.rs at HEAD; verified by Wave 3 SUMMARY and grep-clean. |

### CARRY — v0.8 carry-forward debt (2/2 addressed)

| REQ | Status | Evidence |
|---|---|---|
| CARRY-03 (TD-4) | SATISFIED | Broker auto-creates `REQUESTED` task record on inbound request — naturally absorbed by Wave 2 broker state-machine design per Wave 2 SUMMARY. |
| CARRY-04 (TD-7) | DEFERRED-PER-POLICY | Formally deferred to v0.9 milestone-close audit per D-18, recorded in STATE.md. PLAN.md option (B) "formally defer" honored. |

**Coverage: 28/28 in-scope requirements satisfied or formally deferred per policy.**

---

## Deviations From Literal PLAN Wording (documented, goal-coherent)

### D-09 implementation: type-validation gate, not literal `Vec<AnyBusEnvelope>` swap

`01-03-PLAN.md` must_have line:
> "famp-bus broker drain decoder is rewired from `Vec<serde_json::Value>` to `Vec<AnyBusEnvelope>` at the proto-level AND at the broker handle level"

**Actual implementation:** `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire to preserve BUS-02/03 canonical-JSON `serde::Deserialize` round-trip; broker handler calls `AnyBusEnvelope::decode(&line)` against each line FIRST and only inserts the matching `serde_json::Value` if decode succeeds. First decode failure short-circuits the entire register reply with `BusReply::Err{EnvelopeInvalid}` and never emits `Out::AdvanceCursor`.

**Why this satisfies D-09's intent:** The goal of D-09 is "broker decodes each line via the bus-side typed envelope decoder before populating drained". The intent is preventing structurally-invalid envelopes from being emitted to clients. The type-validation gate achieves this without forcing a manual `Deserialize for BusReply::RegisterOk` and without breaking BUS-02/03's canonical-JSON round-trip contract.

**Documented in:** `01-03-SUMMARY.md:62-64` (key-decisions), `01-03-SUMMARY.md:78` (D-09 satisfied), `01-03-SUMMARY.md:116-120` (d09_evidence). PROP-04 re-asserted GREEN against the typed-decoder gate; new negative case `malformed_drain_line_returns_error_and_does_not_advance_cursor` proves the gate fires on malformed input.

**Verdict:** Goal-coherent deviation, documented and verified. Not a gap.

### Files outside `01-03-PLAN.md` `files_modified:` shipped in atomic commit

`crates/famp/src/runtime/adapter.rs` and `crates/famp-transport-http/src/server.rs` gained `AuditLog` arms in their exhaustive matches. These were necessary for the workspace to compile after the new variant landed; bundling them in the AUDIT-05 atomic commit was the only way to preserve the atomic-bump invariant. Documented in `01-03-SUMMARY.md:67`.

**Verdict:** Necessary fallout absorbed correctly.

---

## Pre-Existing Issues Outside Phase Scope (do not gate closure)

1. **8 listener/E2E test timeouts** (`reqwest::Error { kind: Request, source: TimedOut }` against TLS loopback). Reproduced on Wave 2 commit `ae905ed` with all Wave 3 changes stashed — pre-existing local-environment issue, not a Wave 3 regression. Documented in `01-03-SUMMARY.md` §verification.blocked. Listed as deferred hygiene task.
2. **HTTP transport URL path `/famp/v0.5.1/inbox/{principal}`** intentionally NOT bumped to v0.5.2 — phase scopes only the envelope `famp` field; URL versioning is a separate transport concern. Documented in `01-03-SUMMARY.md:66`.
3. **Stale `"0.5.1"` doc-comment in `crates/famp/src/cli/await_cmd/poll.rs:9`** — `poll.rs` does not validate the version field, so this is stale prose, not active drift. Out of phase scope. Documented in `01-03-SUMMARY.md:128`.

---

## Verdict

**Phase 1 is complete.** All 28 in-scope requirements satisfied or formally deferred per policy. Phase goal achieved: a tokio-free, transport-neutral `famp-bus` library with full TDD/PROP coverage and an atomic v0.5.2 audit_log amendment landed in a single commit. Conformance gates added: `just check-no-tokio-in-bus` and `just check-spec-version-coherence`, both wired into `ci:`. The atomic-bump invariant (AUDIT-05) is now permanently grep-gated against future regression.

**Phase 2** (UDS wire + CLI + MV-MCP rewire + `famp-local hook add`) inherits a stable v0.5.2 envelope surface, the `AnyBusEnvelope` typed dispatcher, the BUS-11 type-level enforcement, and a broker drain handler that already type-validates lines.
