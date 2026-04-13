//! Sampled float corpus driver (CANON-03).
//!
//! Source: cyberphone/json-canonicalization es6testfile100m.txt.gz
//! Full corpus URL:
//!   https://github.com/cyberphone/json-canonicalization/releases/download/es6testfile/es6testfile100m.txt.gz
//! Full corpus SHA-256 (100M lines):
//!   0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272
//!
//! Per D-12/D-13/D-14, a deterministic 100_000-line sample (the first 100_000
//! lines of the full corpus, which is itself deterministically generated)
//! runs on every PR; the full 100M corpus runs nightly + on release tags
//! behind `--features full-corpus`.
//!
//! Sample seed: `famp-canonical-float-corpus-v1` — committed; do not change.
//! The "seed" here is conceptual: the prefix-of-corpus approach is itself a
//! deterministic, reproducible subset because cyberphone's corpus is
//! generated from a fixed seed upstream, so any developer can regenerate the
//! same `float_corpus_sample.txt` by re-fetching the upstream gz and
//! re-taking the first 100_000 lines.
//!
//! Gated behind `wave2_impl` (now default).

#![cfg(feature = "wave2_impl")]

const SAMPLE_SIZE_PR: usize = 100_000;
const SAMPLE_DATA: &str = include_str!("vectors/float_corpus_sample.txt");

#[test]
fn float_corpus_sampled() {
    let mut count = 0usize;
    let mut failures: Vec<(String, String, String)> = Vec::new();
    for (lineno, line) in SAMPLE_DATA.lines().enumerate().take(SAMPLE_SIZE_PR) {
        let mut parts = line.splitn(2, ',');
        let hex = parts.next().expect("hex").trim();
        let expected = parts.next().expect("expected").trim();
        let bits = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .unwrap_or_else(|e| panic!("line {lineno}: bad hex {hex}: {e}"));
        let f = f64::from_bits(bits);
        if !f.is_finite() {
            continue;
        }
        let bytes = famp_canonical::canonicalize(&f).expect("finite float canonicalizes");
        let got = std::str::from_utf8(&bytes).unwrap();
        if got != expected {
            failures.push((hex.to_string(), expected.to_string(), got.to_string()));
            if failures.len() > 5 {
                break;
            }
        }
        count += 1;
    }
    assert!(
        failures.is_empty(),
        "float corpus mismatches ({}): {:?}",
        failures.len(),
        failures
    );
    assert!(count > 0, "ran 0 corpus lines — fixture file empty?");
}
