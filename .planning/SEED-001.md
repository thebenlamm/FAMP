# SEED-001: serde_jcs RFC 8785 Conformance Gate Decision

**Status:** RESOLVED (Phase 1, Plan 03)
**Decision Date:** 2026-04-13
**Decision:** keep serde_jcs

## Context

Per CONTEXT.md D-08, the fallback plan was written FIRST (see
`crates/famp-canonical/docs/fallback.md`, committed in Plan 01-01 as
`cb0d253`) **before** any conformance gate ran. The gate harness was wired
in Plan 01-02 (`a7b8f26`, `4c4c39b`) and executed end-to-end in Plan 01-03
with the full RFC 8785 Appendix B/C/E vector set plus the cyberphone weird
fixture, the supplementary-plane sort fixture, the 100K-line float corpus
sample, NaN/Infinity rejection, and duplicate-key rejection.

This decision honors the locked sequence in D-10 (define API → write
fallback → wire vectors → run gate → record decision with cited evidence)
and the location lock in D-11 (decision lives in `.planning/SEED-001.md`,
not buried in commit messages).

## Gate Criteria & Evidence

All evidence below is from the gate run captured at
`/tmp/famp-canonical-gate.txt` on 2026-04-13. The full nextest summary line
is `Summary [0.168s] 12 tests run: 12 passed, 0 skipped`.

| Criterion | Pass Condition | Result | Evidence |
|-----------|---------------|--------|----------|
| RFC 8785 Appendix B (27 IEEE 754 → ECMAScript number vectors) | All 27 byte-exact | **PASS** | `cargo nextest run -p famp-canonical conformance::rfc8785_appendix_b_all` → `PASS [ 0.016s] famp-canonical::conformance rfc8785_appendix_b_all` (loop iterates over all 27 `(ieee_bits, expected_str)` pairs from RFC 8785 Appendix B; assert_eq! on `String::from_utf8(canonicalize(&f))` for every entry). |
| RFC 8785 Appendix C (structured object with literals/numbers/string) | Byte-exact against published expected output | **PASS** | `cargo nextest run -p famp-canonical conformance::rfc8785_appendix_c_structured` → `PASS [ 0.018s] famp-canonical::conformance rfc8785_appendix_c_structured`. Probe run prior to test commit confirmed `serde_jcs::to_vec` produces exactly `{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}` (118 bytes), matching RFC 8785 §C.1 verbatim. |
| RFC 8785 Appendix E (complex nested object) | Byte-exact against published expected output | **PASS** | `cargo nextest run -p famp-canonical conformance::rfc8785_appendix_e_complex` → `PASS [ 0.015s] famp-canonical::conformance rfc8785_appendix_e_complex`. Output: `{"":"empty","1":{"\n":56,"f":{"F":5,"f":"hi"}},"10":{},"111":[{"E":"no","e":"yes"}],"A":{},"a":{}}` (98 bytes), matching RFC 8785 §E. Exercises lex sort across mixed types, empty key, control-character key, case-sensitive ordering, nested objects, and arrays of objects. |
| cyberphone `weird.json` fixture | Round-trips byte-exact against committed expected | **PASS** | `cargo nextest run -p famp-canonical conformance::cyberphone_weird_fixture` → `PASS [ 0.015s] famp-canonical::conformance cyberphone_weird_fixture`. Fixture pair (`tests/vectors/input/weird.json` 283 B, `tests/vectors/output/weird.json` 214 B) committed verbatim from upstream cyberphone testdata in Plan 01-02 (`4c4c39b`). Covers Latin, Hebrew, CJK, emoji, control characters, and `</script>` XSS escape. |
| Sampled float corpus (100,000 lines, fixed deterministic prefix `famp-canonical-float-corpus-v1`) | 100% match | **PASS** | `cargo nextest run -p famp-canonical float_corpus::float_corpus_sampled` → `PASS [ 0.141s] famp-canonical::float_corpus float_corpus_sampled`. 100,000 randomized cyberphone es6testfile100m double values — zero mismatches. |
| Supplementary-plane key sort (UTF-16 code units) | Sort produces RFC 8785 §3.2.3 expected order | **PASS** | `cargo nextest run -p famp-canonical utf16_supplementary::supplementary_plane_keys_sort_correctly` → `PASS [ 0.009s] famp-canonical::utf16_supplementary supplementary_plane_keys_sort_correctly`. Hand-authored oracle verifies U+0061 (`a`) < U+1F389 (🎉, high surrogate 0xD83C) < U+20BB7 (𠮷, high surrogate 0xD842); canonical bytes match `{"a":2,"🎉":1,"𠮷":3}`. |
| NaN / ±Infinity rejection | Returns `CanonicalError::NonFiniteNumber` | **PASS** | `cargo nextest run -p famp-canonical conformance::nan_rejected conformance::infinity_rejected` → both PASS. Verifies RFC 8785 §3.2.2.2 (non-finite numbers MUST be rejected). |
| Duplicate-key rejection on parse | Returns `CanonicalError::DuplicateKey { key }` | **PASS** | `cargo nextest run -p famp-canonical duplicate_keys::duplicate_key_is_error duplicate_keys::non_duplicate_is_ok` → both PASS. Implemented via custom serde-visitor in `from_*_strict` per D-04..D-07 (orthogonal to `serde_jcs` itself). Plan 01-02 evidence (`4c4c39b`). |

**Aggregate:** 12 / 12 tests PASS. 0 failures. 0 corpus mismatches. 0 byte
divergences across any RFC 8785 published vector.

## Decision Rationale

All gate criteria met. SEED-001 resolves to **keep `serde_jcs 0.2.0`** as
the canonical engine, wrapped behind `famp_canonical::canonicalize` and the
`Canonicalize` blanket trait per D-02.

Specific reasons:

1. **Zero byte divergences across published RFC 8785 vectors.** All three
   IETF-published examples (Appendix B float vectors, Appendix C structured
   object, Appendix E complex object) round-trip byte-exact. This is the
   strictest possible conformance signal: it's a property of the engine, not
   of our test harness.
2. **Zero divergences on 100,000 cyberphone float corpus lines.** The
   cyberphone es6testfile is the same corpus used by every reference
   canonicalizer in the ecosystem; matching it on a deterministic 100K
   sample means matching the reference Java/JavaScript/C# implementations
   on the hardest case (ECMAScript Number.toString rounding edges).
3. **Zero divergences on the cyberphone weird fixture.** Mixed Unicode,
   control characters, surrogate pairs, and XSS-shaped strings all
   round-trip exactly.
4. **The `serde_jcs` "unstable" self-label is about API churn, not
   correctness.** RESEARCH §"SEED-001 Decision Framework" anticipated this:
   the gate validates correctness empirically, and our wrapper trait (D-02)
   insulates downstream callers from any future API churn — if `serde_jcs`
   ships a breaking 0.3.x, we update one file (`canonical.rs`) and downstream
   callers see no diff.
5. **Forking now would be premature work.** The fallback plan
   (`crates/famp-canonical/docs/fallback.md`, 357 lines) remains on disk as
   insurance. Per D-09, we don't build two implementations until the gate
   actually fails. Today the gate is 12 / 12 green — building a parallel
   from-scratch canonicalizer would be invented work.

## Carried Forward

- The wrapper trait API surface (D-02) is independent of engine choice — no
  caller changes if the engine is swapped in a future plan.
- The 100M full-corpus run is required at release-tag time per D-12. Plan
  03 wires that into `.github/workflows/nightly-full-corpus.yml` (cron `0 6
  * * *` + on `v*` tags + manual dispatch). The 100K sampled subset enforced
  on every PR catches formatter drift between full-corpus runs.
- **Re-run the gate on every `serde_jcs` dependency bump.** This gate IS the
  conformance contract — any minor/patch bump must re-clear all 12 tests
  before being merged. CI enforces this automatically.
- **Trigger to revisit:** if a future `serde_jcs` release fails any vector
  in this gate, the next step is to fork to `serde_json_canonicalizer 0.3.2`
  (1.1M downloads, also `ryu-js`-backed, claims 100% RFC 8785 compatibility)
  per RESEARCH §"SEED-001 Decision Framework" — NOT to immediately execute
  the from-scratch fallback. The from-scratch plan is reserved for the case
  where `serde_json_canonicalizer` ALSO fails the same vectors.

## References

- `.planning/phases/01-canonical-json-foundations/01-CONTEXT.md` D-08..D-11
- `.planning/phases/01-canonical-json-foundations/01-RESEARCH.md` §"SEED-001 Decision Framework"
- `.planning/phases/01-canonical-json-foundations/01-01-SUMMARY.md` (fallback plan committed `cb0d253`)
- `.planning/phases/01-canonical-json-foundations/01-02-SUMMARY.md` (Plan 02 test results, 10/10 baseline)
- `crates/famp-canonical/docs/fallback.md` (357-line written fallback)
- `crates/famp-canonical/tests/conformance.rs` (Appendix B/C/E + cyberphone)
- `crates/famp-canonical/tests/float_corpus.rs` (sampled corpus driver)
- `crates/famp-canonical/tests/utf16_supplementary.rs` (supplementary-plane oracle)
- `crates/famp-canonical/tests/duplicate_keys.rs` (strict-parse rejection)
- Gate run output: `/tmp/famp-canonical-gate.txt` (12 tests, 12 PASS, run 2026-04-13)
- RFC 8785 — JSON Canonicalization Scheme: <https://datatracker.ietf.org/doc/html/rfc8785>
