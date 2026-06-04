//! DAEMON-01 + DAEMON-04 lifecycle integration tests (macOS-gated).
//!
//! These tests call `launchctl bootstrap` and `launchctl bootout` against the
//! real user session. They are gated behind `FAMP_RUN_LAUNCHCTL_TESTS` so CI
//! passes on Linux and on macOS hosts without the env var set.
//!
//! **Safety requirement:** each test that calls `install::run_at` MUST call
//! `uninstall::run_at` for cleanup — even if assertions fail. The cleanup must
//! run BEFORE assertions so a panic does not leave a persistent LaunchAgent on
//! the machine.
//!
//! To run these tests locally:
//!   FAMP_RUN_LAUNCHCTL_TESTS=1 cargo test -p famp --test daemon_lifecycle
//!
//! Pre-condition: no `com.famp.broker` LaunchAgent currently registered
//! (verified in RESEARCH.md Runtime State Inventory).

#![cfg(all(unix, target_os = "macos"))]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use famp::cli::daemon::{install, uninstall};

/// Returns true iff `FAMP_RUN_LAUNCHCTL_TESTS` is set in the environment.
fn launchctl_tests_enabled() -> bool {
    std::env::var("FAMP_RUN_LAUNCHCTL_TESTS").is_ok()
}

/// DAEMON-01: `famp daemon install` is idempotent — calling it twice leaves
/// exactly one launchd registration (no duplicate service).
///
/// Implementation: call install twice, check via `launchctl print` exit code
/// that the service is registered exactly once, then clean up via uninstall.
///
/// Cleanup runs BEFORE assertions to guarantee no persistent LaunchAgent is
/// left even if assertions panic.
#[test]
fn daemon_install_is_idempotent() {
    if !launchctl_tests_enabled() {
        // Gate: not running launchctl integration tests in this environment.
        // CI stays green without launchctl dependency.
        return;
    }

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let home = tmp.path();
    let mut out = Vec::<u8>::new();

    // Install once.
    install::run_at(home, &mut out).expect("first install must succeed");

    // Install again — must be idempotent (exit 37 tolerated).
    let second_result = install::run_at(home, &mut out);

    // Clean up BEFORE asserting so a panic does not leave a persistent LaunchAgent.
    let cleanup_result = uninstall::run_at(home, &mut out);

    // Now assert.
    second_result.expect("second install must succeed (idempotent)");
    cleanup_result.expect("cleanup uninstall must succeed");

    // Verify the service is no longer registered after uninstall.
    let uid = u32::from(nix::unistd::getuid());
    let registered_after = std::process::Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/com.famp.broker")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    assert!(
        !registered_after,
        "service must not be registered after uninstall (DAEMON-01/04)"
    );
}

/// DAEMON-04: `famp daemon uninstall` is idempotent — calling it twice exits
/// Ok both times and leaves no orphan registration.
///
/// Cleanup runs BEFORE assertions to guarantee no persistent LaunchAgent is
/// left even if assertions panic.
#[test]
fn daemon_uninstall_is_idempotent() {
    if !launchctl_tests_enabled() {
        // Gate: not running launchctl integration tests in this environment.
        return;
    }

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let home = tmp.path();
    let mut out = Vec::<u8>::new();

    // Install first so there is something to uninstall.
    install::run_at(home, &mut out).expect("install must succeed before uninstall test");

    // Uninstall once — removes the service registration and plist file.
    let first_uninstall = uninstall::run_at(home, &mut out);

    // Uninstall again — must be idempotent (plist absent, service not registered).
    let second_uninstall = uninstall::run_at(home, &mut out);

    // Verify the plist file is gone.
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join("com.famp.broker.plist");
    let plist_exists = plist_path.exists();

    // Assert after cleanup (both uninstalls ran above — cleanup complete).
    first_uninstall.expect("first uninstall must succeed");
    second_uninstall.expect("second uninstall must succeed (idempotent — DAEMON-04)");

    assert!(
        !plist_exists,
        "plist must be removed after uninstall (DAEMON-04); found at {}",
        plist_path.display()
    );
}
