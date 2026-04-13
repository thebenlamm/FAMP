---
gsd_state_version: 1.0
milestone: v0.7
milestone_name: Personal Runtime
status: ready_for_new_milestone
last_updated: "2026-04-13T16:00:00.000Z"
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# STATE: FAMP — Between Milestones

**Last Updated:** 2026-04-13

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-13 after v0.6 milestone)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Planning v0.7 Personal Runtime — minimal usable library on `MemoryTransport` + minimal HTTP transport.

## Current Position

Milestone: **v0.7 Personal Runtime** (not yet started)
Phase: —
Plan: —

Use `/gsd:new-milestone` to initialize v0.7 (requirements → roadmap).

## Last Shipped

- **v0.6 Foundation Crates** (2026-04-13) — `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements, 112/112 tests, `just ci` green. Archived to `.planning/milestones/v0.6-*.md` and `.planning/milestones/v0.6-phases/`. Git tag: `v0.6`.

## Accumulated Context

### Key Decisions Logged (carried forward)

- **Language: Rust** — compiler-checked INV-5 via exhaustive enum `match`
- **Personal Profile before Federation Profile** (adopted 2026-04-12) — v0.6 + v0.7 are the solo-dev finish line; v0.8+ stacks federation semantics on top without substrate churn
- **v0.5.1 spec fork is authority** — all implementation bytes hash against `FAMP-v0.5.1-spec.md`
- **SEED-001 RESOLVED 2026-04-13:** keep `serde_jcs 0.2.0` — 12/12 RFC 8785 conformance gate green; fallback plan on disk as insurance. Evidence in `.planning/SEED-001.md`.
- **`verify_strict`-only public surface** — raw `verify` unreachable from `famp-crypto` public API
- **Domain separation prefix prepended internally** — callers never assemble signing input by hand; `canonicalize_for_signature` is the only sanctioned path
- **Narrow, phase-appropriate error enums** — not one god enum (repeated pattern in Plans 01-01 D-16 and 02-01)
- **15-category flat `ProtocolErrorKind` + exhaustive consumer stub under `#![deny(unreachable_patterns)]`** — new error categories become compile errors in downstream crates
- **`AuthorityScope` hand-written 5×5 `satisfies()` truth table, no `Ord` derive** — authority is a ladder, not a total order

### Open TODOs

- None carried. v0.6 Plan 01-02 test-file clippy hygiene TODO closed 2026-04-13 in the audit tech-debt cleanup pass.

### Known Blockers

- **None.** v0.6 substrate is byte-exact, CI-enforced, and ready to feed v0.7 envelope/transport layers.

## Session Continuity

### Recent Activity

- **2026-04-13:** **v0.6 Foundation Crates milestone shipped and archived.** 3 phases, 9 plans, 16 tasks, 25/25 requirements satisfied. ROADMAP.md and REQUIREMENTS.md archived to `.planning/milestones/v0.6-*.md`; phase directories moved to `.planning/milestones/v0.6-phases/`. PROJECT.md evolved: all v0.6 requirements moved to Validated, Key Decisions annotated with outcomes. Next: `/gsd:new-milestone` for v0.7 Personal Runtime.
- **2026-04-13:** Phase 3 (core-types-invariants) complete. `famp-core` ships Principal/Instance, UUIDv7 IDs, ArtifactId, 15-category `ProtocolErrorKind`, `AuthorityScope` ladder, INV-1..INV-11 anchors, and exhaustive consumer stub. 66/66 famp-core + 112/112 workspace tests green.
- **2026-04-13:** Phase 2 (crypto-foundations) complete. Plan 02-04 closed CRYPTO-07 (SHA-256 content-addressing) with NIST KAT gate.
- **2026-04-13:** Phase 1 (canonical-json-foundations) complete. SEED-001 resolved: keep `serde_jcs`. 12/12 conformance gate green; nightly 100M float corpus workflow armed.

---
*State updated: 2026-04-13 — v0.6 shipped; awaiting v0.7 initialization*
