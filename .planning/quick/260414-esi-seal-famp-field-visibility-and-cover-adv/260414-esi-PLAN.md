---
phase: 260414-esi-seal-famp-field-visibility-and-cover-adv
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp-envelope/tests/adversarial.rs
  - crates/famp-envelope/src/envelope.rs
  - crates/famp-envelope/src/wire.rs
autonomous: true
requirements:
  - PR2.1-HIGH-1-encode-footgun
  - PR2.1-HIGH-2-adversarial-coverage-gaps
must_haves:
  truths:
    - "Struct-literal construction of UnsignedEnvelope with a drifted famp literal fails to compile"
    - "Every adversarial famp-tamper case (missing, non-string, empty, whitespace) is pinned by a typed test on both SignedEnvelope::<RequestBody>::decode and AnySignedEnvelope::decode"
    - "UnsignedEnvelope::new remains the only in-crate writer of FAMP_SPEC_VERSION into the famp field"
    - "Full workspace test + clippy is green after the seal"
  artifacts:
    - path: "crates/famp-envelope/tests/adversarial.rs"
      provides: "7 new tamper cases × 2 decode paths, sharing a single Value-mutation helper"
    - path: "crates/famp-envelope/src/envelope.rs"
      provides: "UnsignedEnvelope.famp sealed (private or pub(crate)); accessor only if a consumer needs it"
    - path: "crates/famp-envelope/src/wire.rs"
      provides: "WireEnvelope.famp tightened to pub(crate) (confirmed struct-level pub(crate))"
  key_links:
    - from: "crates/famp-envelope/tests/adversarial.rs"
      to: "crates/famp-envelope/src/envelope.rs::decode_value famp match arms (lines 252-263)"
      via: "mutate Value → sign_value → serialize → decode → assert variant"
      pattern: "UnsupportedVersion|MissingField|BodyValidation"
    - from: "crates/famp-envelope/src/envelope.rs UnsignedEnvelope { .. } struct-literal sites"
      to: "sealed famp field"
      via: "rustc visibility check"
      pattern: "private field"
---

<objective>
PR #2.1 — close the two HIGH adversarial-review findings from PR #2:

1. **Encode-side footgun:** `UnsignedEnvelope.famp` and `WireEnvelope.famp` are both `pub String`. A struct-literal `UnsignedEnvelope { famp: "0.5.1 ".into(), ... }` bypasses `UnsignedEnvelope::new()` and signs a garbage version, which the same process then self-rejects on round-trip decode. The prior `FampVersion` ZST caught this at compile time; PR #2 regressed it to a runtime error.
2. **Adversarial coverage gaps:** only the `"0.6.0"` tamper is tested. Missing/non-string/empty/whitespace famp cases and both decode paths are uncovered, so the strict `==` comparison has no regression pin.

Scope is **surgical**: two test additions + one visibility seal. Do not touch `peek.rs` (verified false positive — `peek_sender` only reads the `from` field; all full decodes route through `decode_value`).

Purpose: lock byte-exact version handling at both the encode AND decode boundary so canonicalization/signature verification can never disagree on what "0.5.1" means.

Output: sealed field visibility, expanded adversarial suite, green CI.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
</execution_context>

<context>
@.planning/STATE.md
@./CLAUDE.md
@.codebase-review/FINAL_REVIEW.md
@.planning/quick/260414-ecp-wire-unsupportedversion-error-on-envelop/260414-ecp-SUMMARY.md
@crates/famp-envelope/src/envelope.rs
@crates/famp-envelope/src/wire.rs
@crates/famp-envelope/src/lib.rs
@crates/famp-envelope/tests/adversarial.rs
@crates/famp-envelope/tests/roundtrip_signed.rs

<interfaces>
<!-- Confirmed from envelope.rs / wire.rs reads. Executor must not re-explore. -->

`UnsignedEnvelope<B: BodySchema>` at crates/famp-envelope/src/envelope.rs:45-59
```rust
pub struct UnsignedEnvelope<B: BodySchema> {
    pub famp: String,   // ← line 46: SEAL THIS
    pub id: MessageId,
    pub from: Principal,
    pub to: Principal,
    pub scope: EnvelopeScope,
    pub class: MessageClass,
    pub causality: Option<Causality>,
    pub authority: AuthorityScope,
    pub ts: Timestamp,
    pub terminal_status: Option<TerminalStatus>,
    pub idempotency_key: Option<String>,
    pub extensions: Option<BTreeMap<String, Value>>,
    pub body: B,
}
```
Note: `UnsignedEnvelope` is NOT serde-derived — it is translated to `WireEnvelopeRef` for serialize and reconstructed field-by-field from `WireEnvelope` on decode (envelope.rs:289-303). This means **sealing `famp` requires no serde escape hatch at all** — there is no `Deserialize` impl on `UnsignedEnvelope`, and serialize goes via the private `WireEnvelopeRef` (which reads `&self.famp` from inside the same module, so visibility is irrelevant). This is the happy path; no newtype needed.

`WireEnvelope<B>` at crates/famp-envelope/src/wire.rs:37-56
```rust
pub(crate) struct WireEnvelope<B: BodySchema> {
    pub famp: String,   // ← line 39: this field is `pub` inside a `pub(crate)` struct.
                         //   Effective visibility is already pub(crate). Tighten to `pub(crate)`
                         //   anyway for signal + to match `redundant_pub_crate` allow on line 37.
    ...
}
```

`decode_value` famp-check at envelope.rs:249-263 (DO NOT MODIFY — tests must pin CURRENT behavior):
```rust
let root_obj = value.as_object().ok_or_else(|| {
    EnvelopeDecodeError::BodyValidation("envelope root is not a JSON object".into())
})?;
match root_obj.get("famp") {
    None => return Err(EnvelopeDecodeError::MissingField { field: "famp" }),
    Some(Value::String(s)) if s == FAMP_SPEC_VERSION => { /* ok */ }
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

Expected assertion variants per case (pinned to the code above):
- `famp_missing` → `EnvelopeDecodeError::MissingField { field: "famp" }`
- `famp_non_string_number` / `_null` / `_array` → `EnvelopeDecodeError::BodyValidation(msg)` where `msg == "envelope.famp must be a string"`
- `famp_empty_string` → `EnvelopeDecodeError::UnsupportedVersion { found: "" }`
- `famp_leading_whitespace` → `UnsupportedVersion { found: " 0.5.1" }`
- `famp_trailing_newline` → `UnsupportedVersion { found: "0.5.1\n" }`

Existing `tampered_famp_version_bytes()` helper at tests/adversarial.rs:194 is the model. Signs a RequestBody envelope, parses to Value, strips signature, mutates famp, re-signs via `sign_value(&sk(), &value)`, re-inserts signature, returns bytes.
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Adversarial famp coverage (7 cases × 2 decode paths via shared helper)</name>
  <files>crates/famp-envelope/tests/adversarial.rs</files>
  <behavior>
    Each test below pins current behavior. All 7 cases × 2 paths are expected GREEN on arrival (regression pins, not red-gaps) — the PR #2 decode_value already handles all four branches. The value here is the pin: a future "trim" or "normalize" drive-by would flip these red.

    **Shared helper (add once):**
    ```rust
    /// Build a signed RequestBody envelope, parse, apply `mutate` to the root
    /// object's famp field (or let mutate do whatever), strip + re-sign, return bytes.
    /// Callers that want to DELETE the famp field should use mutate to `obj.remove("famp")`
    /// — accepting &mut Map<String, Value> gives full control.
    fn famp_tampered_bytes<F: FnOnce(&mut serde_json::Map<String, Value>)>(mutate: F) -> Vec<u8> {
        let body = RequestBody {
            scope: serde_json::json!({"task": "translate"}),
            bounds: two_key_bounds(),
            natural_language_summary: None,
        };
        let signed = UnsignedEnvelope::<RequestBody>::new(
            id(), alice(), bob(), AuthorityScope::Advisory, ts(), body,
        ).sign(&sk()).unwrap();
        let bytes = signed.encode().unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("signature");
        mutate(obj);
        let new_sig = sign_value(&sk(), &value).unwrap();
        value.as_object_mut().unwrap()
            .insert("signature".to_string(), Value::String(new_sig.to_b64url()));
        serde_json::to_vec(&value).unwrap()
    }
    ```

    **Cases (each with `_typed` and `_any` variant = 14 tests):**

    1. `famp_missing_rejected_typed` / `famp_missing_rejected_any`
       - mutate: `obj.remove("famp");`
       - assert: `matches!(err, EnvelopeDecodeError::MissingField { field } if field == "famp")`
       - negative assert: NOT `UnsupportedVersion` (per user requirement)

    2. `famp_non_string_number_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::from(42));`
       - assert: `matches!(err, EnvelopeDecodeError::BodyValidation(ref msg) if msg == "envelope.famp must be a string")`

    3. `famp_non_string_null_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::Null);`
       - assert: same `BodyValidation("envelope.famp must be a string")`

    4. `famp_non_string_array_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::Array(vec![]));`
       - assert: same `BodyValidation("envelope.famp must be a string")`

    5. `famp_empty_string_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::String(String::new()));`
       - assert: `matches!(err, EnvelopeDecodeError::UnsupportedVersion { ref found } if found.is_empty())`

    6. `famp_leading_whitespace_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::String(" 0.5.1".into()));`
       - assert: `UnsupportedVersion { found } if found == " 0.5.1"`

    7. `famp_trailing_newline_rejected_typed` / `_any`
       - mutate: `obj.insert("famp".into(), Value::String("0.5.1\n".into()));`
       - assert: `UnsupportedVersion { found } if found == "0.5.1\n"`

    Green-on-arrival expectation: all 14 tests pass against unmodified envelope.rs. Document this in a block comment above the new section: `// All cases below are REGRESSION PINS (green on arrival). They lock the four decode_value arms at envelope.rs:252-263 so a future "trim/normalize" drive-by cannot weaken version handling silently.`
  </behavior>
  <action>
    1. Open `crates/famp-envelope/tests/adversarial.rs`.
    2. Just below the existing `tampered_famp_version_rejected_any` test (around line 243), add a new section header comment `// ---------------- famp field edge cases (PR #2.1 — adversarial coverage gap) ----------------` and the regression-pin explanatory comment shown above.
    3. Add the `famp_tampered_bytes` helper exactly as shown in <behavior>.
    4. Add the 14 test functions. Keep bodies tight — 4 lines each (build bytes, decode, unwrap_err, assert). DO NOT duplicate the helper body per test. DO NOT use rstest/macros — a plain helper closure is clearer for 14 cases and the reviewer asked for readable failure messages.
    5. For each `_any` variant, use `AnySignedEnvelope::decode(&bytes, &vk())` (already imported at line 34). For each `_typed`, use `SignedEnvelope::<RequestBody>::decode(&bytes, &vk())`.
    6. Every assert is the `assert!(matches!(err, ...), "expected ..., got {err:?}")` shape used by the existing tests (see lines 229-232 and 239-242). Match that style exactly.
    7. Run `cargo test -p famp-envelope --test adversarial` and confirm all 14 new tests pass green-on-arrival. If any fail, STOP — the code does not match the documented behavior in this plan's <interfaces> block, and Task 2 must not proceed until the drift is understood.
    8. Run `cargo clippy -p famp-envelope --tests -- -D warnings`.
    9. Commit: `test(famp-envelope): pin famp-field decode_value edge cases (missing/non-string/empty/whitespace × typed+any) [PR #2.1]`
  </action>
  <verify>
    <automated>cargo nextest run -p famp-envelope --test adversarial 2>&amp;1 | tail -40</automated>
  </verify>
  <done>
    - 14 new tests added under a clearly-marked section header in adversarial.rs
    - Single shared `famp_tampered_bytes` helper, no per-test signing boilerplate
    - All 14 tests pass on first run (regression pins, not red gaps)
    - No clippy warnings
    - Commit landed
    - Existing `tampered_famp_version_rejected_typed`/`_any` tests still pass untouched
  </done>
</task>

<task type="auto">
  <name>Task 2: Seal famp field visibility on UnsignedEnvelope and WireEnvelope</name>
  <files>
    crates/famp-envelope/src/envelope.rs,
    crates/famp-envelope/src/wire.rs
  </files>
  <action>
    **Scope:** Close the HIGH encode-side footgun. Two surgical field-visibility changes plus a compile-fail doctest.

    **Pre-flight check (MUST run first):**
    ```
    grep -rn '\.famp' crates/ examples/ 2>/dev/null | grep -v target
    ```
    Document in the commit message whether any consumer outside `crates/famp-envelope/src/envelope.rs` reads `.famp` on an `UnsignedEnvelope` or `SignedEnvelope`. Expected result based on the interfaces audit: no external readers exist (SignedEnvelope exposes accessors like `body()`, `from_principal()`, `class()`, but no `famp()` accessor because there's no need — callers trust the type-state). If grep confirms no external readers, **do NOT add a `famp()` accessor**. Only add one if the grep turns up a real consumer.

    **Serde check (already resolved from the interfaces audit, re-confirm in source):**
    `UnsignedEnvelope` has NO `#[derive(Serialize, Deserialize)]` — confirm by inspecting envelope.rs:43-45. It is hand-wired: `sign()` builds a `WireEnvelopeRef` borrowing projection (envelope.rs:152-166), and `decode_value` reconstructs field-by-field from `WireEnvelope` (envelope.rs:289-303). Both code paths live in the same module as `UnsignedEnvelope`, so private fields are directly readable. **No serde escape hatch is needed; option (a)/(b)/(c) from the prompt are unnecessary.** Write this finding into the commit message.

    **Change 1 — envelope.rs line 46:**
    Change `pub famp: String,` → `pub(super) famp: String,` (or just `famp: String,` — private to the module). Prefer fully private (`famp: String,` with no visibility keyword) because `WireEnvelopeRef` and `decode_value` are both in the same module.

    Same pass: also check if any OTHER `pub` fields on `UnsignedEnvelope` can be tightened while we're here. **Do not.** This PR is surgical to `famp`. If you find yourself touching other fields, back out.

    **Change 2 — wire.rs line 39:**
    Change `pub famp: String,` → `pub(crate) famp: String,`. The enclosing struct is already `pub(crate)` so the effective visibility is unchanged, but the explicit `pub(crate)` matches the `#[allow(clippy::redundant_pub_crate)]` already on line 37 and signals intent. Leave the other fields alone — same surgical rule.

    **Change 3 — compile_fail doctest on UnsignedEnvelope:**
    Model on the existing INV-10 compile_fail gates at envelope.rs:68-91. Add a third gate to the `/// # INV-10 ...` doc block on `SignedEnvelope` OR (preferred) add a new doc block on `UnsignedEnvelope` itself at line 38. Place it below the existing `///` docs on `UnsignedEnvelope`:

    ```rust
    /// # Version-drift compile_fail gate (PR #2.1 HIGH-1)
    ///
    /// ```compile_fail
    /// use famp_envelope::UnsignedEnvelope;
    /// use famp_envelope::body::AckBody;
    /// // Must fail: `famp` is a private field. The only way to get a valid
    /// // version literal into an UnsignedEnvelope is UnsignedEnvelope::new(),
    /// // which writes FAMP_SPEC_VERSION. Struct-literal construction with a
    /// // drifted version string is unrepresentable.
    /// let _: UnsignedEnvelope<AckBody> = UnsignedEnvelope {
    ///     famp: "0.5.1 ".to_string(),
    ///     id: unimplemented!(),
    ///     from: unimplemented!(),
    ///     to: unimplemented!(),
    ///     scope: unimplemented!(),
    ///     class: unimplemented!(),
    ///     causality: None,
    ///     authority: unimplemented!(),
    ///     ts: unimplemented!(),
    ///     terminal_status: None,
    ///     idempotency_key: None,
    ///     extensions: None,
    ///     body: unimplemented!(),
    /// };
    /// ```
    ```

    This uses `compile_fail` which runs under `cargo test --doc` and asserts the snippet fails to compile for the stated reason (private field access). Matches the PR #2 INV-10 pattern exactly.

    **Change 4 — verify construction sites still work:**
    - `UnsignedEnvelope::new` (envelope.rs:102) writes `famp: FAMP_SPEC_VERSION.to_string()` inside the same module → still legal.
    - `decode_value` (envelope.rs:289) builds `UnsignedEnvelope { famp: wire.famp, ... }` inside the same module → still legal.
    - `sign()` at line 153 reads `&self.famp` inside the same module → still legal.
    - `encode()` at line 323 reads `&self.inner.famp` inside the same module → still legal.
    No other construction sites exist in-crate (confirmed by the interfaces audit).

    **Verification sequence:**
    1. `cargo build -p famp-envelope` — must compile.
    2. `cargo test -p famp-envelope` — full crate green, including the new 14 adversarial tests from Task 1.
    3. `cargo test -p famp-envelope --doc` — the compile_fail gate must pass (i.e. rustc correctly rejects the snippet).
    4. `cargo test --workspace` — 257+ tests, no regressions, count strictly higher than pre-PR baseline (+14 from Task 1).
    5. `cargo clippy -p famp-envelope --all-targets -- -D warnings` — clean.
    6. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
    7. `grep -rn 'famp:.*String' crates/famp-envelope/src/` — expected outcome: exactly two matches, `envelope.rs:46` (now private) and `wire.rs:39` (now `pub(crate)`). No `pub String` on either line. `WireEnvelopeRef.famp: &'a str` at line 183 is a `&str` (not `String`) and has default (module-private) visibility — leave it.

    **Commit:** `fix(famp-envelope): seal famp field visibility to prevent encode-side version drift [PR #2.1]`

    Commit body should reference the PR #2 regression, note that no serde escape hatch was needed because `UnsignedEnvelope` is hand-wired (not derived), and note the grep result for external `.famp` consumers (expected: none).
  </action>
  <verify>
    <automated>cargo nextest run -p famp-envelope &amp;&amp; cargo test -p famp-envelope --doc &amp;&amp; cargo clippy -p famp-envelope --all-targets -- -D warnings &amp;&amp; cargo nextest run --workspace &amp;&amp; cargo clippy --workspace --all-targets -- -D warnings</automated>
  </verify>
  <done>
    - `crates/famp-envelope/src/envelope.rs:46`: `famp: String` is private (no `pub`)
    - `crates/famp-envelope/src/wire.rs:39`: `famp: String` is `pub(crate)` (explicit)
    - A `compile_fail` doctest on `UnsignedEnvelope` pins the struct-literal footgun
    - `cargo test --doc` passes (the compile_fail snippet compiled-fails as required)
    - No `famp()` accessor added (unless grep found an external consumer, which is expected not to)
    - `cargo test -p famp-envelope` green
    - `cargo test --workspace` green with test count ≥ baseline + 14
    - Both clippy invocations clean
    - grep confirms no remaining `pub famp: String` in `crates/famp-envelope/src/`
    - Commit landed with rationale for zero-serde-escape-hatch
  </done>
</task>

</tasks>

<verification>
- All 14 new adversarial tests pass (Task 1, regression pins).
- Existing `tampered_famp_version_rejected_typed`/`_any` still pass untouched.
- Compile_fail doctest on `UnsignedEnvelope` asserts struct-literal footgun is closed at compile time.
- Workspace test count strictly higher than PR #2 baseline (+14 adversarial + 1 doctest).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `grep -rn 'pub famp' crates/famp-envelope/src/` returns zero matches.
- `peek.rs` untouched (false positive — not in files_modified).
</verification>

<success_criteria>
1. Encode-side footgun closed at the type system: `UnsignedEnvelope { famp: "bad".into(), ... }` fails to compile, verified by a `compile_fail` doctest run under `cargo test --doc`.
2. Decode-side behavior pinned: every branch of the `decode_value` famp match (missing / non-string / empty / whitespace / wrong-version) has a typed test on both `SignedEnvelope::<RequestBody>::decode` and `AnySignedEnvelope::decode`.
3. Zero collateral damage: no serde escape hatch needed, no accessor added, no peek.rs changes, no touched fields beyond `famp` on the two targeted structs.
4. Full workspace `cargo test` + `cargo clippy` green.
5. Two commits total (test commit before seal commit), conventional-commit format, PR #2.1 tag in the subject line.
</success_criteria>

<output>
After completion, create `.planning/quick/260414-esi-seal-famp-field-visibility-and-cover-adv/260414-esi-SUMMARY.md` documenting:
- Final test count delta (+14 adversarial + 1 doctest, confirmed by workspace run)
- Grep result for external `.famp` consumers (expected: none; if any, document the accessor added)
- Confirmation that no serde escape hatch was needed (UnsignedEnvelope is hand-wired, not derived)
- The two commit SHAs
- Explicit note that peek.rs was NOT touched (false-positive from adversarial review)
</output>
