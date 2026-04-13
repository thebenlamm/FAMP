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
fn appendix_b_float_vectors() {
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
fn nan_rejected() {
    let result = famp_canonical::canonicalize(&f64::NAN);
    assert!(result.is_err(), "NaN must be rejected per RFC 8785 §3.2.2.2");
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
