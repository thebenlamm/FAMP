//! DAEMON-05: binary-pickup restart integration test (macOS-gated).
//!
//! This test verifies that `famp daemon restart` picks up a freshly installed
//! on-disk binary via `launchctl kickstart -k`.
//!
//! Gate: `FAMP_RUN_LAUNCHCTL_TESTS` must be set in the environment. Without
//! it, the test body returns immediately (CI green, no launchctl dependency).
//!
//! Full manual validation (VALIDATION.md):
//!   1. Note current `famp -V` (e.g. "famp 0.11.0")
//!   2. Bump the version and run `just install` to replace the on-disk binary
//!   3. Run `famp daemon restart`
//!   4. Confirm `famp daemon status` shows the new daemon build version and a
//!      changed PID — proving the service picked up the replaced binary.

#![cfg(all(unix, target_os = "macos"))]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Returns true iff `FAMP_RUN_LAUNCHCTL_TESTS` is set in the environment.
fn launchctl_tests_enabled() -> bool {
    std::env::var("FAMP_RUN_LAUNCHCTL_TESTS").is_ok()
}

/// DAEMON-05: verify that `famp daemon restart` returns Ok against a live
/// registered service, proving the kickstart -k path executes.
///
/// The full version-swap verification (pid changes, new build_version visible
/// in `famp daemon status`) is the VALIDATION.md manual step — it requires a
/// live registered service and a real binary replacement.
///
/// When `FAMP_RUN_LAUNCHCTL_TESTS` is unset, this test returns immediately
/// (CI green, no launchctl dependency).
#[test]
fn restart_picks_up_new_binary() {
    if !launchctl_tests_enabled() {
        // Gate: not running launchctl integration tests in this environment.
        return;
    }

    // Run `famp daemon restart` against the live service. This calls
    // `launchctl kickstart -k gui/$UID/com.famp.broker` on macOS.
    // Requires: service must be installed and registered (run `famp daemon
    // install` first).
    let status = std::process::Command::new("famp")
        .args(["daemon", "restart"])
        .status()
        .expect("famp daemon restart must be runnable (is `famp` in PATH?)");

    assert!(
        status.success(),
        "famp daemon restart must exit 0 against a live registered service; \
         got {:?}. Is the service installed? Run `famp daemon install` first.",
        status.code()
    );
}
