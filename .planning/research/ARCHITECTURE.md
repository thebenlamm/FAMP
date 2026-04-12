# FAMP Rust Reference — Architecture

**Confidence:** HIGH for structure; MEDIUM for trait shapes (firm up during Phase 1 spec fork).

## Summary

Single cargo workspace with **12 library crates + 1 umbrella crate**. Over-decomposition was reduced from the original 14-15 crate sketch: `negotiate` + `delegate` + `provenance` merged into `famp-protocol`, `transport` trait + `MemoryTransport` merged into one crate, FSM model-checking lives under `#[cfg(test)]` in `famp-fsm`. Public API flows through a single `famp` umbrella crate re-exporting the common types plus hosting the `famp` CLI.

## Final Crate List (12 + umbrella)

| # | Crate | Purpose | Spec |
|---|-------|---------|------|
| 1 | `famp-core` | Shared types (`MessageId`, `ArtifactId`, `Principal`, `Instance`), error enum, invariant constants (INV-1..11), trait-free value objects | §3, §4 |
| 2 | `famp-canonical` | RFC 8785 JCS encoder + decoder, byte-exact serialization, property tests | §7 canonical form |
| 3 | `famp-crypto` | Ed25519 sign/verify with domain-separation prefix, keypair mgmt, base64url unpadded encoding. Exposes only `verify_strict`. | §5, §7 |
| 4 | `famp-envelope` | Envelope schema, encode/decode/validate, mandatory signature enforcement (INV-10), body schemas for all 9 message classes | §7, §8 |
| 5 | `famp-identity` | `Principal`/`Instance`, Agent Card (with federation credential), trust registry stub, capability snapshot binding | §5, §6 |
| 6 | `famp-causality` | Causal relations, semantic ack, freshness windows, replay cache, supersession logic | §7.3, §7.4, §13 |
| 7 | `famp-fsm` | Merged conversation FSM + task FSM + `proptest`/`stateright` models under `#[cfg(test)]`. Compiler-checked terminal states via exhaustive enum `match`. INV-5 enforcement. | §8, §9, §16 |
| 8 | `famp-protocol` | Merged negotiation + commitment + delegation + provenance. These share the FSM deeply and would churn if split. | §10–§12, §14 |
| 9 | `famp-extensions` | Critical/non-critical registry, INV-9 fail-closed, `Extension` trait | §17 |
| 10 | `famp-transport` | `Transport` trait + `MemoryTransport` impl (in-process, ~50 LOC). | §18 |
| 11 | `famp-transport-http` | `axum`-based reference HTTP/1.1 + JSON + TLS binding. Separate because it pulls heavy deps. | §18 |
| 12 | `famp-conformance` | Integration tests, language-neutral JSON fixture vectors, adversarial suite, two-node happy-path tests. | §19 |
| 13 | `famp` (umbrella) | Public re-exports + `famp` CLI binary + `examples/` | — |

### Merges Rationale

- **negotiate + delegate + provenance → famp-protocol**: Tight coupling (commit binds proposal, delegation wraps commit, provenance records both). Spec §10–§14 reference each other. Split forces premature API stabilization that will churn in Phase 6. Re-split in v2 if any concern grows past ~3k LOC.
- **fsm + model checking**: `proptest`/`stateright` tests live next to the state machines they check. A separate `famp-fsm-model` crate would be `#[cfg(test)]`-only with no runtime consumers.
- **transport trait + MemoryTransport**: Trait-only crates add build-graph nodes without value. MemoryTransport is a dev dep for every crate above — belongs in the same crate as the trait.

### PITFALLS-driven caveat

Phase-0 build-time reality check: a 12-crate workspace may spiral incremental build times for a beginner. Consider a **staged decomposition** — start Phases 2–3 with `famp-foundation` (merged core + canonical + crypto + envelope) and split into the 4 final crates at Phase 4 boundary once public APIs are stable. Final target is still 12 + umbrella; the staging is a beginner-velocity concession.

## Dependency DAG (acyclic)

```
                         famp-core
                         /   |   \
                        /    |    \
               famp-canonical |   famp-crypto
                        \    |    /
                         \   |   /
                        famp-envelope
                        /    |    \
                       /     |     \
              famp-identity  |  famp-causality
                       \     |     /
                        \    |    /
                        famp-fsm
                            |
                       famp-protocol  ──────  famp-extensions
                            |                        |
                            └────────┬───────────────┘
                                     |
                              famp-transport
                                     |
                            famp-transport-http
                                     |
                              famp-conformance
                                     |
                                  famp (umbrella + CLI)
```

**Invariants:**
- `core` is root (no FAMP deps)
- `canonical` and `crypto` are parallel utility leaves
- `envelope` is the first "fat" crate; all downstream flows through it
- `identity` and `causality` are parallel peers on `envelope`
- `fsm` joins them
- `protocol` sits above `fsm` — the biggest logic crate
- `extensions` is orthogonal — depends on `envelope` only (fail-closed on any incoming message)
- `transport` depends on `envelope` + `core` only — intentionally does NOT depend on `protocol` so transports stay dumb pipes
- `conformance` depends on everything; `famp` umbrella re-exports

No cycles.

## Key Traits and Owning Crates

| Trait | Crate | Purpose |
|-------|-------|---------|
| `Canonicalize` | `famp-canonical` | `fn canonicalize(&self) -> Vec<u8>` — byte-exact JCS |
| `Signer` / `Verifier` | `famp-crypto` | Abstract over key source (HSM later); default `Ed25519Signer` |
| `BodySchema` | `famp-envelope` | Each message class implements for validation |
| `AgentCardStore` | `famp-identity` | Pluggable card lookup (in-mem default, federation registry later) |
| `ReplayCache` | `famp-causality` | `insert(msg_id, expires_at) -> bool` |
| `StateMachine<S, E>` | `famp-fsm` | Generic FSM with compile-time terminal-state check; concrete: `ConversationFsm`, `TaskFsm` |
| `ProvenanceStore` | `famp-protocol` | Append-only graph, redaction-aware reads |
| `Extension` | `famp-extensions` | `id()`, `is_critical()`, `validate(&Envelope)` |
| `Transport` | `famp-transport` | `async fn send(Envelope) -> Result<()>`, `fn incoming() -> Stream<Envelope>` |

**Design note:** Small traits per concern. A single fat `FampNode` trait is tempting but makes mocking and incremental Phase builds painful.

## Commit Happy Path Data Flow

Trace: Agent A proposes → B counter-proposes → A commits → B delivers.

```
A.propose
  1. protocol::Negotiator::propose(proposal_body)
       └─ builds Envelope (envelope crate)
           └─ canonicalize body (canonical crate)
           └─ compute signature (crypto crate, domain-sep prefix)
           └─ attach causal.prev = None
  2. transport::send(envelope)                    [wire: A → B]

B receives
  3. transport incoming → ingest pipeline:
       a. envelope::decode + signature verify (crypto)
       b. extensions::check_critical (fail-closed if unknown critical ext)
       c. causality::replay_cache.insert (reject dupes)
       d. identity::resolve_sender (agent card lookup)
       e. fsm::ConversationFsm.step(Event::Propose) → new state
       f. protocol::Negotiator.receive_propose → stores proposal
  4. Application layer produces counter-proposal
  5. Same pipeline in reverse (B → A)

A commits
  6. protocol::Committer.commit(proposal_id, terms)
       ├─ FSM check: must be in Negotiated state (compile-time enum match)
       ├─ INV-5 guard: single terminal state
       ├─ build commit envelope, sign, canonicalize
       └─ transport::send

B receives commit
  7. Pipeline (a–d) as above
  8. fsm::TaskFsm.step(Event::Commit) → Committed state
  9. protocol::Committer.receive_commit → binds to original proposal
  10. protocol::Provenance.append(commit_node)

B delivers
  11. protocol::Delivery.build_deliver(task_id, artifact)
  12. Sign, send, provenance append
  13. A receives → TaskFsm.step(Event::Deliver) → Delivered (terminal)
  14. INV-5 enforced: no other terminal state can fire
```

**Choke points:**
- Signatures verified exclusively in `envelope::decode` (INV-10)
- FSM transitions fire in `famp-fsm`, always after envelope + extensions validation
- Canonical form computed only in `famp-canonical`
- Replay protection in `famp-causality`, immediately after signature verify
- Provenance writes in `famp-protocol` after FSM transition succeeds

## Build Order (Phase-Aligned)

| Phase | Crates | Gate |
|-------|--------|------|
| **P0** Toolchain | (scaffold) | `cargo build` empty workspace |
| **P1** Spec fork | (docs) | `FAMP-v0.5.1-spec.md` diff reviewed |
| **P2** Foundations | core, canonical, crypto | RFC 8785 test vectors pass; Ed25519 round-trip |
| **P3** Envelope | envelope | Sign → serialize → parse → verify for all 9 classes |
| **P4** Identity + Causality | identity, causality | Agent Card validated; replay cache works; freshness enforced |
| **P5** FSM | fsm + model check | `stateright` exhausts reachable states; INV-5 proved |
| **P6** Protocol | protocol, extensions | Negotiate → commit → deliver → provenance happy path (MemoryTransport) |
| **P7** Transport | transport, transport-http | Two-node HTTP integration test |
| **P8** Conformance | conformance, famp umbrella + CLI | Fixture vectors pass; adversarial suite green |

## Public API Surface

Single umbrella crate `famp` re-exports the common types.

```rust
use famp::{Node, AgentCard, Envelope, Principal, Instance};
use famp::transport::HttpTransport;
use famp::protocol::{Negotiator, Committer};
use famp::fsm::{ConversationState, TaskState};

let transport = HttpTransport::bind("0.0.0.0:8443").await?;
let node = famp::Node::builder()
    .identity(my_agent_card)
    .signer(ed25519_signer)
    .transport(transport)
    .build()?;
node.run().await?;
```

Feature flags in umbrella toggle heavy deps: `features = ["http"]` pulls `famp-transport-http`; default is `memory` only. Power users can depend on sub-crates directly. Internal refactors don't break consumer code.

## Binary Targets

Ship a CLI `famp` binary hosted in the umbrella crate:

```
famp keygen
famp card new --principal alice.example
famp card verify <card.json>
famp envelope sign <body.json> --key ...
famp envelope verify <envelope.json>
famp canonical <input.json>          # debug aid
famp serve --bind 0.0.0.0:8443
famp fixture run <fixture.json>
```

**Examples** (`famp/examples/`):
- `memory_two_node.rs` — full negotiate→commit→deliver in one process
- `http_node.rs` — minimal HTTP server boot
- `custom_extension.rs` — implementing the `Extension` trait

## Testing Layout

- **Unit tests**: `#[cfg(test)] mod tests` in each crate
- **Property tests**: `proptest` in canonical (JCS round-trip), crypto (sign/verify), envelope (parse/encode fidelity)
- **Model checking**: `stateright` + `proptest` in `famp-fsm` under `#[cfg(test)]`
- **Integration tests**: `famp-conformance/tests/` — two-node, cancellation races, language-neutral JSON fixture vectors (`fixtures/*.json`)
- **Adversarial suite**: `famp-conformance/tests/adversarial/` — one file per attack class
- **Benchmarks**: `famp-conformance/benches/` using `criterion`

## Phase-to-Crate Mapping

| Roadmap Phase | Crates | Deliverable |
|---------------|--------|-------------|
| Phase 0: Toolchain | (scaffold) | rustup installed, workspace builds |
| Phase 1: Spec fork | (docs) | `FAMP-v0.5.1-spec.md` |
| Phase 2: Crypto foundations | core, canonical, crypto | Byte-exact JCS + Ed25519 |
| Phase 3: Envelope | envelope | Signed message round-trip for all 9 classes |
| Phase 4: Identity + causality | identity, causality | Agent Cards, replay cache, freshness |
| Phase 5: State machines | fsm | Model-checked FSMs |
| Phase 6: Protocol logic | protocol, extensions | Negotiate/commit/deliver/delegate/provenance |
| Phase 7: Transports | transport, transport-http | Two-node HTTP |
| Phase 8: Conformance + CLI | conformance, famp umbrella + CLI | Shipping v1 |

## Deviations from Initial Sketch

1. Merged `negotiate` + `delegate` + `provenance` → `famp-protocol`
2. No separate `famp-transport-memory` — merged with `famp-transport`
3. No separate FSM model-checking crate — lives in `famp-fsm` under `cfg(test)`
4. Added `famp` umbrella crate hosting CLI + re-exports
5. Phase-2/3 may temporarily merge crates for beginner build-time relief, split by Phase 4

## Open Questions

- Exact canonicalization behavior for Unicode supplementary-plane sort — RFC 8785 §3.2.3 mandates UTF-16 code-unit order; Rust `BTreeMap` uses UTF-8 byte order. `famp-canonical` must handle this explicitly from commit one.
- Whether `HttpTransport` should land before or after delegation — depends on whether two-node integration tests use HTTP or Memory as default harness.
- Agent Card self-signature fix location — spec fork (§6.1) vs new federation-credential section (§5.5).
