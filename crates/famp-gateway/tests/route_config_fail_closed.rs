//! Phase 11 Plan 08 Task 3 (F-3 + F-4 / INV-J / review section 6) —
//! adversarial startup-fail-closed coverage for `famp-gateway`'s route
//! configuration surface (`--peer`, `--backs`, and the bare-positional-name
//! fallback).
//!
//! All five cases spawn a real `famp-gateway` subprocess (mirroring
//! `process_readiness.rs`'s pattern) and assert on process exit status +
//! stderr content — these are startup-time failures, so there is no live
//! router/registry state to inspect, only the process's own diagnosis.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Startup deadline for broker socket, relaxed from 5s to accommodate parallel
/// test execution where multiple harnesses spawn brokers simultaneously.
/// See crates/famp-gateway/tests/common/gateway_harness.rs::STARTUP_DEADLINE.
const STARTUP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

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
    let start = std::time::Instant::now();
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

/// A `FAMP_HOME` with an already-empty `gateway/peers.keyring` pre-created
/// (mirrors `no_cross_talk.rs`'s setup) so every case in this file gets
/// PAST the keyring-load step and into (or past) the route-map
/// construction this plan changes, rather than failing earlier for an
/// unrelated reason.
fn home_with_empty_peers_keyring() -> tempfile::TempDir {
    let home = tempfile::TempDir::new().unwrap();
    let gateway_dir = home.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).unwrap();
    std::fs::write(gateway_dir.join("peers.keyring"), "").unwrap();
    home
}

/// Run `famp-gateway` with `extra_args` appended after the required
/// `--listen`/`--tls-cert`/`--tls-key`/`--socket` flags, returning its
/// captured output. Never dials the listener (`127.0.0.1:0`, ephemeral,
/// unused) — every case here is a route-config-time failure, so the
/// process should exit before `run_ingress` ever binds meaningfully.
fn run_gateway(sock: &Path, home: &Path, extra_args: &[&str]) -> std::process::Output {
    let fixtures = cross_machine_fixture_dir();
    Command::cargo_bin("famp-gateway")
        .unwrap()
        .arg("--socket")
        .arg(sock)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--tls-cert")
        .arg(fixtures.join("alice.crt"))
        .arg("--tls-key")
        .arg(fixtures.join("alice.key"))
        .env("FAMP_HOME", home)
        // 17-03/D-30: own-domain is now REQUIRED (own-domain-unset is
        // UNCONDITIONALLY startup-fatal) -- must resolve BEFORE the
        // route-config-time failures this file actually tests, or the
        // process would exit for the wrong reason.
        .env("FAMP_OWN_DOMAIN", "hosta.test")
        .args(extra_args)
        .stdin(Stdio::null())
        .output()
        .expect("famp-gateway must be runnable as a subprocess")
}

#[test]
fn duplicate_peer_domain_fails_startup() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let out = run_gateway(
        &sock,
        home.path(),
        &[
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--peer",
            "hostb.test=https://127.0.0.1:9444",
            "alice",
        ],
    );

    assert!(
        !out.status.success(),
        "duplicate --peer domain must be startup-fatal; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("duplicate domain") && stderr.contains("hostb.test"),
        "expected an actionable duplicate-domain message naming the domain; got: {stderr}"
    );
}

#[test]
fn duplicate_backs_principal_fails_startup() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let out = run_gateway(
        &sock,
        home.path(),
        &[
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--backs",
            "agent:hostb.test/bob",
            "--backs",
            "agent:hostb.test/bob",
            "alice",
        ],
    );

    assert!(
        !out.status.success(),
        "duplicate --backs principal must be startup-fatal; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("duplicate principal"),
        "expected an actionable duplicate-principal message; got: {stderr}"
    );
}

#[test]
fn bare_names_with_two_peers_fails_startup_with_actionable_message() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let out = run_gateway(
        &sock,
        home.path(),
        &[
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--peer",
            "hostc.test=https://127.0.0.1:9444",
            "bob",
        ],
    );

    assert!(
        !out.status.success(),
        "bare positional names with 2+ peers must be startup-fatal; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("ready"),
        "'ready' must NOT print before this startup-fatal ambiguity; got stdout:\n{stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--backs"),
        "expected the actionable 'use --backs' message; got: {stderr}"
    );
}

#[test]
fn bare_names_with_exactly_one_peer_still_starts_and_prints_ready() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let child = Command::cargo_bin("famp-gateway")
        .unwrap()
        .arg("--socket")
        .arg(&sock)
        .arg("--listen")
        .arg("127.0.0.1:0")
        .arg("--tls-cert")
        .arg(cross_machine_fixture_dir().join("alice.crt"))
        .arg("--tls-key")
        .arg(cross_machine_fixture_dir().join("alice.key"))
        .arg("--peer")
        .arg("hostb.test=https://127.0.0.1:9443")
        .env("FAMP_HOME", home.path())
        // 17-03/D-30: own-domain is now REQUIRED -- this test asserts
        // the process reaches "ready".
        .env("FAMP_OWN_DOMAIN", "hosta.test")
        .arg("bob")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut guard = ChildGuard::new(child);
    // Poll (bounded) for "ready" on stdout rather than waiting for exit —
    // this process is long-lived (it never exits on its own once ready).
    // ChildGuard's Drop kills + waits it once `guard` goes out of scope.
    let stdout = guard
        .as_mut()
        .and_then(|c| c.stdout.take())
        .expect("child stdout piped");
    let mut reader = BufReader::new(stdout);
    let mut saw_ready = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut line = String::new();
    while std::time::Instant::now() < deadline {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if line.contains("ready") {
                    saw_ready = true;
                    break;
                }
            }
        }
    }
    assert!(
        saw_ready,
        "bare positional names with exactly one --peer must still start and print 'ready'"
    );
}

/// REL-02 (12-02): the bare-positional-name + single-`--peer` fallback in
/// `build_route_map` used to build `agent:{domain}/{name}` and silently
/// SKIP the route (`if let Ok(principal) = ...`) whenever that string
/// failed to parse as a `Principal` -- no error, no route, and the
/// process still printed "ready" and kept running. A `--peer` domain is
/// validated only for non-emptiness (not as a legal Principal authority),
/// so an operator typo here produced a gateway that looks healthy but can
/// never actually relay for the backed name. Must be startup-fatal,
/// naming both the bad domain and the affected principal name, matching
/// every other route-configuration failure in this file.
#[test]
fn invalid_single_peer_domain_fails_startup_instead_of_silently_dropping_route() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let out = run_gateway(
        &sock,
        home.path(),
        &[
            "--peer",
            "not_a_valid_domain=https://127.0.0.1:9443",
            "alice",
        ],
    );

    assert!(
        !out.status.success(),
        "a --peer domain that cannot combine with a backed name into a valid \
         Principal must be startup-fatal, not silently produce a zero-route \
         gateway; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("ready"),
        "'ready' must NOT print for a --peer domain that cannot form a valid \
         route; got stdout:\n{stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not_a_valid_domain") && stderr.contains("alice"),
        "expected an actionable message naming both the invalid domain and \
         the backed name that failed to combine into a route; got: {stderr}"
    );
}

#[test]
fn backs_with_no_matching_peer_fails_startup() {
    ensure_famp_bin_built();
    let broker_tmp = tempfile::TempDir::new().unwrap();
    let sock = broker_tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);
    let home = home_with_empty_peers_keyring();

    let out = run_gateway(
        &sock,
        home.path(),
        &[
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--backs",
            "agent:hostc.test/carol",
            "alice",
        ],
    );

    assert!(
        !out.status.success(),
        "--backs with no matching --peer domain must be startup-fatal; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("hostc.test") && stderr.contains("no matching"),
        "expected a message naming the unmatched domain; got: {stderr}"
    );
}
