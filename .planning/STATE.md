---
gsd_state_version: 1.0
milestone: v0.7
milestone_name: Personal Runtime
status: unknown
last_updated: "2026-04-13T15:01:33.195Z"
progress:
  total_phases: 3
  completed_phases: 3
  total_plans: 9
  completed_plans: 9
---

# STATE: FAMP v0.6 Foundation Crates

**Last Updated:** 2026-04-12

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-12)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 03 — core-types-invariants

## Current Position

Phase: 03 (core-types-invariants) — EXECUTING
Plan: 2 of 2

## Milestone Roadmap Snapshot

- [x] Phase 1: Canonical JSON Foundations (CANON-01..07, SPEC-02, SPEC-18) — **COMPLETE** 2026-04-13, SEED-001 RESOLVED (keep serde_jcs, 12/12 gate green)
- [x] Phase 2: Crypto Foundations (CRYPTO-01..08, SPEC-03, SPEC-19) — **COMPLETE** 2026-04-13, CRYPTO-07 gap closed via plan 02-04 (sha256_artifact_id + NIST KAT gate)
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

- Test-files clippy hygiene sweep (`unwrap_used` / `expect_used` workspace denies) — carried from Plan 01-02, non-blocking for Phase 2

### Resolved (Phase 1)

- ✅ **SEED-001** resolved 2026-04-13: keep `serde_jcs 0.2.0`. 12/12 RFC 8785 conformance gate green (Appendix B/C/E + cyberphone weird + 100K float corpus + supplementary-plane + NaN/Inf + duplicate-key). Decision + cited evidence in `.planning/SEED-001.md`. CI-enforced via `just test-canonical-strict` as blocking prerequisite job; nightly 100M full-corpus wired with SHA-256 integrity check.
- ✅ Phase 1 number formatter decision closed: `serde_jcs`'s `ryu-js` backend proven correct against RFC 8785 Appendix B + 100K cyberphone corpus — no port needed.

### Known Blockers

- **None.** Phase 1 substrate is byte-exact, CI-enforced, and ready to feed Phase 2 Ed25519 signing.

## Session Continuity

### Recent Activity

- **2026-04-13:** **Phase 2 complete — crypto-foundations shipped.** Plan 02-04 closed the single remaining gap flagged by `02-VERIFICATION.md` (CRYPTO-07 SHA-256 content-addressing). Added `famp_crypto::sha256_artifact_id` + `sha256_digest` backed by `sha2::Sha256` (workspace-pinned 0.11.0), gated by three NIST FIPS 180-2 Known Answer Tests (empty string, `"abc"`, 56-byte vector) plus shape-invariant and digest/id-agreement assertions in `tests/sha256_vectors.rs`. All 24 crypto tests pass under `just test-crypto`; clippy `-D warnings` green. Additive-only: `sign.rs`, `verify.rs`, `keys.rs`, `prefix.rs`, `traits.rs`, `error.rs` untouched. Phase 2 score moves from 6/7 → 7/7 truths verified. Auto-fixed during execution (deviation Rule 3): silenced `unused_crate_dependencies` in four pre-existing test binaries via `use sha2 as _;` (matches existing pattern), and wrapped `RustCrypto` in backticks + switched `format!/push_str` to `write!` to clear pedantic lints.
- **2026-04-13:** **Phase 1 complete — canonical-json-foundations shipped.** Plan 03 ran the full SEED-001 gate (12/12 PASS) and recorded the decision: keep `serde_jcs 0.2.0`. RFC 8785 Appendix B/C/E all byte-exact. 100K cyberphone float corpus byte-exact. cyberphone weird.json byte-exact. Supplementary-plane UTF-16 sort correct. NaN/Infinity rejected. Duplicate-key rejected at parse. Gate wired into CI as a dedicated `test-canonical` job that blocks the workspace test matrix on failure (no `continue-on-error`). Nightly `.github/workflows/nightly-full-corpus.yml` downloads the full 100M corpus with SHA-256 integrity check and runs on cron `0 6 * * *` + `v*` tags + manual dispatch. SEED-001 removed from blockers. Phase 1 REQs (CANON-01..07, SPEC-02, SPEC-18) all complete. Ready for Phase 2 (Crypto Foundations).
- **2026-04-12:** **v0.7 scope expansion — two-machine HTTP added.** After the profile split was adopted, clarified that "personally usable" requires cross-machine, not just same-process. v0.7 grows a fourth phase: minimal `famp-transport-http` (axum inbox endpoint + reqwest client + rustls + 1 MB body limit + sig-verification middleware) with a `cross_machine_two_agents.rs` example, and negative tests (unsigned / wrong-key / canonical divergence) run against both transports. TOFU keyring loads peer pubkeys from a local file. TRANS-05 (`.well-known` Agent Card distribution) and TRANS-08 (cancellation-safe spawn-channel send) stay deferred to Federation Profile — no Agent Cards in personal profile, and best-effort send is acceptable. Federation Profile milestone sketch collapsed: old "v0.8 Trusted-Peer Transport" slot removed; v0.8 is now Identity & Cards.
- **2026-04-12:** **Direction change — Personal Profile / Federation Profile split adopted.** Mid-Phase-1 review flagged that the v1 scope (Level 2 + Level 3 conformance, 12-crate library, adversarial matrix, 150+ REQ-IDs) was too large for the actual near-term goal of "a library a single developer can use today." Decision: keep v0.6 Foundation Crates unchanged (canonical/crypto/core are the expensive substrate and must be done once), add **v0.7 Personal Runtime** as the next milestone (minimal envelope + 4-state task FSM + MemoryTransport + trust-on-first-use keyring + same-process two-agent example), and defer everything else — Agent Cards, negotiation, three delegation forms, provenance graph, extensions registry, HTTP transport, the 11-case adversarial matrix, Level 2/3 conformance badges, CLI — to a Federation Profile milestone cluster (v0.8+). PROJECT.md, REQUIREMENTS.md, and ROADMAP.md updated to reflect the split. v0.6 Phase 1 canonical-JSON research already completed; that work carries forward unchanged. SEED-001 canonical-JSON conformance gate is unaffected by the re-scope and is still the gate for Phase 1.
- **2026-04-12:** v0.6 roadmap drafted. Three phases: Canonical JSON (1), Crypto (2), Core Types (3). 25/25 requirements mapped. Research flags on Phases 1 and 2. SEED-001 decision point placed in Phase 1 success criteria.
- **2026-04-12:** v0.5.1 Spec Fork milestone archived; v0.6 Foundation Crates started. Scope: `famp-canonical` + `famp-crypto` + `famp-core`. Phase numbering reset.

---
*State updated: 2026-04-13 — Phase 2 complete, CRYPTO-07 closed*
