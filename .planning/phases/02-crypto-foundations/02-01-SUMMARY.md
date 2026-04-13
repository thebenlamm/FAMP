---
phase: 02-crypto-foundations
plan: 01
subsystem: famp-crypto
tags: [crypto, ed25519, newtype, ingress, base64url]
dependency_graph:
  requires:
    - famp-canonical (CanonicalError wrapped into CryptoError::Canonicalization)
  provides:
    - "famp_crypto::FampSigningKey"
    - "famp_crypto::TrustedVerifyingKey"
    - "famp_crypto::FampSignature"
    - "famp_crypto::CryptoError"
    - "famp_crypto::DOMAIN_PREFIX"
  affects:
    - "Plan 02 (sign/verify) — will consume these newtypes unchanged"
tech_stack:
  added:
    - "ed25519-dalek 2.2.0 (workspace)"
    - "base64 0.22.1 (workspace, URL_SAFE_NO_PAD engine)"
    - "zeroize 1 (derive feature) — drop-time secret wipe via dalek"
    - "subtle 2 — constant-time FampSignature PartialEq"
    - "thiserror 2.0.18 — CryptoError derive"
    - "hex 0.4.3 (dev-dep) — fixture hex decoding"
  patterns:
    - "Free-function-primary + trait-sugar (mirrors Phase 1 D-01)"
    - "Narrow, phase-appropriate error enum (mirrors Phase 1 D-16)"
    - "`TrustedVerifyingKey` newtype — ingress check enforced by type system"
    - "Named must-reject fixtures committed under tests/vectors/"
key_files:
  created:
    - crates/famp-crypto/src/error.rs
    - crates/famp-crypto/src/prefix.rs
    - crates/famp-crypto/src/keys.rs
    - crates/famp-crypto/tests/vectors/must-reject/weak-keys.json
    - crates/famp-crypto/tests/vectors/must-reject/malformed-b64.json
    - crates/famp-crypto/tests/weak_key_rejection.rs
    - crates/famp-crypto/tests/base64_roundtrip.rs
  modified:
    - crates/famp-crypto/Cargo.toml
    - crates/famp-crypto/src/lib.rs
decisions:
  - "Drop-time secret zeroization delegated to ed25519-dalek's own `zeroize`
    feature; do not re-derive `Zeroize`/`ZeroizeOnDrop` on `FampSigningKey`
    because `SigningKey` intentionally does not implement `Zeroize` directly
    (its `Drop` impl wipes the seed). Documented inline above the newtype."
  - "`FampSignature` implements `PartialEq` via `subtle::ConstantTimeEq` on
    raw 64-byte representation; `Eq` marker added for map/set use."
  - "Unused dev/workspace deps (hex, insta, proptest, serde, serde_json,
    zeroize, subtle) declared now for Plan 02 forward-compat; silenced via
    `use _ as _;` shims so workspace `-D unused_crate_dependencies` stays on."
  - "Test modules allow `clippy::expect_used`/`unwrap_used`; production code
    remains unaffected. Resolves Phase 1 carried TODO for famp-crypto test
    hygiene."
metrics:
  duration_minutes: ~15
  tasks_completed: 3
  files_created: 7
  files_modified: 2
  tests_added: 10
  completed: 2026-04-13
requirements_completed:
  - CRYPTO-02
  - CRYPTO-03
  - CRYPTO-06
  - SPEC-19
---

# Phase 02 Plan 01: famp-crypto Newtype Scaffolding Summary

**One-liner:** FAMP-owned Ed25519 newtypes (`FampSigningKey`, `TrustedVerifyingKey`, `FampSignature`) with compiler-enforced weak-key ingress rejection, strict unpadded base64url codec, and the `FAMP-sig-v1\0` domain-separation constant — the unforgeable type boundary every later Phase 2 plan builds on.

## What Shipped

The `famp-crypto` crate gained its type system. Three newtypes wrap
`ed25519-dalek` types with their inner fields `pub(crate)` so no external
code can reach `VerifyingKey::verify` (non-strict). `TrustedVerifyingKey`
is the only verifying-key type reachable from the public API, and its
constructor performs the SPEC §7.1b ingress checks — canonical point
decode followed by `is_weak()` rejection. The type system now guarantees
that any `verify_*` API added in Plan 02 cannot be reached with an
unchecked public key.

Base64url codec methods live on each newtype and route through
`URL_SAFE_NO_PAD`, which strictly rejects `=` padding, the STANDARD
alphabet (`+`/`/`), embedded whitespace, and wrong-length inputs.
`FampSigningKey` redacts its `Debug` output and relies on dalek's own
drop-time zeroization (the zeroize feature is wired in the workspace
dep). `FampSignature` uses `subtle::ConstantTimeEq` for `PartialEq`.

`CryptoError` ships with the Phase 2 variant set from D-27: six narrow
variants including a transparent `Canonicalization(CanonicalError)` wrap
of the Phase 1 error. `DOMAIN_PREFIX` is exposed as a public
`&[u8; 12]` equal to `b"FAMP-sig-v1\0"` (hex `46414d502d7369672d763100`),
verified byte-exact by an inline test.

Three commits land three atomic slices of work:

| Task | Name                                          | Commit    |
| ---- | --------------------------------------------- | --------- |
| 1    | Deps + module layout + `CryptoError` + prefix | `e4cbc33` |
| 2    | Three newtypes with ingress + codec           | `09bc17e` |
| 3    | Must-reject fixtures + external tests         | `1e6afc7` |

## Tests Added

10 tests green via `cargo nextest run -p famp-crypto`:

- `prefix::tests::prefix_bytes_match_spec` — 12-byte DOMAIN_PREFIX hex check
- `keys::tests::identity_point_rejected_as_weak`
- `keys::tests::base64_standard_alphabet_rejected`
- `keys::tests::base64_padded_rejected`
- `keys::tests::debug_signing_key_redacts`
- `keys::tests::signature_partial_eq_constant_time_wrapper`
- `weak_key_rejection::all_weak_key_fixtures_rejected_at_ingress` — 6 named
  small-order / 8-torsion fixtures
- `weak_key_rejection::all_malformed_b64_rejected` — 6 named b64 fixtures
- `base64_roundtrip::signing_key_b64_roundtrip` — proptest
- `base64_roundtrip::signature_b64_roundtrip` — proptest

`cargo clippy -p famp-crypto --all-targets -- -D warnings` is clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `Zeroize` derive on `FampSigningKey` fails to compile**

- **Found during:** Task 2 (`cargo nextest run -p famp-crypto --lib keys`)
- **Issue:** `#[derive(Zeroize, ZeroizeOnDrop)]` on `FampSigningKey(SigningKey)`
  fails because `ed25519_dalek::SigningKey` intentionally does not implement
  `Zeroize` directly — it implements drop-time zeroization via its own `Drop`
  impl (enabled by the `zeroize` feature). The plan's D-08 / Pitfall 4 text
  recommends `ZeroizeOnDrop` but the upstream type doesn't support it.
- **Fix:** Removed the derive; documented inline that drop-time secret wipe
  is delegated to dalek's own `Drop` impl. Kept the `zeroize` workspace dep
  wired for forward-compat and silenced `unused_crate_dependencies` via a
  `use zeroize as _;` shim in `lib.rs`.
- **Files modified:** `crates/famp-crypto/src/keys.rs`,
  `crates/famp-crypto/src/lib.rs`
- **Commit:** `09bc17e`

**2. [Rule 3 — Blocking] Workspace `-D unused_crate_dependencies` fires on
forward-compat deps**

- **Found during:** Task 2 `cargo clippy -- -D warnings`
- **Issue:** Cargo.toml declares `serde`, `serde_json`, `zeroize`, and dev-deps
  `hex`, `insta`, `proptest` for use by Plan 02 and by sibling integration
  tests, but the lib-test and each per-file integration-test compile unit
  sees them as unused and the workspace lint is `-D` in clippy.
- **Fix:** Added `use _ as _;` shims in `lib.rs` (with `#[cfg(test)]` for
  dev-only ones) and at the top of each integration-test file. No
  production code affected.
- **Commit:** `1e6afc7`

**3. [Rule 3 — Blocking] clippy `unnested_or_patterns` on `matches!` macro**

- **Found during:** Task 3 `cargo clippy --all-targets`
- **Issue:** `matches!(res, Err(CryptoError::WeakKey) | Err(CryptoError::InvalidKeyEncoding))`
  triggers the pedantic `unnested_or_patterns` lint.
- **Fix:** Rewrote as `Err(CryptoError::WeakKey | CryptoError::InvalidKeyEncoding)`.
- **Commit:** `1e6afc7`

**4. [Rule 3 — Blocking] Workspace `expect_used`/`unwrap_used` denies fire
in test code**

- **Found during:** Task 3 `cargo clippy --all-targets`
- **Issue:** Workspace clippy config denies `expect_used` and `unwrap_used`
  even in tests; carried as a "Phase 1 known TODO" in STATE.md.
- **Fix:** Added `#![allow(clippy::expect_used, clippy::unwrap_used)]` at
  the top of each integration-test file and on the `keys::tests` module.
  Production code unaffected. This resolves the Phase 1 carried TODO for
  famp-crypto specifically.
- **Commit:** `1e6afc7`

## Requirements Completed

- **CRYPTO-02** — Only `verify_strict` path reachable (no public surface
  exposes `VerifyingKey::verify`); enforced by `pub(crate)` on newtype fields.
- **CRYPTO-03** — Weak public keys rejected at ingress, proven against
  6 named small-order / 8-torsion fixtures.
- **CRYPTO-06** — Base64url unpadded strict codec; proven by proptest
  round-trip + 6 malformed fixtures.
- **SPEC-19** — Ed25519 wire encoding (raw 32/64-byte, base64url unpadded)
  implemented on all three newtypes.

## Known Stubs

None. Plan 02 will add `sign_*` / `verify_*` / `canonicalize_for_signature`
on top of this type boundary without modifying the boundary itself.

## Self-Check: PASSED

- Files exist: `crates/famp-crypto/src/{error,prefix,keys}.rs`,
  `tests/vectors/must-reject/{weak-keys,malformed-b64}.json`,
  `tests/{weak_key_rejection,base64_roundtrip}.rs` — all FOUND
- Commits exist: `e4cbc33`, `09bc17e`, `1e6afc7` — all FOUND in `git log`
- `cargo nextest run -p famp-crypto` — 10/10 green
- `cargo clippy -p famp-crypto --all-targets -- -D warnings` — clean
