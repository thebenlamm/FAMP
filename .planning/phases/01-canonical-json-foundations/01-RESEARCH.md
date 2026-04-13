# Phase 1: Canonical JSON Foundations - Research

**Researched:** 2026-04-12
**Domain:** RFC 8785 JSON Canonicalization Scheme (JCS), `serde_jcs`, Rust serde ecosystem
**Confidence:** MEDIUM (primary dependency `serde_jcs 0.2.0` has one open conformance issue; see SEED-001 Decision Framework)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01/D-02/D-03:** Ship BOTH free function and blanket-impl trait; free function is primary. Exact signatures locked (see CONTEXT.md §D-02).
- **D-04/D-05/D-06/D-07:** Duplicate-key rejection is a separate public API surface on the parse path (`from_slice_strict`, `from_str_strict`). Serde-visitor approach is primary; pre-scan tokenizer is documented fallback only.
- **D-08/D-09/D-10/D-11:** Fallback plan written first (before running RFC 8785 gate). No parallel implementation. Fixed task order: define API → write fallback.md → wire tests → run gate → record SEED-001 decision.
- **D-12/D-13/D-14/D-15:** Float corpus: deterministic sampled subset on every PR; full 100M nightly + release. Full 100M is a required release gate, not per-PR. Sample size at Claude's Discretion (~100K–1M, tune to GHA budget).
- **D-16/D-17/D-18:** `CanonicalError` narrow: `Serialize`, `InvalidJson`, `DuplicateKey`, `NonFiniteNumber`, `InternalCanonicalization`. `thiserror` only — no `anyhow::Result` in public API.
- **D-19/D-20/D-21/D-22:** Artifact-ID helpers (`artifact_id_for_canonical_bytes`, `artifact_id_for_value`) live in `famp-canonical`. `ArtifactIdString` is a placeholder wrapper (String/Cow) for now; strong type comes in Phase 3. `sha2` dep lives in `famp-canonical`.

### Claude's Discretion
- Exact float corpus sample size (start ~100K–1M, tune to GHA budget)
- Internal module layout inside `famp-canonical`
- Exact wording of `CanonicalError` `Display` messages
- Whether `ArtifactIdString` is a type alias or thin newtype
- Test fixture filenames and directory layout under `famp-canonical/tests/vectors/`

### Deferred Ideas (OUT OF SCOPE)
- Strong `ArtifactId` type — Phase 3 (`famp-core`)
- Pre-scan tokenizer fallback for duplicate-key rejection — only if serde-visitor path proves infeasible
- Parallel fallback crate implementation — explicitly rejected as premature
- Full 100M float corpus on every PR
- 15-category protocol error enum integration — Phase 3
- Domain-separation prefix logic — Phase 2 (`famp-crypto`)
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CANON-01 | `famp-canonical` crate wraps `serde_jcs` behind a stable `Canonicalize` trait | API shape locked in CONTEXT.md; research confirms `serde_jcs::to_vec` is the delegation target |
| CANON-02 | RFC 8785 Appendix B test vectors pass as hard CI gate | Test vectors in cyberphone repo + serde_jcs tests cover all Appendix B float cases; Appendix C/E fixtures also available |
| CANON-03 | cyberphone 100M-sample float corpus integrated as CI check | Corpus at `releases/download/es6testfile/es6testfile100m.txt.gz`; SHA-256 checksums documented; deterministic local generation via `numgen.go`/`numgen.js` |
| CANON-04 | UTF-16 key sort verified on supplementary-plane characters | serde_jcs 0.2.0 PR #4 (merged 2026-03-25) fixed UTF-16 vs UTF-8 sort bug; `weird.json` fixture covers surrogate-pair emoji keys; supplementary-plane fixtures must be authored and committed |
| CANON-05 | ECMAScript number formatting verified against cyberphone reference | `serde_jcs` delegates to `ryu-js 0.2` (boa-dev); all integers converted to f64 before ryu_js; open issue #3 was about this but research shows the conversion is now correct — verify at runtime |
| CANON-06 | Duplicate-key rejection on parse | No built-in in `serde_json`; Deserializer adapter pattern required; `serde_with::maps_duplicate_key_is_error` solves for typed structs; for `Value` path, custom Deserializer wrapper is needed |
| CANON-07 | Documented from-scratch fallback plan (~500 LoC) if `serde_jcs` fails conformance | Research establishes what the ~500 LoC plan must cover: RFC 8785 §3.2.3 UTF-16 key sort, §3.2.2.3 ECMAScript number format via `ryu-js` directly, UTF-8 pass-through; `serde_json_canonicalizer 0.3.2` is proven-viable alternative if fork is too costly |
| SPEC-02 | Canonical JSON serialization locked to RFC 8785 JCS | Confirmed: RFC 8785 at https://datatracker.ietf.org/doc/html/rfc8785 is normative reference |
| SPEC-18 | Artifact identifier scheme locked (`sha256:<hex>`) | Confirmed in FAMP-v0.5.1-spec.md §3.6a: `sha256:` + 64 lowercase hex chars, SHA-256 over canonical JSON of artifact body |
</phase_requirements>

---

## Summary

Phase 1 ships `famp-canonical`, the leaf crate that produces byte-exact RFC 8785 JCS output. Everything downstream — Ed25519 signing in Phase 2, artifact IDs in Phase 3, provenance graphs in later milestones — depends on this crate being correct to the byte. A single divergence invalidates every downstream signature.

The good news: `serde_jcs 0.2.0` (published 2026-03-25) landed with a critical bugfix — PR #4 fixed UTF-16 vs UTF-8 key sorting, the primary historical conformance failure. The crate correctly converts all integer types to f64 via `ryu-js 0.2` before formatting. It runs RFC 8785 Appendix B, C, and E vectors and the cyberphone `testdata` suite in its own tests. This is a real positive signal.

The risk: Issue #3 (integer encoding, opened 2021) remains open but review of the 0.2.0 source confirms the integer-to-f64 conversion is in place. The "unstable" label reflects API churn, not known correctness failures. However, zero external parties have published conformance results against `serde_jcs 0.2.0` specifically. The Phase 1 CI gate IS the conformance proof. If it passes, SEED-001 is `keep serde_jcs`. If it fails on any Appendix B/C/E vector or the float corpus sample, SEED-001 becomes `fork to famp-canonical`. A proven fallback exists: `serde_json_canonicalizer 0.3.2` (1.1M downloads, maintained, uses `ryu-js 1.0.1`) can be vendored or forked as the fallback implementation at lower cost than a from-scratch port.

The duplicate-key rejection requirement (CANON-06) has no built-in solution in `serde_json`. A custom Deserializer wrapper is required. The serde maintainers explicitly recommend this pattern. It is ~100 LoC of straightforward serde plumbing — not glamorous, but well-understood.

**Primary recommendation:** Use `serde_jcs 0.2.0` as the canonical engine. Write the fallback plan document first. Run the RFC 8785 gate. Record SEED-001 with evidence. If the gate fails, replace `serde_jcs` with a fork of `serde_json_canonicalizer 0.3.2` rather than building from scratch.

---

## Standard Stack

### Core (Phase 1 — `famp-canonical`)

| Library | Version | Purpose | Why This |
|---------|---------|---------|----------|
| `serde_jcs` | `0.2.0` | RFC 8785 JCS canonicalization | Only serde-integrated RFC 8785 impl; PR #4 fixed UTF-16 sort; Appendix B vectors green in its own test suite |
| `serde` | `1.0.228` (workspace) | Serialize/Deserialize derive | Required by serde_jcs; workspace-pinned |
| `serde_json` | `1.0.149` (workspace) | JSON parse + serde_jcs backing store | serde_jcs is built on serde_json; must stay aligned |
| `sha2` | `0.11.0` (workspace) | SHA-256 for artifact IDs | RustCrypto; same org as ed25519-dalek; `sha2::Sha256::digest()` |
| `thiserror` | `2.0.18` (workspace) | Error derive for `CanonicalError` | Library-only (never `anyhow`) per project convention |

### Feature flags to pin in `famp-canonical/Cargo.toml`

```toml
[dependencies]
serde_jcs = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true, features = ["std"] }
thiserror = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }  # for test helpers
```

Note: `serde_jcs 0.2.0` requires `serde_json` with `features = ["std", "float_roundtrip"]`. The workspace pin (`serde_json = { version = "1.0.149", default-features = false, features = ["std"] }`) is MISSING `float_roundtrip`. This must be added to the workspace `[workspace.dependencies]` block:
```toml
serde_json = { version = "1.0.149", default-features = false, features = ["std", "float_roundtrip"] }
```
Without `float_roundtrip`, round-trip precision for floats is not guaranteed, which could produce non-canonical output. **This is a concrete pre-task action.**

### Fallback Alternative (if SEED-001 gate fails)

| Library | Version | Why |
|---------|---------|-----|
| `serde_json_canonicalizer` | `0.3.2` | 1.1M downloads, RFC-100%-compatible claim, uses `ryu-js 1.0.1`, maintained by evik42; fork cost < from-scratch port |

### Version Verification (live-checked 2026-04-12)

```bash
# Verify before writing any Cargo.toml entries:
cargo search serde_jcs
cargo search serde_json_canonicalizer
cargo search sha2
```

All versions above were confirmed against crates.io API on 2026-04-12.

---

## Architecture Patterns

### Crate Layout

```
crates/famp-canonical/
├── Cargo.toml
├── docs/
│   └── fallback.md              # ~500 LoC fallback plan (written BEFORE gate runs)
├── src/
│   ├── lib.rs                   # Public re-exports: canonicalize, Canonicalize, CanonicalError,
│   │                            #   from_slice_strict, from_str_strict, artifact_id_*
│   ├── error.rs                 # CanonicalError (thiserror)
│   ├── canonical.rs             # canonicalize() free fn + Canonicalize blanket trait
│   ├── strict_parse.rs          # from_slice_strict / from_str_strict + DuplicateKeyDetector
│   └── artifact_id.rs           # artifact_id_for_canonical_bytes, artifact_id_for_value, ArtifactIdString
└── tests/
    ├── conformance.rs           # Appendix B (float IEEE hex), Appendix C, Appendix E, cyberphone testdata
    ├── float_corpus.rs          # Sampled (PR) and full (nightly) float corpus driver
    ├── utf16_supplementary.rs   # Supplementary-plane key sort fixtures (emoji, CJK Ext B)
    ├── duplicate_keys.rs        # Duplicate-key rejection on parse
    └── vectors/
        ├── input/               # cyberphone testdata: arrays, french, structures, unicode, values, weird
        ├── output/              # corresponding expected outputs
        └── supplementary/       # emoji + CJK Ext B input/output fixtures (authored in this phase)
```

### Pattern 1: Canonicalize Trait + Free Function

The `Canonicalize` trait is a blanket impl — it delegates to the free function. Callers use the free function when writing generic utility code; the trait provides ergonomic call-site syntax.

```rust
// src/canonical.rs
// Source: CONTEXT.md D-02

use serde::Serialize;
use crate::error::CanonicalError;

/// Serialize `value` to RFC 8785 JCS canonical JSON bytes.
pub fn canonicalize<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    serde_jcs::to_vec(value).map_err(CanonicalError::from_serde)
}

pub trait Canonicalize: Serialize {
    fn canonicalize(&self) -> Result<Vec<u8>, CanonicalError> {
        crate::canonicalize(self)
    }
}

impl<T: Serialize + ?Sized> Canonicalize for T {}
```

### Pattern 2: CanonicalError (thiserror)

```rust
// src/error.rs
// Source: CONTEXT.md D-17

#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("invalid JSON input: {0}")]
    InvalidJson(serde_json::Error),

    #[error("duplicate key in JSON object: {key:?}")]
    DuplicateKey { key: String },

    #[error("non-finite number (NaN or Infinity) not permitted by RFC 8785")]
    NonFiniteNumber,

    #[error("internal canonicalization error: {0}")]
    InternalCanonicalization(String),
}
```

Note: The `#[from]` on `Serialize` creates ambiguity because `InvalidJson` also takes `serde_json::Error`. Remove `#[from]` and add an explicit `from_serde` helper to distinguish serialization vs parse failures.

### Pattern 3: Strict Parse (Duplicate-Key Rejection)

No built-in exists in `serde_json`. The serde project recommends a Deserializer adapter wrapping `serde_json::Deserializer`. The visitor intercepts map entries and errors on duplicate keys.

```rust
// src/strict_parse.rs — structure only; implementation is ~100 LoC
// Source: Serde documentation + maintainer guidance (github.com/serde-rs/json/issues/1112)

use serde::de::DeserializeOwned;
use crate::error::CanonicalError;

/// Parse JSON bytes rejecting any input with duplicate object keys.
/// This is the sanctioned ingress path for inbound signed JSON.
pub fn from_slice_strict<T: DeserializeOwned>(input: &[u8]) -> Result<T, CanonicalError> {
    let mut de = serde_json::Deserializer::from_slice(input);
    let detector = DuplicateKeyDetector::new(&mut de);
    T::deserialize(detector).map_err(...)
}

pub fn from_str_strict<T: DeserializeOwned>(input: &str) -> Result<T, CanonicalError> {
    let mut de = serde_json::Deserializer::from_str(input);
    let detector = DuplicateKeyDetector::new(&mut de);
    T::deserialize(detector).map_err(...)
}

/// Deserializer adapter that errors on duplicate object keys.
/// Wraps serde_json::Deserializer and intercepts map-entry visiting.
struct DuplicateKeyDetector<'de, R> {
    inner: &'de mut serde_json::Deserializer<R>,
}
// DuplicateKeyDetector implements serde::Deserializer by delegating all methods
// to inner, except deserialize_map/deserialize_struct where it wraps the
// MapAccess visitor to track seen keys in a HashSet and error on duplicates.
```

**Implementation guidance:** The core of `DuplicateKeyDetector` is a custom `MapAccess` impl that:
1. Holds a `HashSet<String>` of keys seen so far
2. On each `next_key_seed` call, deserializes the key, checks membership
3. If already in set: returns `Err(de::Error::custom(format!("duplicate key: {key}")))`
4. If new: inserts and proceeds

This is ~80-100 lines. No unsafe code required.

### Pattern 4: Artifact ID Helper

```rust
// src/artifact_id.rs
// Source: FAMP-v0.5.1-spec.md §3.6a

use sha2::{Sha256, Digest};
use serde::Serialize;
use crate::{canonicalize, CanonicalError};

/// Newtype wrapper (placeholder until famp-core Phase 3 provides strong ArtifactId).
/// Wraps `String` for now; refactors to strong type in Phase 3.
pub struct ArtifactIdString(pub String);

impl ArtifactIdString {
    pub fn as_str(&self) -> &str { &self.0 }
}

impl std::fmt::Display for ArtifactIdString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Compute `sha256:<hex>` over canonical JSON bytes.
/// Input must already be the canonical form (output of `canonicalize()`).
pub fn artifact_id_for_canonical_bytes(bytes: &[u8]) -> ArtifactIdString {
    let hash = Sha256::digest(bytes);
    ArtifactIdString(format!("sha256:{}", hex::encode(hash)))
    // NOTE: use sha2's hex output or format!("{:x}", hash) — no `hex` dep needed
    // sha2::digest::Output<Sha256> implements LowerHex via digest::generic_array
}

/// Canonicalize `value`, then compute its artifact ID.
pub fn artifact_id_for_value<T: Serialize + ?Sized>(
    value: &T,
) -> Result<ArtifactIdString, CanonicalError> {
    let bytes = canonicalize(value)?;
    Ok(artifact_id_for_canonical_bytes(&bytes))
}
```

**Hex formatting note:** `sha2` returns `Output<Sha256>` (a `GenericArray<u8, U32>`). Format via `{:x}` or iterate bytes with `format!("{:02x}", b)`. Do NOT add a `hex` crate dep — use `format!("sha256:{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())` or write a tiny inline helper. The `sha2` + `digest` crates already provide `Digest::finalize()` returning a byte array.

### Pattern 5: RFC 8785 Conformance Test Loop

```rust
// tests/conformance.rs — CI gate (CANON-02)
// Source: cyberphone/json-canonicalization testdata + serde_jcs own vectors

#[test]
fn appendix_b_float_vectors() {
    // IEEE 754 hex -> expected string pairs from RFC 8785 Appendix B
    let cases: &[(u64, &str)] = &[
        (0x0000_0000_0000_0000, "0"),
        (0x8000_0000_0000_0000, "0"),
        (0x0000_0000_0000_0001, "5e-324"),
        // ... (27 cases total, taken verbatim from serde_jcs/tests/basic.rs)
    ];
    for (bits, expected) in cases {
        let f = f64::from_bits(*bits);
        let bytes = famp_canonical::canonicalize(&f).unwrap();
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), *expected);
    }
}

#[test]
fn cyberphone_weird_fixture() {
    let input: serde_json::Value =
        serde_json::from_str(include_str!("vectors/input/weird.json")).unwrap();
    let got = famp_canonical::canonicalize(&input).unwrap();
    let expected = include_bytes!("vectors/output/weird.json");
    assert_eq!(&got, expected);
}
```

### Anti-Patterns to Avoid

- **Using `serde_json::to_vec` directly** — does NOT sort keys or format numbers per RFC 8785. Everything must go through `famp_canonical::canonicalize`.
- **Using `serde_json` with `arbitrary_precision` feature** — changes number internal representation, breaks JCS canonicalization. The workspace pin must NOT include this feature.
- **Using `serde_json` with `preserve_order` feature** — canonical output requires order-independent key handling. Forbidden.
- **Parsing inbound signed bytes with `serde_json::from_slice`** — use `from_slice_strict` for any bytes that carry a signature. Silent duplicate-key merge would allow two structurally-different messages to hash to the same canonical form.
- **Formatting SHA-256 as uppercase hex** — spec §3.6a mandates 64 **lowercase** hex characters. One uppercase letter produces a different artifact ID.
- **Computing artifact ID over raw input bytes** — spec §3.6a mandates SHA-256 over **canonical JSON** of the artifact body, not raw input. This catches canonicalization divergence between implementations.

---

## SEED-001 Decision Framework

**Decision question:** Keep `serde_jcs 0.2.0` as the canonical engine, or fork to an in-house `famp-canonical` implementation?

### Evidence in Favor of Keeping `serde_jcs 0.2.0` (HIGH weight)

1. **UTF-16 sort bug fixed.** Issue #1 (the primary historical conformance failure) was fixed in PR #4, merged 2026-03-25 — the same day as the 0.2.0 release. The fix uses `key.encode_utf16().collect::<Vec<u16>>()` for sort keys, which is the correct RFC 8785 §3.2.3 approach.
2. **Integer encoding via f64 conversion is in place.** Source inspection confirms all integer types (u8..u128, i8..i128) are converted to f64 before `ryu_js::Buffer::format_finite()`. Issue #3 (opened 2021, never fixed) described a problem that is now structurally handled. **Verify at runtime** with the cyberphone test vectors.
3. **Uses `ryu-js 0.2` (boa-dev).** The `ryu-js` crate (`boa-dev/ryu-js`) is the de-facto ECMAScript number-to-string implementation in the Rust ecosystem, used by the Boa JavaScript engine. Version 0.2.x is stable.
4. **Runs the cyberphone `testdata` suite in its own tests.** All 6 fixture pairs (arrays, french, structures, unicode, values, weird) are tested, including the emoji/control-char edge cases in `weird.json`.
5. **Recent release.** 0.2.0 published 2026-03-25 with active maintenance signals (edition 2024, Rust 1.85 MSRV, strict lints).

### Evidence of Risk (MEDIUM weight — requires CI validation)

1. **"Unstable" self-label.** API may change between patch versions. Mitigated by the `famp-canonical` wrapper trait — callers never import `serde_jcs` directly.
2. **Issue #3 still open.** Even though source inspection suggests the issue is resolved by the f64-conversion pattern, the issue was never explicitly closed with a reference to the fix. Treat as unresolved until CI gate proves otherwise.
3. **No external conformance report.** Zero published results from third parties running RFC 8785 vectors against `serde_jcs 0.2.0`. The Phase 1 CI gate IS the first public conformance proof.
4. **Supplementary-plane key sort not explicitly tested by `serde_jcs`.** The `weird.json` fixture covers emoji via escape sequences (`\ud83d\ude02`). Emoji as raw UTF-8 in keys (e.g., a key containing 🎉 literal) must be verified by FAMP's own supplementary-plane fixtures.

### Decision Gate Criteria

| Gate | Pass Condition | Fail Condition |
|------|---------------|----------------|
| RFC 8785 Appendix B (27 float vectors) | All 27 pass | Any failure → investigate serde_jcs issue #3 |
| RFC 8785 Appendix C (structured object) | Byte-exact match | Any mismatch |
| RFC 8785 Appendix E (complex object) | Byte-exact match | Any mismatch |
| cyberphone testdata (6 fixture pairs) | All 6 byte-exact | Any mismatch |
| Sampled float corpus (100K lines, fixed seed) | 100% match | Any formatter divergence |
| Supplementary-plane fixtures (authored in Phase 1) | Expected key order | Wrong order |

**If all pass:** Record SEED-001 as `keep serde_jcs`. Wrap and ship.
**If any fail:** Switch to `serde_json_canonicalizer 0.3.2` as the engine (fork/vendor). Do NOT build from scratch — `serde_json_canonicalizer` uses `ryu-js 1.0.1`, claims 100% RFC compatibility, and has 1.1M downloads. The 500-LoC from-scratch fallback plan (in `fallback.md`) is insurance, not the first fork target.

### Fallback Plan Outline (for `famp-canonical/docs/fallback.md`)

The written plan must cover these sections:

1. **Why from-scratch** — conditions under which `serde_json_canonicalizer` also fails (e.g., same ryu-js issue affects both)
2. **RFC 8785 §3.2.2.3 Number Formatting** — use `ryu-js` crate directly (`boa-dev/ryu-js`); call `Buffer::format_finite(f64_value)`; reject non-finite via `.is_finite()` check
3. **RFC 8785 §3.2.3 Key Sorting** — traverse `serde_json::Value::Object` entries; sort by `key.encode_utf16().collect::<Vec<u16>>()` comparison; reconstruct in sorted order
4. **RFC 8785 §3.2.1 UTF-8 String Pass-Through** — no normalization; output raw UTF-8; escape only `"` and `\` and control chars `\u0000`–`\u001F`
5. **Integer handling** — all integers cast to f64 via `as f64`; values > 2^53 lose precision (matches spec)
6. **Recursive structure traversal** — handle Object, Array, String, Number, Bool, Null
7. **~500 LoC estimate** — number formatter (~50 LoC), key sorter (~80 LoC), recursive serializer (~200 LoC), writer infrastructure (~100 LoC), tests (~100 LoC)
8. **Comparison point** — diff against cyberphone Java reference output for same inputs

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ECMAScript Number.toString formatting | Custom float→string algorithm | `ryu-js 0.2` (via `serde_jcs`) | RFC 8785 §3.2.2.3 mandates ECMAScript semantics; Ryu-js is validated against 100M test vectors; getting this wrong poisons every signature |
| UTF-16 key comparison | Custom string comparator | `serde_jcs`'s Utf16Key sort (fixed in 0.2.0) | Supplementary-plane comparison has surrogate-pair ordering subtleties; wrong sort = different canonical bytes = signature failure |
| SHA-256 | Custom hash | `sha2 0.11.0` | RustCrypto; same org as ed25519-dalek; already in workspace deps |
| Hex encoding of SHA-256 output | Custom hex formatter | `format!("{:02x}", byte)` per-byte (no crate needed) | `sha2::Output<Sha256>` is a byte array; format inline, avoid adding `hex` crate dep for 64 bytes |
| JSON duplicate-key detection | Pre-scan tokenizer | Deserializer adapter wrapping `serde_json::Deserializer` | Two parser surfaces diverge; the adapter approach is what serde maintainers recommend |

**Key insight:** The number formatting problem (ECMAScript §7.1.12.1 "Note 2" Grisu3/Dragon4 algorithm) is notoriously hard to get right from scratch. Even the cyberphone reference repo needed a 100M-sample test corpus to validate their own implementation. Never hand-roll this.

---

## Common Pitfalls

### Pitfall 1: `serde_json` Feature Flag `float_roundtrip` Absent

**What goes wrong:** `serde_json` without `float_roundtrip` does not guarantee that parsing then serializing a float produces byte-identical output. Canonical JSON requires exactly this guarantee.

**Why it happens:** The workspace `Cargo.toml` currently has `features = ["std"]` for `serde_json`. The `float_roundtrip` feature is not included.

**How to avoid:** Add `"float_roundtrip"` to the `serde_json` features in `[workspace.dependencies]` before writing any `famp-canonical` code.

**Warning signs:** A float like `1.0000000000000002` round-trips as `1` (loses the ULP) in a test that should fail.

---

### Pitfall 2: Artifact ID Computed Over Raw Input Bytes

**What goes wrong:** Computing `sha256:<hex>` over the raw JSON string produces a different ID than computing it over the canonical form. Two conformant implementations with identical data would produce different artifact IDs.

**Why it happens:** Spec §3.6a says "SHA-256 computed over the **canonical JSON** of the artifact body." Easy to misread as "SHA-256 over the JSON."

**How to avoid:** Always call `canonicalize(value)` first, then hash the resulting `Vec<u8>`. The helper `artifact_id_for_value` enforces this order.

**Warning signs:** Two implementations that agree on canonicalization produce different artifact IDs for the same logical value.

---

### Pitfall 3: UTF-16 Supplementary Plane Keys Not Tested

**What goes wrong:** Keys containing emoji or CJK Extension B characters (code points U+10000 and above) are represented in UTF-16 as surrogate pairs (two 16-bit code units). Their sort position is determined by the surrogate pair, not the Unicode scalar. Wrong sort = wrong canonical bytes = signature failure.

**Why it happens:** Most test fixtures use ASCII or BMP characters. Supplementary plane keys are absent unless explicitly authored.

**How to avoid:** Author fixtures with raw emoji keys (e.g., `{"🎉": 1, "a": 2}`) and verify the sort order matches the cyberphone JavaScript reference implementation's output. Commit to `tests/vectors/supplementary/`.

**Warning signs:** `"🎉"` (U+1F389, surrogate pair `\uD83C\uDF89`) sorts AFTER `"a"` in correct UTF-16 order (0xD83C > 0x0061), but if the comparator uses UTF-8 bytes, `"a"` (0x61) would sort before `"🎉"` (0xF0 in UTF-8), which is wrong.

**Correction:** In RFC 8785, `"🎉"` (UTF-16: `[0xD83C, 0xDF89]`) sorts AFTER any key whose first UTF-16 code unit is ≤ 0xD83B. "a" is 0x0061, so "a" sorts before "🎉" in both orderings — but the sort comparison order for mixed keys can differ depending on the comparator.

---

### Pitfall 4: Silent Duplicate Key Merge on Signed Input

**What goes wrong:** `serde_json::from_slice` silently keeps the last value for duplicate keys. An adversary could submit `{"role":"admin","role":"user"}` and depending on the deserialization path, the parsed value might differ from what was signed.

**Why it happens:** This is serde_json's documented default behavior. It's easy to forget to use the strict path.

**How to avoid:** All ingress of externally-provided bytes MUST use `from_slice_strict` or `from_str_strict`. Document this prominently in the crate's `lib.rs` doc comment.

**Warning signs:** Tests for the strict path succeed but production code calls `serde_json::from_slice` directly.

---

### Pitfall 5: Using `serde_json::Value::Object` Default (Preserves Insertion Order, Not Sorted)

**What goes wrong:** If `serde_jcs::to_vec` is called on a `serde_json::Value` whose `Object` map was built in non-canonical order, sorting must happen during serialization. Fortunately `serde_jcs` does sort during serialization. However, if anyone calls `serde_json::to_vec` on the same value (e.g., in a test helper), they get non-canonical output and may not notice.

**How to avoid:** All canonicalization goes through `famp_canonical::canonicalize`. Grep for bare `serde_json::to_vec` and `serde_json::to_string` in `famp-canonical` and replace with the canonical path. Add a clippy lint or test assertion.

---

### Pitfall 6: NaN / Infinity in Input Struct

**What goes wrong:** RFC 8785 §3.2.2.2 forbids NaN and Infinity. If a Rust struct contains an f32 or f64 field with value `f64::NAN` or `f64::INFINITY`, `serde_jcs::to_vec` will return an error. Callers that unwrap will panic.

**How to avoid:** Validate struct fields before canonicalization. The `CanonicalError::NonFiniteNumber` variant propagates this error; callers must handle it.

**Warning signs:** `canonicalize(&value).unwrap()` in tests with synthetic struct data.

---

## Code Examples

### Running a Conformance Vector

```rust
// tests/conformance.rs
// Source: serde_jcs/tests/basic.rs pattern + RFC 8785 Appendix B

#[test]
fn rfc8785_appendix_b_all() {
    // (ieee_bits, expected_str) — 27 cases from RFC 8785 Appendix B
    let vectors: &[(u64, &str)] = &[
        (0x0000_0000_0000_0000, "0"),
        (0x8000_0000_0000_0000, "0"),
        (0x0000_0000_0000_0001, "5e-324"),
        (0x8000_0000_0000_0001, "-5e-324"),
        (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
        (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
        (0x4340_0000_0000_0000, "9007199254740992"),
        (0xc340_0000_0000_0000, "-9007199254740992"),
        (0x4430_0000_0000_0000, "295147905179352830000"),
        (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
        (0x44b5_2d02_c7e1_4af6, "1e+23"),
        (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
        (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
        (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
        (0x444b_1ae4_d6e2_ef50, "1e+21"),
        (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
        (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
        (0x41b3_de43_5555_5553, "333333333.3333332"),
        (0x41b3_de43_5555_5554, "333333333.33333325"),
        (0x41b3_de43_5555_5555, "333333333.3333333"),
        (0x41b3_de43_5555_5556, "333333333.3333334"),
        (0x41b3_de43_5555_5557, "333333333.33333343"),
        (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
    ];
    for &(bits, expected) in vectors {
        let f = f64::from_bits(bits);
        let got = famp_canonical::canonicalize(&f).expect("should not fail for finite float");
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            expected,
            "Failed for IEEE bits 0x{bits:016x}"
        );
    }
}

#[test]
fn rfc8785_appendix_b_nan_rejected() {
    let result = famp_canonical::canonicalize(&f64::NAN);
    assert!(result.is_err(), "NaN must be rejected");
}
```

### Float Corpus Sampled Driver

```rust
// tests/float_corpus.rs
// Source: cyberphone/json-canonicalization testdata/README.md

// Full corpus URL:
// https://github.com/cyberphone/json-canonicalization/releases/download/es6testfile/es6testfile100m.txt.gz
// SHA-256 (100M lines): 0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272

// PR budget: run first N lines where N is chosen to stay < 30s on GHA.
// Recommended starting point: N = 100_000 (4MB uncompressed, ~5s on CI).
// Nightly/release: run all 100M.

const SAMPLE_SIZE_PR: usize = 100_000;
const SAMPLE_SEED: &str = "famp-canonical-float-corpus-v1"; // committed seed — do not change

#[test]
fn float_corpus_sampled() {
    // Generation: use numgen.go or numgen.js locally to produce the corpus file.
    // In CI: generate on the fly OR download and cache the gz file.
    // Each line: "<hex-ieee>,<expected>\n"
    // Parse hex as u64, build f64, canonicalize, compare to expected string.
    //
    // Determinism: corpus is deterministically generated (SHA-256 seeded),
    // so any N lines from the start form a stable subset.
    run_corpus_lines(SAMPLE_SIZE_PR);
}

#[cfg(feature = "full-corpus")]  // feature gate for nightly/release
#[test]
fn float_corpus_full() {
    run_corpus_lines(100_000_000);
}
```

### Duplicate Key Rejection Test

```rust
// tests/duplicate_keys.rs

#[test]
fn duplicate_key_is_error() {
    let input = r#"{"a":1,"b":2,"a":3}"#;
    let result = famp_canonical::from_str_strict::<serde_json::Value>(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        famp_canonical::CanonicalError::DuplicateKey { key } => {
            assert_eq!(key, "a");
        }
        other => panic!("expected DuplicateKey, got {:?}", other),
    }
}

#[test]
fn non_duplicate_is_ok() {
    let input = r#"{"a":1,"b":2,"c":3}"#;
    let result = famp_canonical::from_str_strict::<serde_json::Value>(input);
    assert!(result.is_ok());
}
```

### SHA-256 Artifact ID (Inline, No Hex Crate)

```rust
// src/artifact_id.rs
use sha2::{Sha256, Digest};

fn bytes_to_sha256_hex(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for b in hash.iter() {
        use std::fmt::Write as _;
        write!(hex, "{b:02x}").expect("writing to String never fails");
    }
    hex
}

pub fn artifact_id_for_canonical_bytes(bytes: &[u8]) -> ArtifactIdString {
    ArtifactIdString(format!("sha256:{}", bytes_to_sha256_hex(bytes)))
}
```

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo-nextest 0.9.132` |
| Config file | `.config/nextest.toml` or `Cargo.toml [profile.test]` (nextest discovers automatically) |
| Quick run command | `cargo nextest run -p famp-canonical` |
| Full suite command | `cargo nextest run -p famp-canonical --features full-corpus` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| CANON-01 | `canonicalize()` and `Canonicalize` trait compile and delegate correctly | unit | `cargo nextest run -p famp-canonical canonical` | ❌ Wave 0 |
| CANON-02 | RFC 8785 Appendix B/C/E vectors pass | unit | `cargo nextest run -p famp-canonical conformance` | ❌ Wave 0 |
| CANON-03 | 100K-sample float corpus passes | unit | `cargo nextest run -p famp-canonical float_corpus` | ❌ Wave 0 |
| CANON-04 | Supplementary-plane key sort correct | unit | `cargo nextest run -p famp-canonical utf16_supplementary` | ❌ Wave 0 |
| CANON-05 | ECMAScript number formatting matches cyberphone | unit (covered by CANON-02/03) | see CANON-02/03 | ❌ Wave 0 |
| CANON-06 | Duplicate keys rejected at parse time | unit | `cargo nextest run -p famp-canonical duplicate_keys` | ❌ Wave 0 |
| CANON-07 | Fallback plan exists on disk | manual | check `famp-canonical/docs/fallback.md` exists and is non-empty | ❌ Wave 0 |
| SPEC-02 | Spec text references RFC 8785 explicitly | docs/manual | grep in FAMP-v0.5.1-spec.md §4a | ✅ (already in spec) |
| SPEC-18 | `sha256:<hex>` format correct (lowercase, 64 chars) | unit | `cargo nextest run -p famp-canonical artifact_id` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo nextest run -p famp-canonical`
- **Per wave merge:** `cargo nextest run -p famp-canonical` (full suite minus 100M corpus)
- **Phase gate:** Full suite + `--features full-corpus` green before `/gsd:verify-work`

### Wave 0 Gaps (must exist before implementation begins)

- [ ] `crates/famp-canonical/src/lib.rs` — with module declarations
- [ ] `crates/famp-canonical/src/error.rs` — `CanonicalError` type
- [ ] `crates/famp-canonical/src/canonical.rs` — free function + trait
- [ ] `crates/famp-canonical/src/strict_parse.rs` — `DuplicateKeyDetector` adapter
- [ ] `crates/famp-canonical/src/artifact_id.rs` — helpers
- [ ] `crates/famp-canonical/tests/conformance.rs` — Appendix B/C/E gate
- [ ] `crates/famp-canonical/tests/float_corpus.rs` — corpus driver
- [ ] `crates/famp-canonical/tests/utf16_supplementary.rs` — supplementary fixtures
- [ ] `crates/famp-canonical/tests/duplicate_keys.rs` — duplicate key rejection
- [ ] `crates/famp-canonical/tests/vectors/` — cyberphone testdata copied in
- [ ] `crates/famp-canonical/docs/fallback.md` — from-scratch fallback plan
- [ ] `[workspace.dependencies]` — add `float_roundtrip` to `serde_json` features

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `serde_jcs` using `BTreeMap<Vec<u8>, _>` for key sort (UTF-8 bytes) | `serde_jcs 0.2.0` using `Vec<u16>` for UTF-16 sort | 2026-03-25 (PR #4) | Previous versions produced non-canonical output for non-ASCII keys |
| `serde_jcs` relied on `serde_json::CompactFormatter` directly | `serde_jcs 0.2.0` uses `JcsFormatter` wrapping `CompactFormatter` | 2026-03-25 (internal refactor) | Proper number formatting separation |
| `ryu-js 0.2.x` was the only option | `ryu-js 1.0.2` available (boa-dev) | 2025 | `serde_jcs` pins `0.2`; `serde_json_canonicalizer 0.3.2` uses `1.0.1`; both correct |

**Deprecated/outdated:**
- `serde_jcs` versions < 0.2.0: UTF-16 sort bug. Do not use.
- Any `canonical-json` or `olpc-cjson` crate: implements OLPC/Matrix canonical JSON (not RFC 8785). Will produce wrong signatures. Hard incompatibility.
- `serde_json` `arbitrary_precision` feature: breaks JCS number canonicalization. Forbidden.

---

## Open Questions

1. **Float corpus CI approach (local generation vs. download)**
   - What we know: cyberphone provides `numgen.go` and `numgen.js` for local deterministic generation; SHA-256 checksums allow verification at each size tier. Download URL exists for full 100M.
   - What's unclear: Which approach works better for GHA caching — generate in CI (compute cost) vs. download + cache (network cost + cache invalidation).
   - Recommendation: For the sampled (100K) subset, generate in a build script or test setup function using a Rust port of the numgen algorithm. This avoids network deps in CI. Document the generation algorithm in the test file header.

2. **`serde_jcs` issue #3 status in 0.2.0**
   - What we know: Source inspection confirms all integer types are converted to `f64` before `ryu_js::format_finite`. The issue described a "numbers should be encoded according to IEEE 754" problem that the f64 conversion addresses. Issue was not explicitly closed.
   - What's unclear: Whether the original reporter was satisfied; whether there are edge cases with very large `u128` values not caught by the f64 conversion.
   - Recommendation: Include `u64::MAX` (18446744073709551615) and `i64::MIN` in the conformance test suite. These round-trip as `1.8446744073709552e+19` and `-9.223372036854776e+18` in ECMAScript. Verify serde_jcs produces this output.

3. **Supplementary-plane fixture expected output**
   - What we know: RFC 8785 §3.2.3 mandates UTF-16 code unit comparison; emoji keys use surrogate pairs.
   - What's unclear: We need an oracle. The cyberphone JavaScript implementation (`json-canonicalization/node-es6/`) is the reference. The simplest oracle is to run the test JSON through the JS reference and record the expected output.
   - Recommendation: Before authoring supplementary-plane fixtures, run the JS reference (`node json-canon.js`) on the proposed inputs and commit the outputs as golden files.

---

## Sources

### Primary (HIGH confidence)
- RFC 8785 (IETF) — https://datatracker.ietf.org/doc/html/rfc8785 — §3.2.2.3, §3.2.3, §3.1
- `serde_jcs` source code (GitHub) — https://github.com/l1h3r/serde_jcs/blob/main/src/lib.rs — integer handling, UTF-16 sort, ryu_js usage
- `serde_jcs` Cargo.toml — https://github.com/l1h3r/serde_jcs/blob/main/Cargo.toml — exact deps
- `serde_jcs` tests/basic.rs — https://github.com/l1h3r/serde_jcs/blob/main/tests/basic.rs — 27 Appendix B vectors + fixture coverage
- cyberphone/json-canonicalization testdata README — https://github.com/cyberphone/json-canonicalization/blob/master/testdata/README.md — corpus format, SHA-256 checksums, generation algorithm
- FAMP-v0.5.1-spec.md §3.6a — (local file) — `sha256:<hex>` scheme, canonical JSON hashing requirement
- crates.io API (live 2026-04-12) — serde_jcs 0.2.0 published 2026-03-25, 693K downloads

### Secondary (MEDIUM confidence)
- `serde_jcs` issue #1 (GitHub, closed) — https://github.com/l1h3r/serde_jcs/issues/1 — UTF-16 sort bug, fix via PR #4 merged 2026-03-25
- `serde_jcs` issue #3 (GitHub, open) — https://github.com/l1h3r/serde_jcs/issues/3 — integer encoding concern; source inspection suggests resolved in 0.2.0 but not officially closed
- `serde_json_canonicalizer 0.3.2` — https://docs.rs/serde_json_canonicalizer — fallback option; 1.1M downloads; uses `ryu-js 1.0.1`
- serde_json Deserializer API — https://docs.rs/serde_json/latest/serde_json/struct.Deserializer.html — `from_slice`, `from_str` constructors; no built-in duplicate key detection
- serde-rs/json issue #1112 — https://github.com/serde-rs/json/issues/1112 — maintainer recommendation: use Deserializer adapter for duplicate key detection

### Tertiary (LOW confidence)
- WebSearch results for ecosystem patterns — not individually verified against official docs; used for orientation only

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions live-checked from crates.io API 2026-04-12; source inspected
- serde_jcs correctness: MEDIUM — UTF-16 sort verified fixed; integer handling structurally correct; no external conformance proof yet; CI gate resolves this
- Architecture patterns: HIGH — locked in CONTEXT.md; API shape not at Claude's discretion
- Fallback option (serde_json_canonicalizer): MEDIUM — claims RFC compliance; uses ryu-js 1.0.1; not source-inspected for this report
- Duplicate-key strategy: MEDIUM — Deserializer adapter pattern is serde-community consensus; ~100 LoC estimate is confident; implementation not battle-tested in this codebase
- Pitfalls: HIGH — most derived from source inspection + open issues; not speculation

**Research date:** 2026-04-12
**Valid until:** 2026-05-12 (stable ecosystem; serde_jcs is slow-moving; re-verify if serde_jcs releases a new version before Phase 1 completes)
