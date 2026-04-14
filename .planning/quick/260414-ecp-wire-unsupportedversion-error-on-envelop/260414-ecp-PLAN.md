---
phase: 260414-ecp-wire-unsupportedversion
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp-envelope/tests/adversarial.rs
  - crates/famp-envelope/src/envelope.rs
  - crates/famp-envelope/src/version.rs
  - crates/famp-envelope/src/wire.rs
  - crates/famp-envelope/src/lib.rs
  - crates/famp-envelope/tests/smoke.rs
autonomous: true
requirements:
  - PR2-UNSUPPORTED-VERSION
must_haves:
  truths:
    - "A tampered envelope with famp != \"0.5.1\" is rejected by SignedEnvelope::decode with EnvelopeDecodeError::UnsupportedVersion { found } (NOT Malformed / BodyValidation)."
    - "The same tampered envelope decoded through AnySignedEnvelope::decode surfaces the same typed UnsupportedVersion variant."
    - "The error maps to ProtocolErrorKind::Unsupported (already covered by tests/errors.rs — must continue to pass)."
    - "There is exactly ONE famp version-rejection site in the decode path (no belt-and-suspenders)."
    - "All 195+ famp-envelope tests still pass; workspace stays green; clippy -D warnings clean."
  artifacts:
    - path: "crates/famp-envelope/tests/adversarial.rs"
      provides: "New adversarial tests for tampered famp field on both SignedEnvelope<RequestBody>::decode and AnySignedEnvelope::decode"
      contains: "UnsupportedVersion"
    - path: "crates/famp-envelope/src/envelope.rs"
      provides: "decode_value pre-typed-decode check on famp field producing UnsupportedVersion / MissingField"
    - path: "crates/famp-envelope/src/wire.rs"
      provides: "WireEnvelope.famp changed from FampVersion to String (single source of truth for version rejection lives in decode_value)"
  key_links:
    - from: "decode_value (envelope.rs)"
      to: "EnvelopeDecodeError::UnsupportedVersion"
      via: "value.get(\"famp\") peek BEFORE serde_json::from_value::<WireEnvelope<B>>"
      pattern: "UnsupportedVersion \\{ found"
    - from: "adversarial.rs new test"
      to: "decode_value famp check"
      via: "re-signed tampered envelope → SignedEnvelope::<RequestBody>::decode"
      pattern: "assert.*UnsupportedVersion.*0\\.6\\.0"
---

<objective>
Wire the already-defined-but-dead `EnvelopeDecodeError::UnsupportedVersion` variant into the envelope decode path so spec §Δ01 / §19 is honored: a tampered envelope with `famp != "0.5.1"` produces `ProtocolErrorKind::Unsupported`, not `Malformed`.

Purpose: Close PR #2 of the codebase review action plan. Eliminate the false "dead variant" warning, and make the error kind observable to federation counterparties as `unsupported_version`.

Output: Failing adversarial test → targeted fix that collapses the existing hand-written `FampVersion` deserializer into a single `decode_value` check → smoke.rs retargeted → workspace green.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@crates/famp-envelope/src/envelope.rs
@crates/famp-envelope/src/version.rs
@crates/famp-envelope/src/wire.rs
@crates/famp-envelope/src/lib.rs
@crates/famp-envelope/src/error.rs
@crates/famp-envelope/tests/smoke.rs
@crates/famp-envelope/tests/errors.rs
@crates/famp-envelope/tests/adversarial.rs
@crates/famp-envelope/tests/roundtrip_signed.rs
@.codebase-review/FINAL_REVIEW.md
@.codebase-review/PROTOCOL_REPORT.md

<interfaces>
<!-- Executor must use these directly. No codebase re-exploration required. -->

From crates/famp-envelope/src/error.rs (EXISTS — do NOT redefine):
```rust
pub enum EnvelopeDecodeError {
    // ... other variants ...
    MissingField { field: &'static str },
    UnsupportedVersion { found: String },
    MalformedJson(#[from] CanonicalError),
    BodyValidation(String),
    // ...
}

// Already wired (tests/errors.rs locks this):
impl From<EnvelopeDecodeError> for ProtocolError {
    // UnsupportedVersion { .. } => ProtocolErrorKind::Unsupported
}
```

From crates/famp-envelope/src/version.rs:
```rust
pub const FAMP_SPEC_VERSION: &str = "0.5.1"; // KEEP — adversarial test uses it
pub struct FampVersion;                       // unit struct, currently with strict hand-written Deserialize
```

From crates/famp-envelope/src/wire.rs:
```rust
pub(crate) struct WireEnvelope<B: BodySchema> {
    pub famp: FampVersion,   // TO CHANGE → String (or keep as ZST that serializes "0.5.1" and use String on the deserialize struct — executor picks the clean cut; see Task 2)
    // ... rest unchanged
}
```

From crates/famp-envelope/src/envelope.rs (edit site — decode_value, lines 222-247):
```rust
pub(crate) fn decode_value(mut value: Value, verifier: &TrustedVerifyingKey)
    -> Result<Self, EnvelopeDecodeError>
{
    let obj = value.as_object_mut().ok_or_else(|| /* ... */)?;

    // Step 1: strip signature  — UNCHANGED
    // Step 2: verify_value      — UNCHANGED (must run BEFORE famp check? See Task 2 note)
    // Step 3 (NEW PRE-CHECK, inserted here): peek obj.get("famp") → UnsupportedVersion / MissingField
    // Step 4: serde_json::from_value::<WireEnvelope<B>>(value)
    // ...
}
```

From crates/famp-envelope/tests/roundtrip_signed.rs (signing helper pattern to copy):
```rust
// RFC 8032 Test 1 keypair → sk()/vk()
// UnsignedEnvelope::<RequestBody>::new(id, alice, bob, Advisory, ts, body).sign(&sk).encode()
```

From famp-crypto (already imported in adversarial.rs):
```rust
use famp_crypto::sign_value;  // sign_value(&sk, &Value) -> Result<FampSignature, _>
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: RED — adversarial test(s) for tampered famp field</name>
  <files>crates/famp-envelope/tests/adversarial.rs</files>
  <behavior>
    - Test `tampered_famp_version_rejected_typed`: build a valid signed `RequestBody` envelope (reuse the same helpers — `sk()`, `vk()`, `alice()`, `bob()`, `ts()`, `id()`, `two_key_bounds()` — already in this file; pattern from `roundtrip_signed.rs::request_roundtrip`). `encode()`, then parse bytes back to `serde_json::Value`, mutate `obj["famp"] = "0.6.0"`, strip `signature`, re-sign the mutated `Value` with `sign_value(&sk(), &value)`, reinsert signature, `to_vec`. Call `SignedEnvelope::<RequestBody>::decode(&bytes, &vk())`. Assert `matches!(err, EnvelopeDecodeError::UnsupportedVersion { ref found } if found == "0.6.0")`.
    - Test `tampered_famp_version_rejected_any`: identical payload, but call `famp_envelope::AnySignedEnvelope::decode(&bytes, &vk())` instead (re-export via `famp_envelope::AnySignedEnvelope` is already public). Same `UnsupportedVersion { found: "0.6.0" }` assertion. Proves both decode paths converge on the same variant.
    - Both tests MUST fail under current code (current behavior: `serde_jcs/serde` rejects the hand-written visitor → `map_serde_error` → `BodyValidation(String)` → `ProtocolErrorKind::Malformed`). Expected red output: `expected UnsupportedVersion, got BodyValidation(...)`.
  </behavior>
  <action>
    Append the two tests to `crates/famp-envelope/tests/adversarial.rs` under a new `// ---------------- version tampering (PR #2 — UnsupportedVersion wiring) ----------------` banner. Reuse the existing module-top `sk()`, `vk()`, `alice()`, `bob()`, `ts()`, `id()`, `two_key_bounds()` helpers — do NOT duplicate key bytes. Import `RequestBody` (already imported), `sign_value` (add `use famp_crypto::sign_value;` — already listed in the crate's deps; the file currently imports `FampSigningKey, TrustedVerifyingKey` from famp_crypto, so extend that line). Signature re-signing MUST strip the old signature first, inject `"famp": "0.6.0"`, then `sign_value` over the stripped-but-tampered Value, then re-insert the new base64url signature string under `"signature"`. Run `cargo test -p famp-envelope --test adversarial tampered_famp` and CONFIRM RED before handoff. Commit with message `test(envelope): add failing adversarial tests for tampered famp version (PR #2 RED)` on its own so the red→green transition is visible in git history.
  </action>
  <verify>
    <automated>cargo test -p famp-envelope --test adversarial tampered_famp 2>&1 | tee /tmp/pr2-red.log; grep -q "FAILED" /tmp/pr2-red.log &amp;&amp; grep -qE "BodyValidation|Malformed" /tmp/pr2-red.log</automated>
  </verify>
  <done>Two new tests exist in adversarial.rs, both fail with the current code reporting a non-`UnsupportedVersion` variant (BodyValidation expected), and the failing state is committed in isolation.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: GREEN — wire UnsupportedVersion into decode_value + collapse FampVersion</name>
  <files>crates/famp-envelope/src/envelope.rs, crates/famp-envelope/src/wire.rs, crates/famp-envelope/src/version.rs, crates/famp-envelope/src/lib.rs, crates/famp-envelope/tests/smoke.rs</files>
  <behavior>
    - After this task: the two Task 1 tests pass GREEN. All other envelope tests (roundtrip_signed, errors, adversarial, smoke, envelope::tests) still pass. `cargo test --workspace` green. `cargo clippy -p famp-envelope --all-targets -- -D warnings` clean.
    - Exactly ONE version-rejection site lives in the decode path: the new pre-typed-decode check inside `decode_value`. The hand-written strict `Deserialize for FampVersion` visitor is gone.
    - Version check fires AFTER signature verify (Step 2) but BEFORE typed `from_value::<WireEnvelope<B>>` (Step 4). Rationale: spec says a tampered-unsigned famp field is still `unauthorized` (signature fails first); a tampered-and-re-signed famp field is `unsupported_version`. The Task 1 tests re-sign, so verify passes and the famp check fires next — this is the observable spec behavior.
  </behavior>
  <action>
    **Step A — envelope.rs::decode_value (insert new Step 3, renumber existing Step 3→4 etc.):** After `verify_value(...)?` succeeds and BEFORE `serde_json::from_value::<WireEnvelope<B>>`, re-borrow the object and peek the `famp` field:
    ```rust
    // Step 3: reject wrong spec-version BEFORE typed decode so the error kind
    // is `Unsupported` (spec §Δ01 / §19) — not `Malformed` via serde error.
    let obj = value
        .as_object()
        .ok_or_else(|| EnvelopeDecodeError::BodyValidation("envelope root is not a JSON object".into()))?;
    match obj.get("famp") {
        None => return Err(EnvelopeDecodeError::MissingField { field: "famp" }),
        Some(Value::String(s)) if s == crate::FAMP_SPEC_VERSION => { /* ok */ }
        Some(Value::String(s)) => {
            return Err(EnvelopeDecodeError::UnsupportedVersion { found: s.clone() });
        }
        Some(_) => {
            return Err(EnvelopeDecodeError::BodyValidation(
                "envelope.famp must be a string".into(),
            ));
        }
    }
    ```
    Note: at this point `value` is still a mutable root (signature was stripped in Step 1 and verified in Step 2). Because we only need a read-borrow here, prefer `as_object()` (not `as_object_mut()`) to avoid exclusive-borrow hassles with the existing Step 1 `obj` binding — let the earlier binding drop at end of Step 1.

    **Step B — wire.rs: change `WireEnvelope.famp` to `String`.** Replace `pub famp: FampVersion,` with `pub famp: String,`. This removes the serde-level strict check; the only version gate is now the one in decode_value. Remove the `use crate::{..., FampVersion, ...};` import and keep the others. `UnsignedEnvelope.famp` and `WireEnvelopeRef.famp` in envelope.rs must also flip to `String` OR to a typed `&'static str`: **recommended** — collapse to `String` on `UnsignedEnvelope` and `WireEnvelopeRef`, constructed via `FAMP_SPEC_VERSION.to_string()` inside `UnsignedEnvelope::new`. This is ONE string allocation per constructed envelope — negligible.

    **Step C — version.rs: simplify.** Delete `FampVersion` struct entirely along with its `Serialize`, `Deserialize`, and `FampVersionVisitor` impls. Keep only:
    ```rust
    //! The single spec-version string FAMP v0.5.1 uses on the wire.
    pub const FAMP_SPEC_VERSION: &str = "0.5.1";
    ```
    (Rationale: zero consumers remain once wire/envelope flip to `String`; the ZST existed solely to carry the strict deserializer, which is now moot.)

    **Step D — lib.rs: drop `FampVersion` from re-exports.** Change `pub use version::{FampVersion, FAMP_SPEC_VERSION};` to `pub use version::FAMP_SPEC_VERSION;`. Verify no other crate in the workspace imports `famp_envelope::FampVersion` — search `crates/ -name "*.rs" -exec grep -l FampVersion {} \;` and remove any stragglers (should be zero outside famp-envelope itself).

    **Step E — envelope.rs: update UnsignedEnvelope / WireEnvelopeRef.** Change `pub famp: FampVersion` → `pub famp: String`. Change `UnsignedEnvelope::new` to populate `famp: FAMP_SPEC_VERSION.to_string()`. Change `WireEnvelopeRef::famp` to `&'a str` (borrow from `self.inner.famp` in `encode()` / `self.famp` in `sign()`). Remove `crate::FampVersion` from the top-of-file `use crate::{...}` import and add `FAMP_SPEC_VERSION` if needed.

    **Step F — smoke.rs: retarget.** Delete tests `version_literal_roundtrip` (lines 18-24) and `version_rejects_wrong_literal` (lines 26-31) — both target the deleted `FampVersion` type. Remove the `FampVersion` import from the `use famp_envelope::{...}` line. The Task 1 adversarial tests already cover the "wrong version rejected by decode path" property at a strictly higher level; no replacement needed in smoke.rs.

    **Step G — grep self-check.** Run `grep -rn "FampVersion" crates/famp-envelope/` — expected output: **zero matches**. If any remain, hunt them down. `FAMP_SPEC_VERSION` references are fine and expected.

    Commit with message `fix(envelope): wire UnsupportedVersion error for tampered famp field (PR #2 GREEN)` after all tests pass.
  </action>
  <verify>
    <automated>cargo test -p famp-envelope --test adversarial tampered_famp &amp;&amp; cargo test -p famp-envelope &amp;&amp; cargo test -p famp-canonical --no-default-features &amp;&amp; cargo test --workspace &amp;&amp; cargo clippy -p famp-envelope --all-targets -- -D warnings &amp;&amp; test -z "$(grep -rn FampVersion crates/famp-envelope/ || true)"</automated>
  </verify>
  <done>
    Both Task 1 tests pass GREEN. `cargo test -p famp-envelope` reports all tests passing (no regression from the 195+ baseline). `cargo test --workspace` green. `cargo clippy -p famp-envelope --all-targets -- -D warnings` clean. `grep -rn FampVersion crates/famp-envelope/` returns zero matches. `crates/famp-envelope/tests/errors.rs::unsupported_version_maps_to_unsupported` still passes (it's a unit-level mapping test — unchanged by this work, which is the point).
  </done>
</task>

</tasks>

<verification>
- `cargo test -p famp-envelope` — all tests pass, including the two new adversarial tests and the retained `tests/errors.rs::unsupported_version_maps_to_unsupported`.
- `cargo test -p famp-canonical --no-default-features` — PR #1 stays green (independent, but spec'd as a non-regression gate).
- `cargo test --workspace` — full workspace green.
- `cargo clippy -p famp-envelope --all-targets -- -D warnings` — clean.
- `grep -rn "FampVersion" crates/famp-envelope/src/` — **zero matches** (type fully deleted).
- Git history shows RED commit (Task 1) distinct from GREEN commit (Task 2).
</verification>

<success_criteria>
- A tampered envelope with `famp = "0.6.0"` (re-signed so signature verify passes) decoded via `SignedEnvelope::<RequestBody>::decode` or `AnySignedEnvelope::decode` returns `EnvelopeDecodeError::UnsupportedVersion { found: "0.6.0" }`, which maps to `ProtocolErrorKind::Unsupported`.
- `EnvelopeDecodeError::UnsupportedVersion` is no longer dead code — `cargo build -p famp-envelope` with `#![deny(dead_code)]` (if we ever enable it) would not flag it.
- There is exactly ONE version-rejection site in the decode path. No belt-and-suspenders.
- No changes to peek.rs, no changes to HTTP middleware, no changes to canonicalization, no scope creep.
</success_criteria>

<output>
After completion, update the quick task row in `.planning/STATE.md` under "Quick Tasks Completed" with the new entry for `260414-ecp`, and append an audit note to `.codebase-review/FINAL_REVIEW.md` recording that PR #2 shipped with the corrected scope: PROTOCOL specialist's security framing was a false positive (rejection already existed via hand-written visitor), but the error-kind claim was correct and is now fixed.
</output>
