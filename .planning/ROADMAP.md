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

- [ ] **Phase 1: Canonical JSON Foundations** — `famp-canonical` wrapping `serde_jcs` with RFC 8785 Appendix B conformance gate (SEED-001 decision point) and documented ~500 LoC fallback plan
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

**Plans**: TBD

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

**Plans**: TBD

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

**Plans**: TBD

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | 0/0 | Not started | - |
| 2. Crypto Foundations | 0/0 | Not started | - |
| 3. Core Types & Invariants | 0/0 | Not started | - |

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

## Downstream (deferred to v0.7+)

Everything else from REQUIREMENTS.md v1 list (ENV-*, ID-*, CAUS-*, FSM-*, NEGO-*, DEL-*, PROV-*, EXT-*, TRANS-*, CONF-*, CLI-*, SPEC-04..08) is deferred to future milestones. Tracked in REQUIREMENTS.md but not in this roadmap.

---
*Roadmap created: 2026-04-12 — v0.6 Foundation Crates*
