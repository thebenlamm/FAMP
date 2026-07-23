---
phase: 08-signed-cross-host-envelope-trust-bootstrap
plan: 02
subsystem: crypto
tags: [ed25519-dalek, rand, sha2, base64, fingerprint, keygen]

requires:
  - phase: 08-signed-cross-host-envelope-trust-bootstrap (plan 01)
    provides: gateway skeleton / prior Phase 8 groundwork
provides:
  - "FampSigningKey::generate() — CSPRNG (OsRng) Ed25519 keypair constructor"
  - "key_id(&TrustedVerifyingKey) -> String — stable 16-char b64url fingerprint"
affects: [08-03, 08-04, trust-bootstrap-cli, gateway-ingress-verify]

tech-stack:
  added: [rand (workspace dep, newly added to famp-crypto specifically)]
  patterns:
    - "generate() constructors route through OsRng only; no fixed-seed production path"
    - "key_id is diagnostic/UX metadata, never a trust anchor — full pinned pubkey is the sole anchor"

key-files:
  created: []
  modified:
    - crates/famp-crypto/Cargo.toml
    - crates/famp-crypto/src/keys.rs
    - crates/famp-crypto/src/lib.rs
    - crates/famp-crypto/tests/base64_roundtrip.rs
    - crates/famp-crypto/tests/rfc8032_vectors.rs
    - crates/famp-crypto/tests/sha256_vectors.rs
    - crates/famp-crypto/tests/weak_key_rejection.rs
    - crates/famp-crypto/tests/worked_example.rs
    - Cargo.lock

key-decisions:
  - "key_id truncates to 16 base64url chars (~96 bits) per D-03 / RESEARCH Assumption A1, locked by this plan."
  - "generate() is the only sanctioned non-test signing-key constructor; OsRng-only, no fixed-seed/time/PID path in production code."

patterns-established:
  - "Pattern: new direct dep on an integration-test-heavy crate requires `use <dep> as _;` in every test binary that doesn't reference it directly, per the workspace's unused_crate_dependencies lint gate."

requirements-completed: [WIRE-02, TRUST-01]

coverage:
  - id: D1
    description: "FampSigningKey::generate() produces a fresh keypair from OsRng; two calls yield distinct keys; the generated key signs and verifies via the existing sign/verify path."
    requirement: "TRUST-01"
    verification:
      - kind: unit
        ref: "crates/famp-crypto/src/keys.rs#keys::tests::generate_produces_distinct_keys"
        status: pass
      - kind: unit
        ref: "crates/famp-crypto/src/keys.rs#keys::tests::generated_key_signs_and_verifies"
        status: pass
    human_judgment: false
  - id: D2
    description: "key_id(vk) derives a deterministic, 16-char b64url fingerprint of an Ed25519 verifying key, re-exported from the crate root."
    requirement: "WIRE-02"
    verification:
      - kind: unit
        ref: "crates/famp-crypto/src/keys.rs#keys::tests::key_id_is_deterministic_and_16_chars"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-23
status: complete
---

# Phase 8 Plan 2: Keygen + Fingerprint Primitives Summary

**Added `FampSigningKey::generate()` (OsRng CSPRNG keypair constructor) and `key_id()` (16-char b64url Ed25519 fingerprint) to `famp-crypto`, both TDD'd and re-exported.**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-07-23T16:57:19-04:00 (prior HEAD)
- **Completed:** 2026-07-23T17:05:34-04:00
- **Tasks:** 2 completed
- **Files modified:** 9

## Accomplishments
- `FampSigningKey::generate()` constructs a fresh Ed25519 keypair from `rand::rngs::OsRng`, unblocking TRUST-01 (there was previously no keygen path anywhere in the codebase since the v0.9 CLI purge).
- `key_id(&TrustedVerifyingKey) -> String` derives a stable, deterministic, 16-character `URL_SAFE_NO_PAD` fingerprint (`SHA-256(pubkey)` truncated), satisfying the WIRE-02 `sender_key_id` derivation locked by D-03. Documented explicitly as non-anchor diagnostic metadata.
- Both primitives are re-exported from `famp_crypto`'s crate root for downstream `famp` / `famp-gateway` consumption.

## Task Commits

Each task followed RED → GREEN TDD:

1. **Task 1: `FampSigningKey::generate()` with OsRng**
   - `test(08-02)`: `5e6f415` — failing test (RED): `generate()` didn't exist, compile error
   - `feat(08-02)`: `22ba66e` — implementation (GREEN): added `rand` dep, `generate()`, silenced `unused_crate_dependencies` in 5 integration-test binaries
2. **Task 2: `key_id` fingerprint function + re-export**
   - `test(08-02)`: `c4d701f` — failing test (RED): `key_id()` didn't exist, compile error
   - `feat(08-02)`: `616d25f` — implementation (GREEN): `key_id()` added, re-exported from `lib.rs`

**Plan metadata:** committed via this SUMMARY + STATE/ROADMAP update (below)

## Files Created/Modified
- `crates/famp-crypto/Cargo.toml` - added `rand = { workspace = true }` as a direct dependency
- `crates/famp-crypto/src/keys.rs` - `FampSigningKey::generate()`, `key_id()`, both with unit tests
- `crates/famp-crypto/src/lib.rs` - re-export `key_id` alongside existing crypto re-exports
- `crates/famp-crypto/tests/{base64_roundtrip,rfc8032_vectors,sha256_vectors,weak_key_rejection,worked_example}.rs` - added `use rand as _;` to silence the workspace `unused_crate_dependencies` lint in test binaries that don't reference `rand` directly
- `Cargo.lock` - records the new `rand 0.8.5` edge on `famp-crypto`

## Decisions Made
- **key_id truncation locked at 16 b64url chars** (~96 bits) per D-03 / RESEARCH Assumption A1 — sufficient for the own-two-machines eyeball-verification use case; explicitly documented as never a trust anchor (T-08-05 mitigation).
- **generate() is OsRng-only** — no fixed-seed, time-based, or PID-based production path (T-08-04 mitigation); existing `from_bytes([0u8; 32])`-style test fixtures are untouched and remain test-only by convention.
- **rand added directly to famp-crypto's Cargo.toml** rather than relying on the workspace-level dependency being implicitly available — matches Rust's per-crate dependency resolution requirement, as RESEARCH flagged.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Silenced `unused_crate_dependencies` lint in 5 famp-crypto integration test binaries**
- **Found during:** Task 1 (`FampSigningKey::generate()` with OsRng), verified via `just lint`
- **Issue:** Adding `rand` as a direct `famp-crypto` dependency tripped the workspace's `-D unused-crate-dependencies` clippy gate in every integration test binary (`tests/*.rs`) that doesn't reference `rand` directly — each `tests/*.rs` file is its own compilation unit under `cargo test`, so the crate-root `use rand as _;` pattern in `lib.rs` doesn't cover them.
- **Fix:** Added `use rand as _;` to `base64_roundtrip.rs`, `rfc8032_vectors.rs`, `sha256_vectors.rs`, `weak_key_rejection.rs`, and `worked_example.rs`, matching the existing convention already used in those files for `hex`, `insta`, `proptest`, etc.
- **Files modified:** the 5 test files listed above
- **Verification:** `just lint` exits 0 (workspace-wide clippy pedantic/nursery `-D warnings`)
- **Committed in:** `22ba66e` (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to keep the crate lint-clean per the plan's own verify step (`just lint`). No scope creep — purely a consequence of adding the `rand` dependency the plan itself specified.

## Issues Encountered
None beyond the deviation above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `FampSigningKey::generate()` unblocks Plan 03/04's gateway keypair generation + persistence work (`famp peer export`/`import`, TRUST-01 CLI).
- `key_id()` is ready for the WIRE-02 envelope field wiring (`sender_key_id`) and the `famp peer export` human-readable fingerprint line (D-05).
- No blockers identified for downstream plans in this phase.

---
*Phase: 08-signed-cross-host-envelope-trust-bootstrap*
*Completed: 2026-07-23*

## Self-Check: PASSED

All 4 commit hashes (5e6f415, 22ba66e, c4d701f, 616d25f) found in `git log`.
All 8 modified/created files confirmed present on disk.
