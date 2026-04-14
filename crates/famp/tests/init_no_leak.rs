//! IDENT-06 via D-17 mechanism #3.
//!
//! EPISTEMIC LIMIT (RESEARCH Pitfall 8): this test cannot distinguish a
//! legitimately leaked 8-byte run from a coincidental collision on a
//! low-entropy key. We rely on `OsRng` producing high-entropy keys and
//! accept that this is a defense-in-depth test, not a proof.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

/// D-17 mechanism #3: run init, read the seed back from disk, and scan
/// captured stdout+stderr for any 8-byte sliding window of the seed.
#[test]
fn init_output_contains_no_8byte_window_of_secret_seed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    let seed = std::fs::read(home.join("key.ed25519")).expect("read seed");
    assert_eq!(seed.len(), 32);

    // Concatenate captured output and scan for any 8-byte window.
    let mut combined = Vec::<u8>::new();
    combined.extend_from_slice(&out);
    combined.extend_from_slice(&err);

    for window in seed.windows(8) {
        assert!(
            !combined.windows(window.len()).any(|w| w == window),
            "8-byte seed window leaked into captured stdout/stderr"
        );
    }
}
