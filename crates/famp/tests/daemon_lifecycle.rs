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
//!   FAMP_RUN_LAUNCHCTL_TESTS=1 cargo test -p famp --test daemon_lifecycle -- --ignored
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

/// Probe (exit-code-only) whether `com.famp.broker` is registered in this
/// user's gui domain. Mirrors the production `status::launchctl_is_registered`,
/// duplicated here because that helper is `pub(crate)` and unreachable from an
/// integration-test crate.
#[cfg(target_os = "macos")]
fn broker_registered() -> bool {
    let uid = u32::from(nix::unistd::getuid());
    std::process::Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/com.famp.broker")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// DAEMON-01 + DAEMON-04: full install/uninstall lifecycle is idempotent.
///
/// This is ONE sequential test, not two, on purpose: both halves operate on the
/// SAME launchd label (`com.famp.broker`) in the SAME live `gui/$UID` domain.
/// As two separate `#[test]` functions they ran in parallel (cargo's default)
/// and collided — one's `bootstrap`/`bootout` raced the other's, surfacing as
/// `LaunchctlFailed(5)` ("Bootstrap failed: 5: Input/output error" = label
/// already bootstrapped). A shared, process-global resource cannot be exercised
/// by two parallel tests; serializing the whole lifecycle into one test is the
/// fix.
///
/// Sequence: install → install (idempotent no-op) → uninstall → uninstall
/// (idempotent no-op). The plist's `ProgramArguments` points at a binary that
/// does not exist under the temp HOME, so launchd's `RunAtLoad` launch attempt
/// fails and KeepAlive throttles — harmless for the brief window before
/// `bootout`; we assert on registration/plist state, not on a live broker.
///
/// A defensive final `uninstall` runs BEFORE any assertion so a failed
/// assertion can never leave a persistent LaunchAgent on the machine.
///
/// `#[ignore]`: this test mutates the live `gui/$UID` launchd domain, so it is
/// not run by a default `cargo test`. Crucially, `#[ignore]` makes the harness
/// report it as **ignored**, not **passed** — the previous early-`return` gate
/// made a zero-assertion body report PASS, falsely implying DAEMON-01/04 were
/// verified in CI (WR-01). Run explicitly with:
///   FAMP_RUN_LAUNCHCTL_TESTS=1 cargo test -p famp --test daemon_lifecycle -- --ignored
#[test]
#[ignore = "mutates the live launchd gui/$UID domain; run with -- --ignored and FAMP_RUN_LAUNCHCTL_TESTS=1"]
fn daemon_lifecycle_is_idempotent() {
    if !launchctl_tests_enabled() {
        // Belt-and-suspenders: even under `--ignored`, refuse to run against a
        // live session unless explicitly opted in. Emit an explicit SKIP so a
        // green run is never mistaken for a verified one.
        eprintln!(
            "SKIP daemon_lifecycle_is_idempotent: FAMP_RUN_LAUNCHCTL_TESTS unset \
             (set it to exercise the live launchctl bootstrap/bootout path)"
        );
        return;
    }

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let home = tmp.path();
    let mut out = Vec::<u8>::new();

    // 1. Install once — bootstraps the label.
    let install1 = install::run_at(home, &mut out);
    // 2. Install again — must be an idempotent no-op (DAEMON-01). Before the
    //    fix this returned LaunchctlFailed(5) because the code tolerated only
    //    exit 37; now load_macos checks registration first and returns Ok.
    let install2 = install::run_at(home, &mut out);
    let registered_after_install = broker_registered();
    // 3. Uninstall once — boots out + removes the plist.
    let uninstall1 = uninstall::run_at(home, &mut out);
    // 4. Uninstall again — idempotent no-op (DAEMON-04).
    let uninstall2 = uninstall::run_at(home, &mut out);
    let registered_after_uninstall = broker_registered();

    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join("com.famp.broker.plist");
    let plist_exists = plist_path.exists();

    // Defensive cleanup BEFORE assertions: guarantees no leftover LaunchAgent
    // even if an assertion below panics.
    let _ = uninstall::run_at(home, &mut out);

    // Assertions.
    install1.expect("first install must succeed");
    install2.expect("second install must succeed (idempotent — DAEMON-01)");
    assert!(
        registered_after_install,
        "service must be registered after install (DAEMON-01)"
    );
    uninstall1.expect("first uninstall must succeed");
    uninstall2.expect("second uninstall must succeed (idempotent — DAEMON-04)");
    assert!(
        !registered_after_uninstall,
        "service must not be registered after uninstall (DAEMON-04)"
    );
    assert!(
        !plist_exists,
        "plist must be removed after uninstall (DAEMON-04); found at {}",
        plist_path.display()
    );
}
