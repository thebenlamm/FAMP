//! T-11-21 / review HIGH finding #5 (code half): the `famp-gateway`
//! "ready, backing N principal(s)" line must print ONLY after home
//! resolve, own-domain resolve/validate, signing-key load, peers-keyring
//! load, transport build, and the peer route-map have all succeeded — not
//! immediately after `registry.back()`, which is where it printed before
//! this plan (a premature "ready" that masked an unloaded keyring: a false
//! success signal, T-11-21's Repudiation entry).
//!
//! A missing `peers.keyring` is a legal empty keyring (issue #42). The
//! fail-closed ready-ordering proof therefore uses a *corrupt* keyring
//! file: load must fail, the process must exit non-zero, and "ready"
//! must never print. A separate test pins the first-run missing-file
//! path: the process stays up and prints ready.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Startup deadline for broker socket, relaxed from 5s to accommodate parallel
/// test execution where multiple harnesses spawn brokers simultaneously.
/// See crates/famp-gateway/tests/common/gateway_harness.rs::STARTUP_DEADLINE.
const STARTUP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
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

fn gateway_cmd(sock: &Path, home: &Path) -> Command {
    let fixtures = cross_machine_fixture_dir();
    let mut cmd = Command::cargo_bin("famp-gateway").unwrap();
    cmd.arg("--socket")
        .arg(sock)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--tls-cert")
        .arg(fixtures.join("alice.crt"))
        .arg("--tls-key")
        .arg(fixtures.join("alice.key"))
        .env("FAMP_HOME", home)
        .env("FAMP_OWN_DOMAIN", "hosta.test")
        .arg("alice")
        .stdin(Stdio::null());
    cmd
}

#[test]
fn missing_peers_keyring_is_empty_and_prints_ready() {
    ensure_famp_bin_built();

    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    // Fresh FAMP_HOME: no gateway/ directory, no peers.keyring. Signing-key
    // load_or_generate creates gateway/; the missing keyring must then
    // load as empty (issue #42), not exit 1.
    let home_tmp = tempfile::TempDir::new().unwrap();
    let mut child = gateway_cmd(&sock, home_tmp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("famp-gateway must be runnable as a subprocess");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let mut guard = ChildGuard::new(child);

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.contains("famp-gateway: ready,") {
                let _ = tx.send(line);
                break;
            }
        }
    });

    // Watcher, not a drain-to-EOF: collecting all of stderr would block
    // until the still-running gateway exits.
    let (etx, erx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.contains("failed to load peers keyring") {
                let _ = etx.send(line);
                break;
            }
        }
    });

    let ready = rx.recv_timeout(STARTUP_DEADLINE).unwrap_or_else(|_| {
        panic!("missing peers.keyring must print ready within {STARTUP_DEADLINE:?}")
    });
    assert!(
        ready.contains("famp-gateway: ready,"),
        "expected a ready line, got {ready:?}"
    );

    // Ready prints before ingress bind. A sub-millisecond try_wait would
    // miss a gateway that printed ready and then died.
    thread::sleep(Duration::from_millis(500));
    assert!(
        guard
            .as_mut()
            .expect("child still held")
            .try_wait()
            .unwrap()
            .is_none(),
        "gateway must stay up after loading a missing peers.keyring as empty"
    );
    assert!(
        erx.try_recv().is_err(),
        "missing keyring must not emit a peers-keyring load failure"
    );
}

#[test]
fn ready_line_is_never_printed_when_peers_keyring_load_fails() {
    ensure_famp_bin_built();

    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    // Corrupt file, not a missing one: T-11-21's ready-ordering proof
    // needs a load that actually fails. Grammar-invalid content is
    // MalformedEntry, which stays startup-fatal.
    let home_tmp = tempfile::TempDir::new().unwrap();
    let gateway_dir = home_tmp.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).unwrap();
    std::fs::write(gateway_dir.join("peers.keyring"), "this is not a keyring\n").unwrap();

    let out = gateway_cmd(&sock, home_tmp.path())
        .output()
        .expect("famp-gateway must be runnable as a subprocess");

    assert!(
        !out.status.success(),
        "a corrupt peers.keyring must be startup-fatal; got status {:?}, stdout={:?}, stderr={:?}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("famp-gateway: ready,"),
        "'ready' must NOT print before a fatal peers-keyring load failure; got stdout:\n{stdout}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("peers keyring"),
        "expected a peers-keyring-load failure message on stderr; got:\n{stderr}"
    );
}
