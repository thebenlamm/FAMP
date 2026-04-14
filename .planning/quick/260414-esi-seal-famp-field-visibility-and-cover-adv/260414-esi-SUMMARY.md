---
phase: 260414-esi-seal-famp-field-visibility-and-cover-adv
plan: 01
type: quick
tags: [famp-envelope, adversarial-review, PR-2.1, encode-footgun, regression-pin]
requires:
  - PR #2 (260414-ecp wire UnsupportedVersion error)
provides:
  - encode-side famp version drift closed at compile time
  - decode-side famp edge cases pinned by regression tests
affects:
  - crates/famp-envelope/src/envelope.rs
  - crates/famp-envelope/src/wire.rs
  - crates/famp-envelope/tests/adversarial.rs
tech-stack:
  added: []
  patterns:
    - "compile_fail doctest as structural invariant gate"
    - "shared-helper adversarial tamper pattern (closure over Value map)"
key-files:
  created: []
  modified:
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/wire.rs
    - crates/famp-envelope/tests/adversarial.rs
decisions:
  - "No serde escape hatch needed: UnsignedEnvelope is hand-wired, not derived; all in-module readers"
  - "Fully private (no visibility keyword) for envelope.rs famp; pub(crate) for wire.rs famp (explicit for signal)"
  - "No famp() accessor added — grep confirmed zero external consumers"
metrics:
  duration: 15m
  completed: "2026-04-14"
  tests_added: 14
  doctests_added: 1
  workspace_tests_before: 257
  workspace_tests_after: 272
---

# Quick Task 260414-esi: Seal famp Field Visibility and Cover Adversarial Gaps — Summary

## One-Liner

Closed PR #2.1's HIGH findings: sealed `UnsignedEnvelope.famp` + `WireEnvelope.famp` visibility with a `compile_fail` doctest gate, and pinned all four `decode_value` famp branches with 14 regression tests (7 edge cases × typed + any decode paths).

## What Shipped

### Task 1 — Adversarial coverage pins (commit `2e9cf92`)

Added 14 regression-pin tests sharing a single `famp_tampered_bytes` helper in `crates/famp-envelope/tests/adversarial.rs`:

| # | Case                        | Mutation                                     | Expected variant                                        |
|---|----------------------------|----------------------------------------------|--------------------------------------------------------|
| 1 | famp missing               | `obj.remove("famp")`                         | `MissingField { field: "famp" }` (NOT `UnsupportedVersion`) |
| 2 | famp = number              | `insert(..., Value::from(42))`               | `BodyValidation("envelope.famp must be a string")`     |
| 3 | famp = null                | `insert(..., Value::Null)`                   | `BodyValidation("envelope.famp must be a string")`     |
| 4 | famp = array               | `insert(..., Value::Array(vec![]))`          | `BodyValidation("envelope.famp must be a string")`     |
| 5 | famp = ""                  | `insert(..., Value::String(""))`             | `UnsupportedVersion { found: "" }`                     |
| 6 | famp = " 0.5.1"            | leading whitespace                           | `UnsupportedVersion { found: " 0.5.1" }`               |
| 7 | famp = "0.5.1\n"           | trailing newline                             | `UnsupportedVersion { found: "0.5.1\n" }`              |

Each case runs against both `SignedEnvelope::<RequestBody>::decode` (typed) and `AnySignedEnvelope::decode` (any), giving 14 tests total. All green on arrival — these are regression pins, not red gaps. A future "trim/normalize" drive-by on the `decode_value` famp match (envelope.rs:252-263) would flip them red.

Test 1 includes an explicit negative assertion that missing famp must NOT be reported as `UnsupportedVersion`, per plan requirement.

### Task 2 — Visibility seal (commit `bf4c70a`)

Two surgical field-visibility changes and one structural invariant gate:

1. **`crates/famp-envelope/src/envelope.rs:72`** — `pub famp: String` → `famp: String` (fully private).
2. **`crates/famp-envelope/src/wire.rs:39`** — `pub famp: String` → `pub(crate) famp: String` (matches the `redundant_pub_crate` allow on the struct; effective visibility unchanged, explicit for intent).
3. **New `compile_fail` doctest on `UnsignedEnvelope`** — struct-literal construction with a drifted `famp` literal fails to compile (private-field access). Matches the existing PR #2 INV-10 doctest pattern.

## Deviations from Plan

**None** — plan executed exactly as written.

- 14/14 Task 1 tests green on arrival, as the plan predicted.
- One minor clippy `doc_markdown` fix on the helper doc comment (two bare identifiers `RequestBody`/`Value`) before committing. This is not a deviation — it's routine clippy-pedantic compliance during the normal verify step.

## Grep Result — External `.famp` Consumers

```
$ grep -rn '\.famp' crates/ | grep -v target
crates/famp-envelope/src/envelope.rs:153:  famp: &self.famp,          // sign() WireEnvelopeRef
crates/famp-envelope/src/envelope.rs:248:  // decode_value comment
crates/famp-envelope/src/envelope.rs:260:  "envelope.famp must be a string".into()
crates/famp-envelope/src/envelope.rs:290:  famp: wire.famp,           // decode_value reconstruction
crates/famp-envelope/src/envelope.rs:323:  famp: &self.inner.famp,    // encode() WireEnvelopeRef
crates/famp-envelope/src/error.rs:36:     #[error("envelope.famp = ...")]
```

**All five structural readers live inside `crates/famp-envelope/src/envelope.rs`** (same module as `UnsignedEnvelope`), so sealing the field to module-private visibility breaks nothing. **Zero external consumers — no `famp()` accessor added.**

## Zero-Serde-Escape-Hatch Confirmation

`UnsignedEnvelope` has **no `#[derive(Serialize, Deserialize)]`**. It is hand-wired:
- Serialize path: `sign()` at `envelope.rs:150-175` builds a private `WireEnvelopeRef<'a, B>` borrowing view (lines 152-166) and serializes that instead.
- Deserialize path: `decode_value()` at `envelope.rs:222-305` deserializes into the crate-private `WireEnvelope<B>` and then reconstructs `UnsignedEnvelope` field-by-field (lines 289-303) inside the same module.

Both paths are in-module, so private fields are directly accessible. No `#[serde(getter)]`, no newtype wrapper, no `impl Deserialize` dance needed. The plan's `interfaces` audit called this out; source re-inspection confirms it.

## peek.rs

**NOT touched.** The adversarial review flagged `peek.rs` as a potential concern, but `peek_sender` only reads the `from` field — every full decode routes through `decode_value`, which is where the four famp match arms live (and where the regression pins are anchored). False positive, excluded from `files_modified` in the plan, and left untouched.

## Verification Results

| Check                                                  | Result                       |
|--------------------------------------------------------|------------------------------|
| `cargo test -p famp-envelope --test adversarial`       | 28 passed (14 new)           |
| `cargo test -p famp-envelope`                          | green                        |
| `cargo test -p famp-envelope --doc`                    | 4 compile_fail gates pass (incl. the new UnsignedEnvelope gate) |
| `cargo test --workspace`                               | **272 passed** (= 257 prior + 14 adversarial + 1 new doctest) |
| `cargo clippy -p famp-envelope --all-targets -- -D warnings` | clean                  |
| `cargo clippy --workspace --all-targets -- -D warnings`      | clean                  |
| `grep -rn 'pub famp' crates/famp-envelope/src/`        | **zero matches**             |
| `grep -rn 'famp:.*String' crates/famp-envelope/src/`   | exactly 2 (envelope.rs:72 private, wire.rs:39 pub(crate)) |

## Test Count Delta

| Baseline       | After Task 1           | After Task 2              |
|----------------|-----------------------|---------------------------|
| 257            | 271 (+14 adversarial) | **272** (+1 compile_fail doctest) |

Workspace test count strictly monotonic, matches plan prediction exactly.

## Commit SHAs

| Task | SHA        | Subject                                                                                                                |
|------|------------|------------------------------------------------------------------------------------------------------------------------|
| 1    | `2e9cf92`  | `test(famp-envelope): pin famp-field decode_value edge cases (missing/non-string/empty/whitespace × typed+any) [PR #2.1]` |
| 2    | `bf4c70a`  | `fix(famp-envelope): seal famp field visibility to prevent encode-side version drift [PR #2.1]`                         |

## Success Criteria — All Met

- [x] Encode-side footgun closed at the type system (compile_fail doctest firing on private-field access)
- [x] Every decode_value famp branch pinned by a typed test on both decode paths (14/14)
- [x] No serde escape hatch, no accessor, no peek.rs changes, no collateral field touches
- [x] Full workspace `cargo test` + `cargo clippy` green (272 tests, zero warnings)
- [x] Two commits, test-before-seal ordering, conventional commits, `[PR #2.1]` tag in both subjects

## Self-Check: PASSED

Verified all artifacts exist:
- `crates/famp-envelope/tests/adversarial.rs` — FOUND (14 new tests, shared helper)
- `crates/famp-envelope/src/envelope.rs` — FOUND (famp now private, compile_fail doctest added)
- `crates/famp-envelope/src/wire.rs` — FOUND (famp now pub(crate))
- commit `2e9cf92` — FOUND in git log
- commit `bf4c70a` — FOUND in git log
