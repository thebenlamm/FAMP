//! UTF-16 supplementary-plane key sort fixtures (CANON-04, RESEARCH Pitfall 3).
//!
//! Oracle: cyberphone JS reference implementation
//! (https://github.com/cyberphone/json-canonicalization). Plan 02 authors the
//! actual fixture files in tests/vectors/supplementary/ and commits both the
//! input JSON (with raw emoji / CJK Extension B keys) and the expected
//! canonical bytes produced by the JS reference.
//!
//! Until Plan 02 lands, this file is gated behind `wave2_impl` so the missing
//! fixture file does not break the build.

#![cfg(feature = "wave2_impl")]

#[test]
fn supplementary_plane_emoji_keys_sort_by_utf16() {
    // Fixture: tests/vectors/supplementary/emoji_keys.json
    // Contains keys with raw UTF-8 emoji (e.g., "🎉", "🍕") mixed with BMP
    // keys. Expected output is the cyberphone JS reference's canonical bytes.
    let input: serde_json::Value =
        serde_json::from_str(include_str!("vectors/supplementary/emoji_keys.json")).unwrap();
    let got = famp_canonical::canonicalize(&input).unwrap();
    let expected = include_bytes!("vectors/supplementary/emoji_keys.canonical");
    assert_eq!(&got[..], &expected[..], "supplementary-plane key sort mismatch");
}
