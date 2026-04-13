# Requirements: FAMP v0.5.1 Rust Reference Implementation

**Defined:** 2026-04-12
**Restructured:** 2026-04-12 — split into Personal Profile (v0.6 + v0.7) and Federation Profile (v0.8+) per direction change.
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Profile Structure

Requirements are now partitioned across two profiles:

- **Personal Profile (v0.6 + v0.7)** — everything needed to wire two locally-trusted agents in one binary with full signing discipline. ~35 REQ-IDs.
- **Federation Profile (v0.8+)** — Agent Cards, negotiation, delegation, provenance, extensions, HTTP transport, adversarial conformance matrix, Level 2/3 badges. ~120 REQ-IDs. Tracked but not v1-blocking.

Every REQ section below is tagged with its profile. The underlying REQ-IDs are unchanged so existing research, context notes, and cross-references stay valid.

## Current Milestone: v0.6 Foundation Crates (Personal Profile — part 1)

**Goal:** Ship byte-exact canonical JSON + Ed25519 signing/verification + core types — the substrate every downstream FAMP crate signs against.

**In-scope REQ-IDs (25):**

- **Canonical JSON:** CANON-01, CANON-02, CANON-03, CANON-04, CANON-05, CANON-06, CANON-07
- **Crypto:** CRYPTO-01, CRYPTO-02, CRYPTO-03, CRYPTO-04, CRYPTO-05, CRYPTO-06, CRYPTO-07, CRYPTO-08
- **Core Types & Invariants:** CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06
- **Spec prerequisites (carried from v0.5.1):** SPEC-02, SPEC-03, SPEC-18, SPEC-19

## Next Milestone: v0.7 Personal Runtime (Personal Profile — part 2)

**Goal:** A single developer runs a same-process two-agent example with a signed `request → commit → deliver` cycle end to end. Minimal envelope + minimal task FSM + `MemoryTransport` + trust-on-first-use keyring.

**Target in-scope REQ-IDs (~10, final list pinned when v0.7 roadmap is drafted):**

- **Envelope (minimal):** ENV-01, ENV-02, ENV-03, ENV-06 (ack), ENV-07 (request), ENV-09 (commit — without capability snapshot binding), ENV-10 (deliver), ENV-12 (control — cancel variant only), ENV-14 (scope enforcement), ENV-15 (signed round-trip for the 5 message classes shipped)
- **Task FSM (minimal):** FSM-02 (limited to 4 states: REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}), FSM-03 (compile-time terminals), FSM-04, FSM-05, FSM-08 (proptest). **Deferred:** FSM-01 (conversation FSM), FSM-06 (terminal precedence — no competing terminals in personal profile), FSM-07 (stateright).
- **Transport (minimal two-machine story):** TRANS-01 (trait), TRANS-02 (MemoryTransport), TRANS-03 (axum HTTP/1.1 + JSON + TLS binding), TRANS-04 (`POST /famp/v0.5.1/inbox` endpoint per principal), TRANS-06 (rustls-only), TRANS-07 (1 MB body-size limit as tower layer), TRANS-09 (signature verification as HTTP middleware before routing). **Deferred to Federation Profile:** TRANS-05 (`.well-known` Agent Card distribution — no cards in personal profile), TRANS-08 (cancellation-safe spawn-channel send path — best-effort is acceptable for personal use).
- **Conformance (minimal, both transports):** CONF-03 (happy-path two-node MemoryTransport integration), CONF-04 (happy-path two-node HttpTransport integration), CONF-05 (unsigned rejection), CONF-06 (wrong-key rejection), CONF-07 (canonical divergence detection). Negative tests run against **both** transports. **Deferred:** CONF-01..02 (fixture publishing), CONF-08..18 (adversarial matrix, Level 2/3 badges).
- **Causality (minimal):** `in_reply_to` cross-reference only (covered by ENV-13 narrowed to the 5 shipped classes). **Deferred:** CAUS-01..07 (freshness, replay cache, idempotency-key scoping, supersession).

Explicitly **deferred to Federation Profile (v0.8+)**: all `ID-*`, all `CAUS-*`, `FSM-01/06/07`, all `NEGO-*`, all `DEL-*`, all `PROV-*`, all `EXT-*`, `TRANS-03..09`, all `TRANS2-*`, `CONF-01/02/04/08..18`, all `CLI-*`, `SPEC-04/05/06/07/08`, and the envelope message classes `announce` (ENV-04), `describe` (ENV-05), `propose` (ENV-08), `delegate` (ENV-11), plus `control` non-cancel variants.

## All Tracked Requirements (v1 — both profiles)

Requirements originally scoped for a Level 2 + Level 3 conformance release. Section tags (`[Personal V1]` or `[Federation Profile]`) indicate profile. Federation Profile items remain tracked here but are not roadmap-mapped until v0.8+.

### Toolchain [Personal V1 — shipped]

- [x] **TOOL-01**: Rust toolchain installed via `rustup` with pinned version in `rust-toolchain.toml`
- [x] **TOOL-02**: Cargo workspace scaffolded with 12 library crates + 1 umbrella (or staged merge for Phase 2-3)
- [x] **TOOL-03**: `just` task runner installed with common targets (build, test, lint, fmt, bench)
- [x] **TOOL-04**: `cargo-nextest` configured as default test runner
- [x] **TOOL-05**: GitHub Actions CI runs fmt, clippy (strict), build, and nextest on every push
- [x] **TOOL-06**: Workspace `[workspace.dependencies]` block pins every crate version in one place
- [x] **TOOL-07**: Strict `clippy` config with `unsafe_code = "forbid"` at workspace root

### Spec Fork [Personal V1 — mostly shipped; SPEC-04..08 deferred to Federation Profile]

- [x] **SPEC-01**: `FAMP-v0.5.1-spec.md` forked from v0.5 with documented changelog citing review findings
- [x] **SPEC-02**: Canonical JSON serialization locked to RFC 8785 JCS (explicit reference, not paraphrase)
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
- [x] **SPEC-18**: Artifact identifier scheme locked (`sha256:<hex>`)
- [x] **SPEC-19**: Ed25519 key encoding locked (raw 32-byte pub, 64-byte sig, unpadded base64url)
- [x] **SPEC-20**: Spec-version constant defined and referenced by implementations

### Canonical JSON [Personal V1 — v0.6 Phase 1]

- [x] **CANON-01**: `famp-canonical` crate wraps `serde_jcs` behind a stable `Canonicalize` trait
- [x] **CANON-02**: RFC 8785 Appendix B test vectors pass as hard CI gate
- [x] **CANON-03**: cyberphone 100M-sample float corpus integrated as CI check
- [x] **CANON-04**: UTF-16 key sort verified on supplementary-plane characters (emoji, CJK Ext B)
- [x] **CANON-05**: ECMAScript number formatting verified against cyberphone reference
- [x] **CANON-06**: Duplicate-key rejection on parse
- [x] **CANON-07**: Documented from-scratch fallback plan (~500 LoC) if `serde_jcs` fails conformance

### Crypto [Personal V1 — v0.6 Phase 2]

- [ ] **CRYPTO-01**: `famp-crypto` crate exposes `Signer` and `Verifier` traits over Ed25519
- [x] **CRYPTO-02**: Only `verify_strict` exposed; raw `verify` hidden (rejects small-subgroup/weak keys)
- [x] **CRYPTO-03**: Weak-public-key rejection at trust store / Agent Card ingress
- [ ] **CRYPTO-04**: Domain-separation prefix applied before signing per SPEC-03
- [ ] **CRYPTO-05**: RFC 8032 Ed25519 test vectors green in CI
- [x] **CRYPTO-06**: Base64url unpadded encoding for keys and signatures
- [ ] **CRYPTO-07**: SHA-256 content-addressing for artifacts via `sha2` crate
- [ ] **CRYPTO-08**: Constant-time signature verification path (no early-return timing leaks)

### Core Types & Invariants [Personal V1 — v0.6 Phase 3]

- [ ] **CORE-01**: `Principal` and `Instance` identity types with parse/display round-trip
- [ ] **CORE-02**: `MessageId` (UUIDv7) and `ConversationId` / `TaskId` / `CommitmentId` types
- [ ] **CORE-03**: `ArtifactId` content-addressed type with `sha256:` prefix
- [ ] **CORE-04**: Typed error enum with all 15 error categories from spec §15.1
- [ ] **CORE-05**: Invariant constants INV-1 through INV-11 documented in code
- [ ] **CORE-06**: Authority scope enum (advisory, negotiate, commit_local, commit_delegate, transfer)

### Envelope & Messages [MIXED — minimal subset is Personal V1 (v0.7); full set is Federation Profile]

> Personal V1 ships only: ENV-01, ENV-02, ENV-03, ENV-06 (ack), ENV-07 (request), ENV-09 (commit, simplified), ENV-10 (deliver), ENV-12 (control/cancel only), ENV-14 (scope), ENV-15 (round-trip for the 5 classes shipped). Everything else in this section is deferred.

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

### Identity & Agent Card [Federation Profile — deferred]

> Personal V1 uses trust-on-first-use: local `HashMap<Principal, VerifyingKey>`, principal = raw Ed25519 pubkey. No Agent Card, no federation credential, no capability declaration. All `ID-*` requirements are Federation Profile.

- [ ] **ID-01**: `famp-identity` crate with Agent Card struct matching spec §6.1
- [ ] **ID-02**: Agent Card parse/validate including federation credential
- [ ] **ID-03**: Capability declaration with all four claim classes (intrinsic, available, authorized, delegable)
- [ ] **ID-04**: Capability versioning with `card_version` and `min_compatible_version`
- [ ] **ID-05**: Federation trust list stub (`TrustStore` trait with in-memory impl)
- [ ] **ID-06**: Agent Card expiry enforcement on fresh requests; grandfather in-flight commits
- [ ] **ID-07**: `AgentCardStore` trait for pluggable card lookup

### Causality [Federation Profile — deferred]

> Personal V1 uses `in_reply_to` cross-reference only (part of narrowed ENV-13). Freshness windows, replay cache, supersession, idempotency-key scoping are all Federation Profile.

- [ ] **CAUS-01**: `famp-causality` crate with causal relation validation
- [ ] **CAUS-02**: Semantic `ack` processing with 6 disposition values
- [ ] **CAUS-03**: Freshness window enforcement per message class (table in §13.1)
- [ ] **CAUS-04**: Bounded replay cache with `(id, idempotency_key, content_hash)` tuples
- [ ] **CAUS-05**: Replay vs retransmission distinction via idempotency key
- [ ] **CAUS-06**: Supersession handling (original sender only, void prior message)
- [ ] **CAUS-07**: UUIDv7 timestamp vs `ts` field cross-validation

### State Machines [MIXED — minimal task FSM is Personal V1 (v0.7); conversation FSM + stateright deferred]

> Personal V1 ships a 4-state task FSM (REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}) — FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08 only. Deferred: FSM-01 (conversation FSM — no multi-turn flows in personal profile), FSM-06 (terminal precedence — no competing terminals), FSM-07 (stateright model check).

- [ ] **FSM-01**: `famp-fsm` crate with `ConversationFsm` (OPEN → CLOSED)
- [ ] **FSM-02**: `TaskFsm` with all 6 states (REQUESTED, COMMITTED, COMPLETED, FAILED, CANCELLED, REJECTED, EXPIRED)
- [ ] **FSM-03**: Compile-time terminal-state enforcement via exhaustive enum `match` (INV-5)
- [ ] **FSM-04**: State transitions driven by `(class, relation, terminal_status, current_state)` tuple
- [ ] **FSM-05**: Owned state types only — no lifetimes in FSM state enums
- [ ] **FSM-06**: Terminal precedence rule from SPEC-09 enforced
- [ ] **FSM-07**: `stateright` exhaustive model check under `#[cfg(test)]`
- [ ] **FSM-08**: `proptest` property tests for transition legality

### Negotiation & Commitment [Federation Profile — deferred]

> Personal V1 uses direct `request → commit`; no `propose` body, no counter-proposal, no round limits, no capability snapshot binding at commit time. All `NEGO-*` are Federation Profile.

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

### Delegation [Federation Profile — deferred]

> All three delegation forms, transfer timeout, delegation ceiling, silent subcontracting detection — Federation Profile. Personal V1 has no `famp-delegate` crate work.

- [ ] **DEL-01**: `assist` delegation form (delegator stays accountable)
- [ ] **DEL-02**: `subtask` delegation form (delegator retains parent, downstream owns subtask)
- [ ] **DEL-03**: `transfer` delegation form with ownership transition
- [ ] **DEL-04**: Transfer timeout with automatic reversion per SPEC-11
- [ ] **DEL-05**: Delegation ceiling (max_hops, max_fanout, allowed_delegates, forbidden_delegates)
- [ ] **DEL-06**: Delegation rights separate from execution rights (per-commitment)
- [ ] **DEL-07**: Subtask inherits parent policy and bounds validation
- [ ] **DEL-08**: Recursion depth bound enforcement
- [ ] **DEL-09**: Silent subcontracting prohibition check (`provenance_incomplete` error)

### Provenance [Federation Profile — deferred]

> Provenance graph construction, redaction, signed terminal reports — all Federation Profile. Personal V1 ships no `famp-provenance` work.

- [ ] **PROV-01**: Provenance graph construction (commits, delegations, artifacts, policies)
- [ ] **PROV-02**: Canonical serialization of provenance via RFC 8785 JCS
- [ ] **PROV-03**: Provenance attached to terminal `deliver` messages
- [ ] **PROV-04**: Signed provenance (non-repudiable)
- [ ] **PROV-05**: Redaction support with mandatory fields preserved
- [ ] **PROV-06**: Provenance verification against conversation graph
- [ ] **PROV-07**: `ProvenanceStore` trait for pluggable backends

### Extensions [Federation Profile — deferred]

> Critical/non-critical registry, INV-9 fail-closed, INV-8 containment — all Federation Profile. Personal V1 has no extensions framework; new message classes are added by changing the core enum.

- [ ] **EXT-01**: `famp-extensions` crate with `Extension` trait
- [ ] **EXT-02**: Critical vs non-critical extension registry
- [ ] **EXT-03**: Unknown-critical fail-closed rejection (INV-9)
- [ ] **EXT-04**: INV-8 extension containment enforcement (no core semantic redefinition)
- [ ] **EXT-05**: At least one critical and one non-critical reference extension shipped and tested

### Transport [MIXED — Personal V1 (v0.7) ships trait + MemoryTransport + minimal HTTP binding; Agent-Card-dependent pieces defer to Federation Profile]

> Personal V1 (v0.7) ships: TRANS-01 (trait), TRANS-02 (MemoryTransport), TRANS-03 (axum binding), TRANS-04 (inbox endpoint), TRANS-06 (rustls-only), TRANS-07 (body-size limit), TRANS-09 (sig-verification middleware). Deferred to Federation Profile: TRANS-05 (`.well-known` Agent Card distribution — no cards in personal profile), TRANS-08 (cancellation-safe spawn-channel send).

- [ ] **TRANS-01**: `famp-transport` crate with `Transport` trait (async send + incoming stream)
- [ ] **TRANS-02**: `MemoryTransport` in-process impl (~50 LoC, dev dep for all crates)
- [ ] **TRANS-03**: `famp-transport-http` with axum reference HTTP/1.1 + JSON + TLS binding
- [ ] **TRANS-04**: `POST /famp/v0.5.1/inbox` endpoint per principal
- [ ] **TRANS-05**: `GET /famp/v0.5.1/.well-known/famp/<name>.json` Agent Card distribution
- [ ] **TRANS-06**: rustls-only TLS (no OpenSSL)
- [ ] **TRANS-07**: Body-size limit (1MB per spec §18) as tower layer
- [ ] **TRANS-08**: Cancellation-safe send path via spawned task + channel
- [ ] **TRANS-09**: Signature verification runs as HTTP middleware before routing

### Conformance [MIXED — 5-case minimum run on both transports is Personal V1 (v0.7); full adversarial matrix + Level 2/3 badges are Federation Profile]

> Personal V1 ships: CONF-03 (happy-path MemoryTransport), CONF-04 (happy-path HttpTransport), CONF-05 (unsigned rejection), CONF-06 (wrong-key rejection), CONF-07 (canonical divergence detection). Negative tests (CONF-05..07) run against **both** transports. Deferred: fixture-vector publishing (CONF-01/02), the 11-case adversarial matrix (CONF-08..16), and Level 2/3 badges (CONF-17/18).

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

### CLI & Umbrella [Federation Profile — deferred]

> Personal V1 is library-first and ships a single example binary (`examples/personal_two_agents.rs`). The `famp` CLI (keygen, card, envelope sign/verify, serve, fixture run) lands with Federation Profile.

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

**Scope:** v0.6 Foundation Crates milestone only. 25/25 in-scope requirements mapped to exactly one phase (100%). All other REQ-IDs above are deferred to v0.7+ and will receive phase mappings when those milestones are roadmapped.

**Phase numbering:** Reset to Phase 1 for v0.6 (v0.5.1 was a docs-only milestone).

| Requirement | Phase | Status |
|-------------|-------|--------|
| CANON-01 | Phase 1 | Complete |
| CANON-02 | Phase 1 | Complete |
| CANON-03 | Phase 1 | Complete |
| CANON-04 | Phase 1 | Complete |
| CANON-05 | Phase 1 | Complete |
| CANON-06 | Phase 1 | Complete |
| CANON-07 | Phase 1 | Complete |
| SPEC-02  | Phase 1 | Complete |
| SPEC-18  | Phase 1 | Complete |
| CRYPTO-01 | Phase 2 | Pending |
| CRYPTO-02 | Phase 2 | Complete |
| CRYPTO-03 | Phase 2 | Complete |
| CRYPTO-04 | Phase 2 | Pending |
| CRYPTO-05 | Phase 2 | Pending |
| CRYPTO-06 | Phase 2 | Complete |
| CRYPTO-07 | Phase 2 | Pending |
| CRYPTO-08 | Phase 2 | Pending |
| SPEC-03  | Phase 2 | Pending |
| SPEC-19  | Phase 2 | Complete |
| CORE-01  | Phase 3 | Pending |
| CORE-02  | Phase 3 | Pending |
| CORE-03  | Phase 3 | Pending |
| CORE-04  | Phase 3 | Pending |
| CORE-05  | Phase 3 | Pending |
| CORE-06  | Phase 3 | Pending |

---
*Requirements defined: 2026-04-12*
*Last updated: 2026-04-12 — traceability rewritten for v0.6 Foundation Crates milestone (Phase numbering reset to 1)*
