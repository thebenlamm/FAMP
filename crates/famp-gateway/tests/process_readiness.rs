//! T-11-21 / review HIGH finding #5 (code half): the `famp-gateway`
//! "ready, backing N principal(s)" line must print ONLY after home
//! resolve, own-domain resolve/validate, signing-key load, peers-keyring
//! load, transport build, and the peer route-map have all succeeded — not
//! immediately after `registry.back()`, which is where it printed before
//! this plan (a premature "ready" that masked an unloaded keyring: a false
//! success signal, T-11-21's Repudiation entry).
//!
//! This spawns a real `famp-gateway` subprocess against a `FAMP_HOME`
//! whose `gateway/peers.keyring` file is deliberately absent —
//! `Keyring::load_from_file` fails on a missing file (`File::open`
//! returns `NotFound`) — and asserts the process exits non-zero WITHOUT
//! ever printing "ready" to stdout. A regression that moved (or removed)
//! the ordering fix would make this fail by printing "ready" before the
//! keyring load is even attempted.

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
fn ready_line_is_never_printed_when_peers_keyring_load_fails() {
    ensure_famp_bin_built();

    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    // Deliberately fresh `FAMP_HOME`: no `gateway/` directory, no
    // `peers.keyring` file. `load_or_generate` (signing key) creates
    // `gateway/` and succeeds; `Keyring::load_from_file` then fails on
    // the still-absent `peers.keyring` — this is the exact ordering the
    // "ready" line must land after.
    let home_tmp = tempfile::TempDir::new().unwrap();
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
        .arg("alice")
        .stdin(Stdio::null())
        .output()
        .expect("famp-gateway must be runnable as a subprocess");

    assert!(
        !out.status.success(),
        "a missing peers.keyring must be startup-fatal; got status {:?}, stdout={:?}, stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("ready"),
        "'ready' must NOT print before a fatal peers-keyring load failure; got stdout:\n{stdout}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("peers keyring"),
        "expected a peers-keyring-load failure message on stderr; got:\n{stderr}"
    );
}
