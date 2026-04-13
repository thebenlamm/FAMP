#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic
)]

//! UTF-16 supplementary-plane key sort fixtures (CANON-04, RESEARCH Pitfall 3).
//!
//! Oracle approach: rather than depend on the cyberphone JS reference at
//! runtime, the fixture's expected output is derived from the documented
//! RFC 8785 §3.2.3 rule (sort by UTF-16 code unit, lexicographically) and
//! committed verbatim. The math:
//!
//! - "a" → UTF-16 [0x0061]
//! - "🎉" U+1F389 → [0xD83C, 0xDF89]
//! - "𠮷" U+20BB7 → [0xD842, 0xDFB7]
//!
//! Lexicographic order on the leading code unit: 0x0061 < 0xD83C < 0xD842,
//! so the canonical key order is `a`, `🎉`, `𠮷`. The committed
//! `emoji_keys.expected` file contains raw UTF-8 bytes (NOT \uXXXX escapes)
//! per RFC 8785 §3.2.1 (UTF-8 pass-through for output).

#![cfg(feature = "wave2_impl")]

#[test]
fn supplementary_plane_keys_sort_correctly() {
    let input = include_str!("vectors/supplementary/emoji_keys.json");
    let expected = include_bytes!("vectors/supplementary/emoji_keys.expected");
    let parsed: serde_json::Value = serde_json::from_str(input).unwrap();
    let got = famp_canonical::canonicalize(&parsed).unwrap();
    assert_eq!(
        &got[..],
        &expected[..],
        "supplementary-plane key sort mismatch: got {:?}, expected {:?}",
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(expected)
    );
}
