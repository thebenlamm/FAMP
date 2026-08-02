//! 17-03/D-30/INGR-04: own-domain is now UNCONDITIONALLY startup-fatal for
//! every `famp-gateway` process (`main.rs::resolve_own_domain_or_exit`) —
//! prior to this coverage, every claim that an own-domain-unset gateway
//! fails to start was source-level only (a reading of
//! `resolve_own_domain_or_exit`'s every branch calling
//! `std::process::exit(1)`), never proven against a real subprocess.
//!
//! This spawns a real `famp-gateway` subprocess against a `FAMP_HOME` with
//! no `own-domain` file, and with `FAMP_OWN_DOMAIN` explicitly unset
//! (`.env_remove`, hermetic regardless of the outer test process's own
//! environment), and asserts the process exits non-zero with stderr naming
//! the missing configuration. Mirrors `process_readiness.rs`'s and
//! `route_config_fail_closed.rs`'s spawn/assert pattern (real broker +
//! `Command::cargo_bin("famp-gateway").output()`, no `ChildGuard` needed
//! for the gateway itself since it's a one-shot process that exits on its
//! own).

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Startup deadline for broker socket, relaxed from 5s to accommodate parallel
/// test execution where multiple harnesses spawn brokers simultaneously.
/// See crates/famp-gateway/tests/common/gateway_harness.rs::STARTUP_DEADLINE.
const STARTUP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// See `liveness.rs`'s identically-named helper: `famp-gateway`'s test
/// binary crosses a package boundary from `famp`'s own `[[bin]]`, which
/// Cargo does not build as a side effect of `-p famp-gateway` alone.
fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

fn spawn_broker_subprocess(sock: &Path) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .args(["broker", "--socket", sock.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn wait_for_broker_socket(sock: &Path, deadline: Duration) {
    let start = Instant::now();
    loop {
        if std::os::unix::net::UnixStream::connect(sock).is_ok() {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "broker socket at {} never came up within {deadline:?}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn cross_machine_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("famp")
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}

#[test]
fn gateway_exits_nonzero_when_own_domain_is_unset() {
    ensure_famp_bin_built();

    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    // Deliberately fresh `FAMP_HOME`: no `own-domain` file. Combined with
    // `.env_remove("FAMP_OWN_DOMAIN")` below, own-domain resolution
    // (`famp::cli::own_domain::resolve_own_domain`) has neither of its two
    // sources available and must fail.
    let home_tmp = tempfile::TempDir::new().unwrap();
    assert!(
        !home_tmp.path().join("own-domain").exists(),
        "precondition: a fresh FAMP_HOME must have no own-domain file"
    );
    let fixtures = cross_machine_fixture_dir();

    let out = Command::cargo_bin("famp-gateway")
        .unwrap()
        .arg("--socket")
        .arg(&sock)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--tls-cert")
        .arg(fixtures.join("alice.crt"))
        .arg("--tls-key")
        .arg(fixtures.join("alice.key"))
        .env("FAMP_HOME", home_tmp.path())
        .env_remove("FAMP_OWN_DOMAIN")
        .arg("alice")
        .stdin(Stdio::null())
        .output()
        .expect("famp-gateway must be runnable as a subprocess");

    assert!(
        !out.status.success(),
        "an own-domain-unset gateway must be startup-fatal; got status {:?}, stdout={:?}, \
         stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("ready"),
        "'ready' must NOT print before the fatal own-domain resolution failure; got \
         stdout:\n{stdout}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("own-domain not set") && stderr.contains("FAMP_OWN_DOMAIN"),
        "expected an own-domain-not-set failure message naming FAMP_OWN_DOMAIN on stderr; \
         got:\n{stderr}"
    );
}
