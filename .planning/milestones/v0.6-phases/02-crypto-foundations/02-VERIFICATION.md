---
phase: 02-crypto-foundations
verified: 2026-04-13T00:00:00Z
status: passed
score: 7/7 success criteria verified
re_verification:
  previous_status: gaps_found
  previous_score: 6/7
  gaps_closed:
    - "SHA-256 content-addressing available via `sha2` crate (CRYPTO-07)"
  gaps_remaining: []
  regressions: []
---

# Phase 2: Crypto Foundations Verification Report

**Phase Goal:** A user can sign a canonical byte string with Ed25519 using the domain-separation prefix from SPEC-03, and a second implementation (Python worked example from PITFALLS P10) verifies byte-exact. `famp-crypto` exposes only `verify_strict`; raw `verify` is unreachable.

**Verified:** 2026-04-13
**Status:** passed
**Re-verification:** Yes — after 02-04 gap closure for CRYPTO-07

## Goal Achievement

### Observable Truths (Success Criteria from ROADMAP)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp-crypto` exposes `Signer`/`Verifier` traits; only `verify_strict` reachable, raw `verify` not exported | VERIFIED | `crates/famp-crypto/src/traits.rs` defines both traits; `TrustedVerifyingKey` wraps `VerifyingKey` with `pub(crate)` inner field; only `verify_strict` call site in `verify.rs`. |
| 2 | Weak/small-subgroup public keys rejected at ingress with must-reject fixtures | VERIFIED | `is_weak()` ingress check + 5 named torsion-point fixtures in `tests/vectors/must-reject/weak-keys.json`; `all_weak_key_fixtures_rejected_at_ingress` passes. |
| 3 | Domain-separation prefix from SPEC-03 applied before every sign; conformance vector #1 with hex dump; documented in README | VERIFIED | `DOMAIN_PREFIX = b"FAMP-sig-v1\0"` in `prefix.rs`; `canonicalize_for_signature` prepends; `worked-example.json` commits hex dump; README `## Domain separation` section present. |
| 4 | RFC 8032 Ed25519 test vectors pass as hard CI gate | VERIFIED | `all_rfc8032_vectors_byte_exact` passes under `just test-crypto` and CI `test-crypto` job. |
| 5 | §7.1c worked example (Python `jcs 0.2.1` + `cryptography 46.0.7`) verifies byte-exact in Rust; committed as fixture | VERIFIED | `section_7_1c_worked_example_byte_exact` asserts canonicalize output matches `signing_input_hex` byte-for-byte; `PROVENANCE.md` documents toolchain. |
| 6 | Base64url unpadded used for keys (32B) and signatures (64B) per SPEC-19; round-trip proptest green | VERIFIED | `URL_SAFE_NO_PAD` engine; `signing_key_b64_roundtrip` + `signature_b64_roundtrip` proptests pass; padded/standard alphabet rejected. |
| 7 | SHA-256 content-addressing available via `sha2` crate; constant-time verify path documented and tested | **VERIFIED (gap closed by 02-04)** | **SHA-256 half:** `crates/famp-crypto/Cargo.toml` declares `sha2 = { workspace = true }` (line 22). `crates/famp-crypto/src/hash.rs` exports `sha256_artifact_id` + `sha256_digest` backed by `sha2::{Digest, Sha256}`. `lib.rs:44` re-exports both. `tests/sha256_vectors.rs` runs 5 NIST KAT / shape tests byte-exact against FIPS 180-2 vectors (empty, `"abc"`, 56-byte vector). All 5 pass under `just test-crypto`. README has `## Content addressing (CRYPTO-07)` section with code example. **Constant-time half:** `subtle::ConstantTimeEq` on `FampSignature`, `verify_strict` delegation documented in README `## Constant-time verification (CRYPTO-08)` + `## Wrapper audit`. |

**Score:** 7/7 truths verified.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-crypto/src/keys.rs` | Newtypes with ingress checks | VERIFIED | Unchanged since prior verification. |
| `crates/famp-crypto/src/error.rs` | `CryptoError` thiserror enum | VERIFIED | Unchanged. |
| `crates/famp-crypto/src/prefix.rs` | `DOMAIN_PREFIX` + `canonicalize_for_signature` | VERIFIED | Unchanged. |
| `crates/famp-crypto/src/sign.rs` | `sign_value`, `sign_canonical_bytes` | VERIFIED | Unchanged (02-04 additive-only guarantee held). |
| `crates/famp-crypto/src/verify.rs` | `verify_value`, `verify_canonical_bytes` | VERIFIED | Unchanged. |
| `crates/famp-crypto/src/traits.rs` | `Signer`, `Verifier` traits | VERIFIED | Unchanged. |
| `crates/famp-crypto/src/hash.rs` | `sha256_artifact_id` + `sha256_digest`, backed by `sha2` | **VERIFIED (new)** | File exists; `use sha2::{Digest, Sha256}`; returns 71-char `sha256:<64 lowercase hex>` string; `#[must_use]`; clippy-clean (no `.unwrap()`). |
| `crates/famp-crypto/src/lib.rs` | Public re-exports of hash helpers | **VERIFIED (updated)** | `pub mod hash;` present; `pub use hash::{sha256_artifact_id, sha256_digest};` at line 44. |
| `crates/famp-crypto/Cargo.toml` | Declares `sha2 = { workspace = true }` | **VERIFIED (new)** | Line 22: `sha2           = { workspace = true }`. |
| `crates/famp-crypto/tests/sha256_vectors.rs` | NIST KAT byte-exact gate | **VERIFIED (new)** | 5 tests: `nist_kat_empty_string`, `nist_kat_abc`, `nist_kat_56byte_vector`, `artifact_id_shape_invariants`, `digest_and_artifact_id_agree`. All three FIPS 180-2 hex vectors present verbatim. |
| `tests/vectors/must-reject/weak-keys.json` | >=3 weak-key fixtures | VERIFIED | 5 fixtures. |
| `tests/vectors/must-reject/malformed-b64.json` | >=5 bad-encoding fixtures | VERIFIED | 6 fixtures. |
| `tests/vectors/rfc8032/test-vectors.json` | 5 RFC 8032 §7.1 vectors | VERIFIED | Byte-exact gate passes. |
| `tests/vectors/famp-sig-v1/worked-example.json` | §7.1c interop fixture, no placeholders | VERIFIED | No `<COPY FROM SPEC>` remaining. |
| `tests/vectors/famp-sig-v1/PROVENANCE.md` | Documents Python provenance | VERIFIED | `jcs==0.2.1` + `cryptography==46.0.7`. |
| `crates/famp-crypto/README.md` | All required sections incl. CRYPTO-07 | **VERIFIED (updated)** | `## Content addressing (CRYPTO-07)` section present (lines 115-140) with `sha256_artifact_id` code example, `sha256_digest` mention, `sha2 = "0.11.0"` note, and reference to `tests/sha256_vectors.rs` as conformance gate. |
| `justfile` test-crypto recipe | Blocking local gate | VERIFIED | `cargo nextest run -p famp-crypto` auto-discovers new `sha256_vectors` test binary. |
| `.github/workflows/ci.yml` test-crypto job | Blocking CI gate | VERIFIED | Same recipe path; nextest auto-discovery applies. |

### Key Link Verification

| From | To | Via | Status |
|------|-----|-----|--------|
| `keys.rs` | `ed25519_dalek::VerifyingKey::is_weak` | `TrustedVerifyingKey::from_bytes` | WIRED |
| `verify.rs` | `ed25519_dalek::VerifyingKey::verify_strict` | `verify_canonical_bytes` | WIRED (only call site) |
| `prefix.rs` | `famp_canonical::canonicalize` | `canonicalize_for_signature` | WIRED |
| `worked_example.rs` | `worked-example.json` | `include_str!` | WIRED |
| **`hash.rs`** | **`sha2::Sha256`** | **`use sha2::{Digest, Sha256}`** | **WIRED (new)** |
| **`lib.rs`** | **`hash.rs`** | **`pub mod hash; pub use hash::{sha256_artifact_id, sha256_digest}`** | **WIRED (new)** |
| **`tests/sha256_vectors.rs`** | **`famp_crypto::sha256_artifact_id`** | **direct call + `assert_eq!` byte-exact** | **WIRED (new)** |
| `justfile` | `cargo nextest run -p famp-crypto` | `just test-crypto` | WIRED — 24 tests run (19 prior + 5 new), all pass |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CRYPTO-01 | 02-02 | `Signer`/`Verifier` traits over Ed25519 | SATISFIED | `traits.rs` |
| CRYPTO-02 | 02-01 | Only `verify_strict` exposed, raw `verify` hidden | SATISFIED | `pub(crate)` inner field + grep empty |
| CRYPTO-03 | 02-01 | Weak public key rejection at ingress | SATISFIED | `is_weak()` + 5 fixtures |
| CRYPTO-04 | 02-02, 02-03 | Domain-separation prefix applied before signing | SATISFIED | `DOMAIN_PREFIX` + §7.1c test |
| CRYPTO-05 | 02-02 | RFC 8032 vectors green in CI | SATISFIED | `rfc8032_vectors.rs` |
| CRYPTO-06 | 02-01 | Base64url unpadded for keys/sigs | SATISFIED | `URL_SAFE_NO_PAD` + proptests |
| **CRYPTO-07** | **02-02, 02-04** | **SHA-256 content-addressing via `sha2` crate** | **SATISFIED (closed by 02-04)** | **`sha2` dep wired; `sha256_artifact_id`/`sha256_digest` public; 3 NIST FIPS 180-2 KATs + 2 shape tests pass byte-exact; README documented** |
| CRYPTO-08 | 02-02, 02-03 | Constant-time verify path, documented + tested | SATISFIED | `verify_strict` + `subtle::ConstantTimeEq` + README |
| SPEC-03 | 02-03 | Domain-separation prefix with hex-dump worked example | SATISFIED | `worked-example.json` + README |
| SPEC-19 | 02-01 | Raw 32-byte pub, 64-byte sig, unpadded base64url | SATISFIED | Codec + proptests + fixtures |

All 10 requirement IDs accounted for. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | No TODO/FIXME/placeholder/stub detected. `cargo clippy -p famp-crypto --all-targets -- -D warnings` exits 0. `hash.rs` uses `unwrap_or('0')` (statically unreachable) to stay clippy-clean under `unwrap_used = deny`. |

### Test Execution Summary (2026-04-13 re-run)

```
just test-crypto
  cargo nextest run -p famp-crypto
    24 tests run: 24 passed, 0 skipped (across 6 binaries)
    Including new famp-crypto::sha256_vectors binary (5 tests, all PASS):
      - nist_kat_empty_string
      - nist_kat_abc
      - nist_kat_56byte_vector
      - artifact_id_shape_invariants
      - digest_and_artifact_id_agree
  cargo test -p famp-crypto --doc: 1 passed
  cargo clippy -p famp-crypto --all-targets -- -D warnings: clean
```

Additive-only guarantee held: `git status` shows no unstaged changes in `crates/famp-crypto/src/{sign,verify,keys,prefix,traits,error}.rs`.

### Gaps Summary

**None.** The single Phase 2 gap (CRYPTO-07 SHA-256 content-addressing) was closed by Plan 02-04 via a surgical additive patch: `sha2` workspace dependency wired into `famp-crypto/Cargo.toml`, `hash.rs` module added with `sha256_artifact_id` + `sha256_digest`, re-exported from `lib.rs`, gated by 5 NIST FIPS 180-2 KAT / shape tests in `tests/sha256_vectors.rs`, and documented in `README.md` under `## Content addressing (CRYPTO-07)`.

Phase 2 is now **fully verified**: 7/7 ROADMAP success criteria pass as blocking CI gates, all 10 requirement IDs satisfied, no anti-patterns, no regressions against the previously-verified 6 truths. The phase goal — byte-exact sign/verify with domain separation and second-implementation interop — is achieved and gated.

---

_Verified: 2026-04-13_
_Verifier: Claude (gsd-verifier)_
