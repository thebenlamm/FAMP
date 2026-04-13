# Roadmap: FAMP v0.6 Foundation Crates

**Created:** 2026-04-12
**Milestone:** v0.6 Foundation Crates
**Granularity:** standard
**Coverage:** 25/25 v0.6 requirements mapped (100%)
**Core Value:** A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.

## Shipped Milestones

- v0.5.1 Spec Fork — Phases 0–1 (shipped 2026-04-13) — interop contract locked. See [milestones/v0.5.1-phases/](milestones/v0.5.1-phases/).

## Active Milestone

- **v0.6 Foundation Crates** — Phases 1–3. First code milestone. Ships `famp-canonical`, `famp-crypto`, `famp-core`. Phase numbering RESET to 1 (v0.5.1 was a docs-only milestone).

**Milestone goal:** Byte-exact canonical JSON (RFC 8785), Ed25519 sign/verify with domain separation, and compiler-checked core types — the substrate every downstream FAMP crate signs against.

**Milestone success shape:** `just ci` green; RFC 8785 Appendix B vectors byte-exact in CI; worked Ed25519 example from PITFALLS P10 verifies byte-exact in Rust; `famp-core` invariants compile-check via exhaustive enum `match`.

## Phases

- [x] **Phase 1: Canonical JSON Foundations** — `famp-canonical` wrapping `serde_jcs` with RFC 8785 Appendix B conformance gate (SEED-001 RESOLVED 2026-04-13, 12/12 gate green, keep serde_jcs) and documented 357-LoC fallback plan on disk. **Shipped 2026-04-13.**
- [ ] **Phase 2: Crypto Foundations** — `famp-crypto` Ed25519 sign/verify with domain-separation prefix, `verify_strict`-only exposure, weak-key rejection, and byte-exact replay of PITFALLS P10 worked example
- [ ] **Phase 3: Core Types & Invariants** — `famp-core` shared types (Principal, Instance, MessageId UUIDv7, ArtifactId), typed 15-category error enum, and INV-1..INV-11 scaffolding

## Phase Details

### Phase 1: Canonical JSON Foundations

**Goal**: A user can take any JSON value and get byte-exact RFC 8785 JCS output, with CI proving conformance against externally-sourced Appendix B vectors. SEED-001 decision (keep `serde_jcs` vs fork to `famp-canonical`) is made and recorded here.

**Depends on**: Phase 0 (toolchain) + Phase 1 (v0.5.1 spec fork) — both shipped in prior milestone

**Requirements**: CANON-01, CANON-02, CANON-03, CANON-04, CANON-05, CANON-06, CANON-07, SPEC-02, SPEC-18

**Success Criteria** (what must be TRUE):
  1. `famp-canonical` crate exposes a stable `Canonicalize` trait that wraps `serde_jcs` (or the forked fallback) behind an API consumers depend on
  2. RFC 8785 Appendix B test vectors pass as a hard CI gate (not warning, not skipped); SEED-001 decision (keep `serde_jcs` vs fork) is recorded in-repo with rationale
  3. UTF-16 key-sort verified against supplementary-plane fixtures (emoji, CJK Ext B) — fixtures committed and green in CI
  4. ECMAScript number formatting verified against cyberphone reference; 100M-sample float corpus integrated as CI check (full or sampled budget documented)
  5. Duplicate-key JSON inputs are rejected at parse time with a typed error
  6. Documented from-scratch fallback plan (~500 LoC, RFC 8785 §3 key sort + number formatter + UTF-8 pass-through) lives in-repo under `famp-canonical/docs/fallback.md`, not just in conversation
  7. `sha256:<hex>` artifact identifier scheme (SPEC-18) is locked in `famp-canonical`'s artifact-ID helper, consistent with the v0.5.1 spec text

**Plans:** 3 plans
- [x] 01-01-PLAN.md — Wave 0 scaffolding: workspace dep fix, fallback.md (BEFORE gate), test harness skeletons + cyberphone fixtures
- [x] 01-02-PLAN.md — Implement canonicalize/strict_parse/artifact_id sources + author supplementary fixtures + sampled float corpus
- [x] 01-03-PLAN.md — Run RFC 8785 gate, record SEED-001 decision with cited evidence, wire CI + nightly full-corpus workflows

> **HIGHEST-RISK PHASE OF THE PROJECT.** Research flag: `/gsd:research-phase` mandatory. Canonical-JSON correctness is the interop contract — a bug here invalidates every downstream phase. Front-load external vectors. Be prepared to drop `serde_jcs` for the fallback if it fails conformance; the fallback plan must exist *before* the decision.

---

### Phase 2: Crypto Foundations

**Goal**: A user can sign a canonical byte string with Ed25519 using the domain-separation prefix from SPEC-03, and a second implementation (the Python worked example from PITFALLS P10) verifies byte-exact. `famp-crypto` exposes only `verify_strict`; raw `verify` is unreachable.

**Depends on**: Phase 1 (canonical bytes are the input to every signature)

**Requirements**: CRYPTO-01, CRYPTO-02, CRYPTO-03, CRYPTO-04, CRYPTO-05, CRYPTO-06, CRYPTO-07, CRYPTO-08, SPEC-03, SPEC-19

**Success Criteria** (what must be TRUE):
  1. `famp-crypto` exposes `Signer` and `Verifier` traits over Ed25519; only `verify_strict` is reachable from public API, raw `verify` is not exported
  2. Weak / small-subgroup public keys are rejected at trust-store / Agent Card ingress with "must reject" fixtures proving it
  3. Domain-separation prefix from SPEC-03 is applied before every sign operation, ships as conformance vector #1 with a committed hex dump, and is documented in `famp-crypto`'s README
  4. RFC 8032 Ed25519 test vectors pass as a hard CI gate
  5. The worked Ed25519 example from PITFALLS P10 (externally sourced bytes from Python `jcs 0.2.1` + `cryptography 46.0.7`) verifies byte-exact in Rust; committed as a fixture in `famp-crypto/tests/vectors/`
  6. Base64url unpadded encoding used for keys (32 bytes) and signatures (64 bytes) per SPEC-19; round-trip property test green
  7. SHA-256 content-addressing available via `sha2` crate; signature verification path is constant-time (no early-return timing leaks), documented and tested

**Plans:** 3 plans
- [x] 02-01-PLAN.md — Crate scaffold, newtypes, base64url codec, weak-key ingress fixtures (CRYPTO-02/03/06, SPEC-19)
- [x] 02-02-PLAN.md — sign/verify free functions, Signer/Verifier traits, canonicalize_for_signature, RFC 8032 gate (CRYPTO-01/04/05/07/08)
- [x] 02-03-PLAN.md — §7.1c worked-example fixture, byte-exact gate, README + wrapper audit, CI wiring (CRYPTO-04/08, SPEC-03)

> **Research flag**: `/gsd:research-phase` recommended. Ed25519 strictness semantics, small-subgroup attack fixtures, and domain-separation byte layout all warrant upfront scoping.

---

### Phase 3: Core Types & Invariants

**Goal**: `famp-core` ships the shared value types every downstream crate will depend on, with the typed error enum covering all 15 §15.1 categories and invariant constants INV-1..INV-11 documented in code. Adding a new terminal state or error category without updating every consumer is a compile error.

**Depends on**: Phase 2 (types reference `ArtifactId` content-addressing from `famp-crypto`'s `sha2` helper)

**Requirements**: CORE-01, CORE-02, CORE-03, CORE-04, CORE-05, CORE-06

**Success Criteria** (what must be TRUE):
  1. `Principal` and `Instance` identity types parse from and display to their canonical wire strings with round-trip tests
  2. `MessageId` (UUIDv7), `ConversationId`, `TaskId`, `CommitmentId`, and `ArtifactId` (with `sha256:<hex>` prefix) are distinct types that cannot be accidentally swapped at call sites
  3. Typed error enum covers all 15 error categories from spec §15.1; exhaustive `match` against the enum is verified by at least one downstream consumer stub
  4. Invariant constants INV-1 through INV-11 are documented in code (doc comments or const declarations) so every future crate can link to them
  5. Authority scope enum (`advisory`, `negotiate`, `commit_local`, `commit_delegate`, `transfer`) is defined and exhaustive `match` against it compiles across consumer stubs

**Plans:** 2 plans
- [x] 03-01-PLAN.md — Identity (Principal/Instance), UUIDv7 ID newtypes, ArtifactId with narrow parse errors (CORE-01, CORE-02, CORE-03)
- [x] 03-02-PLAN.md — ProtocolErrorKind (15 §15.1 categories), AuthorityScope ladder, INV-1..INV-11 invariants module, exhaustive consumer stub (CORE-04, CORE-05, CORE-06)

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | 3/3 | Complete | 2026-04-13 |
| 2. Crypto Foundations | 0/3 | Planning | - |
| 3. Core Types & Invariants | 0/2 | Planning | - |

## Coverage Summary

**Total v0.6 requirements:** 25
**Mapped:** 25 (100%)
**Orphaned:** 0

| Category | Count | Phase |
|----------|-------|-------|
| CANON | 7 | Phase 1 |
| SPEC (canonical) | 2 | Phase 1 (SPEC-02, SPEC-18) |
| CRYPTO | 8 | Phase 2 |
| SPEC (crypto) | 2 | Phase 2 (SPEC-03, SPEC-19) |
| CORE | 6 | Phase 3 |

## Research Flags

Phases requiring `/gsd:research-phase` before `/gsd:plan-phase`:
- **Phase 1** — highest-risk; RFC 8785 edge cases, SEED-001 decision point
- **Phase 2** — Ed25519 strictness + domain separation byte layout

Phases safe to plan directly:
- **Phase 3** — shared-type patterns are well understood; INV constants are documentation work

## Next Milestone: v0.7 Personal Runtime (preview)

**Direction change (2026-04-12):** the v1 scope is now split into a **Personal Profile** (v0.6 + v0.7) and a **Federation Profile** (v0.8+). v0.7 is the finish line for "something a single developer can personally use"; federation-grade semantics (Agent Cards, negotiation, delegation, provenance, extensions, HTTP transport, adversarial matrix, Level 2/3 badges) all defer to v0.8+.

**Goal:** A single developer can run the same signed `request → commit → deliver` cycle two ways — in one binary via `MemoryTransport`, and across two machines via a minimal HTTP binding with trust bootstrapped from a local keyring file.

**Preview phases** (detailed in `/gsd:new-milestone v0.7` after v0.6 ships):

- **v0.7 Phase 1: Minimal Signed Envelope** — `famp-envelope` with encode/decode/validate, INV-10 mandatory-signature enforcement, body schemas for **only** `request`, `commit`, `deliver`, `ack`, `control/cancel`. Covers ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed — no capability snapshot binding), ENV-10, ENV-12 (cancel variant only), ENV-14, ENV-15.
- **v0.7 Phase 2: Minimal Task Lifecycle** — 4-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`), compile-time terminal enforcement via exhaustive enum `match`, proptest transition-legality tests. No conversation FSM, no stateright, no timeouts. Covers FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08.
- **v0.7 Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example** — `famp-transport` trait + `MemoryTransport` (~50 LoC), local `HashMap<Principal, VerifyingKey>` keyring (principal = raw Ed25519 pubkey, no Agent Card), `famp/examples/personal_two_agents.rs` happy path, three negative tests (unsigned / wrong-key / canonical divergence). Covers TRANS-01, TRANS-02, CONF-03, CONF-05, CONF-06, CONF-07.
- **v0.7 Phase 4: Minimal HTTP Transport + Cross-Machine Example** — `famp-transport-http` axum `POST /famp/v0.5.1/inbox` endpoint, reqwest client send, rustls TLS, 1 MB body-size limit, signature-verification middleware running **before** routing. TOFU keyring loads peer pubkeys from a local file / CLI flags. `famp/examples/cross_machine_two_agents.rs` runs server in one shell and client in another and completes the full signed cycle. Phase 3's three negative tests are re-run against the HTTP transport. Covers TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, CONF-04. **Deferred** inside TRANS-* even for Phase 4: TRANS-05 (`.well-known` Agent Card distribution — no cards), TRANS-08 (cancellation-safe spawn-channel send — best-effort is acceptable).

**Explicitly deferred to Federation Profile (v0.8+)** — not in v0.7:

- `famp-identity` — Agent Card, federation credential, capability declaration, trust store trait (ID-01..07)
- `famp-causality` full — freshness windows, replay cache, supersession, idempotency-key scoping (CAUS-01..07)
- `famp-protocol` / negotiation — propose, counter-propose, round limits, capability snapshot binding (NEGO-01..12)
- `famp-delegate` — assist / subtask / transfer forms, transfer timeout, delegation ceiling, silent-subcontract detection (DEL-01..09)
- `famp-provenance` — graph construction, canonicalization, redaction, signed terminal reports (PROV-01..07)
- `famp-extensions` — critical/non-critical registry, INV-9 fail-closed (EXT-01..05)
- HTTP transport Agent-Card-dependent pieces — TRANS-05 (`.well-known` card distribution) and TRANS-08 (cancellation-safe spawn-channel send). The minimal HTTP binding itself ships in v0.7 Phase 4.
- Adversarial conformance matrix + Level 2/3 badges (CONF-01, CONF-02, CONF-08..18)
- `famp` CLI commands (CLI-01..08)
- `stateright` model checking (FSM-07)
- Conversation FSM + terminal-precedence rule (FSM-01, FSM-06)
- SPEC-04..08 (recipient binding, Agent Card format, card versioning, clock skew, idempotency key format) — matter more across orgs than within one developer's two-machine setup

## Future Milestone Sketch (Federation Profile)

Rough ordering, not committed:

- **v0.8 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` card distribution (TRANS-05), SPEC-04..06
- **v0.9 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path (TRANS-08), SPEC-07/08
- **v0.10 Negotiation & Commitment** — propose, counter-propose, round limits, capability snapshot binding, conversation FSM
- **v0.11 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v0.12 Provenance** — graph, canonicalization, redaction, signed terminal reports
- **v0.13 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v0.14 Adversarial Conformance + Level 2/3 Badges** — full CONF matrix, stateright model checking, conformance-badge automation, `famp` CLI

---
*Roadmap created: 2026-04-12 — v0.6 Foundation Crates*
*Restructured: 2026-04-12 — Personal Profile / Federation Profile split; v0.7 Personal Runtime added as next milestone*
