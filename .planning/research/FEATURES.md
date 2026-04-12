# Feature Research — FAMP v0.5 Reference Implementation

**Domain:** Agent messaging protocol reference library (Rust)
**Researched:** 2026-04-12
**Confidence:** HIGH (sourced directly from FAMP-v0.5 spec sections cited inline)

## Framing

FAMP is a **protocol library**, not an application. "Features" here means the protocol surface area exposed to downstream developers plus the conformance dimensions that define interop. The question is not "what does an end user expect" but "what must a Rust crate export so that a second independent implementation can interop and pass the same conformance vectors."

Target conformance: **Level 2 (Conversational) + Level 3 (Task-capable)** shipped together as v1. Level 1-only is explicitly excluded (PROJECT.md, spec §19).

Complexity scale: **S** (≤1 day), **M** (2–5 days), **L** (1–2 weeks), **XL** (>2 weeks).

---

## Feature Landscape

### Table Stakes (Must Ship in v1)

Every feature below is required for Level 2+3 conformance. Missing any one means a conformant peer cannot interop.

#### Foundation — Canonicalization, Crypto, Core Types

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F1 | RFC 8785 JCS canonical JSON encoder | §7.1, §14.3 | L | — | #1 interop blocker. Spec under-specifies; fork locks to RFC 8785. Must handle Unicode, number normalization, duplicate keys, nested canonicalization of provenance. |
| F2 | Ed25519 sign / verify with domain separation prefix | §5.1, §7.1, INV-10 | M | F1 | `ed25519-dalek`. Domain prefix added in v0.5.1 fork to prevent cross-protocol signature reuse. Raw 32/64-byte keys, unpadded base64url. |
| F3 | Core type system: `Principal`, `Instance`, `Conversation`, `Task`, `Proposal`, `Commitment`, `Artifact`, `Policy` | §3.1–§3.7 | M | — | Every first-class object from §3 gets a Rust type. Enum-based terminal state encoding for INV-5 compiler enforcement. |
| F4 | Error taxonomy — 15 error categories as exhaustive enum | §15.1 | S | F3 | `SIGNATURE_INVALID`, `FRESHNESS_EXPIRED`, `REPLAY_DETECTED`, `UNKNOWN_CRITICAL_EXTENSION`, `NEGOTIATION_BOUND_EXCEEDED`, `POLICY_DENIED`, `CAPABILITY_MISMATCH`, `COMMITMENT_CONFLICT`, `DELEGATION_CEILING`, `TRANSFER_TIMEOUT`, `TASK_EXPIRED`, `CAUSAL_REFERENCE_INVALID`, `CANONICALIZATION_ERROR`, `IDEMPOTENCY_CONFLICT`, `TRUST_UNKNOWN`. Exhaustive `match` forces all handlers to cover every case. |
| F5 | Invariant enforcement module (INV-1…INV-11) | §4 | M | F3, F4 | Each invariant → named check function + test. INV-5 is type-level; INV-9/INV-10/INV-11 are runtime guards. |

#### Identity & Capability

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F6 | Principal identity (long-lived Ed25519 keypair) | §5.1 | S | F2 | Keypair generation, persistence format, principal ID derivation. |
| F7 | Instance identity (per-process ephemeral key, signed by principal) | §5.2 | M | F6 | Delegation certificate from principal → instance. Triple identity (INV-1) composition. |
| F8 | Authority scope encoding | §5.3 | S | F7 | Scopes bound to instance certs. |
| F9 | Federation trust stub (hardcoded trust list) | §5.4 | S | F6 | Real registry is federation-specific; v1 ships pluggable trait + static allowlist. |
| F10 | Agent Card: build, publish, fetch, version-pin | §6.1, §6.3, §6.4 | M | F2, F7 | Card self-signature fixed in v0.5.1 fork to use federation credential (not circular). Version pinning for commit binding. |
| F11 | Capability claim classes & matching | §6.2 | M | F10 | Claim taxonomy + matching predicate for request/propose validation. |

#### Messages & Envelope

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F12 | Envelope encode / decode / validate with mandatory signatures | §7.1, INV-10 | L | F1, F2, F5 | Rejects unsigned messages at decode time. Recipient binding added in v0.5.1 fork. |
| F13 | Message class: `announce` | §8.1 | S | F12 | Body schema defined in v0.5.1 fork. |
| F14 | Message class: `describe` | §8.2 | S | F12 | Card reference + capability exchange. |
| F15 | Message class: `ack` with semantic ack disposition | §8.3, §7.4 | M | F12 | §9.6 conflation with terminal states disambiguated in fork. |
| F16 | Message class: `request` | §8.4 | S | F12 | Task instantiation trigger. |
| F17 | Message class: `propose` (proposal + counter-proposal body schema) | §8.5, §10.2, §10.3 | M | F12 | Body schema undefined in v0.5; written before coding. |
| F18 | Message class: `commit` with capability snapshot binding | §8.6, §11.1, §11.2 | L | F12, F10, F11 | Capability snapshot binding resolves card-version rule contradiction per fork. |
| F19 | Message class: `deliver` (including `interim: true`) | §8.7, §7.3 | M | F12 | Body inspection required; spec §7.3 claim corrected in fork. |
| F20 | Message class: `delegate` (three forms) | §8.8, §12.3, §12.4 | L | F12, F29 | See F29. |
| F21 | Message class: `control` (cancel, nudge, etc., with target in body) | §8.9, §16.3 | M | F12 | Body-carried target per fork correction. |
| F22 | All 11 causal relations: `in_reply_to`, `responds_to`, `acks`, `refines`, `supersedes`, `commits_to`, `delivers_for`, `delegates`, `cancels`, `depends_on`, `derives_from` | §7.3 | M | F12 | Relation type enum + validation that referenced message exists, is fresh, and is causally consistent. |

#### State Machines

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F23 | Task FSM: `REQUESTED → COMMITTED → COMPLETED \| FAILED \| CANCELLED \| REJECTED \| EXPIRED` | §9.5 | L | F3, F4 | Enum + transition table. Terminal states type-level (INV-5). Fixes competing-instance commit race (INV-5 violation) per fork. |
| F24 | Conversation FSM: `OPEN → CLOSED` | §9.7 | S | F23 | Closes on terminal task aggregation. |
| F25 | Terminal precedence rule (delivery-wins default + conditional-lapse override) | §9.6 | M | F23 | Fork fix: conditional-lapse beats delivery-wins; EXPIRED-vs-in-flight-deliver handled. |
| F26 | Model-checked FSM via `proptest` + `stateright` | §9.5 | L | F23, F24 | Validates no invalid transitions under arbitrary interleaving. |

#### Negotiation, Commitment, Delegation

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F27 | Negotiation engine: proposals, counter-proposals, partial acceptance, round limit (INV-11, default 20) | §10, INV-11 | L | F17, F18 | Round counting under supersession clarified in fork. |
| F28 | Commit binding: no silent widening, capability snapshot, conditional commitments | §11.2–§11.4 | L | F18, F11 | Hash of (proposal + commitment + card version) forms binding. |
| F29 | Delegation forms: assist, subtask, transfer (with transfer timeout reversion) | §12.3, §12.4, §12.5 | XL | F20, F23 | Three distinct semantics. Transfer timeout vs in-flight delegate commit race fixed in fork. Delegation ceiling (max depth) enforced. |
| F30 | Silent subcontracting prohibition check | §12.7, INV-6 | M | F29 | Every delegation must emit a `delegate` message; receiver verifies provenance chain. |

#### Freshness, Replay, Supersession

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F31 | Freshness window enforcement | §13.1 | S | F12 | Configurable TTL; rejects stale envelopes with `FRESHNESS_EXPIRED`. |
| F32 | Replay cache with idempotency keys | §13.2 | M | F12, F31 | Bounded LRU keyed by (sender, idempotency_key). Collision surface hardened in fork. |
| F33 | Supersession resolution | §13.3 | M | F22, F27 | `supersedes` relation handling inside negotiation rounds. |
| F34 | Retransmission vs semantic retry distinction | §13.4 | S | F32 | Retransmission uses same idempotency key; retry uses new one. |

#### Provenance

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F35 | Deterministic provenance graph construction | §14.1, §14.2 | L | F22, F1 | DAG built from causal relations; canonical serialization for signing. |
| F36 | Provenance canonicalization | §14.3 | M | F35, F1 | RFC 8785 over graph in topological order. |
| F37 | Minimal provenance principle + redaction | §14.4, §14.5 | M | F35 | Verifiable after redaction — Merkle-style hashing of redacted nodes. |
| F38 | Signed terminal provenance reports | §14.5 | M | F35, F2 | Final report co-signed with task terminal state. |
| F39 | Artifact immutability + `sha256:<hex>` content addressing | §3.6, INV-7 | S | F1 | Hex encoding locked in fork (spec under-specified). |

#### Extensions

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F40 | Extension registry with critical/non-critical flag (INV-9 fail-closed) | §17, INV-8, INV-9 | M | F12, F4 | Unknown critical extension → reject envelope. Non-critical → ignore. Extension containment (INV-8) limits which object types extensions can mutate. |

#### Transport

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F41 | `Transport` trait (send, recv, address resolution) | §18 | S | F12 | Keeps libp2p/NATS/WebSocket viable as later bindings. |
| F42 | `MemoryTransport` (in-process, for tests) | §18 | S | F41 | ~50 lines; enables microsecond-speed full-flow tests. |
| F43 | `HttpTransport` (HTTP/1.1 + JSON over TLS, reference wire) | §18 | L | F41, F12 | `axum` or `hyper`. The wire reference other implementations point at. |

#### Conformance & Test Infrastructure

| # | Feature | Spec § | Complexity | Depends On | Notes |
|---|---------|--------|------------|------------|-------|
| F44 | Conformance test vectors (language-neutral JSON fixtures) | §19 | L | F1, F2, F12 | Byte-exact canonicalization and signature outputs. The interop contract future implementations must hold us to. |
| F45 | Adversarial test suite | §19 | L | all above | Replay, stale commit, canonical divergence, silent delegation, competing commits, round overflow. |
| F46 | Two-node integration test (negotiate → commit → delegate → deliver, + cancellation races) | §19 | M | F42 or F43, F27, F29 | Full happy path + known race corners from review findings. |
| F47 | Level 2 (Conversational) conformance badge | §19 | S | F13–F17, F22 | Self-certification script running a defined vector subset. |
| F48 | Level 3 (Task-capable) conformance badge | §19 | S | F23, F27, F28, F47 | Adds task + negotiation vectors. |

---

### Differentiators (Worth Building, Not Required for Conformance)

Developer-experience features that make FAMP adoptable beyond the spec minimum. Deferred until table stakes green.

| # | Feature | Value Proposition | Spec § | Complexity | Depends On | Notes |
|---|---------|-------------------|--------|------------|------------|-------|
| D1 | State machine visualizer / debug tracer | Developers get lost in task+conversation+negotiation interleaving; a replay-able trace with FSM overlays turns 2-hour debug sessions into 5-minute ones | §9.5, §9.7 | M | F23, F24 | Emit JSON trace events; simple HTML/DOT renderer. |
| D2 | Conversation graph inspector | Visualize causal DAG and provenance graph for a given conversation; invaluable for interop debugging | §7.3, §14 | M | F22, F35 | Reuses canonical provenance graph. |
| D3 | Policy evaluation stub (Google CEL integration) | §3.7 Policy object is an expression language escape hatch; CEL is the de-facto cross-language policy language and unblocks real deployments | §3.7 | L | F3 | Feature-flagged crate; CEL parser via `cel-interpreter`. |
| D4 | Artifact content-addressed store abstraction | §3.6 artifacts need a place to live; trait + local filesystem + S3 impls cover most real usage | §3.6, §14 | M | F39 | `ArtifactStore` trait; `FsStore`, `S3Store`. |
| D5 | Agent builder / high-level agent harness | Raw library is type-heavy; a "typed bot" harness lowers the barrier for first 10 users | — | L | F12, F23, F27 | Opinionated wrapper collapsing common patterns (announce → describe → accept request → negotiate → commit → deliver). |
| D6 | Benchmark suite (criterion.rs) | Publishes p50/p99 envelope sign/verify/canonicalize numbers; future regressions caught automatically | — | S | F1, F2, F12 | `cargo bench`; CI gate on regression. |

---

### Anti-Features (Explicitly NOT Building in v1)

Each item below has been considered and rejected. Documented so scope creep attempts are met with a citation.

| Feature | Why Deferred | Spec Reference | Alternative |
|---------|--------------|----------------|-------------|
| **Multi-party commitment aggregation** | Spec §23 Q1 explicitly defers; semantics for N-party commit are unresolved upstream. Bilateral commitment is sufficient for v1 validation. | §23 Q1 | Bilateral commit only; document workaround as chained bilateral commits. |
| **Cross-federation delegation** | Spec §23 Q3 — peering semantics between federations are undefined. Building it now would invent protocol, not implement it. | §23 Q3 | Single federation per deployment; federation trust stub is intra-fed only. |
| **Streaming (token-by-token) deliver** | Spec §23 Q2 — streaming envelope semantics unresolved. `interim: true` deliveries (F19) cover the real use cases. | §23 Q2, §8.7 | Emit multiple `deliver` messages with `interim: true` until final. |
| **Economic / reputation / payment layers** | Spec §21 exclusion. Out of protocol scope by design; entangling payment semantics with commitment semantics is the mistake every prior agent protocol made. | §21 | External settlement layer; FAMP commitments reference external payment IDs via artifacts. |
| **Cognitive trace disclosure** | Spec §21 exclusion. Exposing agent internal reasoning as a protocol-level concept creates privacy, safety, and adversarial surface with no offsetting interop benefit. | §21 | Optional extension in a future version, behind INV-9 critical flag. |
| **Agent lifecycle management (start/stop/upgrade/monitor)** | Spec §21 exclusion. Orthogonal to messaging; every deployment has its own orchestrator (k8s, systemd, Nomad). | §21 | Deployment-tool concern; document integration notes only. |
| **Tool API standardization** | Spec §21 exclusion. Tools are per-agent internal concerns; standardizing would duplicate MCP/OpenAI function-calling without interop gain. | §21 | Agents describe tool outcomes via capability claims + artifacts, not tool schemas. |
| **Python / TypeScript bindings** | PROJECT.md explicit out-of-scope for v1. Core Rust correctness must be proven first; FFI without a stable core propagates bugs across languages. | — | Post-v1 milestone via PyO3 / wasm-bindgen. Keep FFI surface clean but unwired. |
| **Additional transports (libp2p, NATS, WebSocket)** | PROJECT.md out-of-scope for v1. `Transport` trait keeps them viable without the maintenance burden of shipping them unproven. | §18 | Third parties can implement `Transport` against their preferred substrate. |
| **Level 1-only release** | PROJECT.md key decision. Level 1 alone doesn't exercise signature discipline against real flows; tempts fragile downstream builds. | §19 L1 | Ship L2+L3 together or not at all. |
| **Production deployment tooling (dashboards, packaging, observability)** | PROJECT.md out-of-scope. Library-first; ops concerns are deployment-specific. | — | Optional `tracing` spans throughout; consumer wires their own backend. |
| **Real federation trust registry** | PROJECT.md out-of-scope. Production trust infra is federation-specific (OIDC, X.509, web-of-trust, etc.). | §5.4 | `FederationTrust` trait + hardcoded-list impl for v1; real impls live in consumer crates. |

---

## Feature Dependencies

```
F1 (RFC 8785 canonical JSON)
  ├──> F2 (Ed25519 sign/verify)
  │      ├──> F6 (Principal) ──> F7 (Instance) ──> F8 (Authority scope)
  │      │                             └──> F9 (Federation trust stub)
  │      │                             └──> F10 (Agent Card) ──> F11 (Capability claims)
  │      └──> F12 (Envelope) ──> F13–F21 (Message classes)
  │                                   │
  │                                   ├──> F22 (Causal relations)
  │                                   ├──> F31 (Freshness) ──> F32 (Replay cache) ──> F34
  │                                   ├──> F33 (Supersession)
  │                                   ├──> F40 (Extension registry)
  │                                   └──> F41 (Transport trait) ──> F42 (Memory) / F43 (HTTP)
  ├──> F35 (Provenance graph) ──> F36 (Canonicalization) ──> F37 (Redaction) ──> F38 (Signed report)
  ├──> F39 (Artifact addressing)
  └──> F44 (Conformance vectors)

F3 (Core types) ──> F4 (Error taxonomy) ──> F5 (Invariants)

F23 (Task FSM) ──> F24 (Conv FSM) ──> F25 (Terminal precedence) ──> F26 (Model checker)

F17 (propose) + F18 (commit)
  ├──> F27 (Negotiation engine, INV-11)
  └──> F28 (Commit binding)

F20 (delegate) + F23 (Task FSM) ──> F29 (Delegation forms) ──> F30 (Silent subcontract check)

All above ──> F45 (Adversarial suite) ──> F46 (Two-node integration) ──> F47, F48 (Conformance badges)
```

### Key Dependency Notes

- **F1 is the root of the DAG.** Nothing signs, interops, or conforms without byte-exact canonicalization. If F1 slips, everything slips.
- **F12 (Envelope) gates all 9 message classes.** The envelope decoder/validator must be done before any message-specific body logic.
- **F23 (Task FSM) gates F27, F28, F29.** Negotiation and commitment only make sense once the state a commit moves a task into is type-level correct.
- **F29 (Delegation) is the biggest single feature (XL).** Three delegation forms × transfer-timeout race × delegation ceiling × silent-subcontract check. Expect it to take longer than estimated.
- **F44 (Conformance vectors) must be generated early.** They are both the output of foundation work and the input to every later test. Ship them as soon as F1+F2+F12 stabilize.
- **F26 (Model checker) depends on F23+F24** and should be wired up before F29 lands, because delegation's race conditions are exactly what model checking catches.

---

## Coverage Check (Quality Gate)

### Every first-class protocol object from §3 represented

| §3 Object | Feature |
|-----------|---------|
| 3.1 Agent | F6, F7, F10 |
| 3.2 Conversation | F3, F24 |
| 3.3 Task | F3, F23 |
| 3.4 Proposal | F17, F27 |
| 3.5 Commitment | F18, F28 |
| 3.6 Artifact | F39, (D4) |
| 3.7 Policy | F3, (D3) |

### Every message class from §8 represented

| §8 Class | Feature |
|----------|---------|
| 8.1 announce | F13 |
| 8.2 describe | F14 |
| 8.3 ack | F15 |
| 8.4 request | F16 |
| 8.5 propose | F17 |
| 8.6 commit | F18 |
| 8.7 deliver | F19 |
| 8.8 delegate | F20 |
| 8.9 control | F21 |

### Every invariant from §4 has an enforcement feature

| Invariant | Enforced By |
|-----------|-------------|
| INV-1 Triple identity | F7 (principal+instance+authority composition) |
| INV-2 Explicit causality | F22 (all causal relations mandatory where applicable) |
| INV-3 No implied commitment | F28 (commit is only binding act) |
| INV-4 Bounded tasks | F23 (task FSM has terminal states; deadlines) |
| INV-5 Single terminal state | F23 (enum-typed terminal state, compiler-checked) + F25 (precedence rule) |
| INV-6 Delegation visibility | F30 (silent subcontract prohibition check) |
| INV-7 Artifact immutability | F39 (content-addressed SHA-256) |
| INV-8 Extension containment | F40 |
| INV-9 Unknown-critical fail-closed | F40 |
| INV-10 Mandatory signatures | F12 (decode rejects unsigned) + F2 |
| INV-11 Negotiation bounds | F27 (round counter with default=20) |

### All §23 Open Questions bucketed as anti-features for v1

| §23 Q | Status |
|-------|--------|
| Q1 Multi-party commitment | Anti-feature |
| Q2 Streaming deliver | Anti-feature |
| Q3 Cross-federation delegation | Anti-feature |

---

## MVP Definition

### Launch With (v1 = Level 2 + Level 3 Conformance)

All 48 table-stakes features (F1–F48). No differentiator is in the MVP cut. This is non-negotiable because each feature corresponds to an observable conformance requirement; removing any one fails either L2 or L3 certification.

### Add After Validation (v1.x)

Differentiators in priority order, triggered by first real adopter feedback:

- [ ] **D6 Benchmark suite** — trivial to add, useful immediately, locks in perf baseline before drift
- [ ] **D1 State machine tracer** — first adopter will demand this within the first week
- [ ] **D2 Conversation graph inspector** — first interop-debugging session motivates it
- [ ] **D4 Artifact content-addressed store** — needed once a real deployment stores non-trivial artifacts
- [ ] **D3 Policy evaluation (CEL)** — needed once two agents need machine-readable policy negotiation
- [ ] **D5 Agent builder harness** — needed when adopter count > 3 and onboarding friction dominates

### Future Consideration (v2+)

- [ ] Python / TypeScript bindings (separate milestone)
- [ ] Additional transport bindings (libp2p, NATS, WebSocket)
- [ ] Revisit §23 open questions after upstream spec resolves them

---

## Feature Prioritization Matrix (Differentiators Only)

Table stakes are all P1 by definition; matrix is meaningful only for the differentiator set.

| Feature | Adopter Value | Impl Cost | Priority |
|---------|---------------|-----------|----------|
| D1 State machine tracer | HIGH | MEDIUM | P2 |
| D2 Conversation graph inspector | HIGH | MEDIUM | P2 |
| D3 Policy evaluation (CEL) | MEDIUM | HIGH | P3 |
| D4 Artifact content-addressed store | MEDIUM | MEDIUM | P2 |
| D5 Agent builder harness | HIGH | HIGH | P3 |
| D6 Benchmark suite | MEDIUM | LOW | P2 |

---

## Sources

- `FAMP-v0.5-spec.md` — primary source; every table-stakes feature cites its governing section
- `.planning/PROJECT.md` — scope boundaries, key decisions, out-of-scope list
- Prior review findings (loaded in conversation history) — drove several fork-corrected features (F1 RFC 8785 lock, F2 domain separation, F12 recipient binding, F15 ack disambiguation, F25 precedence fix, F28 capability snapshot binding, F32 idempotency hardening, F39 artifact encoding)

---
*Feature research for: FAMP v0.5 Rust reference implementation (Level 2 + Level 3 conformance)*
*Researched: 2026-04-12*
