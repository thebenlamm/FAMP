//! CLI-07 env-var override. Single test in its own binary = serial by
//! construction (RESEARCH Pitfall 1 — minimize process-env surface).
//!
//! Scope: this test only verifies that env-var routing reaches `run_at`
//! and produces a populated `FAMP_HOME` at the expected path. It does NOT
//! assert stdout/stderr byte contents — D-15 output bytes are covered by
//! `init_happy_path` via `run_at` with writable handles. Keeping this
//! test minimal is deliberate.
//!
//! Edition note: workspace is edition 2021 (root `Cargo.toml`), so
//! `std::env::set_var` / `remove_var` are safe fns and do NOT require
//! an `unsafe` block. If the workspace is ever bumped to edition 2024,
//! wrap the two calls in `unsafe { }`.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

#[test]
fn famp_home_env_var_overrides_default() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("famphome");

    // Edition 2021: set_var is a safe fn.
    std::env::set_var("FAMP_HOME", &home);

    let args = famp::cli::InitArgs { force: false };
    let outcome = famp::cli::init::run(args).expect("init via env");
    assert_eq!(outcome.home, home);
    assert!(home.join("key.ed25519").exists());

    std::env::remove_var("FAMP_HOME");
}
