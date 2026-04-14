---
phase: quick
plan: 260414-cme
subsystem: famp-canonical
tags: [cleanup, feature-gate, rfc8785, conformance]
requires: []
provides:
  - CLEANUP-wave2-feature-gate
affects:
  - crates/famp-canonical/Cargo.toml
  - crates/famp-canonical/tests/conformance.rs
  - crates/famp-canonical/tests/float_corpus.rs
  - crates/famp-canonical/tests/duplicate_keys.rs
  - crates/famp-canonical/tests/utf16_supplementary.rs
  - crates/famp-canonical/tests/artifact_id.rs
tech-stack:
  added: []
  patterns: []
key-files:
  created: []
  modified:
    - crates/famp-canonical/Cargo.toml
    - crates/famp-canonical/tests/conformance.rs
    - crates/famp-canonical/tests/float_corpus.rs
    - crates/famp-canonical/tests/duplicate_keys.rs
    - crates/famp-canonical/tests/utf16_supplementary.rs
    - crates/famp-canonical/tests/artifact_id.rs
decisions:
  - Removed obsolete `wave2_impl` feature gate; production symbols now exist unconditionally in src/ and test files compile under both default and --no-default-features configurations.
metrics:
  duration: ~10m
  completed: 2026-04-14
---

# Quick 260414-cme: Remove Obsolete wave2_impl Feature Gate Summary

Closed FINAL_REVIEW.md PR #1 regression hole by deleting the historical `wave2_impl` feature that silently skipped the entire RFC 8785 conformance suite under `--no-default-features`.

## What Changed

- **`crates/famp-canonical/Cargo.toml`** — deleted `default = ["wave2_impl"]` and `wave2_impl = []`; preserved `full-corpus = []`.
- **`tests/conformance.rs`** — removed `#![cfg(feature = "wave2_impl")]`, rewrote module doc (dropped "Plan 02 will land" stale text), and scrubbed a stale `wave2_impl` reference from the `cyberphone_weird_fixture` comment.
- **`tests/float_corpus.rs`** — removed `#![cfg(...)]` gate and the "Gated behind `wave2_impl` (now default)." doc line; corpus sourcing docs left intact.
- **`tests/duplicate_keys.rs`** — removed gate; rewrote doc to drop "Plan 02 will land" language.
- **`tests/utf16_supplementary.rs`** — removed gate; doc untouched (no stale reference present).
- **`tests/artifact_id.rs`** — removed gate; rewrote doc to drop "until Plan 02 lands" language.

## Verification (proof of parity)

| Check | Result |
| --- | --- |
| `grep -rn wave2_impl crates/famp-canonical/Cargo.toml crates/famp-canonical/tests/` | 0 matches |
| `cargo test -p famp-canonical` | **12 passed** (0 failed, 0 ignored) |
| `cargo test -p famp-canonical --no-default-features` | **12 passed** (0 failed, 0 ignored) — **parity achieved** |
| `cargo clippy -p famp-canonical --all-targets -- -D warnings` | clean |
| `cargo clippy -p famp-canonical --all-targets --no-default-features -- -D warnings` | clean |

Breakdown by test binary (both configurations): artifact_id 2, conformance 6, duplicate_keys 2, float_corpus 1, utf16_supplementary 1 = 12.

Previously, the `--no-default-features` run compiled and reported "ok" while silently compiling out all five test files. With the gate gone, both invocations exercise the same binary set, and the RFC 8785 conformance battery now protects the `--no-default-features` build profile.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Stale Reference] Scrubbed stray `wave2_impl` comment in conformance.rs**
- **Found during:** Task 1 verification
- **Issue:** A historical comment inside `cyberphone_weird_fixture` (lines 125–130) still said "the test only enters the build graph when wave2_impl is enabled". The plan listed explicit doc rewrites for five locations but did not catch this in-body test comment. `grep wave2_impl` would have reported a remaining match and violated the truth list.
- **Fix:** Rewrote the comment to describe the fixture source (cyberphone testdata corpus) without the stale gating reference.
- **Files modified:** `crates/famp-canonical/tests/conformance.rs`
- **Commit:** `e048057` (rolled into Task 1 commit)

## Closes

FINAL_REVIEW.md PR #1: `--no-default-features` no longer skips RFC 8785 conformance tests.

## Commits

- `e048057` — chore(quick-260414-cme): remove obsolete wave2_impl feature gate

## Self-Check: PASSED

- Cargo.toml edits verified in Read tool state (wave2_impl lines deleted, full-corpus preserved).
- All five test files: `#![cfg(feature = "wave2_impl")]` deleted; stale docs rewritten.
- `grep -rn wave2_impl crates/famp-canonical/Cargo.toml crates/famp-canonical/tests/` returned exit 1 (no matches).
- Commit `e048057` present in `git log`.
- Test count parity: 12/12 in both feature configurations.
- Clippy clean under both `-D warnings` invocations.
