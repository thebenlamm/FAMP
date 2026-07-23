#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Phase 07 Plan 03 Task 1: LIVE-02 — a real `famp-gateway` process
// backing N proxied principals keeps them ALL live in
// `famp inspect identities` for as long as it runs, and SIGKILLing it
// reaps ALL N within one broker sweep interval (~1s `TICK_INTERVAL`,
// `crates/famp/src/cli/broker/mod.rs:53`), leaving no orphan holders.
// This subprocess-level assertion also covers LIVE-01 (principals stay
// live across the sweep while the gateway runs).
//
// Design A (07-RESEARCH.md): each proxied principal is a plain
// `Register` carrying the gateway's own PID on its own UDS connection.
// The broker's existing `kill(pid,0)` sweep (unmodified) reaps every
// connection sharing that PID the instant the gateway process dies —
// this test proves that fact against a REAL OS process, not a
// pure-`Broker<E>` unit double (that's plan 07-02's job).
//
// Shape copied from `crates/famp/tests/broker_proxy_semantics.rs`
// (07-PATTERNS.md): `Command::cargo_bin`, `ChildGuard`-wrapped children,
// poll-with-deadline (NEVER a fixed `sleep()`-then-assert — 07-RESEARCH.md
// Pitfall 4).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use famp_inspect_proto::IdentityListReply;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// `Command::cargo_bin("famp")` resolves the sibling `famp` binary via
/// assert_cmd's `legacy_cargo_bin` fallback (a shared-workspace
/// `target/debug/` lookup) rather than the `CARGO_BIN_EXE_famp` env var:
/// Cargo does NOT propagate that var — at compile time OR runtime — to a
/// *different* package's test binary (verified empirically 2026-07-23;
/// `crates/famp/tests/broker_proxy_semantics.rs` only "just works" today
/// because `-p famp` always builds famp's own `[[bin]]` as a side effect
/// of testing that package). `famp-gateway`'s tests cross that package
/// boundary, so `cargo test -p famp-gateway --test liveness` alone, on a
/// clean checkout with no prior `-p famp` build, would panic with
/// `CARGO_BIN_EXE_famp is unset`. Build famp's bin explicitly first so
/// this test is hermetic regardless of invocation order.
fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

/// Spawn a bare `famp broker --socket <path>` daemon subprocess,
/// ChildGuard-wrapped so a panicking test still reaps it.
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

/// Poll (bounded by `deadline`) until the broker's UDS socket accepts a
/// raw connection. `BusClient::connect_no_spawn` (the gateway's own
/// connect path — 07-01-PLAN.md) makes exactly ONE connect attempt with
/// no retry/backoff, unlike the CLI's `connect()`; spawning the gateway
/// before the broker has bound its socket would make the gateway exit(1)
/// immediately on `NotFound`/`ConnectionRefused`, which would look
/// identical to a genuine registration failure. Confirm the socket is up
/// first so a real bug isn't masked by a race.
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

/// Spawn a `famp-gateway --socket <path> <name>...` subprocess backing
/// every name in `names`, ChildGuard-wrapped.
fn spawn_gateway_subprocess(sock: &Path, names: &[&str]) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp-gateway")
            .unwrap()
            .arg("--socket")
            .arg(sock)
            .args(names)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Run `famp inspect identities --json` against `sock` and return the
/// live identity names, or `None` if the call failed (broker not yet up,
/// or an inspect budget-exceeded transient) — callers poll on `None`.
fn live_identity_names(sock: &Path) -> Option<Vec<String>> {
    let output = Command::cargo_bin("famp")
        .ok()?
        .env("FAMP_BUS_SOCKET", sock)
        .args(["inspect", "identities", "--json"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let list: IdentityListReply = serde_json::from_slice(&output.stdout).ok()?;
    Some(list.rows.into_iter().map(|r| r.name).collect())
}

/// Poll (bounded by `deadline`, ~100ms backoff) until every name in
/// `expect_live` is present among the live identities. NEVER a fixed
/// `sleep()`-then-assert (07-RESEARCH.md Pitfall 4) — this is the
/// falsification control: the test would fail here if Design A's
/// registration path were broken, proving the positive side actually
/// happened before we ever kill anything.
fn poll_until_all_live(sock: &Path, expect_live: &[&str], deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(live) = live_identity_names(sock) {
            if expect_live.iter().all(|n| live.iter().any(|l| l == n)) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for {expect_live:?} to appear live"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Poll (bounded by `deadline`) until every name in `expect_gone` is
/// ABSENT from the live identities — i.e. the broker's sweep reaped it.
/// A name left behind past the deadline is an orphan holder (LIVE-02
/// failure).
fn poll_until_all_gone(sock: &Path, expect_gone: &[&str], deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(live) = live_identity_names(sock) {
            if expect_gone.iter().all(|n| !live.iter().any(|l| l == n)) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for {expect_gone:?} to be reaped (orphan holder)"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn live02_gateway_exit_reaps_all_principals() {
    ensure_famp_bin_built();

    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, Duration::from_secs(5));
    let mut gateway = spawn_gateway_subprocess(&sock, &["alice", "bob"]);

    // Falsification control: BOTH principals must be observed live
    // BEFORE we ever touch the gateway process. A test that skipped
    // this and went straight to SIGKILL+poll would trivially "pass" even
    // if registration were broken (never-live counts as "absent" too).
    poll_until_all_live(&sock, &["alice", "bob"], Duration::from_secs(5));

    // SIGKILL the gateway directly (`Child::kill` sends SIGKILL on
    // Unix) — no graceful disconnect frame. The broker only learns the
    // gateway is gone via its periodic Tick -> is_alive(pid) sweep.
    if let Some(child) = gateway.as_mut() {
        child.kill().unwrap();
        child.wait().unwrap();
    }

    // Bounded deadline generously exceeding one ~1s TICK_INTERVAL sweep.
    poll_until_all_gone(&sock, &["alice", "bob"], Duration::from_secs(5));
}
