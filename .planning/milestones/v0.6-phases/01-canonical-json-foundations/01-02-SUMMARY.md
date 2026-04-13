---
phase: 01-canonical-json-foundations
plan: 02
subsystem: canonical-json
tags: [rust, serde_jcs, rfc8785, canonical-json, sha2, thiserror, conformance]

requires:
  - phase: 01-canonical-json-foundations
    plan: 01
    provides: famp-canonical dep wiring, fallback.md, feature-gated test harness scaffolds
provides:
  - "famp-canonical public API: canonicalize() free fn + Canonicalize blanket trait (D-02)"
  - "from_slice_strict / from_str_strict ingress helpers with serde-visitor duplicate-key detection (D-04..D-07)"
  - "ArtifactIdString placeholder + artifact_id_for_canonical_bytes / artifact_id_for_value (D-19..D-22)"
  - "CanonicalError typed enum with 5 variants per D-17"
  - "wave2_impl is now the default feature — full test harness is live"
  - "cyberphone weird.json fixture pair committed (verbatim from upstream)"
  - "Hand-authored UTF-16 supplementary-plane fixture (emoji_keys.json/.expected)"
  - "100_000-line deterministic prefix of cyberphone es6testfile100m float corpus committed"
  - "Raw evidence (10/10 tests PASS) for SEED-001 keep-vs-fork decision in Plan 03"
affects: [01-03 seed-001-decision, 02-crypto-foundations]

tech-stack:
  added: []
  patterns:
    - "serde-visitor strict-parse pass on a discardable StrictTree to reject duplicate object keys at any depth before re-parsing into the caller's target type"
    - "Sentinel-prefixed serde custom error (`__DUPLICATE_KEY__:<key>`) to smuggle structured payloads through the visitor's stringly-typed error channel"
    - "Inline lowercase-hex encoder (no `hex` crate) for sha256:<hex> artifact IDs"
    - "Conformance-vector-as-fixture: `include_bytes!` against committed expected bytes, no runtime oracle dependency"

key-files:
  created:
    - crates/famp-canonical/src/error.rs
    - crates/famp-canonical/src/canonical.rs
    - crates/famp-canonical/src/strict_parse.rs
    - crates/famp-canonical/src/artifact_id.rs
    - crates/famp-canonical/tests/vectors/input/weird.json
    - crates/famp-canonical/tests/vectors/output/weird.json
    - crates/famp-canonical/tests/vectors/supplementary/emoji_keys.json
    - crates/famp-canonical/tests/vectors/supplementary/emoji_keys.expected
    - crates/famp-canonical/tests/vectors/float_corpus_sample.txt
  modified:
    - crates/famp-canonical/Cargo.toml
    - crates/famp-canonical/src/lib.rs
    - crates/famp-canonical/tests/utf16_supplementary.rs
    - crates/famp-canonical/tests/float_corpus.rs

key-decisions:
  - "Lib code is clippy-clean under `-D warnings` with workspace pedantic+all denies. Test code uses `.unwrap()` / `.expect()` (denied at workspace level), so `cargo clippy --tests` would fail — but the plan's acceptance criterion is `cargo clippy -p famp-canonical -- -D warnings` (lib only), which matches existing Plan 01 test stubs that also use unwrap. Tests-clippy hygiene is deferred."
  - "supplementary-plane oracle authored manually (RFC 8785 §3.2.3 sort math) instead of via Node `canonicalize` package — avoids a runtime/network dependency and the math is short enough to verify by hand: U+0061 < U+1F389 (high surrogate 0xD83C) < U+20BB7 (high surrogate 0xD842). Result: `{\"a\":2,\"🎉\":1,\"𠮷\":3}`."
  - "Used `Approach B` (download cyberphone es6testfile100m.txt.gz, take first 100_000 lines) per plan preference. The first 100_000 lines of a deterministically-generated upstream corpus are themselves deterministic, so the seed-vs-prefix distinction collapses."

requirements-completed: [CANON-01, CANON-04, CANON-05, CANON-06, SPEC-18]

metrics:
  duration: ~6min
  tasks: 2
  files_created: 9
  files_modified: 4
  source_loc_added: 374
  fixture_bytes_committed: ~4.0MB

completed: 2026-04-13
---

# Phase 01 Plan 02: Canonical Engine Implementation Summary

**Implemented `famp-canonical` public API surfaces (canonicalize / Canonicalize trait / from_*_strict / artifact_id_*) per locked decisions D-02/D-05/D-17/D-20, landed cyberphone + supplementary-plane + 100K-line float corpus fixtures, and turned 10/10 tests green — strong evidence to keep `serde_jcs 0.2.0` for the SEED-001 decision in Plan 03.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-13T04:02:37Z
- **Completed:** 2026-04-13T04:08:21Z
- **Tasks:** 2
- **Files created:** 9
- **Files modified:** 4

## Source files implemented

| File | LoC | Purpose |
|------|-----|---------|
| `src/error.rs` | 64 | `CanonicalError` enum (5 variants per D-17) + `from_serde` classifier that maps NaN/Infinity to `NonFiniteNumber` |
| `src/canonical.rs` | 38 | `canonicalize()` free fn delegating to `serde_jcs::to_vec` + `Canonicalize` blanket trait per D-02 |
| `src/strict_parse.rs` | 158 | `from_slice_strict` / `from_str_strict` via discardable `StrictTree` visitor with `HashSet` duplicate-key detection (D-04..D-07) |
| `src/artifact_id.rs` | 78 | `ArtifactIdString` placeholder (D-21), inline `bytes_to_lower_hex` (no hex crate), `artifact_id_for_canonical_bytes` + `artifact_id_for_value` (SPEC-18 / D-19..D-22) |
| `src/lib.rs` | 38 | Module declarations + full public re-exports + INGRESS NOTE |

**Total production source LoC added: ~374.**

## Test results — full evidence for SEED-001

| Test file | Test name | Result | Notes |
|-----------|-----------|--------|-------|
| `tests/conformance.rs` | `appendix_b_float_vectors` | ✅ PASS | All 27 RFC 8785 Appendix B IEEE 754 → ECMAScript number vectors byte-exact |
| `tests/conformance.rs` | `nan_rejected` | ✅ PASS | NaN → `CanonicalError::NonFiniteNumber` |
| `tests/conformance.rs` | `infinity_rejected` | ✅ PASS | ±Infinity → `CanonicalError::NonFiniteNumber` |
| `tests/conformance.rs` | `cyberphone_weird_fixture` | ✅ PASS | cyberphone weird.json round-trips byte-exact (Latin/Hebrew/CJK/emoji/control/`</script>`) |
| `tests/duplicate_keys.rs` | `duplicate_key_is_error` | ✅ PASS | `{"a":1,"b":2,"a":3}` → `DuplicateKey { key: "a" }` |
| `tests/duplicate_keys.rs` | `non_duplicate_is_ok` | ✅ PASS | `{"a":1,"b":2,"c":3}` round-trips |
| `tests/artifact_id.rs` | `sha256_known_input` | ✅ PASS | empty bytes → `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `tests/artifact_id.rs` | `sha256_lowercase_only` | ✅ PASS | `"hello world"` → 64 lowercase hex chars |
| `tests/float_corpus.rs` | `float_corpus_sampled` | ✅ PASS | 100_000 cyberphone es6testfile lines, byte-exact |
| `tests/utf16_supplementary.rs` | `supplementary_plane_keys_sort_correctly` | ✅ PASS | U+0061 < U+1F389 < U+20BB7 sort produces expected canonical bytes |

**10/10 PASSING.** Zero failures. Zero corpus mismatches over 100K randomized doubles. Zero divergence on the cyberphone weird-input fixture. Zero RFC 8785 Appendix B vector failures.

This is **strong evidence** for the SEED-001 decision (Plan 03): `serde_jcs 0.2.0` is correct on every test we've thrown at it. The fallback plan (`docs/fallback.md`) remains on disk as insurance, but the recommended Plan 03 outcome is **KEEP `serde_jcs`**.

## Build + lint status

- `cargo build -p famp-canonical` — ✅ exits 0, zero warnings
- `cargo build -p famp-canonical --tests` — ✅ exits 0
- `cargo test -p famp-canonical` — ✅ 10/10 pass
- `cargo clippy -p famp-canonical -- -D warnings` — ✅ clean (lib only, per plan acceptance criterion)
- No `unsafe` keyword anywhere (workspace `unsafe_code = "forbid"`)
- No `anyhow` import in any `src/` file

## Task Commits

1. **Task 1: Implement core production sources** — `a7b8f26` (feat) — Cargo.toml feature default flip + 5 source files
2. **Task 2: Conformance fixtures + 100K float corpus + test rewrites** — `4c4c39b` (test) — fixtures, sample corpus, utf16 + float_corpus test bodies, plus folded-in lib clippy fixes (Self::, map_or_else, doc-link rephrase)

**Plan metadata commit:** pending (this SUMMARY + STATE/ROADMAP updates)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Workspace clippy denies `unwrap_used` / `expect_used` on tests**
- **Found during:** Task 2 verification (`cargo clippy --tests`)
- **Issue:** Workspace `[lints.clippy]` denies `unwrap_used` and `expect_used`. The float_corpus test (and the existing Plan 01 stubs in duplicate_keys.rs / artifact_id.rs / conformance.rs) all use `.unwrap()` and `.expect()` in test bodies, so `cargo clippy --tests` fails workspace-wide. The plan's acceptance criterion is `cargo clippy -p famp-canonical -- -D warnings` (no `--tests` flag), which matches the Plan 01 baseline.
- **Fix:** Scoped clippy enforcement to lib only (`cargo clippy -p famp-canonical -- -D warnings`) per the plan's acceptance criterion. Lib clippy is fully clean. Tests-clippy hygiene (rewriting unwraps to `?` / `assert!`) is **deferred** as a follow-up — it would touch every existing test stub and is out of scope for Plan 02.
- **Files modified:** None (scope decision, not a code change).
- **Tracked for follow-up:** A future plan should add per-test-file `#![allow(clippy::unwrap_used, clippy::expect_used)]` or rewrite tests to avoid the lints. Not blocking SEED-001.
- **Committed in:** N/A (scope decision documented here).

**2. [Rule 1 — Bug] Lib clippy violations introduced in Task 1**
- **Found during:** Task 2 verification (`cargo clippy -p famp-canonical -- -D warnings` after Task 1 commit)
- **Issue:** Four pedantic-clippy violations in Task 1 lib code: `clippy::doc_link_with_quotes` in canonical.rs, two `clippy::use_self` violations in error.rs, one `clippy::option_if_let_else` violation in strict_parse.rs.
- **Fix:** Rephrased the doc link, switched to `Self::` in `from_serde`, and used `Option::map_or_else` in `map_serde_err`. Behavior unchanged; tests still 10/10 green.
- **Files modified:** `src/canonical.rs`, `src/error.rs`, `src/strict_parse.rs`
- **Verification:** `cargo clippy -p famp-canonical -- -D warnings` exits 0
- **Committed in:** `4c4c39b` (folded into Task 2 commit since they were small and Task 1 was already pushed)

**3. [Rule 1 — Bug] StrictTree dead-field warnings**
- **Found during:** Task 1 (`cargo build -p famp-canonical`)
- **Issue:** `StrictTree` enum exists only to drive a Visitor; its field payloads are intentionally never read, which triggers `dead_code` warnings.
- **Fix:** Added `#[allow(dead_code)]` to the enum with an explanatory comment.
- **Files modified:** `src/strict_parse.rs`
- **Verification:** Clean build with zero warnings.
- **Committed in:** `a7b8f26` (Task 1)

---

**Total deviations:** 3 auto-fixed (1 scoping decision, 2 clippy/dead-code fixes). **Zero deviations from the locked API surface** — `canonicalize`, `Canonicalize`, `from_slice_strict`, `from_str_strict`, `CanonicalError`, `ArtifactIdString`, `artifact_id_for_canonical_bytes`, `artifact_id_for_value` all match CONTEXT.md D-02/D-05/D-17/D-20 byte-for-byte.

## Issues Encountered

- **`cargo` PATH dance** carried over from Plan 01. Each invocation needed `export PATH="$HOME/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH"` because the executor shell doesn't pick up `~/.cargo/bin` automatically. Not blocking; documented in Plan 01 SUMMARY.
- **Test-files clippy hygiene** is workspace-wide, not famp-canonical-specific. A follow-up plan should sweep all test files for `unwrap_used` / `expect_used` violations and either rewrite or per-file allow.

## SEED-001 Evidence Brief (input to Plan 03)

Plan 03 will weigh whether to KEEP `serde_jcs 0.2.0` or FORK to in-house `famp-canonical` per the fallback plan. Plan 02 produced the following evidence against `serde_jcs`:

| Test category | Vectors run | Failures | Verdict |
|---------------|-------------|----------|---------|
| RFC 8785 Appendix B IEEE 754 → ECMAScript number | 27 | 0 | ✅ exact |
| RFC 8785 NaN / ±Infinity rejection | 3 | 0 | ✅ exact |
| cyberphone weird.json (Latin/Hebrew/CJK/emoji/control/XSS) | 1 round-trip | 0 | ✅ byte-exact |
| UTF-16 supplementary-plane key sort | 1 | 0 | ✅ exact |
| cyberphone es6testfile100m sampled prefix | 100_000 lines | 0 | ✅ byte-exact |
| Duplicate-key rejection | 2 | 0 | ✅ enforced via our visitor (orthogonal to serde_jcs) |
| SHA-256 artifact ID round-trip | 2 | 0 | ✅ exact |

**Recommended Plan 03 outcome:** KEEP `serde_jcs 0.2.0`. Fallback plan stays on disk as documentation. Re-run the gate on every dependency bump.

## Next Phase Readiness

- **Plan 03 (SEED-001 decision)** can begin immediately. All evidence Plan 03 needs is in this SUMMARY + the green test suite.
- **Phase 2 (famp-crypto)** can also begin in parallel-after if desired: `canonicalize()` is stable, byte-exact, and ready to feed Ed25519 signing with the v0.5.1 §7.1 domain-separation prefix.
- **No blockers.** Build green, lib clippy clean, all tests passing.

---

## Self-Check

- [x] `crates/famp-canonical/src/error.rs` exists with all 5 variants (`Serialize`, `InvalidJson`, `DuplicateKey`, `NonFiniteNumber`, `InternalCanonicalization`) — verified
- [x] `crates/famp-canonical/src/canonical.rs` contains `serde_jcs::to_vec` literal and `impl<T: Serialize + ?Sized> Canonicalize for T` — verified
- [x] `crates/famp-canonical/src/strict_parse.rs` contains `HashSet` and `__DUPLICATE_KEY__` literals — verified
- [x] `crates/famp-canonical/src/artifact_id.rs` contains `Sha256::digest`, `format!("sha256:`, and `bytes_to_lower_hex` (no `hex` crate import) — verified
- [x] `crates/famp-canonical/src/lib.rs` re-exports all 8 public items — verified
- [x] `crates/famp-canonical/Cargo.toml` has `default = ["wave2_impl"]` — verified
- [x] `crates/famp-canonical/tests/vectors/input/weird.json` exists, non-empty (283 bytes) — verified
- [x] `crates/famp-canonical/tests/vectors/output/weird.json` exists, non-empty (214 bytes) — verified
- [x] `crates/famp-canonical/tests/vectors/supplementary/emoji_keys.json` contains literal 🎉 (UTF-8 F0 9F 8E 89) — verified
- [x] `crates/famp-canonical/tests/vectors/supplementary/emoji_keys.expected` exists, non-empty (25 bytes) — verified
- [x] `crates/famp-canonical/tests/vectors/float_corpus_sample.txt` has exactly 100000 lines — verified
- [x] `cargo build -p famp-canonical` exits 0 — verified
- [x] `cargo test -p famp-canonical` 10/10 pass — verified
- [x] `cargo clippy -p famp-canonical -- -D warnings` clean (lib) — verified
- [x] No `unsafe` keyword in `src/` — verified
- [x] No `anyhow` import in `src/` — verified
- [x] Commits exist: `a7b8f26`, `4c4c39b` — verified via `git log`

## Self-Check: PASSED

---
*Phase: 01-canonical-json-foundations*
*Completed: 2026-04-13*
