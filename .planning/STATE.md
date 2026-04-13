---
gsd_state_version: 1.0
milestone: v0.7
milestone_name: Personal Runtime
status: Ready to plan
last_updated: "2026-04-13T21:57:55.378Z"
progress:
  total_phases: 4
  completed_phases: 3
  total_plans: 10
  completed_plans: 10
  percent: 100
---

# STATE: FAMP — v0.7 Personal Runtime

**Last Updated:** 2026-04-13

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-13 after v0.6 milestone)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 03 — memorytransport-tofu-keyring-same-process-example

## Current Position

Phase: 4
Plan: Not started

## Last Shipped

- **Phase 02 Plan 03** (2026-04-13) — FSM-03 consumer stub under `#![deny(unreachable_patterns)]` + FSM-08 proptest 100-tuple legality matrix. 5 FSM requirements all satisfied. 200/200 workspace tests. Commits: `4460297`, `507c565`.
- **Phase 02 Plan 02** (2026-04-13) — `famp-fsm` crate: 5-state `TaskFsm` engine with single-function transition table (5 legal arrows), terminal immutability, 12 deterministic fixture tests. FSM-02, FSM-04, FSM-05 satisfied. 195/195 workspace tests. Commits: `26f7f13`, `7b79019`.
- **Phase 02 Plan 01** (2026-04-13) — Type lift: `MessageClass` and `TerminalStatus` moved to `famp-core`. Backward-compatible re-exports in `famp-envelope`. 184/184 workspace tests green. `famp-fsm` crate layering constraint (D-D1) now satisfiable.
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
- **v0.7 TOFU keyring stays local-file** — `HashMap<Principal, VerifyingKey>`, principal = raw Ed25519 pubkey, loaded from file or `--peer` CLI flag. Explicitly not an "identity system" or "trust store"; Agent Cards defer to v0.8.
- **v0.7 adversarial matrix = 3 cases × 2 transports, not 18** — CONF-05/06/07 own the three cases on `MemoryTransport`; Phase 4 extends the same matrix to HTTP without introducing new CONF-0x requirements.
- **ENV-09 and ENV-12 are intentionally narrowed for v0.7** — ENV-09 ships with no capability-snapshot binding; ENV-12 ships cancel-only (no supersede, no close). The wider v0.6-catalog forms defer to Federation Profile.
- **D-B5 + D-D1 resolved (2026-04-13, Phase 02 Plan 01):** `TerminalStatus` lifted to `famp-core` alongside `MessageClass`; `famp-fsm` depends only on `famp-core`, never on `famp-envelope`. Backward-compatible re-export pattern established: define in lower crate, `pub use` in former home.
- **relation field dropped from TaskTransitionInput (2026-04-13, Phase 02 Plan 02):** D-B3 resolved — no v0.7 legal arrow needs relation inspection. `TaskTransitionInput` carries only `class` and `terminal_status`.
- **FSM step() and accessors are const fn (2026-04-13, Phase 02 Plan 02):** All transition arms operate on Copy enums; clippy `missing_const_for_fn` (pedantic) required this. Correct and free.
- **Clippy pedantic test file pattern (2026-04-13, Phase 02 Plan 02):** Test integration files use `#![allow(clippy::unwrap_used, unused_crate_dependencies)]` per famp-envelope precedent. Consistent pattern across all famp-* test files.
- **match_same_arms allowed in FSM-03 consumer stub (2026-04-13, Phase 02 Plan 03):** Intentionally separate arms in is_terminal (Requested=>false, Committed=>false) document each variant explicitly — the purpose of the exhaustiveness gate. Merging to or-pattern defeats FSM-03's compile-time documentation intent.
- **#[cfg(test)] use X as _; pattern for lib.rs (2026-04-13, Phase 02 Plan 03):** When a dev-dep is used only by integration tests, add `#[cfg(test)] use X as _;` after module-level `//!` doc comments in lib.rs to silence `unused_crate_dependencies` on the lib target. Pattern from famp-envelope, now extended to famp-fsm.

### Open TODOs

- None carried. v0.6 Plan 01-02 test-file clippy hygiene TODO closed 2026-04-13.

### Known Blockers

- **None.** v0.6 substrate is byte-exact, CI-enforced, and ready to feed v0.7 envelope/transport layers.

## Session Continuity

### Recent Activity

- **2026-04-13:** **Phase 02 Plan 03 complete.** FSM-03 compile-time consumer stub + FSM-08 proptest 100-tuple legality matrix. All 5 Phase 2 FSM requirements satisfied. 200/200 workspace tests. Commits: `4460297`, `507c565`.
- **2026-04-13:** **Phase 02 Plan 02 complete.** `famp-fsm` 5-state TaskFsm engine. 5-arrow transition table, terminal immutability, 12 fixture tests, clippy pedantic clean. 195/195 workspace tests. Commits: `26f7f13`, `7b79019`.
- **2026-04-13:** **Phase 02 Plan 01 complete.** Type lift: `MessageClass` and `TerminalStatus` to `famp-core`. D-D1 crate layering blocker resolved. 184/184 workspace tests. Commits: `65c8ac1`, `012b807`.
- **2026-04-13:** **v0.7 roadmap canonicalized.** 4 phases, 32 requirements, 100% coverage. Phase 1 (Minimal Signed Envelope) queued for `/gsd:plan-phase 1`.
- **2026-04-13:** **v0.6 Foundation Crates milestone shipped and archived.** 3 phases, 9 plans, 16 tasks, 25/25 requirements satisfied. ROADMAP.md and REQUIREMENTS.md archived to `.planning/milestones/v0.6-*.md`; phase directories moved to `.planning/milestones/v0.6-phases/`. PROJECT.md evolved: all v0.6 requirements moved to Validated, Key Decisions annotated with outcomes.
- **2026-04-13:** Phase 3 (core-types-invariants) complete. `famp-core` ships Principal/Instance, UUIDv7 IDs, ArtifactId, 15-category `ProtocolErrorKind`, `AuthorityScope` ladder, INV-1..INV-11 anchors, and exhaustive consumer stub. 66/66 famp-core + 112/112 workspace tests green.
- **2026-04-13:** Phase 2 (crypto-foundations) complete. Plan 02-04 closed CRYPTO-07 (SHA-256 content-addressing) with NIST KAT gate.
- **2026-04-13:** Phase 1 (canonical-json-foundations) complete. SEED-001 resolved: keep `serde_jcs`. 12/12 conformance gate green; nightly 100M float corpus workflow armed.

---
*State updated: 2026-04-13 — Phase 02 Plan 03 complete; Phase 02 all plans done (FSM-02, FSM-03, FSM-04, FSM-05, FSM-08 satisfied); awaiting verifier*
