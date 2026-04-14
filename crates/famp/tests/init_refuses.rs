//! CLI-01 `AlreadyInitialized` refusal when `FAMP_HOME` is non-empty and
//! `--force` is not passed.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

use famp::cli::CliError;

#[test]
fn refuses_non_empty_without_force() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(home.join("key.ed25519"), b"stale").unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    match famp::cli::init::run_at(&home, false, &mut out, &mut err) {
        Err(CliError::AlreadyInitialized { existing_files }) => {
            assert!(
                existing_files.iter().any(|p| p.ends_with("key.ed25519")),
                "existing_files should list key.ed25519, got {existing_files:?}"
            );
        }
        other => panic!("expected AlreadyInitialized, got {other:?}"),
    }

    // The stale file is NOT touched
    assert_eq!(std::fs::read(home.join("key.ed25519")).unwrap(), b"stale");
}
