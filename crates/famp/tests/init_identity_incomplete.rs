//! IDENT-05 Phase 1 slice via `load_identity`.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

use famp::cli::CliError;

#[test]
fn load_identity_reports_missing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(&home, false, &mut out, &mut err).expect("init");

    std::fs::remove_file(home.join("tls.key.pem")).unwrap();

    match famp::cli::init::load_identity(&home) {
        Err(CliError::IdentityIncomplete { missing }) => {
            assert!(missing.ends_with("tls.key.pem"));
        }
        other => panic!("expected IdentityIncomplete, got {other:?}"),
    }
}

#[test]
fn load_identity_rejects_relative_home() {
    match famp::cli::init::load_identity(std::path::Path::new("relative/path")) {
        Err(CliError::HomeNotAbsolute { .. }) => {}
        other => panic!("expected HomeNotAbsolute, got {other:?}"),
    }
}
