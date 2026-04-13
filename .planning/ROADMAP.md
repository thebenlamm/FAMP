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

### 📋 v0.7 Personal Runtime (Next)

**Goal:** A single developer can run the same signed `request → commit → deliver` cycle two ways — same-process via `MemoryTransport`, and cross-machine via a minimal HTTP binding — with trust bootstrapped from a local keyring file.

**Finish line:** `cargo run --example personal_two_agents` (one binary) and `cargo run --example cross_machine_two_agents` (two shells/machines) both complete the signed cycle; three negative tests (unsigned / wrong-key / canonical divergence) fail closed on both transports; `just ci` green.

- [ ] **Phase 1: Minimal Signed Envelope** — `famp-envelope` encode/decode/validate, INV-10 mandatory-signature enforcement, body schemas for `request`, `commit`, `deliver`, `ack`, `control/cancel` only (ENV-01/02/03/06/07, ENV-09 narrowed, ENV-10, ENV-12 cancel-only, ENV-14/15)
- [ ] **Phase 2: Minimal Task Lifecycle** — 4-state FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`), compile-time terminal enforcement via exhaustive enum `match`, proptest transition-legality tests (FSM-02 narrowed, FSM-03/04/05/08)
- [ ] **Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example** — `famp-transport` trait + `MemoryTransport` (~50 LoC), local `HashMap<Principal, VerifyingKey>` keyring, `personal_two_agents.rs` happy path, three negative tests (TRANS-01/02, CONF-03/05/06/07)
- [ ] **Phase 4: Minimal HTTP Transport + Cross-Machine Example** — `famp-transport-http` axum `POST /famp/v0.5.1/inbox`, reqwest client, rustls TLS, 1 MB body limit, sig-verification middleware before routing, `cross_machine_two_agents.rs`, negative tests re-run on HTTP (TRANS-03/04/06/07/09, CONF-04). **Deferred:** TRANS-05 (`.well-known` cards), TRANS-08 (cancellation-safe send).

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
| 1. Canonical JSON Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 2. Crypto Foundations | v0.6 | 4/4 | Complete | 2026-04-13 |
| 3. Core Types & Invariants | v0.6 | 2/2 | Complete | 2026-04-13 |
| 1. Minimal Signed Envelope | v0.7 | 0/? | Not started | - |
| 2. Minimal Task Lifecycle | v0.7 | 0/? | Not started | - |
| 3. MemoryTransport + TOFU Keyring | v0.7 | 0/? | Not started | - |
| 4. Minimal HTTP Transport | v0.7 | 0/? | Not started | - |

---
*Roadmap reorganized: 2026-04-13 — v0.6 Foundation Crates archived; v0.7 Personal Runtime queued as next milestone*
