# FAMP — Federated Agent Messaging Protocol (Reference Implementation)

## What This Is

A Rust reference implementation of FAMP (Federated Agent Messaging Protocol) v0.5 — a protocol defining semantics for communication among autonomous AI agents within a trusted federation. The implementation provides a conformance-grade library covering identity, causality, negotiation, commitment, delegation, and provenance across three protocol layers, plus a reference HTTP transport binding.

## Core Value

**A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.** If canonicalization or signature verification disagrees, nothing else matters.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Fork spec to `FAMP-v0.5.1-spec.md` and resolve identified ambiguities/bugs (canonical JSON, body schemas, state-machine holes)
- [ ] Rust toolchain bootstrap (install rustup, pin toolchain, workspace scaffold)
- [ ] `famp-canonical` crate implementing RFC 8785 JCS canonical JSON
- [ ] `famp-crypto` crate wrapping Ed25519 sign/verify with domain separation
- [ ] `famp-core` types, errors, invariants (INV-1 through INV-11)
- [ ] `famp-envelope` encode/decode/validate with mandatory signatures
- [ ] `famp-identity` principal/instance, Agent Card, federation trust stub
- [ ] `famp-causality` relations, semantic ack, freshness, replay cache
- [ ] `famp-fsm` conversation + task state machines (compiler-checked terminal states)
- [ ] `famp-negotiate` proposals, counter-proposals, commit binding, round limits
- [ ] `famp-delegate` three delegation forms, transfer timeout, delegation ceiling
- [ ] `famp-provenance` deterministic provenance graph, redaction, signed terminal reports
- [ ] `famp-extensions` critical/non-critical registry, INV-9 fail-closed
- [ ] `famp-transport` trait + `MemoryTransport` (in-process) + `HttpTransport` (reference wire)
- [ ] Conformance test vectors published as language-neutral JSON fixtures
- [ ] State machine model checking via `proptest` + `stateright`
- [ ] Adversarial test suite (replay, stale commit, canonical divergence, silent delegation, competing commits, round overflow)
- [ ] Two-node integration test exercising negotiate → commit → delegate → deliver happy path and cancellation races
- [ ] Level 2 (Conversational) + Level 3 (Task-capable) conformance

### Out of Scope

- **Level 1-only release** — tempts users to build on a base without signature discipline exercised against real flows
- **Python/TypeScript bindings in v1** — core must be proven first; bindings follow as separate milestone
- **Additional transports (libp2p, NATS, WebSocket)** — `Transport` trait leaves them open; only HTTP+JSON reference in v1
- **Multi-party commitment profiles** — spec §23 Q1 explicitly defers; bilateral only in v1
- **Cross-federation delegation** — spec §23 Q3; bilateral peering not defined
- **Streaming (token-by-token) deliver** — spec §23 Q2; `interim: true` deliveries sufficient for v1
- **Real federation trust registry** — stub with hardcoded trust list; production trust infra is federation-specific
- **Economic/reputation/payment layers** — spec §21 exclusions stand
- **Agent lifecycle management** (start/stop/upgrade/monitor) — out of protocol scope per §21
- **Production deployment tooling** (packaging, observability, dashboards) — library-first; ops concerns deferred

## Context

**Starting state:** Repository contains only `FAMP-v0.5-spec.md` (1178 lines, the protocol spec itself). No code, no git history, no Rust toolchain installed.

**Prior review findings (4 parallel review agents):** Loaded in conversation history. Key categories:

1. **Canonical JSON** is the #1 blocker — spec says "sorted keys, no whitespace" but doesn't reference RFC 8785 JCS. Without lockdown, two conformant implementations will produce different bytes and signature verification will fail. Must resolve in Phase 1 (spec fork).

2. **State-machine holes** — real spec bugs, not just under-specification:
   - §9.6 ack-disposition conflated with terminal-state crystallization
   - §7.3 "no body inspection" claim is false (`interim` flag, partial acceptance subset, control target live in body)
   - Transfer-timeout reversion vs. in-flight delegate commit race
   - EXPIRED vs. in-flight deliver not covered by delivery-wins default
   - INV-5 violated by competing-instance commits during resolution window
   - Conditional-lapse loses to delivery-wins (should win)
   - Negotiation round counting under supersession ambiguous
   - Capability snapshot binding contradicts card-version rule

3. **Under-specified body schemas** — `commit`, `propose`, `deliver`, `control` bodies undefined. Must write schemas before coding or implementations cannot interop.

4. **Security gaps** — no domain-separation prefix in signatures, no recipient binding, idempotency key collision surface, Agent Card self-signature is circular (needs federation credential), SHA-256 artifact encoding unspecified.

**Developer background:** User is new to Rust. Phase 0 covers toolchain install, workspace scaffold, and basic `cargo test` loop before any FAMP code is written.

**Why Rust:** Ed25519 and canonical JSON demand byte-exact behavior; `match` on enums makes INV-5 (single terminal state) and the task FSM compiler-checked in ways a Python/TS implementation can only approximate at runtime. One Rust core can later feed Python/TS/Go bindings via wasm-bindgen or PyO3.

## Constraints

- **Tech stack**: Rust (stable, latest). `ed25519-dalek` for signatures, `serde` + custom canonicalizer for RFC 8785 JCS, `proptest` + `stateright` for state-machine model checking, `axum` or `hyper` for HTTP transport reference.
- **Tech stack (deferred)**: No Python/TS bindings in v1; keep FFI surface clean but unwired.
- **Transport**: HTTP/1.1 + JSON over TLS as reference wire; in-process `MemoryTransport` for tests. Other transports live behind the `Transport` trait.
- **Conformance target**: Level 2 + Level 3 in one milestone. Level 1-only is explicitly not a release target.
- **Spec fidelity**: v0.5.1 fork is the authority for this implementation. All diffs from v0.5 documented with reviewer rationale.
- **Security**: Every message signed (INV-10); unsigned messages rejected. Ed25519 non-negotiable. Domain separation prefix added in v0.5.1 fork.
- **Developer onboarding**: Rust toolchain install is Phase 0; assume zero prior Rust experience.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Language: Rust | Compiler-checked invariants (INV-5 via enum `match`), byte-exact Ed25519 and canonical JSON, single core can feed future bindings | — Pending |
| Ship Level 2 + Level 3 together as v1 | Level 1 alone doesn't exercise signature discipline against real message flows; tempts fragile downstream builds | — Pending |
| Fork spec to v0.5.1 rather than write profile addendum | State-machine findings are real bugs not ambiguities; profile-that-contradicts-spec causes interop confusion | — Pending |
| Both `MemoryTransport` and `HttpTransport` in v1 | Memory transport is ~50 lines and makes full-flow tests run in microseconds; HTTP is the wire reference everyone points at | — Pending |
| RFC 8785 JCS for canonical JSON | Only widely-reviewed canonical JSON spec; avoids inventing our own corner-case rules for Unicode/number/duplicate-key handling | — Pending |
| Ed25519 key encoding: raw 32-byte pub / 64-byte sig, unpadded base64url | Matches `ed25519-dalek` defaults; simplest interop contract | — Pending |
| Artifact IDs: `sha256:<hex>` prefix scheme | SHA-256 is "RECOMMENDED" in spec; hex encoding is canonical and unambiguous | — Pending |
| Test strategy: conformance vectors → FSM model checking → adversarial suite → two-node integration | Each layer catches a distinct failure class; vectors are the interop contract future implementations hold us to | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-12 after initialization*
