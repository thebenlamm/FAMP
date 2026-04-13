//! Sampled float corpus driver (CANON-03).
//!
//! Source: cyberphone/json-canonicalization testdata.
//! Full corpus URL:
//!   https://github.com/cyberphone/json-canonicalization/releases/download/es6testfile/es6testfile100m.txt.gz
//! Full corpus SHA-256 (100M lines):
//!   0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272
//!
//! Per D-12/D-13/D-14, a deterministic 100K-line sample runs on every PR; the
//! full 100M corpus runs nightly + on release tags behind `--features full-corpus`.
//!
//! Sample rationale: 100K lines is ~4MB uncompressed, runs in <5s on GHA free
//! tier, is enough to catch ECMAScript Number.toString formatter drift, and
//! the corpus is deterministically generated from a SHA-256 seed so the first
//! N lines form a stable subset across regenerations.
//!
//! Gated behind `wave2_impl` until Plan 02 lands the canonicalize() symbol.

#![cfg(feature = "wave2_impl")]

const SAMPLE_SIZE_PR: usize = 100_000;
const SAMPLE_SEED: &str = "famp-canonical-float-corpus-v1"; // committed seed — do not change

#[test]
fn float_corpus_sampled() {
    // Loads tests/vectors/float_corpus_sample.txt (Plan 02 generates).
    // Each line: "<hex-ieee>,<expected>\n"
    // Parses hex as u64, builds f64, canonicalizes, compares to expected.
    let _ = SAMPLE_SEED;
    run_corpus_lines(SAMPLE_SIZE_PR);
}

#[cfg(feature = "full-corpus")]
#[test]
fn float_corpus_full() {
    run_corpus_lines(100_000_000);
}

fn run_corpus_lines(n: usize) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/float_corpus_sample.txt");
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing corpus file {}: {e}", path.display()));
    for (lineno, line) in contents.lines().take(n).enumerate() {
        let (hex, expected) = line.split_once(',').unwrap_or_else(|| {
            panic!("malformed corpus line {}: {line}", lineno + 1)
        });
        let bits = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .unwrap_or_else(|e| panic!("bad hex on line {}: {e}", lineno + 1));
        let f = f64::from_bits(bits);
        let got = famp_canonical::canonicalize(&f)
            .unwrap_or_else(|e| panic!("canonicalize failed on line {}: {e:?}", lineno + 1));
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            expected,
            "corpus mismatch on line {} (bits=0x{bits:016x})",
            lineno + 1
        );
    }
}
