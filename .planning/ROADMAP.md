# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- 📋 **v0.7 Personal Runtime** — Phases 1–4 (next). Minimal usable library on two transports.
- 📋 **v0.8+ Federation Profile** — Identity & Cards, Causality, Negotiation, Delegation, Provenance, Extensions, Adversarial Conformance.

## Phases

<details>
<summary>✅ v0.5.1 Spec Fork (Phases 0–1) — SHIPPED 2026-04-13</summary>

- [x] Phase 0: Toolchain & Workspace Scaffold — completed 2026-04-13
- [x] Phase 1: Spec Fork (FAMP-v0.5.1) — completed 2026-04-13

Archive: [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md)

</details>

<details>
<summary>✅ v0.6 Foundation Crates (Phases 1–3) — SHIPPED 2026-04-13</summary>

- [x] Phase 1: Canonical JSON Foundations (3/3 plans) — completed 2026-04-13 — SEED-001 resolved, RFC 8785 gate 12/12 green
- [x] Phase 2: Crypto Foundations (4/4 plans) — completed 2026-04-13 — Ed25519 `verify_strict`, §7.1c worked example byte-exact, NIST KATs green
- [x] Phase 3: Core Types & Invariants (2/2 plans) — completed 2026-04-13 — Principal/Instance, UUIDv7 IDs, ArtifactId, 15-category ProtocolErrorKind, AuthorityScope, INV-1..11

Archive: [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md) · Phases: [milestones/v0.6-phases/](milestones/v0.6-phases/)

</details>

### 📋 v0.7 Personal Runtime (Active)

**Goal:** A single developer can run the same signed `request → commit → deliver` cycle two ways — same-process via `MemoryTransport`, and cross-machine via a minimal HTTP binding — with trust bootstrapped from a local keyring file.

**Finish line:** `cargo run --example personal_two_agents` (one binary) and `cargo run --example cross_machine_two_agents` (two shells/machines) both complete the signed cycle; the three adversarial cases (unsigned / wrong-key / canonical divergence) fail closed on both transports; `just ci` green.

**Phase numbering:** milestone-local, reset to Phase 1. v0.6 ended at Phase 3 but phase numbers are not continuous across milestones.

- [ ] **Phase 1: Minimal Signed Envelope** — `famp-envelope` with INV-10 mandatory-signature enforcement and body schemas for the five shipped message classes (`request`, `commit`, `deliver`, `ack`, `control/cancel` only)
- [ ] **Phase 2: Minimal Task Lifecycle** — 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`), compiler-checked terminals, proptest transition legality
- [ ] **Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example** — `Transport` trait, in-process `MemoryTransport`, local-file TOFU keyring, `personal_two_agents` example, three adversarial cases green on `MemoryTransport`
- [ ] **Phase 4: Minimal HTTP Transport + Cross-Machine Example** — `famp-transport-http` (axum inbox + reqwest client + rustls + 1 MB body limit + pre-routing sig-verification middleware), `cross_machine_two_agents` example, the Phase 3 adversarial matrix extended to HTTP

## Phase Details

### Phase 1: Minimal Signed Envelope
**Goal:** `famp-envelope` encodes, decodes, and signature-verifies every message class the Personal Runtime actually emits, and rejects anything else at the type level.
**Depends on:** v0.6 substrate (`famp-canonical`, `famp-crypto`, `famp-core`)
**Requirements:** ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed), ENV-10, ENV-12 (cancel-only), ENV-14, ENV-15 (10 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp-envelope` round-trips every shipped message class (`request`, `commit`, `deliver`, `ack`, `control/cancel`) through `famp-canonical` and `famp-crypto::verify_strict` under a `proptest` generator, byte-exact both directions.
  2. Decoding an envelope without a signature returns a typed `ProtocolError` (INV-10 enforced) — unsigned messages are unreachable from any downstream consumer, not merely logged.
  3. Every body struct uses `#[serde(deny_unknown_fields)]`; adding an unknown key at any depth produces a decode error in a committed fixture test.
  4. `ENV-12 (cancel-only)` is enforced at the type level: there is no constructor or deserialize path that yields a `control` body with `supersede` or `close`; the wider v0.6-catalog form is explicitly gated out for v0.7.
  5. `ENV-09 (narrowed)` contains no capability-snapshot binding; the commit body schema compiles and round-trips without any reference to Agent Cards, and this omission is documented inline with a pointer to v0.8.
**Plans:** 1/3 plans executed
- [x] 01-01-PLAN.md — Crate scaffold + primitive types (class/scope/version/timestamp) + error skeleton + §7.1c vector 0 fixtures on disk
- [ ] 01-02-PLAN.md — Sealed BodySchema trait + five shipped body types with ENV-09 and ENV-12 narrowings enforced at the type level
- [ ] 01-03-PLAN.md — Type-state UnsignedEnvelope/SignedEnvelope + decode pipeline + AnySignedEnvelope dispatch + vector 0 byte-exact regression + full adversarial + proptest suite

### Phase 2: Minimal Task Lifecycle
**Goal:** The 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`) is compiler-checked and every illegal transition is unreachable, not merely rejected at runtime.
**Depends on:** Phase 1
**Requirements:** FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08 (5 requirements)
**Success Criteria** (what must be TRUE):
  1. `TaskFsm` exposes exactly 5 states (1 initial + 1 intermediate + 3 terminals); adding or removing a variant causes a hard compile error in a downstream consumer stub under `#![deny(unreachable_patterns)]` (INV-5, FSM-03).
  2. `FSM-02 (narrowed)` is enforced: no `REJECTED`, no `EXPIRED`, no timeout-driven transitions exist in the public API. The wider v0.6-catalog form is gated out for v0.7.
  3. `proptest` transition-legality tests enumerate the full `(class, relation, terminal_status, current_state)` tuple space and assert: every legal tuple is accepted, every illegal tuple is rejected with a typed error, zero panics.
  4. FSM state types are fully owned (no lifetimes, no `&str`/`&[u8]` in the public enum), so state can be moved across threads and stored without borrow gymnastics.
**Plans:** TBD

### Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example
**Goal:** A single developer runs `request → commit → deliver → ack` end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on `MemoryTransport`.
**Depends on:** Phase 2
**Requirements:** TRANS-01, TRANS-02, KEY-01, KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07 (10 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp-transport` exposes a `Transport` trait (async send + incoming stream); `MemoryTransport` is an in-process implementation (~50 LoC, no network, no TLS) usable as a `dev-dependency` from any crate that needs an end-to-end fixture.
  2. The TOFU keyring is a local-file `HashMap<Principal, VerifyingKey>` where principal = raw 32-byte Ed25519 pubkey. It loads from a one-line-per-principal file (base64url-unpadded) **or** from `--peer <principal>:<pubkey>` CLI flags. There is no Agent Card, no federation credential, no pluggable trust store — that scope is v0.8.
  3. `cargo run --example personal_two_agents` exits `0` and prints a typed conversation trace containing, in order, a signed `request`, `commit`, `deliver`, and `ack` over `MemoryTransport` (CONF-03).
  4. The three adversarial cases (CONF-05 unsigned / CONF-06 wrong-key / CONF-07 canonical divergence) each fail closed with a distinct typed `ProtocolError` when injected into `MemoryTransport`; no panics, no silent drops, no generic `Error::Other`.
  5. The keyring file format is round-trip tested (load → save → load produces byte-identical bytes) and committed as a fixture.
**Plans:** TBD

### Phase 4: Minimal HTTP Transport + Cross-Machine Example
**Goal:** The same signed cycle runs across two processes over HTTPS, bootstrapped from the same TOFU keyring, and the Phase 3 adversarial matrix is extended to `HttpTransport` — no new conformance categories are introduced.
**Depends on:** Phase 3
**Requirements:** TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, EX-02, CONF-04 (7 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp-transport-http` exposes an axum `POST /famp/v0.5.1/inbox` endpoint per principal and a `reqwest` client send path, both running on `rustls` via `rustls-platform-verifier` (no OpenSSL), with a 1 MB request-body limit enforced as a `tower` layer (TRANS-03/04/06/07).
  2. Signature verification runs as HTTP middleware **before** routing (TRANS-09): unsigned or wrong-key requests are rejected at the tower layer and never reach handler code — verified by a test that asserts the handler closure is not entered on an adversarial case.
  3. Running `cross_machine_two_agents` as a server in one shell and a client in another completes a signed `request → commit → deliver → ack` cycle over real HTTPS, with both ends loading the other's pubkey from a local keyring file or `--peer` flag (CONF-04, EX-02). Exit code `0`.
  4. **The existing Phase 3 adversarial matrix is extended to `HttpTransport`.** The same three cases (unsigned / wrong-key / canonical divergence) that passed on `MemoryTransport` now also fail closed on HTTP with the same typed errors — six test rows total across the two transports, derivative of CONF-05/06/07, no new CONF-0x requirements introduced in this phase.
  5. `.well-known` Agent Card distribution (TRANS-05) and the cancellation-safe spawn-channel send path (TRANS-08) are explicitly absent; the crate compiles and the examples run without them, and their omission is documented inline with a pointer to v0.8+.
**Plans:** TBD

## Future Milestone Sketch (Federation Profile)

Rough ordering, not committed:

- **v0.8 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` distribution (TRANS-05), SPEC-04..06
- **v0.9 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path (TRANS-08), SPEC-07/08
- **v0.10 Negotiation & Commitment** — propose/counter-propose, round limits, capability snapshot binding, conversation FSM
- **v0.11 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v0.12 Provenance** — graph, canonicalization, redaction, signed terminal reports
- **v0.13 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v0.14 Adversarial Conformance + Level 2/3 Badges** — full CONF matrix, stateright model checking, conformance-badge automation, `famp` CLI

## Progress Table

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | v0.6 | 1/3 | In Progress|  |
| 2. Crypto Foundations | v0.6 | 4/4 | Complete | 2026-04-13 |
| 3. Core Types & Invariants | v0.6 | 2/2 | Complete | 2026-04-13 |
| 1. Minimal Signed Envelope | v0.7 | 0/? | Not started | - |
| 2. Minimal Task Lifecycle | v0.7 | 0/? | Not started | - |
| 3. MemoryTransport + TOFU Keyring + Same-Process Example | v0.7 | 0/? | Not started | - |
| 4. Minimal HTTP Transport + Cross-Machine Example | v0.7 | 0/? | Not started | - |

---
*Roadmap updated: 2026-04-13 — v0.7 Personal Runtime roadmap canonicalized (4 phases, 32 requirements, 100% coverage)*
