---
phase: quick
plan: 260414-cme
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp-canonical/Cargo.toml
  - crates/famp-canonical/tests/conformance.rs
  - crates/famp-canonical/tests/float_corpus.rs
  - crates/famp-canonical/tests/duplicate_keys.rs
  - crates/famp-canonical/tests/utf16_supplementary.rs
  - crates/famp-canonical/tests/artifact_id.rs
autonomous: true
requirements:
  - CLEANUP-wave2-feature-gate
must_haves:
  truths:
    - "cargo test -p famp-canonical passes"
    - "cargo test -p famp-canonical --no-default-features passes AND runs the same test count as the default run (conformance/float_corpus/duplicate_keys/utf16_supplementary/artifact_id tests are no longer skipped)"
    - "grep -rn wave2_impl crates/famp-canonical/ returns zero matches"
    - "cargo clippy -p famp-canonical --all-targets -- -D warnings passes"
  artifacts:
    - path: crates/famp-canonical/Cargo.toml
      provides: "No wave2_impl feature; [features] section either removed or contains only full-corpus"
    - path: crates/famp-canonical/tests/conformance.rs
      provides: "No cfg(feature = wave2_impl) attribute; module doc rewritten"
    - path: crates/famp-canonical/tests/float_corpus.rs
      provides: "No cfg(feature = wave2_impl) attribute; module doc rewritten"
    - path: crates/famp-canonical/tests/duplicate_keys.rs
      provides: "No cfg(feature = wave2_impl) attribute; module doc rewritten"
    - path: crates/famp-canonical/tests/utf16_supplementary.rs
      provides: "No cfg(feature = wave2_impl) attribute"
    - path: crates/famp-canonical/tests/artifact_id.rs
      provides: "No cfg(feature = wave2_impl) attribute; module doc rewritten"
  key_links:
    - from: "crates/famp-canonical/tests/*.rs"
      to: "crates/famp-canonical/src/ (canonicalize, from_str_strict, CanonicalError, artifact_id_for_canonical_bytes)"
      via: "unconditional compilation (no cfg gate)"
      pattern: "^#!\\[cfg\\(feature"
---

<objective>
Remove the obsolete `wave2_impl` feature gate from `famp-canonical`. Plan 02 has shipped; the gated production symbols exist unconditionally in src/. The feature currently hides five test files from `cargo test --no-default-features`, silently skipping the entire RFC 8785 conformance suite — a regression hole flagged in FINAL_REVIEW.md as PR #1.

Purpose: Close the regression hole so `--no-default-features` runs the full conformance battery; remove dead historical scaffolding.
Output: `wave2_impl` is gone from Cargo.toml and all five test files; both `cargo test` invocations run the same tests.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
</execution_context>

<context>
@crates/famp-canonical/Cargo.toml
@crates/famp-canonical/tests/conformance.rs
@crates/famp-canonical/tests/float_corpus.rs
@crates/famp-canonical/tests/duplicate_keys.rs
@crates/famp-canonical/tests/utf16_supplementary.rs
@crates/famp-canonical/tests/artifact_id.rs

<notes>
- The `full-corpus` feature is a SEPARATE, still-active feature gating the nightly 100M-line corpus run. Do NOT touch it.
- No other workspace crate sets `default-features = false` on famp-canonical (verified by scope), so removing the default feature has no downstream impact.
- `docs/fallback.md` may still reference wave2_impl historically — that is explicitly out of scope per constraints.
</notes>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Delete wave2_impl from Cargo.toml and all five test files</name>
  <files>
    crates/famp-canonical/Cargo.toml,
    crates/famp-canonical/tests/conformance.rs,
    crates/famp-canonical/tests/float_corpus.rs,
    crates/famp-canonical/tests/duplicate_keys.rs,
    crates/famp-canonical/tests/utf16_supplementary.rs,
    crates/famp-canonical/tests/artifact_id.rs
  </files>
  <action>
    1. `crates/famp-canonical/Cargo.toml`: In the `[features]` section, delete the `default = ["wave2_impl"]` line AND the `wave2_impl = []` line. Keep `full-corpus = []`. The resulting `[features]` section should contain only `full-corpus = []` (do NOT delete the section — `full-corpus` still needs it).

    2. `tests/conformance.rs`: Delete line 19 (`#![cfg(feature = "wave2_impl")]`). Rewrite the module doc (currently lines 8–17) so it no longer mentions `wave2_impl` or "Plan 02". Replace with a concise description of what the test covers. Suggested rewrite:
       ```
       //! RFC 8785 Appendix B conformance vector harness.
       //!
       //! Runs all 27 IEEE 754 → ECMAScript Number.toString pairs from
       //! RFC 8785 Appendix B against `canonicalize`. Transcribed verbatim
       //! from `.planning/phases/01-canonical-json-foundations/01-RESEARCH.md`
       //! §"Code Examples" → `rfc8785_appendix_b_all`.
       ```

    3. `tests/float_corpus.rs`: Delete line 30 (`#![cfg(feature = "wave2_impl")]`). Remove the final paragraph of the module doc comment (line 28: `//! Gated behind \`wave2_impl\` (now default).`) and the blank `//!` line above it if present. Leave the rest of the doc intact (it documents the corpus source, which is still accurate).

    4. `tests/duplicate_keys.rs`: Delete line 16 (`#![cfg(feature = "wave2_impl")]`). Rewrite the final doc paragraph (lines 13–14) so it no longer references `wave2_impl` or Plan 02. Suggested rewrite of the whole doc:
       ```
       //! Duplicate-key rejection on strict-parse path (CANON-01, D-04..D-07).
       //!
       //! Verifies `from_str_strict` returns `CanonicalError::DuplicateKey` on
       //! JSON objects with repeated keys. Verbatim from
       //! `.planning/phases/01-canonical-json-foundations/01-RESEARCH.md`
       //! §"Duplicate Key Rejection Test".
       ```

    5. `tests/utf16_supplementary.rs`: Delete line 24 (`#![cfg(feature = "wave2_impl")]`). The existing module doc does NOT reference wave2_impl — leave it unchanged.

    6. `tests/artifact_id.rs`: Delete line 17 (`#![cfg(feature = "wave2_impl")]`). Rewrite the final doc paragraph (lines 14–15) so it no longer references `wave2_impl` or Plan 02. Suggested rewrite of the whole doc:
       ```
       //! `sha256:<hex>` artifact ID helper (SPEC-18, CANON-06, D-19..D-22).
       //!
       //! Asserts:
       //!  1. Empty-input SHA-256 matches the well-known constant.
       //!  2. Output is always lowercase hex per spec §3.6a (no uppercase).
       ```

    Do NOT modify any other files. Do NOT touch `full-corpus`. Do NOT touch `docs/fallback.md`.
  </action>
  <verify>
    <automated>grep -rn "wave2_impl" crates/famp-canonical/Cargo.toml crates/famp-canonical/tests/ ; test $? -eq 1</automated>
  </verify>
  <done>
    Zero `wave2_impl` references remain in Cargo.toml or the five test files. `full-corpus = []` still present in `[features]`. Module doc comments rewritten with no dangling "gated behind" / "Plan 02 will land" language.
  </done>
</task>

<task type="auto">
  <name>Task 2: Verify both feature configurations run the same test count and clippy is clean</name>
  <files>(verification only — no file changes)</files>
  <action>
    Run the following commands from the repo root and confirm all pass:

    1. `cargo test -p famp-canonical` — record the total test count from the summary line (e.g., "test result: ok. N passed").
    2. `cargo test -p famp-canonical --no-default-features` — record the total test count. It MUST equal the count from step 1 (this is the whole point: previously, the --no-default-features run silently skipped the five gated files).
    3. `cargo clippy -p famp-canonical --all-targets -- -D warnings` — must pass with zero warnings.
    4. `cargo clippy -p famp-canonical --all-targets --no-default-features -- -D warnings` — must also pass.

    If test counts differ between steps 1 and 2, investigate: either another cfg gate is hiding tests, or Task 1 missed a file. If clippy fails on `--no-default-features` due to an `unused_crate_dependencies` lint (previously masked when test files were cfg-gated out), fix by removing the now-unused `#![allow(unused_crate_dependencies, …)]` entries only if clippy specifically demands it — otherwise leave them.
  </action>
  <verify>
    <automated>cargo test -p famp-canonical && cargo test -p famp-canonical --no-default-features && cargo clippy -p famp-canonical --all-targets -- -D warnings && cargo clippy -p famp-canonical --all-targets --no-default-features -- -D warnings</automated>
  </verify>
  <done>
    Both `cargo test` invocations pass with identical test counts. Both clippy invocations pass with -D warnings. FINAL_REVIEW.md PR #1 regression hole is closed.
  </done>
</task>

</tasks>

<verification>
- `grep -rn wave2_impl crates/famp-canonical/Cargo.toml crates/famp-canonical/tests/` returns no matches.
- `cargo test -p famp-canonical` and `cargo test -p famp-canonical --no-default-features` report the same passing test count.
- `cargo clippy -p famp-canonical --all-targets -- -D warnings` is clean under both default and `--no-default-features`.
</verification>

<success_criteria>
- `wave2_impl` feature deleted from Cargo.toml; `full-corpus` preserved.
- Five `#![cfg(feature = "wave2_impl")]` attributes deleted.
- Stale doc comments rewritten on conformance.rs, float_corpus.rs, duplicate_keys.rs, artifact_id.rs (utf16_supplementary.rs docs untouched — no stale reference).
- Test count parity between default and `--no-default-features` runs.
- Clippy clean under both configurations.
</success_criteria>

<output>
After completion, create `.planning/quick/260414-cme-remove-obsolete-wave2-impl-feature-gate-/260414-cme-SUMMARY.md` recording the final test count (proof of parity) and confirming FINAL_REVIEW.md PR #1 is closed.
</output>
