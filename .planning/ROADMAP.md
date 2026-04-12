# Roadmap: FAMP v0.5 Rust Reference Implementation

**Created:** 2026-04-12
**Granularity:** standard
**Coverage:** 153/153 v1 requirements mapped
**Core Value:** A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.

## Phases

- [ ] **Phase 0: Toolchain & Workspace Scaffold** — Rust install, cargo workspace, CI skeleton (learning-budgeted)
- [ ] **Phase 1: Spec Fork v0.5.1** — Docs-only; resolve ambiguities, lock canonicalization, domain separation, body schemas
- [ ] **Phase 2: Canonical + Crypto Foundations** — RFC 8785 JCS, Ed25519 strict, core types, invariants (HIGHEST RISK)
- [ ] **Phase 3: Envelope + Message Schemas** — 9 message classes, mandatory signatures, signed round-trip
- [ ] **Phase 4: Identity + Causality** — Agent Cards, trust stub, replay cache, freshness, supersession
- [ ] **Phase 5: State Machines + Model Checking** — Conversation + Task FSMs, compile-time terminal-state enforcement, `stateright` exhaustive exploration
- [ ] **Phase 6: Protocol Logic + Extensions** — Merged negotiate/commit/delegate/provenance + extension registry (LARGEST LOGIC PHASE)
- [ ] **Phase 7: Transport (Memory + HTTP)** — `Transport` trait, `MemoryTransport`, reference `HttpTransport` over rustls
- [ ] **Phase 8: Conformance, Adversarial Suite, CLI** — Vector tests, attack suite, `famp` umbrella CLI, L2+L3 badges

## Phase Details

### Phase 0: Toolchain & Workspace Scaffold

**Goal**: A green `cargo build` + `cargo nextest run` on an empty 12-crate workspace, with strict lints and CI enforcing the loop on every push. User is new to Rust — budget generously for learning the `cargo` edit-build-test cycle before any FAMP code is written.

**Depends on**: Nothing (first phase)

**Requirements**: TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05, TOOL-06, TOOL-07

**Success Criteria** (what must be TRUE):
  1. `rustup` installed with version pinned via `rust-toolchain.toml`; `cargo --version` reproducible across machines
  2. Cargo workspace with 12 library crates + 1 umbrella (`famp`) scaffolded; `cargo build --workspace` succeeds on empty lib.rs stubs
  3. `just build`, `just test`, `just lint`, `just fmt` targets all green; `cargo-nextest` is the default test runner
  4. GitHub Actions CI runs fmt + clippy (strict, `unsafe_code = "forbid"`) + build + nextest on every push, green on main
  5. All crate versions pinned once via `[workspace.dependencies]`; no drift possible across crates

**Plans**: 3 plans
  - [x] 0-01-PLAN.md — Toolchain pin + repo hygiene (rust-toolchain.toml, licenses, README, .gitignore)
  - [ ] 0-02-PLAN.md — Workspace scaffold (root Cargo.toml with [workspace.dependencies] + lints, 13 crate stubs)
  - [ ] 0-03-PLAN.md — Justfile + nextest config + GitHub Actions CI workflow

---

### Phase 1: Spec Fork v0.5.1

**Goal**: `FAMP-v0.5.1-spec.md` exists with every ambiguity from the 4-reviewer audit resolved in writing, with a documented changelog from v0.5. No Rust code in this phase — output is pure documentation that locks the interop contract before anyone writes bytes against it.

**Depends on**: Phase 0 (repo + docs layout)

**Requirements**: SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-06, SPEC-07, SPEC-08, SPEC-09, SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14, SPEC-15, SPEC-16, SPEC-17, SPEC-18, SPEC-19, SPEC-20

**Success Criteria** (what must be TRUE):
  1. `FAMP-v0.5.1-spec.md` committed with a changelog section citing each review finding that drove a change
  2. Canonical JSON section explicitly references RFC 8785 JCS (not paraphrased) and documents UTF-16 sort and ECMAScript number formatting expectations
  3. Signature section includes a byte-level hex-dump worked example of the domain-separation prefix, and the signature binds the `to` field for recipient anti-replay
  4. Body schemas for all five message classes (`commit`, `propose`, `deliver`, `control`, `delegate`) are defined inline with field-level types and constraints
  5. All eight state-machine holes (§9.6, §7.3, transfer-timeout race, EXPIRED-vs-deliver, conditional-lapse precedence, competing-instance commits, supersession rounds, card-version drift) have explicit resolutions documented with rationale
  6. Concrete numeric defaults committed: clock skew tolerance (±60s), validity windows, idempotency key format (128-bit random, `(sender, recipient)` scope), artifact ID scheme (`sha256:<hex>`), Ed25519 encoding (raw bytes, unpadded base64url), and a spec-version constant string

**Plans**: TBD

> **Research flag**: This phase needs `/gsd:research-phase` before `/gsd:plan-phase` — spec-fork decisions drive the rest of v1 and must be validated against RFC 8785 edge cases + reviewer findings.

---

### Phase 2: Canonical + Crypto Foundations

**Goal**: A user can take any JSON value and get byte-exact RFC 8785 output, sign it with Ed25519, and watch a second implementation verify the signature — and CI proves this against externally-sourced test vectors. Ships `famp-core`, `famp-canonical`, `famp-crypto` (possibly merged as `famp-foundation` per ARCHITECTURE staging note).

**Depends on**: Phase 1 (spec locks canonical form + domain separation bytes)

**Requirements**: CANON-01, CANON-02, CANON-03, CANON-04, CANON-05, CANON-06, CANON-07, CRYPTO-01, CRYPTO-02, CRYPTO-03, CRYPTO-04, CRYPTO-05, CRYPTO-06, CRYPTO-07, CRYPTO-08, CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06

**Success Criteria** (what must be TRUE):
  1. RFC 8785 Appendix B test vectors all pass as a hard CI gate (not a warning, not skipped)
  2. RFC 8032 Ed25519 test vectors all pass in CI; supplementary-plane (emoji, CJK Ext B) UTF-16 sort vectors pass
  3. `famp-crypto` exposes only `verify_strict`; raw `verify` is inaccessible; weak/small-subgroup public keys are rejected at ingress with fixtures proving rejection
  4. Domain-separation prefix from SPEC-03 is applied before every sign; shipped as conformance vector #1 with hex dump
  5. cyberphone 100M-sample float corpus runs in CI (sampled or full); duplicate-key JSON inputs are rejected at parse
  6. `famp-core` exposes `Principal`, `Instance`, `MessageId` (UUIDv7), `ArtifactId` (`sha256:` prefix), typed error enum covering all 15 §15.1 categories, and invariant constants INV-1..INV-11
  7. Fallback plan for `serde_jcs` (~500 LoC from-scratch JCS) is documented in-repo, not just discussed

**Plans**: TBD

> **HIGHEST-RISK PHASE.** Research flag: `/gsd:research-phase` mandatory. Canonical-JSON and Ed25519 correctness are the interop contract — a bug here invalidates everything downstream. Front-load external vectors and be prepared to drop `serde_jcs` for the fallback if it fails conformance.

---

### Phase 3: Envelope + Message Schemas

**Goal**: Every FAMP message class can be built, signed, serialized, parsed, and verified round-trip — with unsigned messages hard-rejected and unknown fields refused. Ships `famp-envelope`.

**Depends on**: Phase 2 (canonical form + signatures)

**Requirements**: ENV-01, ENV-02, ENV-03, ENV-04, ENV-05, ENV-06, ENV-07, ENV-08, ENV-09, ENV-10, ENV-11, ENV-12, ENV-13, ENV-14, ENV-15

**Success Criteria** (what must be TRUE):
  1. Envelope struct matches spec §7.1 field-for-field; `deny_unknown_fields` enforced on envelope and every body schema
  2. Unsigned messages are rejected at decode with the INV-10 error; a test asserts this for every message class
  3. All 9 message classes (`announce`, `describe`, `ack`, `request`, `propose`, `commit`, `deliver`, `delegate`, `control`) have body structs implementing `BodySchema`, with signed round-trip tests (build → sign → encode → decode → verify) all green
  4. All 11 causal relations validated against their allowed message classes; invalid (class, relation) pairs rejected
  5. Scope enforcement (standalone / conversation / task) verified per message class against spec §7

**Plans**: TBD

---

### Phase 4: Identity + Causality

**Goal**: A user can publish an Agent Card, have it verified against a federation trust stub, and send messages that are correctly ordered, de-duplicated, and expired per the freshness window. Ships `famp-identity` and `famp-causality`.

**Depends on**: Phase 3 (envelope available to bind card lookups + replay cache)

**Requirements**: ID-01, ID-02, ID-03, ID-04, ID-05, ID-06, ID-07, CAUS-01, CAUS-02, CAUS-03, CAUS-04, CAUS-05, CAUS-06, CAUS-07

**Success Criteria** (what must be TRUE):
  1. Agent Card with federation credential parses, validates, and round-trips; card versioning with `card_version` + `min_compatible_version` enforced
  2. `TrustStore` and `AgentCardStore` traits exist with in-memory stub impls; card expiry enforced on fresh requests but grandfathered for in-flight commits per SPEC-06
  3. All 6 `ack` disposition values are processed semantically (not just stored); replay cache keyed on `(id, idempotency_key, content_hash)` rejects duplicates within bounded memory
  4. Freshness window table from spec §13.1 enforced per message class; stale messages rejected with the correct error category
  5. Supersession restricted to original sender; void-prior semantics and UUIDv7-vs-`ts` cross-validation tested

**Plans**: TBD

---

### Phase 5: State Machines + Model Checking

**Goal**: The conversation and task FSMs are compile-time safe (exhaustive `match` on owned enum states, INV-5 enforced at the type level) and `stateright` exhaustively explores the reachable state space in CI without finding an invariant violation. Ships `famp-fsm`.

**Depends on**: Phase 4 (identity + causality events feed FSMs)

**Requirements**: FSM-01, FSM-02, FSM-03, FSM-04, FSM-05, FSM-06, FSM-07, FSM-08

**Success Criteria** (what must be TRUE):
  1. `ConversationFsm` (OPEN → CLOSED) and `TaskFsm` (REQUESTED, COMMITTED, COMPLETED, FAILED, CANCELLED, REJECTED, EXPIRED) compile with exhaustive `match`; adding a state without updating handlers is a compile error
  2. Transitions driven by the `(class, relation, terminal_status, current_state)` tuple, with owned state types (zero lifetimes in state enums)
  3. `stateright` explores every reachable `(state × event)` pair under `#[cfg(test)]` and finds no INV-5 violations
  4. `proptest` property tests assert transition legality across random event sequences
  5. Terminal precedence rule from SPEC-09 (ack-disposition vs terminal-state-crystallization) is enforced and tested against the ambiguous cases named in the spec fork

**Plans**: TBD

> **Research flag**: `/gsd:research-phase` recommended — `stateright` ergonomics and FSM lifetime/ownership design decisions warrant upfront exploration to avoid mid-phase rewrites.

---

### Phase 6: Protocol Logic + Extensions

**Goal**: End-to-end negotiate → commit → deliver → delegate → provenance flows work in-process against `MemoryTransport`, with the critical-extension registry fail-closing on unknown extensions. Ships merged `famp-protocol` plus `famp-extensions`. This is the largest logic phase in v1 — four tightly-coupled concerns share the FSM and were fused to prevent premature API churn.

**Depends on**: Phase 5 (FSM is the substrate for all protocol transitions)

**Requirements**: NEGO-01, NEGO-02, NEGO-03, NEGO-04, NEGO-05, NEGO-06, NEGO-07, NEGO-08, NEGO-09, NEGO-10, NEGO-11, NEGO-12, DEL-01, DEL-02, DEL-03, DEL-04, DEL-05, DEL-06, DEL-07, DEL-08, DEL-09, PROV-01, PROV-02, PROV-03, PROV-04, PROV-05, PROV-06, PROV-07, EXT-01, EXT-02, EXT-03, EXT-04, EXT-05

**Success Criteria** (what must be TRUE):
  1. Two in-process agents execute a complete negotiate → counter → commit → deliver flow via `MemoryTransport`, driven by the FSM, with every transition logged in a deterministic provenance graph
  2. All three delegation forms (`assist`, `subtask`, `transfer`) work end-to-end; transfer timeout auto-reverts per SPEC-11; delegation ceiling (max_hops, max_fanout, allow/forbid lists) enforced
  3. Silent subcontracting is detected and rejected with `provenance_incomplete` error; a test reproduces the attack and asserts rejection
  4. Negotiation round limit (INV-11, default 20) enforced; partial-acceptance commit with explicit scope subset works; competing-instance commit resolution per SPEC-14 tested
  5. Provenance graph is canonicalized via RFC 8785, attached to terminal `deliver`, signed, and verifies against the conversation graph; redaction preserves mandatory fields
  6. Unknown-critical extension is rejected with INV-9 (fail-closed); unknown non-critical extension is ignored and passes through; one reference critical extension and one reference non-critical extension ship and are tested

**Plans**: TBD

> **Research flag**: `/gsd:research-phase` recommended — concurrency testing tool choice (`loom` vs `shuttle`) and the shape of `famp-protocol`'s public API should be scoped before planning. Budget generously.

---

### Phase 7: Transport (Memory + HTTP)

**Goal**: Two independent processes on localhost exchange signed FAMP envelopes over real HTTP + TLS, terminating a full negotiate → commit → deliver flow. Ships `famp-transport` (trait + `MemoryTransport`) and `famp-transport-http` (axum-based reference).

**Depends on**: Phase 6 (protocol produces the envelopes the transport will carry)

**Requirements**: TRANS-01, TRANS-02, TRANS-03, TRANS-04, TRANS-05, TRANS-06, TRANS-07, TRANS-08, TRANS-09

**Success Criteria** (what must be TRUE):
  1. `Transport` trait uses native `async fn` (no `#[async_trait]`) with a send method and an incoming stream; `MemoryTransport` is ~50 LoC and used as a dev dep across crates
  2. Two `famp` binaries exchange a signed commit over `POST /famp/v0.5.1/inbox` on `HttpTransport` with rustls-only TLS (no OpenSSL, no `native-tls`)
  3. Agent Cards are served at `GET /famp/v0.5.1/.well-known/famp/<name>.json`; a second node resolves, caches, and verifies a card over HTTP
  4. Body-size limit of 1MB (spec §18) enforced as a tower layer; oversized requests rejected before deserialization
  5. Signature verification runs as HTTP middleware before routing; unsigned or invalid-signature requests never reach protocol code; send path is cancellation-safe via spawned task + channel

**Plans**: TBD

---

### Phase 8: Conformance, Adversarial Suite, CLI

**Goal**: The implementation ships with an externally-sourced JSON fixture vector suite that future implementations can run, a full adversarial test suite covering every attack class identified in the pitfalls review, and a `famp` CLI that a user can invoke for every core operation. Level 2 and Level 3 badge scripts run green in CI. Ships `famp-conformance` and the `famp` umbrella + CLI.

**Depends on**: Phase 7 (both transports needed for adversarial + happy-path integration)

**Requirements**: CONF-01, CONF-02, CONF-03, CONF-04, CONF-05, CONF-06, CONF-07, CONF-08, CONF-09, CONF-10, CONF-11, CONF-12, CONF-13, CONF-14, CONF-15, CONF-16, CONF-17, CONF-18, CLI-01, CLI-02, CLI-03, CLI-04, CLI-05, CLI-06, CLI-07, CLI-08

**Success Criteria** (what must be TRUE):
  1. Language-neutral JSON fixture vectors live in `famp-conformance/fixtures/`, sourced externally (RFC 8785 Appendix B, RFC 8032, second-implementation generation); none are self-generated from `famp-canonical` output
  2. Two-node happy-path integration passes on both `MemoryTransport` and `HttpTransport`, exercising propose → counter → commit → deliver end-to-end
  3. Every adversarial case is green: unsigned rejection, wrong-key rejection, canonicalization divergence, stale commit, replay duplicate, unknown-critical extension (INV-9), negotiation round overflow (INV-11), silent subcontracting, competing commits from two instances, cancellation race with final delivery, drop-at-every-`.await` injection, key rotation mid-conversation
  4. `famp` CLI implements `keygen`, `card new`, `card verify`, `envelope sign`, `envelope verify`, `canonical`, `serve --bind`, and `fixture run`; examples `memory_two_node.rs`, `http_node.rs`, `custom_extension.rs` all compile and run
  5. Level 2 and Level 3 conformance badge scripts run automated in CI and produce pass/fail reports; both pass on main

**Plans**: TBD

> **Research flag**: `/gsd:research-phase` recommended — external fixture sourcing (which second implementation? which cyberphone corpus subset?) and adversarial test framework selection should be scoped before planning.

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 0. Toolchain & Workspace Scaffold | 1/3 | In Progress|  |
| 1. Spec Fork v0.5.1 | 0/0 | Not started | - |
| 2. Canonical + Crypto Foundations | 0/0 | Not started | - |
| 3. Envelope + Message Schemas | 0/0 | Not started | - |
| 4. Identity + Causality | 0/0 | Not started | - |
| 5. State Machines + Model Checking | 0/0 | Not started | - |
| 6. Protocol Logic + Extensions | 0/0 | Not started | - |
| 7. Transport (Memory + HTTP) | 0/0 | Not started | - |
| 8. Conformance, Adversarial Suite, CLI | 0/0 | Not started | - |

## Coverage Summary

**Total v1 requirements:** 153
**Mapped:** 153 (100%)
**Orphaned:** 0

| Category | Count | Phase |
|----------|-------|-------|
| TOOL | 7 | Phase 0 |
| SPEC | 20 | Phase 1 |
| CANON | 7 | Phase 2 |
| CRYPTO | 8 | Phase 2 |
| CORE | 6 | Phase 2 |
| ENV | 15 | Phase 3 |
| ID | 7 | Phase 4 |
| CAUS | 7 | Phase 4 |
| FSM | 8 | Phase 5 |
| NEGO | 12 | Phase 6 |
| DEL | 9 | Phase 6 |
| PROV | 7 | Phase 6 |
| EXT | 5 | Phase 6 |
| TRANS | 9 | Phase 7 |
| CONF | 18 | Phase 8 |
| CLI | 8 | Phase 8 |

## Research Flags

Phases requiring `/gsd:research-phase` before `/gsd:plan-phase`:
- **Phase 1** — spec-fork decisions are load-bearing
- **Phase 2** — highest-risk; canonical JSON + crypto correctness
- **Phase 5** — `stateright` + FSM ownership design
- **Phase 6** — largest logic phase; concurrency testing tooling + public API shape
- **Phase 8** — external fixture sourcing + adversarial framework

Phases safe to plan directly:
- **Phase 0** — standard Rust toolchain/workspace patterns
- **Phase 3** — schema work follows Phase 2 patterns
- **Phase 4** — well-understood identity/causality patterns
- **Phase 7** — axum + rustls are standard

---
*Roadmap created: 2026-04-12*
