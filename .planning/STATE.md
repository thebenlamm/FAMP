---
gsd_state_version: 1.0
milestone: v0.6
milestone_name: "Foundation Crates — byte-exact canonical JSON (RFC 8785), Ed25519 sign/verify with domain separation, and compiler-checked core types (INV-1..11)"
status: defining_requirements
last_updated: "2026-04-12T00:00:00.000Z"
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# STATE: FAMP v0.6 Foundation Crates

**Last Updated:** 2026-04-12

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-12)

**Core Value:** A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.

**Current focus:** v0.6 Foundation Crates — `famp-canonical`, `famp-crypto`, `famp-core`.

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-12 — Milestone v0.6 started

## Accumulated Context

### Key Decisions Logged (carried from v0.5.1)

- **Language: Rust** — Compiler-checked INV-5 via exhaustive enum `match`; byte-exact Ed25519 + canonical JSON
- **Ship Level 2 + Level 3 together** — L1-only doesn't exercise signature discipline
- **v0.5.1 spec fork is authority** — all implementation bytes hash against `FAMP-v0.5.1-spec.md`
- **`serde_jcs` wrapped in `famp-canonical`** with RFC 8785 CI gate + documented ~500 LoC fallback (SEED-001)
- **Only `verify_strict`** exposed from `famp-crypto`; weak keys rejected at ingress
- **Domain separation prefix** added in v0.5.1 §7.1 — `famp-crypto` must implement with hex-dump worked example
- **Ed25519 wire encoding:** raw 32-byte pub / 64-byte sig, unpadded base64url
- **Phase numbering reset to 1 for v0.6** — v0.5.1 was a doc milestone; v0.6 is first code milestone

### Open TODOs (carried)

- Phase 1 number formatter decision: `ryu-js` vs port from cyberphone C reference
- SEED-001: `serde_jcs` RFC 8785 conformance gate + fallback plan (now in scope for v0.6)

### Known Blockers

- **`serde_jcs` correctness unknown** on RFC 8785 edge cases — fallback plan ready if CI gate fails; this milestone resolves it

## Session Continuity

### Recent Activity

- **2026-04-12:** v0.5.1 Spec Fork milestone archived; v0.6 Foundation Crates started. Scope: `famp-canonical` + `famp-crypto` + `famp-core`. Phase numbering reset.

---
*State initialized: 2026-04-12*
