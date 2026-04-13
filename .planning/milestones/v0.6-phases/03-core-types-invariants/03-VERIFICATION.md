---
phase: 03-core-types-invariants
verified: 2026-04-13T00:00:00Z
status: passed
score: 10/10 must-haves verified
---

# Phase 3: Core Types & Invariants Verification Report

**Phase Goal:** Ship `famp-core` shared value-type substrate: Principal/Instance identity, four distinct UUIDv7 ID newtypes, ArtifactId with `sha256:<hex>` invariant, flat `ProtocolErrorKind` (15 variants), `AuthorityScope` ladder, and `invariants` module (INV-1..INV-11).
**Verified:** 2026-04-13
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Plan 03-01 + 03-02)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Principal parses `agent:<authority>/<name>` and rejects instance-bearing strings | VERIFIED | `identity.rs` L-L263; `identity_roundtrip` 23 tests incl. `principal_rejects_instance_tail`, underscore/non-ASCII/length rejection |
| 2 | Instance parses `agent:<authority>/<name>#<instance_id>` and rejects principal-only strings | VERIFIED | `identity.rs`; tests cover `instance_requires_tail`, serde round-trip |
| 3 | MessageId/ConversationId/TaskId/CommitmentId are distinct types not swappable | VERIFIED | `ids.rs` via `define_uuid_newtype!` macro (4 invocations); `distinct_type_ids` test asserts TypeId distinctness |
| 4 | ArtifactId parses `sha256:<64-lowercase-hex>` and rejects uppercase/non-sha256 | VERIFIED | `artifact.rs` 91 lines; `artifact_roundtrip` 12 tests incl. NIST KAT empty-input vector |
| 5 | Every wire-facing type round-trips Display↔FromStr and Serialize↔Deserialize byte-for-byte | VERIFIED | Manual serde impls (not derive) across identity.rs, ids.rs, artifact.rs; round-trip tests pass |
| 6 | ProtocolErrorKind has exactly 15 unit variants matching spec §15.1 wire strings | VERIFIED | `error.rs` — 15 variants via grep count; `error_wire_strings` fixture tests all 15 bidirectionally |
| 7 | AuthorityScope has exactly 5 unit variants matching spec §5.3 wire strings | VERIFIED | `scope.rs`; `scope_wire_strings` 6 tests |
| 8 | AuthorityScope::satisfies implements locked 5×5 ladder truth table | VERIFIED | `scope.rs` private `const fn rank`; `scope_satisfies::truth_table_matches` 25-entry hand-written table |
| 9 | invariants module exposes INV_1..INV_11 public items with non-empty doc comments | VERIFIED | `invariants.rs` — all 11 `pub const INV_N` present; `invariants_present` 4 tests (presence, docs>20 chars, resolve, distinct) |
| 10 | Exhaustive consumer stub compiles match over every variant with no wildcard — adds CI gate | VERIFIED | `exhaustive_consumer_stub.rs` `#![deny(unreachable_patterns)]`, no `_ =>` arms (grep confirmed), 15 + 5 arms |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-core/src/identity.rs` | Principal, Instance, Parse errors | VERIFIED | 263 lines, substantive, wired in lib.rs |
| `crates/famp-core/src/ids.rs` | 4 UUIDv7 newtypes with new_v7 | VERIFIED | 66 lines, macro-generated, wired |
| `crates/famp-core/src/artifact.rs` | ArtifactId + ParseArtifactIdError | VERIFIED | 91 lines, substantive, wired |
| `crates/famp-core/src/error.rs` | ProtocolErrorKind (15) + ProtocolError | VERIFIED | 78 lines, 15 variants confirmed, wired |
| `crates/famp-core/src/scope.rs` | AuthorityScope + satisfies + ParseAuthorityScopeError | VERIFIED | 84 lines, private `rank`, wired |
| `crates/famp-core/src/invariants.rs` | INV_1..INV_11 pub const with docs | VERIFIED | 56 lines, 11 constants, doc comments per test |
| `crates/famp-core/tests/exhaustive_consumer_stub.rs` | Exhaustive compile gate | VERIFIED | 108 lines, no wildcard, `deny(unreachable_patterns)` |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `lib.rs` | all 6 src modules | `pub mod` + `pub use` re-exports (invariants stays namespaced per D-26) | WIRED |
| `Cargo.toml` | uuid v7+serde, serde derive, thiserror | workspace dep inheritance with features | WIRED (verified by successful test build) |
| `exhaustive_consumer_stub.rs` | `ProtocolErrorKind` + `AuthorityScope` | exhaustive `match` without wildcard | WIRED |
| `error_wire_strings.rs` | spec §15.1 wire strings | 15-entry fixture table, bidirectional serde | WIRED |
| `scope_satisfies.rs` | D-32 ladder | hand-written 25-entry const truth table | WIRED |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CORE-01 | 03-01 | Principal/Instance with parse/display round-trip | SATISFIED | Truths #1, #2, #5 |
| CORE-02 | 03-01 | MessageId/ConversationId/TaskId/CommitmentId UUIDv7 | SATISFIED | Truth #3, #5 |
| CORE-03 | 03-01 | ArtifactId `sha256:` content-addressed type | SATISFIED | Truth #4, #5 |
| CORE-04 | 03-02 | Typed error enum with 15 §15.1 categories | SATISFIED | Truths #6, #10 |
| CORE-05 | 03-02 | INV-1..INV-11 documented in code | SATISFIED | Truth #9 |
| CORE-06 | 03-02 | AuthorityScope enum with ladder | SATISFIED | Truths #7, #8, #10 |

No orphaned requirements. Both plans' `requirements:` frontmatter fully cover CORE-01..06.

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| — | None detected | — | — |

Scanned all 7 src files and 8 test files:
- No `to_lowercase`/`to_ascii_lowercase` in identity.rs (D-07 honored)
- No `From<CanonicalError>` / `From<CryptoError>` into ProtocolErrorKind (D-22 honored)
- No `Ord`/`PartialOrd` derive on `AuthorityScope` (D-31 honored — only on ids.rs newtypes, which is distinct and intended)
- No `pub fn rank` — only private `const fn rank` (D-33 honored)
- No `_ =>` wildcard arms in `exhaustive_consumer_stub.rs` (D-24 honored)
- No TODO/FIXME/placeholder/stub patterns

### Test Execution

`cargo nextest run -p famp-core` — **66 tests passed, 0 skipped, 0 failed** across 8 test binaries:
- identity_roundtrip (23)
- ids_roundtrip (7)
- artifact_roundtrip (12)
- error_wire_strings (6)
- scope_wire_strings (6)
- scope_satisfies (5)
- invariants_present (4)
- exhaustive_consumer_stub (3)

### Gaps Summary

None. All phase 3 Success Criteria satisfied; all six CORE requirements fully implemented, wired into `lib.rs`, fixture-gated, and CI-enforceable via the exhaustive consumer stub. `famp-core` is ready to be consumed by `famp-envelope` in v0.7.

---

*Verified: 2026-04-13*
*Verifier: Claude (gsd-verifier)*
