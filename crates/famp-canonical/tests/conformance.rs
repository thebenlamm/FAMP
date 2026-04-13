#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic
)]

//! RFC 8785 Appendix B conformance vector harness.
//!
//! These tests are gated behind `--features wave2_impl` because they reference
//! public symbols (`canonicalize`, `CanonicalError`) that do not yet exist in
//! the crate. Plan 02 will land the production code and enable this feature
//! in CI.
//!
//! Source: RFC 8785 Appendix B (27 IEEE 754 → ECMAScript Number.toString
//! pairs), transcribed verbatim from `.planning/phases/01-canonical-json-foundations/01-RESEARCH.md`
//! §"Code Examples" → `rfc8785_appendix_b_all`.

#![cfg(feature = "wave2_impl")]

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

/// RFC 8785 Appendix C — structured object example.
///
/// Input/expected transcribed verbatim from
/// <https://datatracker.ietf.org/doc/html/rfc8785#appendix-C>.
/// Verified byte-exact against `serde_jcs 0.2.0` during Plan 03.
#[test]
fn rfc8785_appendix_c_structured() {
    let input = r#"{
        "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
        "string": "\u20ac$\u000F\nA'\u0042\u0022\u005c\\\"\/",
        "literals": [null, true, false]
    }"#;
    let value: serde_json::Value =
        serde_json::from_str(input).expect("Appendix C input parses as JSON");
    let got = famp_canonical::canonicalize(&value).expect("Appendix C canonicalizes");
    let expected: &[u8] =
        b"{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"\xe2\x82\xac$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}";
    assert_eq!(
        got.as_slice(),
        expected,
        "RFC 8785 Appendix C structured object must canonicalize byte-exact"
    );
}

/// RFC 8785 Appendix E — complex nested object example.
///
/// Input/expected from <https://datatracker.ietf.org/doc/html/rfc8785#appendix-E>.
/// Exercises lexicographic key sort across mixed-type values, empty objects,
/// nested objects, arrays of objects, an empty key, control-character keys,
/// and case-sensitive ordering (uppercase before lowercase per UTF-16 code
/// unit comparison).
#[test]
fn rfc8785_appendix_e_complex() {
    let input = r#"{
  "1": {"f": {"f": "hi","F": 5} ,"\n": 56.0},
  "10": { },
  "":  "empty",
  "a": { },
  "111": [ {"e": "yes","E": "no" } ],
  "A": { }
}"#;
    let value: serde_json::Value =
        serde_json::from_str(input).expect("Appendix E input parses as JSON");
    let got = famp_canonical::canonicalize(&value).expect("Appendix E canonicalizes");
    let expected: &[u8] =
        b"{\"\":\"empty\",\"1\":{\"\\n\":56,\"f\":{\"F\":5,\"f\":\"hi\"}},\"10\":{},\"111\":[{\"E\":\"no\",\"e\":\"yes\"}],\"A\":{},\"a\":{}}";
    assert_eq!(
        got.as_slice(),
        expected,
        "RFC 8785 Appendix E complex object must canonicalize byte-exact"
    );
}

#[test]
fn nan_rejected() {
    let result = famp_canonical::canonicalize(&f64::NAN);
    assert!(
        result.is_err(),
        "NaN must be rejected per RFC 8785 §3.2.2.2"
    );
}

#[test]
fn infinity_rejected() {
    assert!(famp_canonical::canonicalize(&f64::INFINITY).is_err());
    assert!(famp_canonical::canonicalize(&f64::NEG_INFINITY).is_err());
}

#[test]
fn cyberphone_weird_fixture() {
    // Plan 02 will populate tests/vectors/input/weird.json + output/weird.json
    // from the cyberphone testdata corpus. Until then, this test will fail to
    // compile (include_str!/include_bytes! on missing files), which is the
    // intended behavior — the test only enters the build graph when
    // wave2_impl is enabled, AND Plan 02 ships fixtures and production code
    // together.
    let input: serde_json::Value =
        serde_json::from_str(include_str!("vectors/input/weird.json")).unwrap();
    let got = famp_canonical::canonicalize(&input).unwrap();
    let expected = include_bytes!("vectors/output/weird.json");
    assert_eq!(&got[..], &expected[..]);
}
