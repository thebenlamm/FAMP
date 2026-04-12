# STATE: FAMP v0.5 Rust Reference Implementation

**Last Updated:** 2026-04-12

## Project Reference

**Core Value:** A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one. If canonicalization or signature verification disagrees, nothing else matters.

**Current Focus:** Roadmap approved — ready for Phase 0 (Toolchain & Workspace Scaffold).

## Current Position

**Milestone:** v1 (Level 2 + Level 3 conformance)
**Phase:** 0 — Toolchain & Workspace Scaffold
**Plan:** Not yet planned
**Status:** Roadmap created; awaiting `/gsd:plan-phase 0`
**Progress:** `[░░░░░░░░░]` 0/9 phases complete

## Performance Metrics

| Metric | Value |
|--------|-------|
| Phases complete | 0 / 9 |
| Requirements validated | 0 / 153 |
| Requirements mapped | 153 / 153 (100%) |
| CI status | Not yet established |

## Accumulated Context

### Key Decisions Logged

- **Language: Rust** — Compiler-checked INV-5 via exhaustive enum `match`; byte-exact Ed25519 + canonical JSON; single core can later feed Python/TS bindings
- **Ship Level 2 + Level 3 together** — L1-only doesn't exercise signature discipline
- **Fork spec to v0.5.1** — review findings are real spec bugs, not ambiguities
- **12-crate workspace + umbrella** — DAG acyclic; Phase 2-3 may temporarily merge to `famp-foundation` for beginner build velocity
- **`serde_jcs` wrapped in `famp-canonical`** with RFC 8785 CI gate + documented ~500 LoC fallback
- **Only `verify_strict`** exposed from `famp-crypto`; weak keys rejected at ingress
- **tokio + axum + rustls + reqwest** — no async-std, no OpenSSL, no actix
- **Native `async fn` in traits** (Rust ≥1.75), no `#[async_trait]`
- **`MemoryTransport` + `HttpTransport` both in v1** — memory runs flows in microseconds, HTTP is the wire reference

### Open TODOs

- Run `/gsd:research-phase 1` before planning Phase 1 (spec fork)
- Phase 2 number formatter decision: `ryu-js` vs port from cyberphone C reference
- Phase 6/7 concurrency testing: `loom` vs `shuttle`
- Phase 8: identify second implementation (Python?) for independent vector generation

### Known Blockers

- **User is new to Rust** — Phase 0 must budget for `cargo` edit-build-test loop learning before FAMP code begins
- **`serde_jcs` correctness unknown** on RFC 8785 edge cases — fallback plan ready if CI gate fails
- **No git history yet** — repo contains only `FAMP-v0.5-spec.md` + `.planning/`

## Session Continuity

### Next Session Starts With

1. Review `.planning/ROADMAP.md` if anything unclear
2. Run `/gsd:research-phase 0` (optional — Phase 0 is low-risk) OR proceed directly to `/gsd:plan-phase 0`
3. Phase 0 deliverables: rustup install, workspace scaffold, CI green on empty stubs

### Recent Activity

- **2026-04-12:** Project initialized; PROJECT.md, REQUIREMENTS.md (153 v1 reqs across 16 categories), research/ (SUMMARY, ARCHITECTURE, PITFALLS) created
- **2026-04-12:** ROADMAP.md created — 9 phases derived from research DAG; 100% requirement coverage validated
- **2026-04-12:** STATE.md initialized

---
*State initialized: 2026-04-12*
