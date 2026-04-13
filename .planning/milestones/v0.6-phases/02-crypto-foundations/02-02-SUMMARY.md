---
phase: 02-crypto-foundations
plan: 02
subsystem: famp-crypto
tags: [crypto, ed25519, sign, verify, domain-separation, rfc8032]
dependency_graph:
  requires:
    - "famp-crypto Plan 01 (FampSigningKey, TrustedVerifyingKey, FampSignature, CryptoError, DOMAIN_PREFIX)"
    - "famp_canonical::canonicalize (Phase 1)"
  provides:
    - "famp_crypto::sign_value"
    - "famp_crypto::sign_canonical_bytes"
    - "famp_crypto::verify_value"
    - "famp_crypto::verify_canonical_bytes"
    - "famp_crypto::canonicalize_for_signature"
    - "famp_crypto::Signer"
    - "famp_crypto::Verifier"
  affects:
    - "Plan 03 (§7.1c worked-example fixture) — will consume verify_canonical_bytes + canonicalize_for_signature unchanged"
    - "Future famp-envelope — sign_envelope / verify_envelope will wrap sign_canonical_bytes"
tech_stack:
  added: []
  patterns:
    - "Free-function-primary + trait-sugar (Phase 1 D-01, continued)"
    - "DOMAIN_PREFIX prepended internally — callers never assemble signing input"
    - "verify_strict-only route — non-strict verify is unreachable from public API"
    - "External vectors as hard CI gate (Phase 1 pattern applied to Ed25519 primitive)"
key_files:
  created:
    - crates/famp-crypto/src/sign.rs
    - crates/famp-crypto/src/verify.rs
    - crates/famp-crypto/src/traits.rs
    - crates/famp-crypto/tests/vectors/rfc8032/test-vectors.json
    - crates/famp-crypto/tests/rfc8032_vectors.rs
  modified:
    - crates/famp-crypto/src/prefix.rs
    - crates/famp-crypto/src/lib.rs
decisions:
  - "canonicalize_for_signature lives in prefix.rs alongside DOMAIN_PREFIX — the
    prefix and its only sanctioned caller live together, so there is no
    public path that can drift out of sync."
  - "Signer / Verifier traits are pure sugar: every method body is a single
    delegation to the free function. No trait-specific logic, no alt path.
    Proven byte-equal to free-function output in tests."
  - "RFC 8032 §7.1 vectors exercise raw ed25519-dalek (no FAMP prefix). The
    algorithm gate is deliberately separate from the FAMP protocol gate
    (Plan 03 §7.1c worked example) so a future Ed25519-library swap can be
    validated without touching the FAMP layer."
  - "Full 1023-byte RFC 8032 TEST 1024 vector committed verbatim in JSON
    rather than generated at test time — matches Phase 1 external-vector
    discipline and keeps the fixture diffable."
metrics:
  duration_minutes: ~10
  tasks_completed: 3
  files_created: 5
  files_modified: 2
  tests_added: 8
  completed: 2026-04-13
requirements_completed:
  - CRYPTO-01
  - CRYPTO-04
  - CRYPTO-05
  - CRYPTO-07
  - CRYPTO-08
---

# Phase 02 Plan 02: famp-crypto Sign/Verify Operations Summary

**One-liner:** FAMP-prefix-internalizing `sign_value` / `verify_value` / `sign_canonical_bytes` / `verify_canonical_bytes` free functions plus `Signer` / `Verifier` traits, gated by all 5 RFC 8032 §7.1 Ed25519 test vectors — the operations layer Plan 03's §7.1c worked example will build on unchanged.

## What Shipped

Plan 01 delivered the type boundary; Plan 02 delivers the operations. Every
byte that enters FAMP's signature path now passes through functions that
prepend `DOMAIN_PREFIX` (`FAMP-sig-v1\0`, 12 bytes) internally before
touching Ed25519. Callers cannot assemble signing input manually by
accident — `sign_canonical_bytes` and `verify_canonical_bytes` own that
concatenation, and the only public surface that exposes the raw
prefix-prepended bytes is `canonicalize_for_signature`, which lives in
`prefix.rs` alongside the prefix constant itself.

Verification routes exclusively through
`ed25519_dalek::VerifyingKey::verify_strict`. A `grep '.verify('` sweep of
`crates/famp-crypto/src/` returns zero hits — only `verify_strict` appears,
and it is reached only via `TrustedVerifyingKey`, which itself can only be
constructed via the weak-key-rejecting ingress path from Plan 01. The
type system now guarantees that no reachable code path can verify a
signature against an unchecked public key using the legacy non-strict
verifier.

`Signer` and `Verifier` traits are implemented on `FampSigningKey` and
`TrustedVerifyingKey` as pure delegation to the free functions. A test
asserts `<FampSigningKey as Signer>::sign_value(&sk, &v) ==
sign::sign_value(&sk, &v)` — the trait layer is byte-equal sugar, not an
alternate code path. No `ed25519_dalek::Signer` impl exists for the FAMP
newtypes; FAMP signing rules (domain prefix, canonical-JSON input) are
protocol-specific and are not generic Ed25519.

The RFC 8032 §7.1 test-vector file ships all 5 vectors (TEST 1 empty
message, TEST 2 1-byte, TEST 3 2-byte, TEST 1024 1023-byte, TEST SHA(abc))
as literal hex in a single JSON file. A single integration test
(`all_rfc8032_vectors_byte_exact`) iterates the vectors and asserts:
(1) derived public key matches vector pk; (2) `SigningKey::sign(msg)`
produces exactly the vector signature bytes; (3) `verify_strict` accepts
the vector signature. These vectors exercise the **Ed25519 primitive**
directly — the FAMP `DOMAIN_PREFIX` is not applied, because RFC 8032 signs
the raw message. This is the algorithm gate; Plan 03's §7.1c worked example
is the separate protocol gate.

Three commits land three atomic slices:

| Task | Name                                                   | Commit    |
| ---- | ------------------------------------------------------ | --------- |
| 1    | sign/verify free fns + canonicalize_for_signature      | `e2724a1` |
| 2    | Signer/Verifier traits as thin sugar                   | `6aefe01` |
| 3    | RFC 8032 §7.1 vectors as hard CI gate                  | `0bdd434` |

## Tests Added

8 tests added, 18/18 total for the crate green via `cargo nextest run -p famp-crypto`:

- `verify::tests::roundtrip_value` — sign_value/verify_value on a typed json value
- `verify::tests::roundtrip_canonical_bytes` — byte-level round-trip
- `verify::tests::tampered_payload_fails` — mutated bytes → `VerificationFailed`
- `verify::tests::tampered_signature_fails` — single-bit-flipped sig → `VerificationFailed`
- `verify::tests::canonicalize_for_signature_starts_with_prefix` — prefix byte check + length equality
- `traits::tests::trait_sugar_matches_free_fn` — trait output byte-equal to free-fn output
- `traits::tests::trait_canonical_bytes_sugar_matches_free_fn` — canonical-bytes path sugar
- `rfc8032_vectors::all_rfc8032_vectors_byte_exact` — 5-vector algorithm gate

`cargo clippy -p famp-crypto --all-targets -- -D warnings` is clean.
`! grep -rn '\.verify(' crates/famp-crypto/src/` — verified absent.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `clippy::too_long_first_doc_paragraph` on traits.rs module doc**

- **Found during:** Task 2 `cargo clippy --all-targets -- -D warnings`
- **Issue:** The module-level doc comment on `traits.rs` was a single
  four-line paragraph which tripped the workspace-denied pedantic lint.
- **Fix:** Split into a one-line first paragraph + a second paragraph. No
  semantic change.
- **Commit:** `0bdd434`

**2. [Rule 3 — Blocking] `hex` workspace dev-dep unused in lib-test compile unit**

- **Found during:** Task 3 `cargo clippy --all-targets -- -D warnings`
- **Issue:** Plan 01 had intentionally silenced `hex` as `#[cfg(test)] use hex as _;`
  to satisfy workspace `-D unused_crate_dependencies`. Task 1's lib.rs rewrite
  dropped the shim because the lib was moving toward direct use via integration
  tests; the lib-test compile unit still couldn't see `hex`.
- **Fix:** Restored the `#[cfg(test)] use hex as _;` shim in `lib.rs`. `hex`
  is used by the integration test `rfc8032_vectors.rs`, which has its own
  compile unit; the shim is only needed for the lib-test unit.
- **Commit:** `0bdd434`

**3. [Rule 3 — Blocking] Workspace `-D unused_crate_dependencies` fires on rfc8032_vectors integration test**

- **Found during:** Task 3 clippy gate
- **Issue:** The `rfc8032_vectors.rs` integration test compile unit only uses
  `ed25519_dalek`, `serde`, `serde_json`, and `hex` — every other workspace
  dep (`famp-canonical`, `famp-crypto`, `base64`, `insta`, `proptest`,
  `subtle`, `thiserror`, `zeroize`) is declared in Cargo.toml but unused by
  this specific binary, tripping the workspace lint.
- **Fix:** Added `use {name} as _;` shims at the top of `rfc8032_vectors.rs`
  for every unused workspace crate. Mirrors the pattern Plan 01 established
  for `weak_key_rejection.rs` and `base64_roundtrip.rs`.
- **Commit:** `0bdd434`

## Requirements Completed

- **CRYPTO-01** — `sign_value` / `sign_canonical_bytes` / `verify_value` /
  `verify_canonical_bytes` free functions exist; Ed25519 sign/verify with
  domain-separation prefix applied internally.
- **CRYPTO-04** — Every verification route reaches `verify_strict` only.
  Non-strict `verify` is not called from anywhere in `crates/famp-crypto/src/`.
- **CRYPTO-05** — `DOMAIN_PREFIX` is prepended by `sign_canonical_bytes` and
  `verify_canonical_bytes` internally. `canonicalize_for_signature` is the
  only public surface that materializes prefix-prepended bytes.
- **CRYPTO-07** — `sha2` is already pulled in via `famp-canonical`'s
  artifact-ID helpers (Phase 1 D-19/D-20); Plan 02 adds no new code but
  the transitive availability stands. Confirmed by successful `cargo build`
  with `famp-canonical::artifact_id_*` reachable via dependency.
- **CRYPTO-08** — RFC 8032 §7.1 vectors all 5 byte-exact; algorithm gate green.

## Known Stubs

None. Plan 03 will wrap these operations in the §7.1c worked-example fixture
without modifying the operations layer itself.

## Self-Check: PASSED

- Files exist:
  - `crates/famp-crypto/src/sign.rs` — FOUND
  - `crates/famp-crypto/src/verify.rs` — FOUND
  - `crates/famp-crypto/src/traits.rs` — FOUND
  - `crates/famp-crypto/tests/vectors/rfc8032/test-vectors.json` — FOUND
  - `crates/famp-crypto/tests/rfc8032_vectors.rs` — FOUND
- Commits exist: `e2724a1`, `6aefe01`, `0bdd434` — all FOUND in `git log`
- `cargo nextest run -p famp-crypto` — 18/18 green
- `cargo clippy -p famp-crypto --all-targets -- -D warnings` — clean
- `grep -rn '\.verify(' crates/famp-crypto/src/` — zero hits (verify_strict only)
