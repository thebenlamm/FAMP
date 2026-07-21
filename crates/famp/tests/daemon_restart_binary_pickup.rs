//! DAEMON-05 / issues #9+#20: binary-pickup restart integration test (macOS-gated).
//!
//! Verifies that `famp daemon restart`:
//! 1. exits 0 against a live registered service,
//! 2. prints a success line (`broker restarted  pid=…`),
//! 3. leaves `famp inspect broker` reporting HEALTHY **immediately** after
//!    return (readiness poll — issue #9; no race window for `restart && famp …`).
//!
//! macOS path is now bootout+bootstrap+kickstart (LWCR refresh, issue #20),
//! not bare `kickstart -k`.
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

    /// DAEMON-05 + #9: `famp daemon restart` returns Ok only after HEALTHY,
    /// and prints the success line. Requires a live registered service.
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

        // Requires: service must be installed and registered (run `famp daemon
        // install` first). Uses the `famp` on PATH (typically `just install`).
        let output = std::process::Command::new("famp")
            .args(["daemon", "restart"])
            .output()
            .expect("famp daemon restart must be runnable (is `famp` in PATH?)");

        assert!(
            output.status.success(),
            "famp daemon restart must exit 0 against a live registered service; \
             got {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("broker restarted") && stdout.contains("pid="),
            "restart must print a success line with pid; got stdout:\n{stdout}"
        );

        // Issue #9: immediately after restart returns Ok, inspect must be HEALTHY
        // (no race window for `famp daemon restart && famp …` scripts).
        let inspect = std::process::Command::new("famp")
            .args(["inspect", "broker"])
            .output()
            .expect("famp inspect broker must be runnable");
        let inspect_out = String::from_utf8_lossy(&inspect.stdout);
        assert!(
            inspect.status.success() && inspect_out.contains("HEALTHY"),
            "inspect broker must be HEALTHY immediately after restart returns; \
             exit={:?} stdout:\n{inspect_out}\nstderr:\n{}",
            inspect.status.code(),
            String::from_utf8_lossy(&inspect.stderr),
        );
    }
} // mod macos_only
