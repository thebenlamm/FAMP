# famp-canonical — From-Scratch Fallback Plan

**Written:** 2026-04-12 per **D-08** (fallback-first discipline).
**Status:** Contingency plan — **not active**.
**Active engine:** `serde_jcs 0.2.0` (pending SEED-001 conformance gate, Plan 03).
**Preferred fallback (if gate fails):** Fork / vendor `serde_json_canonicalizer 0.3.2`
(see RESEARCH §"SEED-001 Decision Framework"). It uses `ryu-js 1.0.1`, claims
100% RFC 8785 compatibility, and has 1.1M downloads. **This document covers the
from-scratch path of last resort** — only invoked if both `serde_jcs` AND
`serde_json_canonicalizer` exhibit the same defect (e.g., a shared `ryu-js`
formatter bug that affects both).

This is a **written plan**, not code, per **D-09**. No parallel implementation
exists. The plan exists on disk so that, if SEED-001 fails at midnight on a
Friday, the path forward is already designed and we are not panicking into
architecture.

The total estimated implementation budget is **~530 LoC** of Rust + ~100 LoC of
tests, targeting RFC 8785 §3 conformance against the cyberphone test corpus.

---

## 1. Trigger Conditions

We abandon the wrapper-around-`serde_jcs` strategy AND skip the
`serde_json_canonicalizer` preferred fallback only under one of the following
conditions, all of which require **evidence** committed to
`.planning/SEED-001.md` before this plan is activated:

1. **Shared `ryu-js` formatter bug.** The Phase 1 conformance gate fails on a
   specific RFC 8785 Appendix B float vector, AND we can demonstrate that
   `serde_json_canonicalizer` exhibits the same wrong output for the same input.
   Both crates depend on `ryu-js` (different majors: `serde_jcs` uses `0.2.x`,
   `serde_json_canonicalizer` uses `1.0.x`), so this is unlikely but possible
   for a deep algorithmic defect in the underlying Grisu3 / Dragon4 path.
2. **Both crates disagree with the cyberphone Java reference on the same
   `weird.json` fixture.** This would indicate a class of UTF-16 surrogate-pair
   handling defect that neither Rust crate addresses.
3. **Both crates accept duplicate keys silently** AND no upstream patch is in
   sight (low probability — duplicate-key handling is in our wrapper layer, not
   the canonicalizer).
4. **Both crates require a Rust toolchain newer than our pinned `1.87.0`** AND
   the upgrade itself is blocked.

If none of the above is demonstrated, the from-scratch path is **not** taken.
The cost of maintaining a hand-rolled JCS implementation (audit surface, test
corpus, ECMAScript number-formatting subtleties) is high enough that we will
prefer to vendor `serde_json_canonicalizer` even if we have to patch it.

---

## 2. RFC 8785 §3.2.2.3 — Number Formatting

RFC 8785 §3.2.2.3 mandates ECMAScript `Number.prototype.toString` semantics.
This is the single hardest part of any from-scratch implementation; we do
**not** attempt to derive it.

**Strategy:** depend on `ryu-js` (`boa-dev/ryu-js`) directly. This is the
de-facto Rust port of the ECMAScript number formatter, used by the Boa JS
engine. Call `ryu_js::Buffer::format_finite(f64_value)` for every finite
input; reject non-finite values via `f.is_finite()` before invoking
`Buffer::format_finite`.

```rust
use ryu_js::Buffer;

fn format_number(value: f64) -> Result<String, CanonicalError> {
    if !value.is_finite() {
        return Err(CanonicalError::NonFiniteNumber);
    }
    let mut buf = Buffer::new();
    Ok(buf.format_finite(value).to_string())
}
```

**Rules:**
- Reject `NaN`, `+Infinity`, `-Infinity` via `f.is_finite()` (RFC 8785 §3.2.2.2
  forbids non-finite values).
- Pass only `f64` to `format_finite`. Integer types are converted via §5 below
  before reaching this function.
- Do not attempt zero normalization (`-0` vs `0`) here; `ryu-js` already emits
  ECMAScript-correct strings for both signed zeros (`"0"` for both, per
  Number.prototype.toString).

**Validation:** the same RFC 8785 Appendix B 27-vector test array used for the
`serde_jcs` gate (see `tests/conformance.rs`) MUST pass against this formatter
before any code lands.

**Estimated LoC:** ~50.

---

## 3. RFC 8785 §3.2.3 — Key Sorting

RFC 8785 §3.2.3 mandates that object members be sorted by their key's UTF-16
code unit sequence, lexicographically, code unit by code unit. **This is not
the same as UTF-8 byte order.** Supplementary plane characters (e.g., emoji,
CJK Extension B) are encoded as UTF-16 surrogate pairs, and their sort position
is determined by the leading surrogate (always in `0xD800..=0xDBFF`), which
sorts AFTER all BMP characters in `0x0000..=0xD7FF` but BEFORE any character in
`0xDC00..=0xFFFF`.

**Strategy:** convert each key to a `Vec<u16>` via `str::encode_utf16` and use
that as the comparison key.

```rust
fn sort_object_entries(entries: &mut Vec<(String, serde_json::Value)>) {
    entries.sort_by(|a, b| {
        let a_u16: Vec<u16> = a.0.encode_utf16().collect();
        let b_u16: Vec<u16> = b.0.encode_utf16().collect();
        a_u16.cmp(&b_u16)
    });
}
```

**Performance note:** for hot paths, cache the `Vec<u16>` per key during the
sort to avoid re-encoding on each comparison. For Phase 1 / fallback, the
naive recompute is acceptable — message sizes are bounded by the 1 MB ingress
limit (TRANS-XX) and key counts are typically small.

**Validation:** the supplementary-plane fixtures in
`tests/vectors/supplementary/` MUST pass. These fixtures use literal raw UTF-8
emoji keys (e.g., `{"🎉": 1, "a": 2}`) — see RESEARCH Pitfall 3 for the
specific surrogate-pair example.

**Estimated LoC:** ~80.

---

## 4. RFC 8785 §3.2.1 — UTF-8 String Pass-Through

RFC 8785 §3.2.1 mandates that JSON strings be output as raw UTF-8 with **no
Unicode normalization** (no NFC/NFD/NFKC/NFKD transformation). Only the
following characters MUST be escaped:

| Character | Escape sequence |
|---|---|
| `"` (U+0022) | `\"` |
| `\` (U+005C) | `\\` |
| Control characters U+0000..=U+001F | `\u00XX` (lowercase hex, 4 digits) |

Specifically:
- `\b` (U+0008) → `\b`
- `\t` (U+0009) → `\t`
- `\n` (U+000A) → `\n`
- `\f` (U+000C) → `\f`
- `\r` (U+000D) → `\r`
- All other U+0000..=U+001F → `\u00XX`

**No Unicode normalization.** Two strings that look identical but differ in
NFC vs NFD are different canonical bytes. This is intentional — normalization
would prevent round-tripping arbitrary user data.

**Strategy:** iterate `str::chars()`, write a single byte for each ASCII byte
that does not require escaping, write the literal UTF-8 bytes for all
non-ASCII characters, and write the escape sequence for the special cases
above.

```rust
fn write_json_string(out: &mut Vec<u8>, s: &str) {
    out.push(b'"');
    for ch in s.chars() {
        match ch {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
            }
            c => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}
```

**Estimated LoC:** ~60.

---

## 5. Integer Handling

RFC 8785 represents all numbers as IEEE 754 double-precision floats per
ECMAScript semantics. There is no "integer" type at the canonical-output level.
All integer inputs are cast to `f64` before being passed to the number
formatter (§2).

**Behavior at f64 precision boundary:**
- Integer values in `−2^53..=2^53` (`-9_007_199_254_740_992..=9_007_199_254_740_992`)
  round-trip exactly.
- Integer values outside this range (e.g., `u64::MAX`, `i64::MIN`) **lose
  precision** during the cast. This is intentional and matches the spec —
  callers that need lossless integer transport must encode the value as a
  string.

**Strategy:**

```rust
fn integer_to_canonical(n: i128) -> Result<String, CanonicalError> {
    // Cast loses precision above 2^53; this matches ECMAScript Number.toString.
    format_number(n as f64)
}

// All integer types (u8..u128, i8..i128) flow through `as f64` before
// reaching format_number. This is structurally identical to the path
// `serde_jcs` already takes internally.
```

**Validation:** test vectors `9007199254740992` (2^53) and `-9007199254740992`
(both in RFC 8785 Appendix B) MUST round-trip exactly through this path. The
boundary at 2^53 is the test that catches off-by-one errors.

**Anti-pattern:** do NOT enable `serde_json` `arbitrary_precision` to "preserve"
large integers. That feature changes the internal representation of
`serde_json::Number` and breaks JCS canonicalization. See RESEARCH
"What NOT to Use".

**Estimated LoC:** ~30.

---

## 6. Recursive Structure Traversal

The canonicalizer walks `serde_json::Value` recursively, dispatching to the
appropriate formatter for each branch.

```rust
fn write_value(out: &mut Vec<u8>, value: &serde_json::Value)
    -> Result<(), CanonicalError>
{
    match value {
        serde_json::Value::Null => out.extend_from_slice(b"null"),
        serde_json::Value::Bool(true) => out.extend_from_slice(b"true"),
        serde_json::Value::Bool(false) => out.extend_from_slice(b"false"),
        serde_json::Value::Number(n) => {
            // serde_json::Number stores either i64, u64, or f64 internally.
            let f = if let Some(i) = n.as_i64() {
                i as f64
            } else if let Some(u) = n.as_u64() {
                u as f64
            } else if let Some(f) = n.as_f64() {
                f
            } else {
                return Err(CanonicalError::InternalCanonicalization);
            };
            out.extend_from_slice(format_number(f)?.as_bytes());
        }
        serde_json::Value::String(s) => write_json_string(out, s),
        serde_json::Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(out, item)?;
            }
            out.push(b']');
        }
        serde_json::Value::Object(map) => {
            // Collect entries, sort by UTF-16 code units (§3), then write.
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by(|a, b| {
                let a_u16: Vec<u16> = a.0.encode_utf16().collect();
                let b_u16: Vec<u16> = b.0.encode_utf16().collect();
                a_u16.cmp(&b_u16)
            });
            out.push(b'{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_json_string(out, k);
                out.push(b':');
                write_value(out, v)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}
```

**Whitespace:** none. RFC 8785 forbids any whitespace between tokens in the
canonical output.

**Estimated LoC:** ~200 (with error wiring and helpers).

---

## 7. Estimated LoC Budget

| Section | Component | LoC |
|---|---|---|
| §2 | Number formatter (ryu-js wrapper, finite check, error mapping) | ~50 |
| §3 | Key sorter (UTF-16 comparator, entry collection, perf note) | ~80 |
| §4 | UTF-8 string writer (escape table, control-char loop) | ~60 |
| §5 | Integer handling (typed dispatch into format_number) | ~30 |
| §6 | Recursive `serde_json::Value` traversal (object/array/string/number/bool/null) | ~200 |
| — | Writer plumbing (`Vec<u8>` buffer, error type wiring, public `canonicalize` entry point) | ~100 |
| — | Tests (port the existing `tests/conformance.rs`, `tests/duplicate_keys.rs`, `tests/artifact_id.rs` against the new path; add fallback-specific edge cases) | ~100 |
| **Total** | | **~530** |

This budget assumes the from-scratch implementation lives behind the same
public API (`canonicalize`, `Canonicalize`, `from_slice_strict`,
`from_str_strict`, `artifact_id_for_canonical_bytes`, `artifact_id_for_value`)
so callers experience zero source-level disruption when the engine swaps.

---

## 8. Verification Approach

The from-scratch implementation is verified against the **same** corpus the
`serde_jcs` wrapper is gated on:

1. **RFC 8785 Appendix B (27 float vectors)** — `tests/conformance.rs`
   `rfc8785_appendix_b_all`. All 27 vectors MUST byte-match.
2. **RFC 8785 Appendix C/E (structured object vectors)** — same harness.
3. **cyberphone testdata fixtures (6 pairs)** — arrays, french, structures,
   unicode, values, weird. All 6 input/output pairs MUST byte-match.
4. **Sampled float corpus (100K lines, fixed seed)** —
   `tests/float_corpus.rs::float_corpus_sampled`. 100% match required.
5. **Full 100M float corpus** (nightly + release tag) —
   `tests/float_corpus.rs::float_corpus_full` behind `--features full-corpus`.
6. **Supplementary-plane key sort fixtures** —
   `tests/vectors/supplementary/`. Reuse the cyberphone JS reference
   implementation's output as the oracle; commit both inputs and expected
   canonical bytes.
7. **Duplicate-key rejection** — `tests/duplicate_keys.rs`. Strict-parse
   variants reject `{"a":1,"b":2,"a":3}` with
   `CanonicalError::DuplicateKey { key: "a" }`.
8. **Artifact ID determinism** — `tests/artifact_id.rs`. Empty input MUST hash
   to
   `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
   (SHA-256 of empty input, well-known constant).

**Reference oracle:** the cyberphone Java reference implementation
(<https://github.com/cyberphone/json-canonicalization>) is the source of truth
for any input on which the Rust implementations disagree. Diff the byte output
of the from-scratch path against the Java reference's output for the same
input; the from-scratch path is wrong if it diverges, regardless of what
`serde_jcs` or `serde_json_canonicalizer` produce.

**Test harness re-use:** the test files created in Plan 01 (this plan) live
behind `#[cfg(feature = "wave2_impl")]` and reference the same public API
symbols (`canonicalize`, `from_str_strict`, `artifact_id_for_canonical_bytes`,
`CanonicalError`). The fallback implementation reuses the same harness without
modification — only the implementation behind the API changes.

---

*End of fallback plan. Reviewed against RFC 8785 §3 sections 3.2.1, 3.2.2.3,
3.2.3 on 2026-04-12.*
