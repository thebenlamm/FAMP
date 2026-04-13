---
phase: 01-canonical-json-foundations
plan: 01
subsystem: canonical-json
tags: [rust, serde_jcs, serde_json, sha2, thiserror, rfc8785, cargo-workspace]

requires:
  - phase: 00-toolchain-workspace-scaffold
    provides: Rust 1.87 toolchain, cargo workspace, famp-canonical crate stub
provides:
  - "famp-canonical crate dependency wiring (serde_jcs, serde_json+float_roundtrip, sha2, thiserror)"
  - "Workspace serde_json pin updated to enable float_roundtrip (RESEARCH Pitfall 1)"
  - "fallback.md written on disk before any RFC 8785 conformance gate runs (D-08, CANON-07)"
  - "RFC 8785 Appendix B 27-vector test harness (gated wave2_impl)"
  - "Sampled float corpus driver with committed seed famp-canonical-float-corpus-v1 (CANON-03)"
  - "Duplicate-key, supplementary-plane, and SHA-256 artifact-id test stubs"
  - "Justfile test-canonical recipe for fast feedback loop"
affects: [01-02 canonical-engine-implementation, 01-03 seed-001-decision, 02-crypto-foundations]

tech-stack:
  added: [serde_jcs 0.2.0, sha2 0.11.0, thiserror 2.0.18 (wired into famp-canonical)]
  patterns:
    - "Workspace dependency pinning + per-crate { workspace = true } reference"
    - "Feature-gated test scaffolding (#![cfg(feature = \"wave2_impl\")]) so test files compile against an empty crate and unlock when production code lands"

key-files:
  created:
    - crates/famp-canonical/docs/fallback.md
    - crates/famp-canonical/tests/conformance.rs
    - crates/famp-canonical/tests/float_corpus.rs
    - crates/famp-canonical/tests/utf16_supplementary.rs
    - crates/famp-canonical/tests/duplicate_keys.rs
    - crates/famp-canonical/tests/artifact_id.rs
    - crates/famp-canonical/tests/vectors/input/.gitkeep
    - crates/famp-canonical/tests/vectors/output/.gitkeep
    - crates/famp-canonical/tests/vectors/supplementary/.gitkeep
  modified:
    - Cargo.toml
    - crates/famp-canonical/Cargo.toml
    - Justfile

key-decisions:
  - "sha2 0.11.0 has no `std` feature; enable `default` (alloc + oid) instead. Plan said `features = [\"std\"]`, which fails to resolve."
  - "Test files are #![cfg(feature = \"wave2_impl\")]-gated rather than `#[ignore]` so they don't even enter the build graph until Plan 02 enables the feature; this avoids brittle stub bodies."

patterns-established:
  - "Fallback-first discipline (D-08): contingency design committed before any go/no-go gate runs"
  - "Test scaffolding via cfg-feature gate, not #[ignore], so missing future symbols don't block today's build"
  - "Workspace deps stay default-features=false; per-crate Cargo.toml opts in to the specific features the crate needs"

requirements-completed: [CANON-07, SPEC-02]

duration: ~15min
completed: 2026-04-13
---

# Phase 01 Plan 01: famp-canonical Wave 0 Scaffolding Summary

**Wired famp-canonical dependencies (serde_jcs/serde_json+float_roundtrip/sha2/thiserror), wrote the 357-line RFC 8785 from-scratch fallback plan to disk per D-08, and scaffolded five feature-gated test harnesses including the verbatim 27-vector RFC 8785 Appendix B float corpus.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-13T03:45:00Z (approx)
- **Completed:** 2026-04-13T03:59:53Z
- **Tasks:** 3
- **Files modified:** 12 (3 modified, 9 created)

## Accomplishments

- Workspace `serde_json` pin now includes `float_roundtrip` — closes RESEARCH Pitfall 1 before any canonicalize() code is written.
- `famp-canonical/docs/fallback.md` exists on disk **before** the SEED-001 conformance gate runs, satisfying D-08 (fallback-first discipline) and CANON-07.
- RFC 8785 Appendix B's full 27-vector float test array is transcribed verbatim into `tests/conformance.rs` and gated on `wave2_impl` — Plan 02 unlocks it.
- Sampled float corpus driver (`tests/float_corpus.rs`) committed with the immutable seed `famp-canonical-float-corpus-v1` and 100K sample size per D-13/D-14.
- `cargo build -p famp-canonical` and `cargo build -p famp-canonical --tests` both pass cleanly with the new dep graph.
- `just test-canonical` recipe added for fast feedback loop.

## Task Commits

1. **Task 1: Workspace dep fix + famp-canonical Cargo.toml + Justfile recipe** — `200c9ba` (chore)
2. **Task 2: Write famp-canonical/docs/fallback.md (BEFORE running any gate)** — `cb0d253` (docs)
3. **Task 3: Test harness skeletons + cyberphone fixture directories committed** — `88fc35d` (test)

**Plan metadata commit:** pending (this SUMMARY + STATE/ROADMAP updates)

## Files Created/Modified

- `Cargo.toml` — added `float_roundtrip` to workspace `serde_json` features (RESEARCH Pitfall 1 closed)
- `crates/famp-canonical/Cargo.toml` — wired serde_jcs/serde/serde_json/sha2/thiserror deps; added `wave2_impl` and `full-corpus` feature flags
- `Justfile` — added `test-canonical` recipe
- `crates/famp-canonical/docs/fallback.md` — 357-line written fallback plan with all 8 required RFC 8785 §3 sections
- `crates/famp-canonical/tests/conformance.rs` — RFC 8785 Appendix B 27 float vectors (verbatim) + NaN/infinity rejection + cyberphone weird.json fixture stub
- `crates/famp-canonical/tests/float_corpus.rs` — sampled (100K) + full (100M, behind `full-corpus`) corpus drivers with committed seed
- `crates/famp-canonical/tests/utf16_supplementary.rs` — supplementary-plane key sort fixture stub
- `crates/famp-canonical/tests/duplicate_keys.rs` — `from_str_strict` duplicate-key rejection (verbatim from RESEARCH)
- `crates/famp-canonical/tests/artifact_id.rs` — empty-input SHA-256 known-constant + lowercase-only checks
- `crates/famp-canonical/tests/vectors/{input,output,supplementary}/.gitkeep` — cyberphone fixture dirs ready for Plan 02

## Decisions Made

- **sha2 feature pin: `default` not `std`.** The plan body specified `sha2 = { workspace = true, features = ["std"] }`, but `sha2 0.11.0` has no `std` feature (its features are `default`/`alloc`/`oid`/`zeroize`). Cargo refuses to resolve this. Switched to `features = ["default"]` which gives `alloc + oid`. Acceptance criterion (`grep -q 'sha2 = { workspace = true'`) still passes. Recorded as Deviation #1 below.
- **Test gating via cfg-feature, not `#[ignore]`.** The plan offered both options; chose `#![cfg(feature = "wave2_impl")]` per the plan's preferred path so the missing future symbols (`canonicalize`, `from_str_strict`, `artifact_id_for_canonical_bytes`, `CanonicalError`) don't break today's build. Plan 02 enables the feature flag in CI when production code lands.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] sha2 0.11.0 has no `std` feature**
- **Found during:** Task 1 (`cargo build -p famp-canonical`)
- **Issue:** Plan body specified `sha2 = { workspace = true, features = ["std"] }`. Cargo failed to resolve: `the package `famp-canonical` depends on `sha2`, with features: `std` but `sha2` does not have these features`. Inspection of `cargo info sha2` confirmed available features are `default`, `alloc`, `oid`, `zeroize` — no `std`.
- **Fix:** Changed to `sha2 = { workspace = true, features = ["default"] }`. The workspace pin uses `default-features = false`, so this opts into the upstream default (`alloc + oid`) which is exactly what the artifact-id helper will need.
- **Files modified:** `crates/famp-canonical/Cargo.toml`
- **Verification:** `cargo build -p famp-canonical` exits 0; acceptance criterion `grep -q 'sha2 = { workspace = true'` still passes.
- **Committed in:** `200c9ba` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Single feature-name correction; no scope change. The plan's intent (sha2 available with std-flavored features for the artifact-id helper) is preserved by the upstream default feature set.

## Issues Encountered

- **`cargo` not on `PATH` in shell.** The rustup-installed toolchains live at `~/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin/cargo` but the executor's shell didn't have either `~/.cargo/bin` or the toolchain bin on `PATH`. Worked around by exporting `PATH` inline for each cargo invocation. Not a deviation — the toolchain is correctly pinned via `rust-toolchain.toml`. Future task runs in this session should `export PATH="$HOME/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH"` or rely on `just` recipes which inherit the same environment.
- **Unused-crate-dependencies warnings on famp-canonical lib + test crates.** Expected — the deps are wired ahead of Plan 02 implementation. Will resolve when `canonicalize()` and friends actually use them. No action taken (out of scope per `unused_crate_dependencies = "warn"` in workspace lints).

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 02 (canonical engine implementation) can begin.** All deps resolve, the fallback plan is on disk, and the test harness is ready. Plan 02 will:
  1. Implement `canonicalize()`, `Canonicalize` trait, `from_slice_strict`/`from_str_strict`, `CanonicalError`, and `artifact_id_for_canonical_bytes`/`artifact_id_for_value` per D-02/D-05/D-17/D-20.
  2. Add the cyberphone test fixtures (input/output/supplementary directories already exist).
  3. Generate `tests/vectors/float_corpus_sample.txt` (100K lines from cyberphone es6testfile100m).
  4. Enable the `wave2_impl` feature flag in CI so the test harness becomes live.
- **No blockers.** Build is green, deps resolve, acceptance criteria met.

---

## Self-Check

- [x] `Cargo.toml` contains `float_roundtrip` — verified
- [x] `crates/famp-canonical/Cargo.toml` contains `serde_jcs = { workspace = true }` — verified
- [x] `crates/famp-canonical/Cargo.toml` contains `sha2 = { workspace = true` — verified
- [x] `crates/famp-canonical/Cargo.toml` contains `thiserror = { workspace = true` — verified
- [x] No `arbitrary_precision` or `preserve_order` in either Cargo.toml — verified (grep returned nothing)
- [x] `Justfile` has `test-canonical` recipe — verified
- [x] `cargo build -p famp-canonical` exits 0 — verified
- [x] `cargo build -p famp-canonical --tests` exits 0 — verified
- [x] `crates/famp-canonical/docs/fallback.md` exists with 357 lines and all required literals (`RFC 8785 §3.2.2.3`, `RFC 8785 §3.2.3`, `RFC 8785 §3.2.1`, `encode_utf16`, `ryu-js`, `Buffer::format_finite`, `serde_json_canonicalizer`) — verified
- [x] All 5 test files exist with required literals (first/last RFC 8785 vector, duplicate key string, sha256 known constant, SAMPLE_SIZE_PR/100_000/famp-canonical-float-corpus-v1) — verified
- [x] All 3 vectors subdirectory `.gitkeep` files exist — verified
- [x] Commits exist: `200c9ba`, `cb0d253`, `88fc35d` — verified via `git log`

## Self-Check: PASSED

---
*Phase: 01-canonical-json-foundations*
*Completed: 2026-04-13*
