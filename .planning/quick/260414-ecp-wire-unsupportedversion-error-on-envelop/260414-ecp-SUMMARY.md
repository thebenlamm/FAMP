---
phase: 260414-ecp-wire-unsupportedversion
plan: 01
subsystem: famp-envelope
type: quick
tags: [envelope, decode, error-kind, pr2, spec-fidelity]
requires:
  - famp-envelope::SignedEnvelope::decode_value
  - famp-envelope::EnvelopeDecodeError::UnsupportedVersion
  - famp-core::ProtocolErrorKind::Unsupported
provides:
  - decode-time rejection of tampered `famp` field as UnsupportedVersion
  - single version-rejection site on the decode path
affects:
  - famp-envelope (envelope.rs, wire.rs, version.rs, lib.rs, tests/smoke.rs, tests/adversarial.rs)
tech-stack:
  added: []
  patterns:
    - peek-before-typed-decode for targeted error kinds
    - collapse hand-written serde visitor when a single decode-path gate suffices
key-files:
  created: []
  modified:
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/wire.rs
    - crates/famp-envelope/src/version.rs
    - crates/famp-envelope/src/lib.rs
    - crates/famp-envelope/tests/adversarial.rs
    - crates/famp-envelope/tests/smoke.rs
decisions:
  - "WireEnvelope.famp is String, not FampVersion: single source of truth for version rejection lives in decode_value, not in a serde visitor."
  - "Version check runs AFTER signature verify, BEFORE typed decode: tampered-unsigned famp fails as Unauthorized (signature mismatch); tampered-and-re-signed famp fails as UnsupportedVersion. Both behaviors are observable to counterparties as the correct spec error kind."
  - "Delete FampVersion struct outright rather than keep it as a serialization-only ZST: zero consumers remain, and the ZST existed solely to carry the strict deserializer that is now moot."
metrics:
  duration: ~15min
  tasks: 2
  tests_added: 2
  tests_removed: 2
  tests_total_workspace: 257
  commits: 2
  completed: 2026-04-14
---

# Quick Task 260414-ecp: Wire UnsupportedVersion error on envelope decode — Summary

One-liner: **A tampered `famp` field now decodes to `EnvelopeDecodeError::UnsupportedVersion` → `ProtocolErrorKind::Unsupported` via a single pre-typed-decode peek in `SignedEnvelope::decode_value`, and the dead `FampVersion` ZST / hand-written Deserialize visitor is gone.**

## What shipped

Closes PR #2 of the codebase-review action plan: the previously-defined-but-unreached `EnvelopeDecodeError::UnsupportedVersion` variant is now the observable decode result when a federation counterparty sends an envelope with `famp != "0.5.1"`. Before this change, such an envelope was rejected via the hand-written `FampVersion` serde visitor, which surfaced as `BodyValidation(String)` → `ProtocolErrorKind::Malformed` — incorrect under spec §Δ01 / §19, which specify `unsupported_version` as the error kind.

## Task-by-task

### Task 1 (RED) — `test(envelope): add failing adversarial tests for tampered famp version (PR #2 RED)`
**Commit:** `fec0f9f`

Appended two tests to `crates/famp-envelope/tests/adversarial.rs` under a new "version tampering (PR #2)" banner, reusing the existing `sk()` / `vk()` / `alice()` / `bob()` / `ts()` / `id()` / `two_key_bounds()` helpers. Both build a valid signed `RequestBody` envelope, mutate `obj["famp"] = "0.6.0"`, strip the old signature, re-sign via `famp_crypto::sign_value`, and re-insert the signature.

- `tampered_famp_version_rejected_typed` — decodes through `SignedEnvelope::<RequestBody>::decode`, asserts `UnsupportedVersion { found: "0.6.0" }`.
- `tampered_famp_version_rejected_any` — same bytes, decodes through `AnySignedEnvelope::decode`, asserts the same variant. Confirms both decode paths converge on a single rejection site.

Both tests failed under the pre-fix code with the expected diagnostic:
```
expected UnsupportedVersion { found: "0.6.0" }, got BodyValidation(
    "invalid value: string \"0.6.0\", expected the literal string \"0.5.1\"")
```
RED state committed in isolation so the red→green transition is visible in git history.

### Task 2 (GREEN) — `fix(envelope): wire UnsupportedVersion error for tampered famp field (PR #2 GREEN)`
**Commit:** `8d14341`

Six edits:

1. **`crates/famp-envelope/src/envelope.rs::decode_value`** — inserted a new Step 3 after `verify_value` and before `serde_json::from_value::<WireEnvelope<B>>`:
   ```rust
   let root_obj = value.as_object().ok_or_else(|| { .. })?;
   match root_obj.get("famp") {
       None => return Err(EnvelopeDecodeError::MissingField { field: "famp" }),
       Some(Value::String(s)) if s == FAMP_SPEC_VERSION => { /* ok */ }
       Some(Value::String(s)) => return Err(EnvelopeDecodeError::UnsupportedVersion { found: s.clone() }),
       Some(_) => return Err(EnvelopeDecodeError::BodyValidation("envelope.famp must be a string".into())),
   }
   ```
   Existing steps renumbered (old Step 3 → Step 4, Step 4 → Step 5, Step 5 → Step 6).

2. **`crates/famp-envelope/src/wire.rs`** — `WireEnvelope.famp` changed from `FampVersion` to `String`. This is the entire serde-level decoupling: the wire struct no longer carries a strict-visitor ZST.

3. **`crates/famp-envelope/src/envelope.rs`** — `UnsignedEnvelope.famp` changed to `String` (constructed via `FAMP_SPEC_VERSION.to_string()` in `UnsignedEnvelope::new`); `WireEnvelopeRef.famp` changed to `&'a str` (borrowed from `self.famp` / `self.inner.famp`).

4. **`crates/famp-envelope/src/version.rs`** — deleted the `FampVersion` unit struct, its `Serialize`, `Deserialize`, and `FampVersionVisitor` impls. Only the `pub const FAMP_SPEC_VERSION: &str = "0.5.1";` constant remains.

5. **`crates/famp-envelope/src/lib.rs`** — dropped `FampVersion` from the public re-export: `pub use version::FAMP_SPEC_VERSION;`.

6. **`crates/famp-envelope/tests/smoke.rs`** — deleted `version_literal_roundtrip` and `version_rejects_wrong_literal` (both targeted the deleted type). Removed the `FampVersion` import and the doc-comment reference. The two Task 1 adversarial tests cover the "wrong version rejected by decode path" property at a strictly higher level.

## Verification

| Gate | Result |
|---|---|
| `cargo test -p famp-envelope --test adversarial tampered_famp` | **2/2 passing** (was 0/2 at RED) |
| `cargo test -p famp-envelope` | all passing (tests/adversarial 14; envelope::tests 5; prop_roundtrip 10; roundtrip_signed 7; vector_zero 5; smoke 3 — smoke lost 2 version tests, adversarial gained 2, net ±0) |
| `cargo test -p famp-canonical --no-default-features` | green (PR #1 non-regression gate) |
| `cargo test --workspace` | **257/257 passing** |
| `cargo clippy -p famp-envelope --all-targets -- -D warnings` | clean |
| `grep -rn "FampVersion" crates/famp-envelope/` | **zero matches** |
| `crates/famp-envelope/tests/errors.rs::unsupported_version_maps_to_unsupported` | still passing (ProtocolError mapping unchanged; that was always the point) |

## Success criteria — all satisfied

- [x] Tampered envelope with `famp = "0.6.0"` (re-signed) decoded via `SignedEnvelope::<RequestBody>::decode` or `AnySignedEnvelope::decode` returns `EnvelopeDecodeError::UnsupportedVersion { found: "0.6.0" }` → `ProtocolErrorKind::Unsupported`.
- [x] `EnvelopeDecodeError::UnsupportedVersion` is no longer dead code.
- [x] Exactly ONE version-rejection site on the decode path (the new Step 3 in `decode_value`). No belt-and-suspenders.
- [x] No changes to `peek.rs`, HTTP middleware, canonicalization, or scope outside `famp-envelope`.
- [x] Workspace stays green, clippy clean, 195+ famp-envelope test baseline preserved.

## Deviations from Plan

None. Plan executed exactly as written. The borrow plan (drop NLL on the Step 1 `obj` binding before reborrowing `value.as_object()` in Step 3) worked as predicted — no lifetime contortion required.

## Scope guard audit (did not touch)

- `peek.rs` — intentionally untouched; `peek_sender` runs before `decode_value` and reads `from`, not `famp`. Out of scope.
- HTTP middleware (`famp-transport-http`) — intentionally untouched; the error-kind fix propagates automatically through `ProtocolError` conversion.
- Canonicalization (`famp-canonical`) — intentionally untouched; version-string handling is a decode-path concern, not a canonicalization concern.

## Known Stubs

None. No hardcoded empty values, no placeholder text, no unwired components.

## Self-Check: PASSED

- **FOUND:** `crates/famp-envelope/src/envelope.rs` (modified)
- **FOUND:** `crates/famp-envelope/src/wire.rs` (modified)
- **FOUND:** `crates/famp-envelope/src/version.rs` (modified)
- **FOUND:** `crates/famp-envelope/src/lib.rs` (modified)
- **FOUND:** `crates/famp-envelope/tests/adversarial.rs` (modified, +2 tests)
- **FOUND:** `crates/famp-envelope/tests/smoke.rs` (modified, -2 tests)
- **FOUND commit:** `fec0f9f` — test(envelope): add failing adversarial tests for tampered famp version (PR #2 RED)
- **FOUND commit:** `8d14341` — fix(envelope): wire UnsupportedVersion error for tampered famp field (PR #2 GREEN)
- **FOUND:** `grep -rn FampVersion crates/famp-envelope/` → zero matches
