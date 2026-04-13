---
phase: 03-core-types-invariants
plan: 01
subsystem: core-types
tags: [rust, famp-core, uuid, serde, thiserror, identity, ids, artifact-id]

requires:
  - phase: 01-canonical-json-foundations
    provides: canonical-JSON artifact-id producer (sha256:<hex>) that this type parses
  - phase: 02-crypto-foundations
    provides: NIST SHA-256 KAT vector shared with ArtifactId boundary fixture
provides:
  - "Principal / Instance strict-ASCII identity types with separate parsers"
  - "Four distinct UUIDv7 newtypes: MessageId, ConversationId, TaskId, CommitmentId"
  - "ArtifactId with sha256:<64-lowercase-hex> parse-time invariant"
  - "Narrow ParsePrincipalError / ParseInstanceError / ParseArtifactIdError enums that do NOT cross-convert into ProtocolErrorKind"
  - "Full Display / FromStr / Serialize / Deserialize quad on every wire-facing type"
affects: [03-02, famp-envelope, famp-fsm, famp-identity, famp-causality]

tech-stack:
  added: [uuid, thiserror, serde]
  patterns:
    - "macro-generated newtype families for type-distinct wrappers over a shared primitive (ids.rs)"
    - "narrow, type-local parse error enums — no From<_> into future protocol-category enums"
    - "manual Serialize/Deserialize that delegates to FromStr/Display so the string form is the canonical wire form"

key-files:
  created:
    - crates/famp-core/src/identity.rs
    - crates/famp-core/src/ids.rs
    - crates/famp-core/src/artifact.rs
    - crates/famp-core/tests/identity_roundtrip.rs
    - crates/famp-core/tests/ids_roundtrip.rs
    - crates/famp-core/tests/artifact_roundtrip.rs
  modified:
    - crates/famp-core/Cargo.toml
    - crates/famp-core/src/lib.rs

key-decisions:
  - "Principal and Instance use separate parsers; cross-shape inputs are errors, not silently trimmed (D-01)"
  - "ID newtypes generated via macro to guarantee identical Display/FromStr/serde shape across the four types while remaining distinct at the type level (D-10)"
  - "ArtifactId stores the original validated string as-is and exposes only `as_str()` — no `algorithm()` accessor until a second hash lands (D-16)"
  - "Identity Serialize/Deserialize is manual (collect_str + FromStr), not derive, so the canonical wire form stays a string and not a struct (D-09)"
  - "`#[cfg(test)] use serde_json as _;` in lib.rs silences the workspace unused-crate-dependencies lint for the lib-test profile without affecting the dev-dep declaration"

patterns-established:
  - "Macro-driven newtype definitions (define_uuid_newtype!) for sibling types with identical trait surface"
  - "Private `validate_authority` / `validate_name_or_instance_id` helpers shared between Principal and Instance parsers"
  - "Narrow Parse*Error enums follow Phase 2 `CryptoError` discipline — compiler-checked match exhaustiveness, no upstream conversions"

requirements-completed: [CORE-01, CORE-02, CORE-03]

duration: 9min
completed: 2026-04-13
---

# Phase 3 Plan 1: Core Identity, ID, and Artifact Types Summary

**Ship compiler-enforced identity, UUIDv7 ID newtypes, and parsed `sha256:<hex>` ArtifactId with full Display / FromStr / serde round-trip quad — the vocabulary every downstream FAMP crate will depend on.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-04-13T14:43:38Z
- **Completed:** 2026-04-13T14:52:41Z
- **Tasks:** 4 / 4
- **Files created:** 6
- **Files modified:** 2
- **Tests:** 42 (23 identity + 7 ids + 12 artifact)

## Accomplishments

- `Principal` and `Instance` parse strict-ASCII DNS-style authority + `[A-Za-z0-9._-]{1,64}` name/instance-id. Case-sensitive byte-for-byte round trip. Underscore, non-ASCII, over-length, cross-shape inputs all rejected at parse time.
- Four distinct UUIDv7 newtypes — `MessageId`, `ConversationId`, `TaskId`, `CommitmentId` — share a macro-generated surface with centralized `new_v7()`. Canonical hyphenated form only; 32-char `uuid::simple` form rejected at `FromStr` and at deserialize.
- `ArtifactId` enforces the full `sha256:<64-lowercase-hex>` invariant at construction with a narrow three-variant `ParseArtifactIdError`. Uppercase hex, mixed case, wrong length, non-sha256 algorithms, and missing prefix all rejected. The Phase 2 NIST KAT empty-input vector round-trips cleanly via the boundary fixture.
- 42/42 tests green under `cargo nextest run -p famp-core`; `cargo clippy -p famp-core --all-targets -- -D warnings` and `cargo fmt -p famp-core -- --check` both clean.
- Per D-08 / D-18 / D-22 / D-35, no parse error implements `From<_>` into any future `ProtocolErrorKind` — boundary crates will do the translation explicitly.

## Task Commits

1. **Task 1: Add famp-core dependencies and module scaffolding** — `19e42b2` (chore)
2. **Task 2: Implement Principal and Instance identity types (CORE-01)** — `0eb9bd2` (feat)
3. **Task 3: Implement UUIDv7 ID newtypes (CORE-02)** — `d82a4ff` (feat)
4. **Task 4: Implement ArtifactId with sha256:<hex> invariant (CORE-03)** — `9d21d47` (feat)

## Files Created/Modified

- `crates/famp-core/Cargo.toml` — added serde(derive), thiserror, uuid(v7+serde) deps; serde_json dev-dep
- `crates/famp-core/src/lib.rs` — module declarations + public re-exports + cfg(test) serde_json shim
- `crates/famp-core/src/identity.rs` — Principal, Instance, ParsePrincipalError, ParseInstanceError, private validators
- `crates/famp-core/src/ids.rs` — `define_uuid_newtype!` macro + 4 newtype invocations
- `crates/famp-core/src/artifact.rs` — ArtifactId newtype, ParseArtifactIdError, FromStr/TryFrom/serde impls
- `crates/famp-core/tests/identity_roundtrip.rs` — 23 tests (happy path, boundary, rejection, serde)
- `crates/famp-core/tests/ids_roundtrip.rs` — 7 tests (round trip, distinctness, non-hyphenated rejection, non-string wire-form rejection)
- `crates/famp-core/tests/artifact_roundtrip.rs` — 12 tests (lowercase accept, uppercase/mixed/length/algo/prefix reject, TryFrom variants, KAT fixture)

## Decisions Made

- **Manual Serialize/Deserialize, not derive.** Delegating to `FromStr`/`Display` locks the wire form to the canonical string per D-09 — a `#[derive(Serialize, Deserialize)]` on `Principal` would have emitted struct-shaped JSON.
- **Macro for ID newtypes.** A single `define_uuid_newtype!` macro keeps the four types' trait surfaces byte-identical while preserving compile-time distinctness (D-10). Avoids the maintenance hazard of four 50-line copy-pasted blocks drifting over time.
- **Validated string stored as-is in `ArtifactId`.** Rather than parsing into `(algo, hex)` tuples, the type owns the original validated string and exposes `as_str()` only. Keeps the invariant simple and defers the "second algorithm" generalization to the future moment it actually matters (D-16).
- **Private `validate_authority` / `validate_name_or_instance_id` helpers** shared between `Principal::from_str` and `Instance::from_str`. Single source of truth for D-04 / D-05 / D-06 validation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `const fn as_uuid` required by clippy nursery**
- **Found during:** Task 3 (ids.rs clippy pass)
- **Issue:** Workspace `clippy::nursery = warn` + `-D warnings` elevated `missing_const_for_fn` to an error on `pub fn as_uuid(&self) -> &uuid::Uuid`.
- **Fix:** Added `const` to the accessor.
- **Committed in:** `d82a4ff`

**2. [Rule 3 - Blocking] `#[allow(clippy::struct_field_names)]` on Instance**
- **Found during:** Task 2 (identity.rs clippy pass)
- **Issue:** `clippy::pedantic` flagged the `instance_id` field on `Instance` as "field name starts with the struct's name". Renaming would have changed the public accessor API for no semantic benefit.
- **Fix:** Local `#[allow]` on the struct.
- **Committed in:** `0eb9bd2`

**3. [Rule 3 - Blocking] Test file clippy hygiene allow-list**
- **Found during:** Task 2 verification
- **Issue:** Workspace `unwrap_used = "deny"` / `expect_used = "deny"` blocked all three integration-test files (`#[test]` bodies depend heavily on `unwrap()`).
- **Fix:** Added `#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]` at the top of each test file. Matches the open TODO already logged in STATE.md under "Test-files clippy hygiene sweep" carried from Plan 01-02 — not new scope, continuing the existing pattern.
- **Committed in:** `0eb9bd2`, `d82a4ff`, `9d21d47`

**4. [Rule 3 - Blocking] `#[cfg(test)] use serde_json as _;` in lib.rs**
- **Found during:** Task 2 clippy pass
- **Issue:** Workspace `unused_crate_dependencies = "warn"` elevated to error complained that `serde_json` (declared as a dev-dep for integration tests under `tests/`) was unused by the library test profile.
- **Fix:** Added a cfg(test) sink import. Matches the `use sha2 as _;` pattern documented in STATE.md from Phase 2.
- **Committed in:** `0eb9bd2`

**5. [Rule 3 - Blocking] `cargo fmt -p famp-core` rewrites**
- **Found during:** Task 4 fmt --check
- **Issue:** rustfmt reformatted a handful of lines in `identity.rs`, `lib.rs`, and `identity_roundtrip.rs` (multi-line argument lists, alphabetized module order).
- **Fix:** Applied `cargo fmt -p famp-core`.
- **Committed in:** `9d21d47`

---

**Total deviations:** 5 auto-fixed (all Rule 3 — blocking issues resolved to unblock clippy / fmt gates). No scope creep; no behavior changes beyond what the plan specified.
**Impact on plan:** None. All five are mechanical lint-gate adjustments matching patterns already documented in prior-phase STATE.md carryovers.

## Issues Encountered

- **`unwrap_used` lint denied in `FromStr` impl inside macro.** The plan's illustrative snippet used `uuid::Uuid::parse_str("").unwrap_err()` to synthesize a canonical `uuid::Error` for non-hyphenated inputs. This trips `unwrap_used = "deny"`. Rewrote to `uuid::Uuid::parse_str("!")?;` which propagates the error via `?` and is lint-clean. Same observable behavior.
- **Pre-existing `cargo` not on PATH in the execution shell.** Resolved by sourcing `~/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin` into `PATH` for the duration of the session. No project change needed.

## Next Phase Readiness

- `famp-core` now exports `Principal`, `Instance`, `MessageId`, `ConversationId`, `TaskId`, `CommitmentId`, `ArtifactId` plus their narrow parse-error enums. All types ship with the full round-trip quad and are clippy-clean under workspace pedantic + nursery settings.
- **Ready for Plan 03-02:** ProtocolErrorKind / AuthorityScope / `invariants` module — the remaining surface needed to close CORE-04..06. No blockers.
- Boundary fixture with `famp-canonical`'s `sha256_artifact_id` is pre-wired via the NIST empty-input KAT vector and will pair trivially when Phase 4 wires an envelope crate.

## Self-Check: PASSED

- Files exist: identity.rs, ids.rs, artifact.rs, tests/{identity,ids,artifact}_roundtrip.rs — FOUND
- Commits exist: 19e42b2, 0eb9bd2, d82a4ff, 9d21d47 — FOUND in `git log`
- Tests: 42/42 green under `cargo nextest run -p famp-core`
- Lint: `cargo clippy -p famp-core --all-targets -- -D warnings` clean
- Format: `cargo fmt -p famp-core -- --check` clean

---
*Phase: 03-core-types-invariants*
*Completed: 2026-04-13*
