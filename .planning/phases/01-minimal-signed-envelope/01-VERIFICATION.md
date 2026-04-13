---
phase: 01-minimal-signed-envelope
verified: 2026-04-13T00:00:00Z
status: passed
score: 5/5 success criteria verified
tests_run: 73
tests_passed: 73
note: "REQUIREMENTS.md has stale ENV-09/ENV-12 checkboxes marked 'Pending' — code and tests enforce both narrowings at the type level; suggest updating REQUIREMENTS.md."
---

# Phase 01: Minimal Signed Envelope — Verification Report

**Phase Goal:** `famp-envelope` encodes, decodes, and signature-verifies every message class the Personal Runtime actually emits, and rejects anything else at the type level.

**Verified:** 2026-04-13
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Success Criteria (from ROADMAP.md)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Round-trips every shipped class via famp-canonical + verify_strict, byte-exact both directions (proptest) | VERIFIED | `prop_roundtrip.rs` — 6 proptests pass (request/commit/deliver/ack/control roundtrip + tamper); `roundtrip_signed.rs` — 7 roundtrip tests pass |
| 2 | Decoding unsigned envelope returns typed `ProtocolError` (INV-10); unsigned is unreachable downstream | VERIFIED | `adversarial::missing_signature_rejected` passes; `error.rs` maps `MissingSignature \| InvalidSignatureEncoding \| SignatureInvalid → ProtocolErrorKind::Unauthorized`; `compile_fail_unsigned.rs` INV-10 marker present |
| 3 | Every body uses `deny_unknown_fields`; unknown keys at any depth fail decode | VERIFIED | `body_shapes::unknown_body_field_nested_rejected`, `adversarial::unknown_envelope_field_rejected`, `adversarial::unknown_body_field_nested_rejected_at_body_level` pass |
| 4 | ENV-12 (cancel-only) enforced at type level: no constructor/deserialize yields `control` with `supersede`/`close` | VERIFIED | `body_shapes::control_supersede_rejected`, `control_close_rejected`, `control_revert_transfer_rejected`, `control_cancel_if_not_started_rejected` pass; `ControlAction` is single-variant `{Cancel}` in `src/body/control.rs` |
| 5 | ENV-09 (narrowed): no capability_snapshot in CommitBody; documented with v0.8 pointer | VERIFIED | `body_shapes::commit_body_rejects_capability_snapshot` passes; `src/body/commit.rs` has no capability_snapshot field |

**Score:** 5/5 truths verified.

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/famp-envelope/Cargo.toml` | VERIFIED | famp-canonical/famp-crypto/famp-core path deps wired |
| `crates/famp-envelope/src/class.rs` (33 L) | VERIFIED | `MessageClass` 5 variants, snake_case serde |
| `crates/famp-envelope/src/scope.rs` (26 L) | VERIFIED | `EnvelopeScope` 3 variants |
| `crates/famp-envelope/src/version.rs` (49 L) | VERIFIED | `FampVersion` literal "0.5.1" |
| `crates/famp-envelope/src/timestamp.rs` (80 L) | VERIFIED | Byte-preserving newtype, P6 comment present |
| `crates/famp-envelope/src/error.rs` (114 L) | VERIFIED | 18-variant `EnvelopeDecodeError`; exhaustive map to `ProtocolError` |
| `crates/famp-envelope/src/body/{mod,request,commit,deliver,ack,control,bounds}.rs` | VERIFIED | All 7 files present; sealed `BodySchema` trait |
| `crates/famp-envelope/src/envelope.rs` (520 L) | VERIFIED | `UnsignedEnvelope<B>` / `SignedEnvelope<B>` type-state (plan min 150 L) |
| `crates/famp-envelope/src/dispatch.rs` (62 L) | VERIFIED | `AnySignedEnvelope` enum + manual class dispatch (no serde tag) |
| `crates/famp-envelope/src/wire.rs` (50 L) | VERIFIED | Private `WireEnvelope` decode plumbing |
| `tests/vectors/vector_0/envelope.json` | VERIFIED | §7.1c.7 signed wire envelope |
| `tests/vectors/vector_0/canonical.hex` (648 chars = 324 B) | VERIFIED | Matches spec; `canonical.len() == 324` asserted in test |
| `tests/vectors/vector_0/signing_input.hex` | VERIFIED | §7.1c.5 DOMAIN_PREFIX ǁ canonical |
| `tests/vectors/vector_0/signature.hex` (128 chars = 64 B) | VERIFIED | §7.1c.6 raw Ed25519 |
| `tests/vectors/vector_0/signature.b64url` | VERIFIED | `k2aqzthUx4mHNZCNLi2XMgiQX9gOL5P-UFcQ9Y8O0fyS47nXoZswss8YT3A1Utr8-RyoEyH1f6aJ0aloZdC2CA` |
| `tests/vector_zero.rs` (125 L) | VERIFIED | 5 tests; all pass |
| `tests/compile_fail_unsigned.rs` (25 L) | VERIFIED | INV-10 type-level marker |

### Key Link Verification

| From | To | Via | Status |
|------|-----|-----|--------|
| `envelope.rs` | `famp_crypto::verify_value` | SignedEnvelope::decode → verify | WIRED (present in 520-line envelope.rs; vector_zero tests succeed through verify_strict path) |
| `envelope.rs` | `famp_canonical::from_slice_strict` | SignedEnvelope::decode → strict parse | WIRED (vector_zero uses `from_slice_strict`) |
| `dispatch.rs` | `SignedEnvelope::<B>::decode_value` | shared private core | WIRED (`AnySignedEnvelope::decode` routes vector 0 → `Ack`, rejects unknown class before signature verify) |
| `vector_zero.rs` | `tests/vectors/vector_0/envelope.json` | fs::read + decode | WIRED (test passes) |

### Vector-0 Byte-Exact Invariant

The load-bearing interop anchor. Test assertions:

1. `vector_0_canonical_bytes_byte_exact`: strip signature → RFC 8785 canonicalize → matches `canonical.hex` → asserts `canonical.len() == 324`. PASS.
2. `vector_0_signature_reproduces_byte_exact`: re-sign the stripped envelope with RFC 8032 Test 1 key → matches `signature.hex` (64 bytes). PASS.
3. `vector_0_decodes_through_signed_envelope`: `SignedEnvelope::<AckBody>::decode` through full pipeline with Test 1 verifying key. PASS; body disposition = Accepted.
4. `any_signed_envelope_dispatches_vector_0_to_ack`: router returns `AnySignedEnvelope::Ack`. PASS.
5. `any_signed_envelope_rejects_delegate_class`: unknown class short-circuits with `UnknownClass { delegate }` before signature verification. PASS.

**Byte-exact reproducibility confirmed end-to-end.**

### Test Suite Summary

`cargo nextest run -p famp-envelope`: **73 tests passed, 0 skipped, 0 failed** (0.191s).

Coverage:
- smoke (5): primitive type round-trips
- errors (5): EnvelopeDecodeError → ProtocolError mapping (exhaustive, no `Other`)
- body_shapes (19): all bodies, ENV-09/ENV-12 narrowings, deny_unknown_fields at depth
- roundtrip_signed (7): per-class sign/decode byte-stable
- adversarial (11): full D-D4 matrix with distinct typed errors
- prop_roundtrip (10): proptest sign/decode/tamper for all five classes
- vector_zero (5): §7.1c regression
- compile_fail_unsigned (1): INV-10 type-state marker

### Requirements Coverage

Plan frontmatter requirement IDs:
- 01-01: ENV-01, ENV-02, ENV-15
- 01-02: ENV-06, ENV-07, ENV-09, ENV-12, ENV-14
- 01-03: ENV-01, ENV-03, ENV-10, ENV-14, ENV-15

Union: **ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09, ENV-10, ENV-12, ENV-14, ENV-15** (10 IDs).
ROADMAP Phase 1 requirements line: **ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed), ENV-10, ENV-12 (cancel-only), ENV-14, ENV-15** (10 IDs). **Match — no orphans, no gaps.**

| Requirement | Source Plan | Status | Evidence |
|-------------|-------------|--------|----------|
| ENV-01 (typed Envelope) | 01-01, 01-03 | SATISFIED | `UnsignedEnvelope<B>`/`SignedEnvelope<B>` type-state in `envelope.rs` (520 L); vector_zero decodes |
| ENV-02 (deny_unknown_fields everywhere) | 01-01 | SATISFIED | `adversarial::unknown_envelope_field_rejected`, `body_shapes::unknown_body_field_nested_rejected` pass |
| ENV-03 (INV-10 mandatory signature) | 01-03 | SATISFIED | `adversarial::missing_signature_rejected`, `compile_fail_unsigned`, error maps to `Unauthorized` |
| ENV-06 (ack body) | 01-02 | SATISFIED | `body/ack.rs`; `ack_body_all_dispositions_roundtrip_and_reject_unknown`, vector 0 is ack/accepted |
| ENV-07 (request body) | 01-02 | SATISFIED | `body/request.rs`; `request_body_roundtrip`, `request_body_missing_bounds_fails` |
| ENV-09 (narrowed — no capability_snapshot) | 01-02 | SATISFIED | `body/commit.rs` has no field; `commit_body_rejects_capability_snapshot` passes. **NOTE:** REQUIREMENTS.md checklist line 19 still marked `[ ]` — stale. |
| ENV-10 (deliver + terminal_status) | 01-03 | SATISFIED | `body/deliver.rs`; interim/terminal/failed tests pass; `deliver_interim_with_terminal_status_rejected` |
| ENV-12 (cancel-only) | 01-02 | SATISFIED | `ControlAction = {Cancel}` single variant; `control_supersede/close/revert_transfer/cancel_if_not_started_rejected` all pass. **NOTE:** REQUIREMENTS.md checklist line 21 still marked `[ ]` — stale. |
| ENV-14 (scope enforcement) | 01-02, 01-03 | SATISFIED | `BodySchema::SCOPE` const per body; `adversarial::class_body_mismatch_rejected` |
| ENV-15 (signed round-trip all classes) | 01-01, 01-03 | SATISFIED | `roundtrip_signed.rs` 7 tests pass (all 5 classes + 2 deliver variants) |

### Anti-Patterns Scan

| Pattern | Result |
|---------|--------|
| `serde(flatten)` | 0 matches in `crates/famp-envelope/src/` (plan mandate honored) |
| `serde(tag =` | 0 matches (manual dispatch in `dispatch.rs`) |
| `ProtocolErrorKind::Other` in error.rs | 0 matches (exhaustive explicit mapping) |
| `TODO`/`FIXME`/`PLACEHOLDER` in src | Not observed during code inspection (520+175+114 L inspected) |
| Timestamp normalization | `src/timestamp.rs` documents `PITFALL P6` and preserves bytes |

### Human Verification Required

None. Phase 1 is a pure library with deterministic byte-exact semantics. All success criteria are programmatically verifiable and pass.

## Gaps

None.

## Observations (Non-Blocking)

1. **Stale REQUIREMENTS.md checkboxes.** Lines 19 and 21 of `.planning/REQUIREMENTS.md` still show `[ ]` for ENV-09 (narrowed) and ENV-12 (cancel-only), and lines 111/113 of the mapping table show "Pending". Code, fixtures, and tests enforce both narrowings at the type level — these checkboxes should be flipped to `[x]` / "Complete". This does not block phase status because the actual code satisfies the success criteria; it is a documentation hygiene item.

## Summary

Phase 1 achieves its goal byte-for-byte. The §7.1c vector 0 is reproducible end-to-end (324 B canonical, 64 B Ed25519 signature), INV-10 is enforced at the type level, both ENV-09 and ENV-12 narrowings are unrepresentable, and all 10 declared requirements are satisfied by running tests (73/73 green). Phase is ready to proceed to Phase 2 (Minimal Task Lifecycle).

---

_Verified: 2026-04-13_
_Verifier: Claude (gsd-verifier)_
