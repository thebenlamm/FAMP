# Requirements: FAMP v0.5 Rust Reference Implementation

**Defined:** 2026-04-12
**Core Value:** A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.

## Current Milestone: v0.6 Foundation Crates

**Goal:** Ship byte-exact canonical JSON + Ed25519 signing/verification + core types — the substrate every downstream FAMP crate signs against.

**In-scope REQ-IDs (25):**

- **Canonical JSON:** CANON-01, CANON-02, CANON-03, CANON-04, CANON-05, CANON-06, CANON-07
- **Crypto:** CRYPTO-01, CRYPTO-02, CRYPTO-03, CRYPTO-04, CRYPTO-05, CRYPTO-06, CRYPTO-07, CRYPTO-08
- **Core Types & Invariants:** CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06
- **Spec prerequisites (carried from v0.5.1):** SPEC-02, SPEC-03, SPEC-18, SPEC-19

Everything else in this document (ENV-*, ID-*, CAUS-*, FSM-*, NEGO-*, DEL-*, PROV-*, EXT-*, TRANS-*, CONF-*, CLI-*, and SPEC-04/05/06/07/08) is deferred to future milestones (v0.7+). Traceability table below will be rewritten by the roadmapper to reflect v0.6 phase numbering only; downstream requirements are tracked in the full v1 list above.

## v1 Requirements

Requirements for Level 2 + Level 3 conformance release. Each maps to a roadmap phase.

### Toolchain

- [x] **TOOL-01**: Rust toolchain installed via `rustup` with pinned version in `rust-toolchain.toml`
- [x] **TOOL-02**: Cargo workspace scaffolded with 12 library crates + 1 umbrella (or staged merge for Phase 2-3)
- [x] **TOOL-03**: `just` task runner installed with common targets (build, test, lint, fmt, bench)
- [x] **TOOL-04**: `cargo-nextest` configured as default test runner
- [x] **TOOL-05**: GitHub Actions CI runs fmt, clippy (strict), build, and nextest on every push
- [x] **TOOL-06**: Workspace `[workspace.dependencies]` block pins every crate version in one place
- [x] **TOOL-07**: Strict `clippy` config with `unsafe_code = "forbid"` at workspace root

### Spec Fork

- [x] **SPEC-01**: `FAMP-v0.5.1-spec.md` forked from v0.5 with documented changelog citing review findings
- [ ] **SPEC-02**: Canonical JSON serialization locked to RFC 8785 JCS (explicit reference, not paraphrase)
- [ ] **SPEC-03**: Signature domain-separation byte format specified with hex-dump worked example
- [ ] **SPEC-04**: Signature covers `to` field (recipient binding) to prevent cross-recipient replay
- [ ] **SPEC-05**: Agent Card includes federation credential field (resolves circular self-signature)
- [ ] **SPEC-06**: Agent Card versioning rules pinned for key rotation without breaking in-flight commits
- [ ] **SPEC-07**: Clock skew tolerance and validity window concrete defaults (±60s / 5min recommended)
- [ ] **SPEC-08**: Idempotency key format fixed (128-bit random bytes, scope `(sender, recipient)`)
- [x] **SPEC-09**: §9.6 terminal precedence clarified — ack-disposition vs terminal-state-crystallization distinguished
- [x] **SPEC-10**: §7.3 "no body inspection" claim reconciled — explicit list of envelope-level fields for FSM transitions
- [x] **SPEC-11**: Transfer-timeout reversion race resolved (tiebreak rule for in-flight delegate commit)
- [x] **SPEC-12**: EXPIRED vs in-flight deliver race resolved
- [x] **SPEC-13**: Conditional-lapse precedence over delivery-wins (committer-side cancellation wins)
- [x] **SPEC-14**: Competing-instance commit intermediate state defined (INV-5 hole closed)
- [x] **SPEC-15**: Negotiation round counting under supersession pinned
- [x] **SPEC-16**: Capability snapshot binding vs card version drift contradiction resolved
- [x] **SPEC-17**: Body schemas defined for `commit`, `propose`, `deliver`, `control`, `delegate`
- [ ] **SPEC-18**: Artifact identifier scheme locked (`sha256:<hex>`)
- [ ] **SPEC-19**: Ed25519 key encoding locked (raw 32-byte pub, 64-byte sig, unpadded base64url)
- [x] **SPEC-20**: Spec-version constant defined and referenced by implementations

### Canonical JSON

- [ ] **CANON-01**: `famp-canonical` crate wraps `serde_jcs` behind a stable `Canonicalize` trait
- [ ] **CANON-02**: RFC 8785 Appendix B test vectors pass as hard CI gate
- [ ] **CANON-03**: cyberphone 100M-sample float corpus integrated as CI check
- [ ] **CANON-04**: UTF-16 key sort verified on supplementary-plane characters (emoji, CJK Ext B)
- [ ] **CANON-05**: ECMAScript number formatting verified against cyberphone reference
- [ ] **CANON-06**: Duplicate-key rejection on parse
- [ ] **CANON-07**: Documented from-scratch fallback plan (~500 LoC) if `serde_jcs` fails conformance

### Crypto

- [ ] **CRYPTO-01**: `famp-crypto` crate exposes `Signer` and `Verifier` traits over Ed25519
- [ ] **CRYPTO-02**: Only `verify_strict` exposed; raw `verify` hidden (rejects small-subgroup/weak keys)
- [ ] **CRYPTO-03**: Weak-public-key rejection at trust store / Agent Card ingress
- [ ] **CRYPTO-04**: Domain-separation prefix applied before signing per SPEC-03
- [ ] **CRYPTO-05**: RFC 8032 Ed25519 test vectors green in CI
- [ ] **CRYPTO-06**: Base64url unpadded encoding for keys and signatures
- [ ] **CRYPTO-07**: SHA-256 content-addressing for artifacts via `sha2` crate
- [ ] **CRYPTO-08**: Constant-time signature verification path (no early-return timing leaks)

### Core Types & Invariants

- [ ] **CORE-01**: `Principal` and `Instance` identity types with parse/display round-trip
- [ ] **CORE-02**: `MessageId` (UUIDv7) and `ConversationId` / `TaskId` / `CommitmentId` types
- [ ] **CORE-03**: `ArtifactId` content-addressed type with `sha256:` prefix
- [ ] **CORE-04**: Typed error enum with all 15 error categories from spec §15.1
- [ ] **CORE-05**: Invariant constants INV-1 through INV-11 documented in code
- [ ] **CORE-06**: Authority scope enum (advisory, negotiate, commit_local, commit_delegate, transfer)

### Envelope & Messages

- [ ] **ENV-01**: `famp-envelope` crate with typed Envelope struct matching spec §7.1
- [ ] **ENV-02**: Envelope encode/decode with `deny_unknown_fields` everywhere
- [ ] **ENV-03**: Mandatory signature enforcement on decode (INV-10) — unsigned messages rejected
- [ ] **ENV-04**: `announce` message class with body schema
- [ ] **ENV-05**: `describe` message class with body schema
- [ ] **ENV-06**: `ack` message class with body schema (6 disposition values)
- [ ] **ENV-07**: `request` message class with body schema
- [ ] **ENV-08**: `propose` message class with body schema (scope, bounds, terms, delegation perms)
- [ ] **ENV-09**: `commit` message class with body schema (capability snapshot included)
- [ ] **ENV-10**: `deliver` message class with body schema and envelope-level `terminal_status`
- [ ] **ENV-11**: `delegate` message class with body schema (form, commitment_ref, downstream, ceiling)
- [ ] **ENV-12**: `control` message class with body schema (cancels, supersedes, closes)
- [ ] **ENV-13**: All 11 causal relations validated against allowed message classes
- [ ] **ENV-14**: Scope enforcement (standalone / conversation / task)
- [ ] **ENV-15**: Envelope signed-message round-trip test for every message class

### Identity & Agent Card

- [ ] **ID-01**: `famp-identity` crate with Agent Card struct matching spec §6.1
- [ ] **ID-02**: Agent Card parse/validate including federation credential
- [ ] **ID-03**: Capability declaration with all four claim classes (intrinsic, available, authorized, delegable)
- [ ] **ID-04**: Capability versioning with `card_version` and `min_compatible_version`
- [ ] **ID-05**: Federation trust list stub (`TrustStore` trait with in-memory impl)
- [ ] **ID-06**: Agent Card expiry enforcement on fresh requests; grandfather in-flight commits
- [ ] **ID-07**: `AgentCardStore` trait for pluggable card lookup

### Causality

- [ ] **CAUS-01**: `famp-causality` crate with causal relation validation
- [ ] **CAUS-02**: Semantic `ack` processing with 6 disposition values
- [ ] **CAUS-03**: Freshness window enforcement per message class (table in §13.1)
- [ ] **CAUS-04**: Bounded replay cache with `(id, idempotency_key, content_hash)` tuples
- [ ] **CAUS-05**: Replay vs retransmission distinction via idempotency key
- [ ] **CAUS-06**: Supersession handling (original sender only, void prior message)
- [ ] **CAUS-07**: UUIDv7 timestamp vs `ts` field cross-validation

### State Machines

- [ ] **FSM-01**: `famp-fsm` crate with `ConversationFsm` (OPEN → CLOSED)
- [ ] **FSM-02**: `TaskFsm` with all 6 states (REQUESTED, COMMITTED, COMPLETED, FAILED, CANCELLED, REJECTED, EXPIRED)
- [ ] **FSM-03**: Compile-time terminal-state enforcement via exhaustive enum `match` (INV-5)
- [ ] **FSM-04**: State transitions driven by `(class, relation, terminal_status, current_state)` tuple
- [ ] **FSM-05**: Owned state types only — no lifetimes in FSM state enums
- [ ] **FSM-06**: Terminal precedence rule from SPEC-09 enforced
- [ ] **FSM-07**: `stateright` exhaustive model check under `#[cfg(test)]`
- [ ] **FSM-08**: `proptest` property tests for transition legality

### Negotiation & Commitment

- [ ] **NEGO-01**: Proposal struct matching spec §10.2 requirements
- [ ] **NEGO-02**: Counter-proposal via `proposes_against` (full proposal, not diff)
- [ ] **NEGO-03**: Negotiation round limit enforcement (INV-11, default 20)
- [ ] **NEGO-04**: Round counting under supersession per SPEC-15
- [ ] **NEGO-05**: Multiple concurrent proposals from same sender (no implicit supersession)
- [ ] **NEGO-06**: Partial acceptance commit with explicit scope subset
- [ ] **NEGO-07**: `commit` binds proposal via `commits_against` (exactly one proposal)
- [ ] **NEGO-08**: Capability snapshot captured and bound at commit time
- [ ] **NEGO-09**: Conditional commitment with machine-evaluable conditions
- [ ] **NEGO-10**: Commitment ID stable across supersession chains
- [ ] **NEGO-11**: Supersession renegotiation path (no return to REQUESTED)
- [ ] **NEGO-12**: Competing-commit resolution per SPEC-14

### Delegation

- [ ] **DEL-01**: `assist` delegation form (delegator stays accountable)
- [ ] **DEL-02**: `subtask` delegation form (delegator retains parent, downstream owns subtask)
- [ ] **DEL-03**: `transfer` delegation form with ownership transition
- [ ] **DEL-04**: Transfer timeout with automatic reversion per SPEC-11
- [ ] **DEL-05**: Delegation ceiling (max_hops, max_fanout, allowed_delegates, forbidden_delegates)
- [ ] **DEL-06**: Delegation rights separate from execution rights (per-commitment)
- [ ] **DEL-07**: Subtask inherits parent policy and bounds validation
- [ ] **DEL-08**: Recursion depth bound enforcement
- [ ] **DEL-09**: Silent subcontracting prohibition check (`provenance_incomplete` error)

### Provenance

- [ ] **PROV-01**: Provenance graph construction (commits, delegations, artifacts, policies)
- [ ] **PROV-02**: Canonical serialization of provenance via RFC 8785 JCS
- [ ] **PROV-03**: Provenance attached to terminal `deliver` messages
- [ ] **PROV-04**: Signed provenance (non-repudiable)
- [ ] **PROV-05**: Redaction support with mandatory fields preserved
- [ ] **PROV-06**: Provenance verification against conversation graph
- [ ] **PROV-07**: `ProvenanceStore` trait for pluggable backends

### Extensions

- [ ] **EXT-01**: `famp-extensions` crate with `Extension` trait
- [ ] **EXT-02**: Critical vs non-critical extension registry
- [ ] **EXT-03**: Unknown-critical fail-closed rejection (INV-9)
- [ ] **EXT-04**: INV-8 extension containment enforcement (no core semantic redefinition)
- [ ] **EXT-05**: At least one critical and one non-critical reference extension shipped and tested

### Transport

- [ ] **TRANS-01**: `famp-transport` crate with `Transport` trait (async send + incoming stream)
- [ ] **TRANS-02**: `MemoryTransport` in-process impl (~50 LoC, dev dep for all crates)
- [ ] **TRANS-03**: `famp-transport-http` with axum reference HTTP/1.1 + JSON + TLS binding
- [ ] **TRANS-04**: `POST /famp/v0.5.1/inbox` endpoint per principal
- [ ] **TRANS-05**: `GET /famp/v0.5.1/.well-known/famp/<name>.json` Agent Card distribution
- [ ] **TRANS-06**: rustls-only TLS (no OpenSSL)
- [ ] **TRANS-07**: Body-size limit (1MB per spec §18) as tower layer
- [ ] **TRANS-08**: Cancellation-safe send path via spawned task + channel
- [ ] **TRANS-09**: Signature verification runs as HTTP middleware before routing

### Conformance

- [ ] **CONF-01**: `famp-conformance` crate with language-neutral JSON fixture vectors
- [ ] **CONF-02**: Fixture vectors sourced externally (RFC 8785 Appendix B, RFC 8032, second-impl generation)
- [ ] **CONF-03**: Happy-path two-node integration (MemoryTransport): propose → counter → commit → deliver
- [ ] **CONF-04**: Happy-path two-node integration (HttpTransport): same flow over real HTTP
- [ ] **CONF-05**: Adversarial suite: unsigned message rejection
- [ ] **CONF-06**: Adversarial suite: wrong-key signature rejection
- [ ] **CONF-07**: Adversarial suite: canonicalization divergence detection
- [ ] **CONF-08**: Adversarial suite: stale commit rejection
- [ ] **CONF-09**: Adversarial suite: replay/duplicate detection
- [ ] **CONF-10**: Adversarial suite: unknown-critical extension rejection (INV-9)
- [ ] **CONF-11**: Adversarial suite: negotiation round overflow (INV-11)
- [ ] **CONF-12**: Adversarial suite: silent subcontracting detection
- [ ] **CONF-13**: Adversarial suite: competing commits from two instances
- [ ] **CONF-14**: Adversarial suite: cancellation race with final delivery
- [ ] **CONF-15**: Adversarial suite: drop-at-every-await cancellation injection
- [ ] **CONF-16**: Adversarial suite: key rotation mid-conversation
- [ ] **CONF-17**: Level 2 conformance badge script (automated check)
- [ ] **CONF-18**: Level 3 conformance badge script (automated check)

### CLI & Umbrella

- [ ] **CLI-01**: `famp` umbrella crate re-exports public API with feature flags (`memory`, `http`)
- [ ] **CLI-02**: `famp keygen` command
- [ ] **CLI-03**: `famp card new` + `famp card verify` commands
- [ ] **CLI-04**: `famp envelope sign` + `famp envelope verify` commands
- [ ] **CLI-05**: `famp canonical <input.json>` debug aid
- [ ] **CLI-06**: `famp serve --bind` reference HTTP node
- [ ] **CLI-07**: `famp fixture run <fixture.json>` conformance vector runner
- [ ] **CLI-08**: Examples: `memory_two_node.rs`, `http_node.rs`, `custom_extension.rs`

## v2 Requirements

Deferred to future milestone. Tracked but not in current roadmap.

### Language Bindings

- **BIND-01**: Python bindings via PyO3
- **BIND-02**: TypeScript bindings via wasm-bindgen

### Developer Experience

- **DX-01**: State machine execution tracer (D1)
- **DX-02**: Conversation/provenance graph inspector (D2)
- **DX-03**: CEL-based policy evaluation (D3)
- **DX-04**: Artifact content-addressed store abstraction (D4)
- **DX-05**: High-level agent harness / builder (D5)
- **DX-06**: Criterion benchmark suite (D6)

### Additional Transports

- **TRANS2-01**: libp2p transport binding
- **TRANS2-02**: NATS transport binding
- **TRANS2-03**: WebSocket transport binding

### Spec Extensions

- **EXT2-01**: Multi-party commitment profile
- **EXT2-02**: Streaming deliver (token-by-token)
- **EXT2-03**: Cross-federation delegation

## Out of Scope

| Feature | Reason |
|---------|--------|
| Level 1-only release | Doesn't exercise signature discipline under real flows; tempts fragile downstream builds |
| Python/TS bindings in v1 | Core must be proven first; bindings follow as separate milestone |
| libp2p/NATS/WebSocket transports | `Transport` trait leaves them open; only HTTP+JSON reference in v1 |
| Multi-party commitment | Spec §23 Q1 explicitly defers; bilateral only in v1 |
| Cross-federation delegation | Spec §23 Q3; bilateral peering not defined |
| Streaming deliver | Spec §23 Q2; `interim: true` sufficient for v1 |
| Real federation trust registry | Federation-specific; stub with hardcoded trust list |
| Economic/reputation/payment layers | Spec §21 exclusion |
| Cognitive trace disclosure | Spec §21 exclusion |
| Agent lifecycle management | Spec §21 exclusion |
| Tool API standardization | Spec §21 exclusion |
| Production deployment tooling | Library-first; ops concerns deferred |
| OpenSSL / native-tls | rustls-only — single TLS story |
| SIMD JSON parsers | `serde_json` only — one source of truth |
| `actix-web` / `warp` | axum is the sole HTTP reference |
| `async-std` / `smol` | tokio only |
| `#[async_trait]` | Native async fn in traits (Rust ≥1.75) |

## Traceability

**Coverage:** 153/153 v1 requirements mapped to exactly one phase (100%).

| Requirement | Phase | Status |
|-------------|-------|--------|
| TOOL-01 | Phase 0 | Complete |
| TOOL-02 | Phase 0 | Complete |
| TOOL-03 | Phase 0 | Complete |
| TOOL-04 | Phase 0 | Complete |
| TOOL-05 | Phase 0 | Complete |
| TOOL-06 | Phase 0 | Complete |
| TOOL-07 | Phase 0 | Complete |
| SPEC-01 | Phase 1 | Complete |
| SPEC-02 | Phase 1 | Pending |
| SPEC-03 | Phase 1 | Pending |
| SPEC-04 | Phase 1 | Pending |
| SPEC-05 | Phase 1 | Pending |
| SPEC-06 | Phase 1 | Pending |
| SPEC-07 | Phase 1 | Pending |
| SPEC-08 | Phase 1 | Pending |
| SPEC-09 | Phase 1 | Complete |
| SPEC-10 | Phase 1 | Complete |
| SPEC-11 | Phase 1 | Complete |
| SPEC-12 | Phase 1 | Complete |
| SPEC-13 | Phase 1 | Complete |
| SPEC-14 | Phase 1 | Complete |
| SPEC-15 | Phase 1 | Complete |
| SPEC-16 | Phase 1 | Complete |
| SPEC-17 | Phase 1 | Complete |
| SPEC-18 | Phase 1 | Pending |
| SPEC-19 | Phase 1 | Pending |
| SPEC-20 | Phase 1 | Complete |
| CANON-01 | Phase 2 | Pending |
| CANON-02 | Phase 2 | Pending |
| CANON-03 | Phase 2 | Pending |
| CANON-04 | Phase 2 | Pending |
| CANON-05 | Phase 2 | Pending |
| CANON-06 | Phase 2 | Pending |
| CANON-07 | Phase 2 | Pending |
| CRYPTO-01 | Phase 2 | Pending |
| CRYPTO-02 | Phase 2 | Pending |
| CRYPTO-03 | Phase 2 | Pending |
| CRYPTO-04 | Phase 2 | Pending |
| CRYPTO-05 | Phase 2 | Pending |
| CRYPTO-06 | Phase 2 | Pending |
| CRYPTO-07 | Phase 2 | Pending |
| CRYPTO-08 | Phase 2 | Pending |
| CORE-01 | Phase 2 | Pending |
| CORE-02 | Phase 2 | Pending |
| CORE-03 | Phase 2 | Pending |
| CORE-04 | Phase 2 | Pending |
| CORE-05 | Phase 2 | Pending |
| CORE-06 | Phase 2 | Pending |
| ENV-01 | Phase 3 | Pending |
| ENV-02 | Phase 3 | Pending |
| ENV-03 | Phase 3 | Pending |
| ENV-04 | Phase 3 | Pending |
| ENV-05 | Phase 3 | Pending |
| ENV-06 | Phase 3 | Pending |
| ENV-07 | Phase 3 | Pending |
| ENV-08 | Phase 3 | Pending |
| ENV-09 | Phase 3 | Pending |
| ENV-10 | Phase 3 | Pending |
| ENV-11 | Phase 3 | Pending |
| ENV-12 | Phase 3 | Pending |
| ENV-13 | Phase 3 | Pending |
| ENV-14 | Phase 3 | Pending |
| ENV-15 | Phase 3 | Pending |
| ID-01 | Phase 4 | Pending |
| ID-02 | Phase 4 | Pending |
| ID-03 | Phase 4 | Pending |
| ID-04 | Phase 4 | Pending |
| ID-05 | Phase 4 | Pending |
| ID-06 | Phase 4 | Pending |
| ID-07 | Phase 4 | Pending |
| CAUS-01 | Phase 4 | Pending |
| CAUS-02 | Phase 4 | Pending |
| CAUS-03 | Phase 4 | Pending |
| CAUS-04 | Phase 4 | Pending |
| CAUS-05 | Phase 4 | Pending |
| CAUS-06 | Phase 4 | Pending |
| CAUS-07 | Phase 4 | Pending |
| FSM-01 | Phase 5 | Pending |
| FSM-02 | Phase 5 | Pending |
| FSM-03 | Phase 5 | Pending |
| FSM-04 | Phase 5 | Pending |
| FSM-05 | Phase 5 | Pending |
| FSM-06 | Phase 5 | Pending |
| FSM-07 | Phase 5 | Pending |
| FSM-08 | Phase 5 | Pending |
| NEGO-01 | Phase 6 | Pending |
| NEGO-02 | Phase 6 | Pending |
| NEGO-03 | Phase 6 | Pending |
| NEGO-04 | Phase 6 | Pending |
| NEGO-05 | Phase 6 | Pending |
| NEGO-06 | Phase 6 | Pending |
| NEGO-07 | Phase 6 | Pending |
| NEGO-08 | Phase 6 | Pending |
| NEGO-09 | Phase 6 | Pending |
| NEGO-10 | Phase 6 | Pending |
| NEGO-11 | Phase 6 | Pending |
| NEGO-12 | Phase 6 | Pending |
| DEL-01 | Phase 6 | Pending |
| DEL-02 | Phase 6 | Pending |
| DEL-03 | Phase 6 | Pending |
| DEL-04 | Phase 6 | Pending |
| DEL-05 | Phase 6 | Pending |
| DEL-06 | Phase 6 | Pending |
| DEL-07 | Phase 6 | Pending |
| DEL-08 | Phase 6 | Pending |
| DEL-09 | Phase 6 | Pending |
| PROV-01 | Phase 6 | Pending |
| PROV-02 | Phase 6 | Pending |
| PROV-03 | Phase 6 | Pending |
| PROV-04 | Phase 6 | Pending |
| PROV-05 | Phase 6 | Pending |
| PROV-06 | Phase 6 | Pending |
| PROV-07 | Phase 6 | Pending |
| EXT-01 | Phase 6 | Pending |
| EXT-02 | Phase 6 | Pending |
| EXT-03 | Phase 6 | Pending |
| EXT-04 | Phase 6 | Pending |
| EXT-05 | Phase 6 | Pending |
| TRANS-01 | Phase 7 | Pending |
| TRANS-02 | Phase 7 | Pending |
| TRANS-03 | Phase 7 | Pending |
| TRANS-04 | Phase 7 | Pending |
| TRANS-05 | Phase 7 | Pending |
| TRANS-06 | Phase 7 | Pending |
| TRANS-07 | Phase 7 | Pending |
| TRANS-08 | Phase 7 | Pending |
| TRANS-09 | Phase 7 | Pending |
| CONF-01 | Phase 8 | Pending |
| CONF-02 | Phase 8 | Pending |
| CONF-03 | Phase 8 | Pending |
| CONF-04 | Phase 8 | Pending |
| CONF-05 | Phase 8 | Pending |
| CONF-06 | Phase 8 | Pending |
| CONF-07 | Phase 8 | Pending |
| CONF-08 | Phase 8 | Pending |
| CONF-09 | Phase 8 | Pending |
| CONF-10 | Phase 8 | Pending |
| CONF-11 | Phase 8 | Pending |
| CONF-12 | Phase 8 | Pending |
| CONF-13 | Phase 8 | Pending |
| CONF-14 | Phase 8 | Pending |
| CONF-15 | Phase 8 | Pending |
| CONF-16 | Phase 8 | Pending |
| CONF-17 | Phase 8 | Pending |
| CONF-18 | Phase 8 | Pending |
| CLI-01 | Phase 8 | Pending |
| CLI-02 | Phase 8 | Pending |
| CLI-03 | Phase 8 | Pending |
| CLI-04 | Phase 8 | Pending |
| CLI-05 | Phase 8 | Pending |
| CLI-06 | Phase 8 | Pending |
| CLI-07 | Phase 8 | Pending |
| CLI-08 | Phase 8 | Pending |

---
*Requirements defined: 2026-04-12*
*Last updated: 2026-04-12 — traceability filled during roadmap creation*
