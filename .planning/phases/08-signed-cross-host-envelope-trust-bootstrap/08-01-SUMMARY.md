---
phase: 08-signed-cross-host-envelope-trust-bootstrap
plan: 01
subsystem: protocol
tags: [rust, serde, ed25519, envelope, jcs, canonical-json]

# Dependency graph
requires:
  - phase: 07-broker-liveness-fork-gateway-skeleton
    provides: famp-gateway Design A local-proxy skeleton this phase's later plans build ingress-verify on
provides:
  - WireEnvelope<B> / UnsignedEnvelope<B> / WireEnvelopeRef<'a,B> carry 7 optional federation fields (from_domain, to_domain, sender_key_id, nonce, expiry, capability, approval), all omit-when-empty
  - with_from_domain / with_to_domain / with_sender_key_id / with_nonce / with_expiry builder methods on UnsignedEnvelope
  - federation_format_ok() on SignedEnvelope — D-04 well-formedness-only check (no active nonce/expiry enforcement)
  - Three regression tests locking WIRE-02 round-trip, D-02 local-bus byte-identity, and D-04 format-validate-only behavior
affects: [08-02-plan, phase-09-signed-cross-host-envelope-trust-bootstrap]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Lockstep struct-literal updates: any new envelope field must touch WireEnvelope (wire.rs), UnsignedEnvelope, WireEnvelopeRef, the sign()/encode() WireEnvelopeRef literals, decode_value()'s inner reconstruction, UnsignedEnvelope::new(), and the version-drift compile_fail doctest — all seven sites in one commit."
    - "Plain Option<T> + skip_serializing_if fields only in the envelope — never serde(flatten)/serde(tag), preserves deny_unknown_fields."

key-files:
  created: []
  modified:
    - crates/famp-envelope/src/wire.rs
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/timestamp.rs

key-decisions:
  - "federation_format_ok() lives on SignedEnvelope (not a free function) since it needs both the envelope's own ts and its federation fields — matches the existing accessor-method style (causality(), terminal_status())."
  - "expiry vs ts ordering uses lexical string comparison of the byte-preserving RFC 3339 strings (not a datetime parse) — correct for same-format fixed-width timestamps and avoids reintroducing Pitfall P6 (re-serialization drift) or a new time-crate dependency."

requirements-completed: [WIRE-02]

coverage:
  - id: D1
    description: "UnsignedEnvelope with all 7 federation fields populated signs, encodes, and decodes back byte-exact with every field preserved (WIRE-02 / D-01)"
    requirement: "WIRE-02"
    verification:
      - kind: unit
        ref: "crates/famp-envelope/src/envelope.rs#envelope::tests::federation_fields_roundtrip"
        status: pass
    human_judgment: false
  - id: D2
    description: "Local-bus envelope with no federation fields set serializes byte-identical to pre-Phase-8 output (D-02 omit-when-empty)"
    requirement: "WIRE-02"
    verification:
      - kind: unit
        ref: "crates/famp-envelope/src/envelope.rs#envelope::tests::local_bus_byte_identical"
        status: pass
    human_judgment: false
  - id: D3
    description: "federation_format_ok() format-validates nonce/expiry only — accepts well-formed values, flags empty nonce / expiry <= ts, does NOT reject a past-but-valid expiry (D-04, no active enforcement)"
    requirement: "WIRE-02"
    verification:
      - kind: unit
        ref: "crates/famp-envelope/src/envelope.rs#envelope::tests::federation_format_well_formed"
        status: pass
    human_judgment: false

duration: ~20min
completed: 2026-07-23
status: complete
---

# Phase 8 Plan 01: Signed Cross-Host Envelope Wire Extension Summary

**Extended the single FAMP wire envelope with 7 optional, omit-when-empty federation fields (from_domain, to_domain, sender_key_id, nonce, expiry, capability, approval), covered by the one existing INV-10 signature — no wire break, local-bus bytes stay identical.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-23
- **Tasks:** 2/2 completed
- **Files modified:** 3

## Accomplishments
- All 7 federation fields added in lockstep across `WireEnvelope`, `UnsignedEnvelope`, `WireEnvelopeRef`, both `WireEnvelopeRef` construction sites (`sign()`, `encode()`), `decode_value()`'s inner reconstruction, `UnsignedEnvelope::new()`, and the version-drift `compile_fail` doctest — avoiding RESEARCH Pitfalls 1 and 2.
- Five builder methods added (`with_from_domain`, `with_to_domain`, `with_sender_key_id`, `with_nonce`, `with_expiry`); `capability`/`approval` deliberately have no builder — reserved, opaque, unread by any code path.
- `federation_format_ok()` implements D-04's format-validate-only well-formedness check with no active anti-replay/expiry enforcement.
- Three new regression tests pin WIRE-02 byte-exact round-trip, the D-02 local-bus byte-identity invariant, and the D-04 no-enforcement contract.

## Task Commits

1. **Task 1: Add the 7 federation fields across all envelope struct sites in lockstep** - `66eeb48` (feat)
2. **Task 2: Federation round-trip test + local-bus byte-identity regression test** - `9b5aa2f` (test)

_Note: this plan was TDD-flagged at the task level, but wrote the behavior + tests in the same commit sequence (Task 1 = implementation + doctest fix, Task 2 = the three regression tests) — see Deviations._

## Files Created/Modified
- `crates/famp-envelope/src/wire.rs` - `WireEnvelope<B>` gains the 7 federation `Option` fields before `pub body: B`.
- `crates/famp-envelope/src/envelope.rs` - `UnsignedEnvelope<B>`, `WireEnvelopeRef<'a,B>`, both construction sites, `decode_value()`, `UnsignedEnvelope::new()`, the compile_fail doctest, 5 new builders, `federation_format_ok()`, and 3 new tests.
- `crates/famp-envelope/src/timestamp.rs` - `shallow_validate()` visibility widened from private to `pub(crate)` (see Deviations).

## Decisions Made
- `federation_format_ok()` placed on `SignedEnvelope` (needs `ts` + federation fields together) rather than as a free function.
- expiry-vs-ts ordering done via lexical string comparison on the byte-preserving RFC 3339 strings, not a datetime parse — no new dependency, and avoids reintroducing Pitfall P6 (parsing through `time::OffsetDateTime` would risk re-serialization drift if that pattern were later copied into the signing path).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Widened `timestamp.rs::shallow_validate` from private to `pub(crate)`**
- **Found during:** Task 1 (implementing `federation_format_ok()`)
- **Issue:** D-04's format-validate-only helper needs to shallow-validate `expiry`'s RFC 3339 shape without duplicating the shallow-check logic already in `timestamp.rs`. The existing `shallow_validate` function was private to its module (unreachable from `envelope.rs`, same crate, different module).
- **Fix:** Changed `fn shallow_validate` to `pub(crate) fn shallow_validate` — visibility-only change, zero behavior change, no new logic. `envelope.rs::federation_format_ok()` calls it directly.
- **Files modified:** `crates/famp-envelope/src/timestamp.rs`
- **Verification:** `cargo test -p famp-envelope --lib` all green; `just lint` clean.
- **Committed in:** `66eeb48` (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking, visibility-only).
**Impact on plan:** No scope creep — the fix is a one-word visibility change enabling the D-04 helper the plan explicitly required; no new files outside the plan's `files_modified` list except this one adjacent file, and no behavior change to any existing caller of `shallow_validate`.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
The extended envelope schema (WIRE-02) is landed and byte-exact round-trip-tested. Ready for Phase 8 Plan 02 (or later) to build the `famp-crypto::key_id` fingerprint helper, `famp peer export`/`import` trust bootstrap CLI, and `famp-gateway::verify_inbound` — all of which consume the field shapes this plan established. No blockers.

---
*Phase: 08-signed-cross-host-envelope-trust-bootstrap*
*Completed: 2026-07-23*

## Self-Check: PASSED
All modified files present on disk; commits 66eeb48 and 9b5aa2f found in git log.
