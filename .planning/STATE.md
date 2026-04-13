---
gsd_state_version: 1.0
milestone: v0.7
milestone_name: Personal Runtime
status: unknown
last_updated: "2026-04-13T04:10:10.176Z"
progress:
  total_phases: 3
  completed_phases: 0
  total_plans: 3
  completed_plans: 2
---

# STATE: FAMP v0.6 Foundation Crates

**Last Updated:** 2026-04-12

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-12)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 01 — canonical-json-foundations

## Current Position

Phase: 01 (canonical-json-foundations) — EXECUTING
Plan: 3 of 3

## Milestone Roadmap Snapshot

- [ ] Phase 1: Canonical JSON Foundations (CANON-01..07, SPEC-02, SPEC-18) — HIGHEST RISK, research flagged
- [ ] Phase 2: Crypto Foundations (CRYPTO-01..08, SPEC-03, SPEC-19) — research flagged
- [ ] Phase 3: Core Types & Invariants (CORE-01..06)

## Accumulated Context

### Key Decisions Logged (carried from v0.5.1)

- **Language: Rust** — Compiler-checked INV-5 via exhaustive enum `match`; byte-exact Ed25519 + canonical JSON
- **Ship Level 2 + Level 3 together** — L1-only doesn't exercise signature discipline
- **v0.5.1 spec fork is authority** — all implementation bytes hash against `FAMP-v0.5.1-spec.md`
- **`serde_jcs` wrapped in `famp-canonical`** with RFC 8785 CI gate + documented ~500 LoC fallback (SEED-001 — decision lands in Phase 1)
- **Only `verify_strict`** exposed from `famp-crypto`; weak keys rejected at ingress
- **Domain separation prefix** added in v0.5.1 §7.1 — `famp-crypto` must implement with hex-dump worked example
- **Ed25519 wire encoding:** raw 32-byte pub / 64-byte sig, unpadded base64url
- **Phase numbering reset to 1 for v0.6** — v0.5.1 was a doc milestone; v0.6 is first code milestone

### Open TODOs (carried)

- Phase 1 number formatter decision: `ryu-js` vs port from cyberphone C reference
- SEED-001: `serde_jcs` RFC 8785 conformance gate + fallback plan — must be resolved in Phase 1

### Known Blockers

- **`serde_jcs` correctness unknown** on RFC 8785 edge cases — fallback plan ready if CI gate fails; Phase 1 resolves it

## Session Continuity

### Recent Activity

- **2026-04-12:** **v0.7 scope expansion — two-machine HTTP added.** After the profile split was adopted, clarified that "personally usable" requires cross-machine, not just same-process. v0.7 grows a fourth phase: minimal `famp-transport-http` (axum inbox endpoint + reqwest client + rustls + 1 MB body limit + sig-verification middleware) with a `cross_machine_two_agents.rs` example, and negative tests (unsigned / wrong-key / canonical divergence) run against both transports. TOFU keyring loads peer pubkeys from a local file. TRANS-05 (`.well-known` Agent Card distribution) and TRANS-08 (cancellation-safe spawn-channel send) stay deferred to Federation Profile — no Agent Cards in personal profile, and best-effort send is acceptable. Federation Profile milestone sketch collapsed: old "v0.8 Trusted-Peer Transport" slot removed; v0.8 is now Identity & Cards.
- **2026-04-12:** **Direction change — Personal Profile / Federation Profile split adopted.** Mid-Phase-1 review flagged that the v1 scope (Level 2 + Level 3 conformance, 12-crate library, adversarial matrix, 150+ REQ-IDs) was too large for the actual near-term goal of "a library a single developer can use today." Decision: keep v0.6 Foundation Crates unchanged (canonical/crypto/core are the expensive substrate and must be done once), add **v0.7 Personal Runtime** as the next milestone (minimal envelope + 4-state task FSM + MemoryTransport + trust-on-first-use keyring + same-process two-agent example), and defer everything else — Agent Cards, negotiation, three delegation forms, provenance graph, extensions registry, HTTP transport, the 11-case adversarial matrix, Level 2/3 conformance badges, CLI — to a Federation Profile milestone cluster (v0.8+). PROJECT.md, REQUIREMENTS.md, and ROADMAP.md updated to reflect the split. v0.6 Phase 1 canonical-JSON research already completed; that work carries forward unchanged. SEED-001 canonical-JSON conformance gate is unaffected by the re-scope and is still the gate for Phase 1.
- **2026-04-12:** v0.6 roadmap drafted. Three phases: Canonical JSON (1), Crypto (2), Core Types (3). 25/25 requirements mapped. Research flags on Phases 1 and 2. SEED-001 decision point placed in Phase 1 success criteria.
- **2026-04-12:** v0.5.1 Spec Fork milestone archived; v0.6 Foundation Crates started. Scope: `famp-canonical` + `famp-crypto` + `famp-core`. Phase numbering reset.

---
*State updated: 2026-04-12 — roadmap created*
