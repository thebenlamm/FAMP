//! DAEMON-05: binary-pickup restart integration test (macOS-gated).
//!
//! This test verifies that `famp daemon restart` picks up a freshly installed
//! on-disk binary via `launchctl kickstart -k`.
//!
//! Gate: this test is `#[ignore]`d (so a default `cargo test` reports it as
//! IGNORED, not PASSED) AND requires `FAMP_RUN_LAUNCHCTL_TESTS` set. Run with:
//!   FAMP_RUN_LAUNCHCTL_TESTS=1 cargo test -p famp --test daemon_restart_binary_pickup -- --ignored
//!
//! Full manual validation (VALIDATION.md):
//!   1. Note current `famp -V` (e.g. "famp 0.11.0")
//!   2. Bump the version and run `just install` to replace the on-disk binary
//!   3. Run `famp daemon restart`
//!   4. Confirm `famp daemon status` shows the new daemon build version and a
//!      changed PID — proving the service picked up the replaced binary.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// macOS-only — wrapped in a cfg'd module so the crate-root
// `#![allow(unused_crate_dependencies)]` survives on Linux (a file-level
// `#![cfg(false)]` would strip sibling inner attrs along with the body,
// re-firing the `unused_crate_dependencies` lint against the empty crate).
#[cfg(all(unix, target_os = "macos"))]
mod macos_only {

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
    /// `#[ignore]`: requires a live registered service, so it is not run by a
    /// default `cargo test` — and is reported as **ignored**, not **passed**, so a
    /// zero-assertion CI run is never mistaken for verified coverage (WR-01). Run
    /// explicitly with:
    ///   FAMP_RUN_LAUNCHCTL_TESTS=1 cargo test -p famp --test daemon_restart_binary_pickup -- --ignored
    #[test]
    #[ignore = "requires a live registered launchd service; run with -- --ignored and FAMP_RUN_LAUNCHCTL_TESTS=1"]
    fn restart_picks_up_new_binary() {
        if !launchctl_tests_enabled() {
            // Belt-and-suspenders even under `--ignored`: emit an explicit SKIP so a
            // green run is never mistaken for a verified one.
            eprintln!(
                "SKIP restart_picks_up_new_binary: FAMP_RUN_LAUNCHCTL_TESTS unset \
             (set it to exercise the live `famp daemon restart` path)"
            );
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
} // mod macos_only
