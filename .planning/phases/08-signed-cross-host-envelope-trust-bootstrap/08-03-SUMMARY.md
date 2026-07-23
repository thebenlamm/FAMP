---
phase: 08-signed-cross-host-envelope-trust-bootstrap
plan: 03
subsystem: gateway
tags: [ed25519, verify-strict, trust-bootstrap, ingress-verification, pure-function]

requires:
  - phase: 08-signed-cross-host-envelope-trust-bootstrap (plan 01)
    provides: federation wire fields on the envelope (WIRE-02)
  - phase: 08-signed-cross-host-envelope-trust-bootstrap (plan 02)
    provides: FampSigningKey::generate() + key_id() fingerprint primitives
provides:
  - "RejectReason enum (famp-gateway) — InvalidSignature / UnpinnedKey{principal}, D-08's two distinct loud reasons"
  - "verify_inbound<B: BodySchema>(bytes, &Keyring) -> Result<SignedEnvelope<B>, RejectReason> — pure, transport-agnostic gateway ingress verification"
affects: [08-04, phase-9-http-transport-handler]

tech-stack:
  added: []
  patterns:
    - "Two-pass decode: peek_sender (unverified) -> keyring.get (TRUST-02 gate) -> SignedEnvelope::decode (verify_strict-backed)"
    - "verify_inbound is a pure Result-returning function — no I/O, no bus write, no state mutation on any path (D-08)"

key-files:
  created:
    - crates/famp-gateway/src/verify.rs
  modified:
    - crates/famp-gateway/Cargo.toml
    - crates/famp-gateway/src/error.rs
    - crates/famp-gateway/src/lib.rs
    - crates/famp-gateway/src/main.rs
    - Cargo.lock

key-decisions:
  - "RejectReason kept as its own enum in error.rs (not folded into GatewayError) — mirrors GatewayError's per-variant doc discipline and keeps the D-08 two-reason contract visually distinct from broker-connection errors."
  - "verify_inbound never constructs a raw ed25519_dalek::VerifyingKey — routes exclusively through TrustedVerifyingKey / SignedEnvelope::decode (verify_strict internally)."

patterns-established:
  - "TDD RED stub for a pure composed function: an always-Err(InvalidSignature) placeholder is a legitimate RED state even when 2 of 4 behavior tests already pass against it (they assert the reject-with-InvalidSignature outcome the stub trivially produces) — only the accept path and the UnpinnedKey-specific reject path are load-bearing RED signal."

requirements-completed: [WIRE-01, TRUST-02]

coverage:
  - id: D1
    description: "verify_inbound accepts a well-formed envelope signed by the sender's pinned key and returns the typed SignedEnvelope."
    requirement: "WIRE-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::accepts_pinned_valid"
        status: pass
    human_judgment: false
  - id: D2
    description: "An unsigned or signature-invalid (tampered / wrong-pinned-key) envelope is rejected with RejectReason::InvalidSignature before any bus write or state mutation."
    requirement: "WIRE-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::rejects_unsigned"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::rejects_bad_signature"
        status: pass
    human_judgment: false
  - id: D3
    description: "An envelope whose sender principal is absent from the pinned keyring is rejected with RejectReason::UnpinnedKey{principal} — no implicit trust, no auto-pin."
    requirement: "TRUST-02"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::rejects_unpinned_key"
        status: pass
    human_judgment: false

duration: 8min
completed: 2026-07-23
status: complete
---

# Phase 8 Plan 3: Gateway Ingress Verification (verify_inbound) Summary

**Pure `verify_inbound(bytes, &keyring) -> Result<SignedEnvelope<B>, RejectReason>` composing `peek_sender` -> keyring lookup -> `verify_strict`-backed decode, proving WIRE-01 and TRUST-02 in-process with zero transport/bus wiring.**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-07-23T17:10:19-04:00 (prior HEAD, 08-02 complete)
- **Completed:** 2026-07-23T17:12:50-04:00
- **Tasks:** 2 completed
- **Files modified:** 5 (+ Cargo.lock)

## Accomplishments

- Added `famp-keyring` and `famp-envelope` as direct dependencies of `famp-gateway` (previously only reachable indirectly via the `famp` re-export, which does not expose `BodySchema` or `peek_sender`).
- `RejectReason` enum in `error.rs`: exactly two variants, `InvalidSignature` and `UnpinnedKey { principal }`, each carrying a doc comment naming the D-08 decision it maps to — never a flat "rejected."
- `verify_inbound<B: BodySchema>(bytes: &[u8], keyring: &Keyring) -> Result<SignedEnvelope<B>, RejectReason>` in the new `verify.rs`: peeks the sender via `famp_envelope::peek_sender` (no verification yet), hard-rejects with `UnpinnedKey` if the peeked principal has no pinned key (TRUST-02, no auto-pin), otherwise runs `SignedEnvelope::decode` (which internally calls `verify_strict` over the `FAMP-sig-v1\0` domain prefix) and maps any decode/verify failure to `InvalidSignature`.
- Four unit tests, all TDD RED->GREEN: `accepts_pinned_valid`, `rejects_unsigned`, `rejects_bad_signature`, `rejects_unpinned_key` — each also asserts the keyring's `len()` is unchanged after the call, confirming the D-08 no-mutation contract.
- Zero raw `ed25519_dalek::VerifyingKey` construction anywhere in `verify.rs` (only a doc-comment mention) — the sole crypto surface touched is `TrustedVerifyingKey` / `SignedEnvelope::decode`.

## Task Commits

1. **Task 1: `RejectReason` enum + gateway crate dependencies**
   - `feat(08-03)`: `6b885c0` — added `famp-keyring`/`famp-envelope` deps, `RejectReason` enum, placeholder `verify` module (temporary unused-crate-dep silencers in `lib.rs`/`main.rs` until Task 2 wires real usage)
2. **Task 2: `verify_inbound` pure function + unit tests (TDD)**
   - `test(08-03)`: `d9395bb` — RED: 4 tests against a stub `verify_inbound` that always returns `InvalidSignature` (2/4 fail: `accepts_pinned_valid`, `rejects_unpinned_key`)
   - `feat(08-03)`: `e8a3f2c` — GREEN: real `peek_sender -> keyring.get -> SignedEnvelope::decode` composition; all 4 tests pass; Task-1 silencers removed from `lib.rs` (the lib now genuinely uses both deps); `main.rs`'s silencer stays (the bin target still doesn't reference them directly)

**Plan metadata:** committed via this SUMMARY + STATE/ROADMAP update (below)

## Files Created/Modified

- `crates/famp-gateway/Cargo.toml` - added `famp-keyring` and `famp-envelope` as direct `[dependencies]`
- `crates/famp-gateway/src/error.rs` - added `RejectReason` enum (`InvalidSignature`, `UnpinnedKey { principal }`)
- `crates/famp-gateway/src/lib.rs` - `pub mod verify;`, re-export `RejectReason` and `verify_inbound`
- `crates/famp-gateway/src/main.rs` - added a temporary `use famp_envelope as _; use famp_keyring as _;` silencer (the bin target is a separate compilation unit from the lib and doesn't reference either dep)
- `crates/famp-gateway/src/verify.rs` (new) - `verify_inbound` + 4 unit tests
- `Cargo.lock` - records the new intra-workspace dependency edges

## Decisions Made

- **`RejectReason` stays a standalone enum, not folded into `GatewayError`** — matches the `GatewayError` per-variant doc discipline (`08-PATTERNS.md`) and keeps the D-08 two-reason contract visually and semantically distinct from broker-connection failures.
- **`main.rs` keeps a `famp_envelope`/`famp_keyring` silencer** (not removed alongside the lib's) — the bin target genuinely has no reference to either crate; Phase 9's HTTP transport handler is expected to be the first caller that gives the bin a real reason to reference them.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Placeholder `verify.rs` created in Task 1, not Task 2**
- **Found during:** Task 1, running `cargo build -p famp-gateway`
- **Issue:** The plan's Task 1 action instructs adding `pub mod verify;` to `lib.rs`, but `verify.rs` itself is listed as a Task 2 file. A `mod` declaration with no backing file does not compile.
- **Fix:** Created a minimal doc-comment-only `verify.rs` placeholder in Task 1's commit so the crate builds; Task 2 then RED/GREEN-built the real content into the same file.
- **Files modified:** `crates/famp-gateway/src/verify.rs` (created early), `crates/famp-gateway/src/lib.rs`, `crates/famp-gateway/src/main.rs` (temporary `unused_crate_dependencies` silencers for the two new deps until Task 2's real usage lands)
- **Verification:** `cargo build -p famp-gateway && just lint` both clean after Task 1; silencers removed once genuinely unnecessary (Task 2, lib side only)
- **Committed in:** `6b885c0` (Task 1)

**2. [Rule 1 - Bug] `clippy::redundant_clone` on two test helper calls**
- **Found during:** Task 2 GREEN, running `just lint`
- **Issue:** `rejects_unsigned` and `rejects_bad_signature` called `from.clone()` when pinning the keyring even though `from` isn't used again afterward in those two tests (unlike `accepts_pinned_valid`/`rejects_unpinned_key`, which do reuse it for an equality assertion).
- **Fix:** Removed the unnecessary `.clone()` in both call sites, moving `from` by value into `pin_tofu`.
- **Files modified:** `crates/famp-gateway/src/verify.rs`
- **Verification:** `just lint` exits 0; `cargo test -p famp-gateway --lib verify` still 4/4 green
- **Committed in:** `e8a3f2c` (Task 2 GREEN)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Neither changes scope or behavior — both are mechanical consequences of the plan's own task/file split and the workspace's `-D warnings` clippy gate.

## Issues Encountered
None beyond the deviations above.

## User Setup Required
None — no external service configuration required. This is a pure in-process function; no gateway process, socket, or CLI surface is touched (Phase 9 wires the live transport).

## Next Phase Readiness

- `verify_inbound` is ready for Plan 04's `famp peer export`/`import` round-trip test (the pinning half of the TRUST-01/TRUST-02 story) and for Phase 9's HTTP transport handler, which just needs to feed it the raw request body.
- `RejectReason`'s two variants give Phase 9 everything it needs to map rejections onto distinct HTTP 4xx responses without re-deriving the distinction.
- No blockers identified for downstream plans in this phase.

---
*Phase: 08-signed-cross-host-envelope-trust-bootstrap*
*Completed: 2026-07-23*

## Self-Check: PASSED

All 3 commit hashes (6b885c0, d9395bb, e8a3f2c) found in `git log`.
All 5 modified/created files confirmed present on disk.
