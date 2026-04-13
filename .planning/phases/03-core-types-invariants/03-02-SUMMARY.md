---
phase: 03-core-types-invariants
plan: 02
subsystem: core-types
tags: [rust, famp-core, thiserror, serde, error-vocabulary, authority-ladder, invariants]

requires:
  - phase: 03-core-types-invariants
    provides: "Plan 03-01 Principal/Instance, ID newtypes, ArtifactId"
provides:
  - "ProtocolErrorKind — 15-variant flat enum (FAMP v0.5.1 §15.1) with locked snake_case wire form"
  - "ProtocolError wrapper with optional detail (no Serialize, D-21)"
  - "AuthorityScope — 5-variant enum (§5.3) with explicit satisfies() ladder and ParseAuthorityScopeError"
  - "invariants module: INV_1..INV_11 public doc-bearing constants"
  - "Compile-checked exhaustive-match consumer stub — adding a variant without updating it is a hard compile error"
  - "just test-core recipe mirroring test-canonical / test-crypto"
affects: [famp-envelope, famp-fsm, famp-identity, famp-causality, famp-transport]

tech-stack:
  added: []
  patterns:
    - "Flat unit-variant enum with serde(rename_all) as the wire-stable vocabulary"
    - "Semantic ladder via private `rank` helper — no Ord derive, exhaustive match as compile gate"
    - "Namespaced doc-anchor constants for intra-doc-link invariants"
    - "Exhaustive consumer stub as CI-enforced compile gate for enum completeness"

key-files:
  created:
    - crates/famp-core/src/error.rs
    - crates/famp-core/src/scope.rs
    - crates/famp-core/src/invariants.rs
    - crates/famp-core/tests/error_wire_strings.rs
    - crates/famp-core/tests/scope_wire_strings.rs
    - crates/famp-core/tests/scope_satisfies.rs
    - crates/famp-core/tests/invariants_present.rs
    - crates/famp-core/tests/exhaustive_consumer_stub.rs
  modified:
    - crates/famp-core/src/lib.rs
    - Justfile

key-decisions:
  - "ProtocolErrorKind is a flat unit-variant enum — no structured payload on variants (D-19). Exhaustive match + wire-stable serde form outweighs the convenience of per-variant context."
  - "ProtocolError wrapper ships without Serialize — the wire shape `{error, detail}` is the envelope crate's responsibility, not famp-core (D-21)."
  - "AuthorityScope uses a private `rank` helper and explicit `satisfies`, not derived Ord — the ladder is semantic, not lexical; declaration-order coupling would be a hazard (D-31/D-33)."
  - "Truth table for satisfies is hand-written as a 25-entry const array — no programmatic derivation, per D-32."
  - "invariants are `pub const INV_N = \"INV-N\"` with substantive doc comments; constants stay namespaced (no root re-export) so intra-doc links read `famp_core::invariants::INV_10` (D-26)."
  - "Exhaustive consumer stub uses `#![deny(unreachable_patterns)]` to reinforce that no wildcard `_ =>` arm can sneak in later."

requirements-completed: [CORE-04, CORE-05, CORE-06]

duration: "4min"
completed: 2026-04-13
---

# Phase 3 Plan 2: ProtocolErrorKind, AuthorityScope, and Invariants Summary

**Lock the wire-error vocabulary, authority-ladder semantics, and INV-1..INV-11 documentation anchors that every downstream FAMP crate will match against — closing Phase 3's success criteria 3, 4, and 5.**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-04-13T14:55:33Z
- **Completed:** 2026-04-13T14:59:52Z
- **Tasks:** 4 / 4
- **Files created:** 8
- **Files modified:** 2
- **Tests:** 24 new (6 error_wire_strings + 6 scope_wire_strings + 5 scope_satisfies + 4 invariants_present + 3 exhaustive_consumer_stub) bringing famp-core total to 66/66 green

## Accomplishments

- **`ProtocolErrorKind`** ships as a 15-variant flat enum (D-19) with `#[serde(rename_all = "snake_case")]`, matching spec §15.1 byte-for-byte. Wire strings (`malformed`, `unauthorized`, `out_of_scope`, `commitment_missing`, etc.) are locked by a 15-entry fixture table that asserts both directions of round-trip plus `Display` + unknown-variant rejection.
- **`ProtocolError`** wrapper pairs a kind with optional detail. Implements `std::error::Error` via `thiserror` but deliberately does NOT derive `Serialize` — the wire shape is the envelope crate's job (D-21). No `From<CanonicalError>` or `From<CryptoError>` conversions — boundary crates must translate explicitly (D-22).
- **`AuthorityScope`** ships as a 5-variant enum (Advisory → Transfer) with `Display` / `FromStr` / serde all delegating to a private `as_wire` helper. `satisfies` routes through a private `rank(u8)` helper whose exhaustive match locks the ladder to the intended advisory < negotiate < commit_local < commit_delegate < transfer ordering regardless of declaration order. No `Ord`/`PartialOrd` derives.
- **5×5 satisfies truth table** hand-written as a 25-entry `const` array (D-32), asserted against `satisfies` with reflexivity and boundary spot-checks.
- **`invariants` module** publishes `INV_1..INV_11` as namespaced `pub const` anchors with substantive rustdoc drawn from FAMP v0.5 §3 invariants text. A fixture test scans `include_str!("../src/invariants.rs")` to enforce: (a) exactly 11 `pub const INV_` declarations, (b) each preceded by a doc-comment block totalling >20 characters, (c) all 11 constants resolve and are distinct.
- **Exhaustive consumer stub** compiles `describe_error` / `describe_scope` with no `_ =>` wildcard arms, under `#![deny(unreachable_patterns)]`. Adding a 16th `ProtocolErrorKind` or 6th `AuthorityScope` variant without updating the stub is a hard compile error. Includes a cross-module smoke test exercising the full Phase 3 public surface (Principal, Instance, MessageId, ArtifactId, ProtocolErrorKind, AuthorityScope, INV_10).
- **Justfile** gains `test-core` recipe mirroring `test-canonical` / `test-crypto`.
- **All gates green:** `cargo nextest run -p famp-core` = 66/66 across 8 test binaries; `cargo clippy -p famp-core --all-targets -- -D warnings` clean; `cargo fmt -p famp-core -- --check` clean.

## Task Commits

1. **Task 1: ProtocolErrorKind + ProtocolError wrapper (CORE-04)** — `c19c8fb` (feat)
2. **Task 2: AuthorityScope enum with satisfies ladder (CORE-06)** — `92fe796` (feat)
3. **Task 3: invariants module with INV_1..INV_11 anchors (CORE-05)** — `dead093` (feat)
4. **Task 4: Exhaustive consumer stub + test-core Just recipe** — `6b66678` (feat)

## Files Created/Modified

### Created
- `crates/famp-core/src/error.rs` — ProtocolErrorKind (15 variants) + ProtocolError wrapper + module docs on D-22 boundary discipline
- `crates/famp-core/src/scope.rs` — AuthorityScope + ParseAuthorityScopeError + private rank/as_wire helpers + satisfies ladder
- `crates/famp-core/src/invariants.rs` — 11 `pub const INV_N: &str = "INV-N"` anchors with substantive doc comments
- `crates/famp-core/tests/error_wire_strings.rs` — 6 tests (fixture length, serialize, deserialize, Display, unknown rejected, wrapper Error impl)
- `crates/famp-core/tests/scope_wire_strings.rs` — 6 tests (fixture length, serialize, deserialize, Display/FromStr, unknown rejected × 2)
- `crates/famp-core/tests/scope_satisfies.rs` — 5 tests (table length, full 25-entry truth table, reflexivity, transfer→advisory, advisory↛negotiate)
- `crates/famp-core/tests/invariants_present.rs` — 4 tests (11 consts present, 11 substantive docs, resolve, distinct)
- `crates/famp-core/tests/exhaustive_consumer_stub.rs` — 3 tests (all 15 kinds, all 5 scopes, Phase 3 surface smoke)

### Modified
- `crates/famp-core/src/lib.rs` — added `pub mod error|scope|invariants` + root re-exports for ProtocolError/ProtocolErrorKind/AuthorityScope/ParseAuthorityScopeError (INV_N stays namespaced per D-26)
- `Justfile` — added `test-core` recipe

## Decisions Made

- **Flat ProtocolErrorKind, no per-variant payloads.** Trading the ergonomics of attached context for compile-checked exhaustive match + wire-stable serde. Boundary crates that need context wrap `ProtocolError { kind, detail }`.
- **No `Serialize` on `ProtocolError`.** Deferring the wire shape to the envelope crate avoids premature commitment to `{error, detail}` sibling-field layout vs a tagged algebraic representation. Envelope will own the field positions.
- **Private `rank` helper.** The exhaustive match inside `rank` is what makes adding a 6th `AuthorityScope` variant a compile error — not declaration-order semantics leaked into a public API. Ranks stay implementation detail.
- **Hand-written 25-entry truth table.** D-32 demands no programmatic derivation; the test IS the spec, and reviewers can eyeball 25 rows.
- **`include_str!` + line-count doc check.** Chose `include_str!` over build scripts or trybuild: zero infrastructure, runs in the normal test profile, catches silent deletion of both constants and their doc comments.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `clippy::doc_markdown` flagged bare `snake_case` / ladder words in rustdoc**
- **Found during:** Task 1, Task 2 verification
- **Issue:** Workspace `clippy::pedantic = warn` + `-D warnings` elevated `doc_markdown` to an error on phrases like "snake_case per-variant" and "advisory < negotiate < commit_local < commit_delegate < transfer" in the error/scope module docs.
- **Fix:** Wrapped identifiers in backticks (`` `snake_case` ``, `` `commit_delegate` ``, etc.) in `src/error.rs` and `src/scope.rs`; added `#![allow(clippy::doc_markdown)]` to `tests/scope_satisfies.rs` where ladder text spans the full module-level comment.
- **Commits:** `c19c8fb`, `92fe796`

**2. [Rule 3 - Blocking] `clippy::missing_const_for_fn` on `describe_error` / `describe_scope`**
- **Found during:** Task 4 verification
- **Issue:** Workspace `clippy::nursery = warn` + `-D warnings` elevated `missing_const_for_fn` on the two exhaustive-match helpers.
- **Fix:** Added `const` to both signatures. No behavior change.
- **Commit:** `6b66678`

**3. [Rule 3 - Blocking] `cargo fmt` reflow across Plan 03-02 files**
- **Found during:** Task 4 final gate
- **Issue:** rustfmt reflowed derive lists, import lines, and a couple of long test-file lines.
- **Fix:** Applied `cargo fmt -p famp-core`. Mechanical, no semantic changes.
- **Commit:** `6b66678`

---

**Total deviations:** 3 auto-fixed (all Rule 3 — blocking lint/format gates). No scope creep; no behavior changes. Every fix is a pattern already established in Plan 03-01's deviation log.

## Issues Encountered

- **Rustfmt reflowed line 31 of `tests/error_wire_strings.rs`** (splitting `(ProtocolErrorKind::DelegationForbidden, "delegation_forbidden")` onto two lines because the original single-line form exceeded width). Mechanical, committed with Task 4's fmt pass.
- **Pre-existing `cargo` not on PATH in the execution shell** — resolved by exporting `$HOME/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin` into `PATH` for the session, matching Plan 03-01's carryover note. No project change needed.

## Phase 3 Success Criteria Map

| # | Criterion | Closed by |
|---|---|---|
| 1 | Principal and Instance parse/display round-trip | Plan 03-01 (identity_roundtrip, 23 tests) |
| 2 | Five distinct ID newtypes cannot be swapped | Plan 03-01 (ids_roundtrip, 7 tests) |
| 3 | Typed error enum covers all 15 §15.1 categories; exhaustive match verified by consumer stub | **Plan 03-02 Task 1 + Task 4** (error_wire_strings + exhaustive_consumer_stub) |
| 4 | INV-1..INV-11 documented in code | **Plan 03-02 Task 3** (invariants_present, 4 tests) |
| 5 | AuthorityScope defined with exhaustive match verified by consumer stub | **Plan 03-02 Task 2 + Task 4** (scope_wire_strings + scope_satisfies + exhaustive_consumer_stub) |

## Next Phase Readiness

- **Phase 3 complete.** `famp-core` now exports the full v0.6 core-types surface: `Principal`, `Instance`, `MessageId`, `ConversationId`, `TaskId`, `CommitmentId`, `ArtifactId`, `ProtocolErrorKind`, `ProtocolError`, `AuthorityScope`, `ParseAuthorityScopeError`, and namespaced `invariants::INV_1..INV_11`. All wire forms are fixture-locked; adding any new enum variant is a hard compile error until the consumer stub is updated.
- **v0.6 Foundation Crates milestone is code-complete.** `famp-canonical` (Phase 1) + `famp-crypto` (Phase 2) + `famp-core` (Phase 3) all ship with CI-gated conformance tests. Ready for v0.7 Personal Runtime (`famp-envelope` as the first consumer of this core vocabulary).
- **No blockers.** Test-files clippy-hygiene TODO carried from Plan 01-02 remains an open non-blocking item (all new Plan 03-02 test files already include the `unwrap_used`/`expect_used`/`unused_crate_dependencies` allow header; no sweep needed for new code).

## Known Stubs

None. Every constant, function, and type added in this plan is fully wired and exercised by tests.

## Self-Check: PASSED

- Files exist: error.rs, scope.rs, invariants.rs, tests/{error_wire_strings,scope_wire_strings,scope_satisfies,invariants_present,exhaustive_consumer_stub}.rs — FOUND
- Commits exist: c19c8fb, 92fe796, dead093, 6b66678 — FOUND in `git log`
- Tests: 66/66 green under `cargo nextest run -p famp-core` (8 test binaries)
- Lint: `cargo clippy -p famp-core --all-targets -- -D warnings` clean
- Format: `cargo fmt -p famp-core -- --check` clean
- Just recipe: `just test-core` runs nextest + doc tests (0 doctests currently) and exits 0

---
*Phase: 03-core-types-invariants*
*Completed: 2026-04-13*
