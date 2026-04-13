---
phase: 02-crypto-foundations
plan: 04
subsystem: famp-crypto
tags: [crypto, sha256, content-addressing, gap-closure, CRYPTO-07]
requires:
  - famp-crypto crate (from plans 02-01..02-03)
  - workspace-pinned sha2 = "0.11.0"
provides:
  - famp_crypto::sha256_artifact_id helper
  - famp_crypto::sha256_digest helper
  - NIST FIPS 180-2 KAT conformance gate
affects:
  - crates/famp-crypto/Cargo.toml (+sha2 dep)
  - crates/famp-crypto/src/lib.rs (+pub mod hash + re-exports)
  - crates/famp-crypto/tests/*.rs (added `use sha2 as _;` to silence unused-dep lint)
tech-stack:
  added:
    - sha2 0.11.0 (RustCrypto, workspace-pinned)
  patterns:
    - Single-path content-addressing helper wraps sha2::Sha256
    - NIST KATs as byte-exact blocking tests
    - `use <dep> as _;` stanza for dev-only deps per integration-test compile unit
key-files:
  created:
    - crates/famp-crypto/src/hash.rs
    - crates/famp-crypto/tests/sha256_vectors.rs
  modified:
    - crates/famp-crypto/Cargo.toml
    - crates/famp-crypto/src/lib.rs
    - crates/famp-crypto/README.md
    - crates/famp-crypto/tests/rfc8032_vectors.rs
    - crates/famp-crypto/tests/weak_key_rejection.rs
    - crates/famp-crypto/tests/worked_example.rs
    - crates/famp-crypto/tests/base64_roundtrip.rs
decisions:
  - Use `unwrap_or('0')` in nibble-to-hex loop because `u32::from(b >> 4)` is always 0..=15 and `char::from_digit(.., 16)` never returns None; `unwrap_or` keeps clippy's `unwrap_used = deny` happy without masking real bugs.
  - Use `core::fmt::Write::write!` into the pre-allocated String in the agreement test to satisfy `clippy::format_push_string`.
  - Silence `unused_crate_dependencies` in four existing test binaries via `use sha2 as _;` rather than gating sha2 on a cargo feature — matches the pre-existing pattern for subtle/thiserror/zeroize.
metrics:
  tasks_completed: 3
  tasks_planned: 3
  duration_minutes: ~15
  completed_date: 2026-04-13
---

# Phase 2 Plan 04: CRYPTO-07 SHA-256 Content-Addressing Gap Closure — Summary

**One-liner:** Added `famp_crypto::sha256_artifact_id` backed by `sha2::Sha256`, gated by three NIST FIPS 180-2 SHA-256 KATs plus two shape/agreement invariants, closing the single remaining Phase 2 gap flagged in `02-VERIFICATION.md`.

## What Shipped

### Production code (~30 LoC)

`crates/famp-crypto/src/hash.rs` — new module exposing two `#[must_use]` functions:

- `sha256_digest(bytes: &[u8]) -> [u8; 32]` — raw digest accessor, infallible.
- `sha256_artifact_id(bytes: &[u8]) -> String` — returns 71-character `sha256:<64-lowercase-hex>` form matching the spec `artifact-id` scheme. Hand-rolled lowercase-hex loop avoids pulling a separate hex crate for production code and stays lint-clean under `clippy::unwrap_used = deny`.

`crates/famp-crypto/src/lib.rs` — `pub mod hash;` wired in alphabetical position between `error` and `keys`; `pub use hash::{sha256_artifact_id, sha256_digest};` re-exported at crate root.

`crates/famp-crypto/Cargo.toml` — adds `sha2 = { workspace = true }` to `[dependencies]` (workspace pin already present at root: `sha2 = "0.11.0"`).

### Tests (5 new, all byte-exact)

`crates/famp-crypto/tests/sha256_vectors.rs`:

1. `nist_kat_empty_string` — `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
2. `nist_kat_abc` — `sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` (FIPS 180-2 §B.1)
3. `nist_kat_56byte_vector` — `sha256:248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1` (FIPS 180-2 §B.2)
4. `artifact_id_shape_invariants` — enforces length == 71, prefix `sha256:`, 64-char lowercase ASCII hex tail.
5. `digest_and_artifact_id_agree` — cross-check: hex-encoded `sha256_digest` output equals `sha256_artifact_id` suffix byte-for-byte.

### Documentation

`crates/famp-crypto/README.md` — new `## Content addressing (CRYPTO-07)` section inserted immediately before `## Constant-time verification (CRYPTO-08)`, containing the public-API contract, a worked `"abc"` example, a reference to the `sha2 0.11.0` workspace pin, and a pointer at `tests/sha256_vectors.rs` as the conformance gate.

## Verification Evidence

- `cargo nextest run -p famp-crypto` — **24 tests passed, 0 skipped, 0 failed** (19 pre-existing + 5 new).
- `cargo test -p famp-crypto --doc` — 1 doctest passed.
- `cargo clippy -p famp-crypto --all-targets -- -D warnings` — clean (zero warnings).
- `just test-crypto` — green end-to-end; `sha256_vectors` binary is auto-discovered by nextest (no justfile change needed).

NIST KATs used (verbatim per FIPS 180-2 Appendix B):

| Input | Expected `sha256:` hex |
|-------|------------------------|
| `b""` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `b"abc"` | `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` |
| `b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"` | `248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1` |

## Additive-Only Guarantee (plan constraint)

`git diff 7a23525..HEAD -- crates/famp-crypto/src/{sign,verify,keys,prefix,traits,error}.rs` reports **zero changes**. All six forbidden files are untouched; every verified truth from `02-VERIFICATION.md` (truths 1–6 + CRYPTO-08 half of truth 7) remains intact.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Silence `unused_crate_dependencies` in four existing test binaries**
- **Found during:** Task 2, first `cargo clippy -p famp-crypto --all-targets -- -D warnings` run
- **Issue:** Adding `sha2` to `[dependencies]` made the workspace lint `unused_crate_dependencies = warn` (promoted to deny under `-D warnings`) fire in every integration-test compile unit that did not reference `sha2` — specifically `rfc8032_vectors.rs`, `weak_key_rejection.rs`, `worked_example.rs`, and `base64_roundtrip.rs`. This is the exact pattern the existing tests already handle for `subtle`, `thiserror`, `zeroize`, etc. — each integration test file has a `use <dep> as _;` silencing stanza.
- **Fix:** Added a single `use sha2 as _;` line to each of the four existing test files, preserving alphabetical ordering inside the existing stanza. No semantic change to test behavior.
- **Files modified:** `tests/rfc8032_vectors.rs`, `tests/weak_key_rejection.rs`, `tests/worked_example.rs`, `tests/base64_roundtrip.rs`.
- **Commit:** `e958178` (rolled into Task 2 commit since compile would otherwise break mid-plan).

**2. [Rule 3 — Blocking] Clippy `doc_markdown` on `RustCrypto`**
- **Found during:** Task 2, first clippy run.
- **Issue:** `clippy::pedantic` → `doc_markdown` flagged the bare word `RustCrypto` in the hash.rs module doc comment. Workspace denies pedantic.
- **Fix:** Wrapped in backticks: `` `RustCrypto` ``.
- **Commit:** `e958178`.

**3. [Rule 3 — Blocking] Clippy `format_push_string` in `digest_and_artifact_id_agree`**
- **Found during:** Task 2, first clippy run.
- **Issue:** `expected.push_str(&format!("{b:02x}"))` tripped `clippy::format_push_string` under pedantic-deny.
- **Fix:** Switched to `write!(&mut expected, "{b:02x}").unwrap()` with `use std::fmt::Write as _;` and a top-of-file `#![allow(clippy::unwrap_used)]` matching the other test binaries.
- **Commit:** `e958178`.

### Environment note (not a deviation, but worth recording)

The project's Rust toolchain lives under `~/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin/` and is NOT on the default shell PATH in the sandbox used to execute this plan — `which cargo` returned 127. Every `cargo`/`just` invocation in this plan was prefixed with `export PATH="$HOME/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH";`. This is an environment issue, not a code issue, and is outside the scope of this plan to fix. Logging here so future execution agents on this machine don't re-discover it from scratch.

## CRYPTO-07 Status

**CLOSED.** Truth 7 of Phase 2 — "SHA-256 content-addressing available via `sha2` crate" — is now byte-verified in code:

- Cargo dep wired.
- Public helper reachable from crate root.
- Three NIST KATs asserted byte-exact in a blocking integration test.
- Constant-time half of truth 7 (already VERIFIED per 02-VERIFICATION.md) untouched.

Phase 2 score moves from **6/7 truths verified (gap_found)** to **7/7 truths verified**. Ready for re-verification.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | `32cac08` | `chore(02-04): add sha2 workspace dep to famp-crypto` |
| 2 | `e958178` | `feat(02-04): sha256_artifact_id helper + NIST KAT gate (CRYPTO-07)` |
| 3 | `93a154d` | `docs(02-04): document sha256_artifact_id under CRYPTO-07 section` |

## Known Stubs

None. The helper is fully wired end-to-end: public API → sha2::Sha256 → NIST KAT gate → README documentation → CI via `just test-crypto`.

## Self-Check: PASSED

All 6 created/modified files verified on disk; all 3 task commits verified in `git log`.
