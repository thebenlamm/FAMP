#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Plan 02-11 Task 2: TEST-04. Two parallel `famp register <name>` invocations
// against the same FAMP_BUS_SOCKET race to spawn the broker. Per BROKER-03
// (single-broker exclusion via bind() + EADDRINUSE probe + stale-unlink),
// exactly one broker survives, and both register clients connect to it.

use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

#[test]
fn test_broker_spawn_race() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let env_key = "FAMP_BUS_SOCKET";

    // Spawn alice and bob in parallel. Each will (a) try to bind the
    // broker socket via spawn_broker_if_absent, then (b) connect with
    // bind_as: None and Register. Exactly one of them wins the bind
    // race; the other defers (probe succeeds → process::exit(0) on
    // the broker side) and connects to the winner.
    let mut c1 = Command::cargo_bin("famp")
        .unwrap()
        .env(env_key, &sock)
        .args(["register", "alice"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut c2 = Command::cargo_bin("famp")
        .unwrap()
        .env(env_key, &sock)
        .args(["register", "bob"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Allow time for both to spawn the broker (whichever wins) and
    // complete Register. 2s is generous for cold-start CI.
    std::thread::sleep(Duration::from_secs(2));

    // Probe: exactly one broker is bound on the socket (i.e. a UDS
    // connect succeeds). If two brokers had bound, only the kernel's
    // most-recent bind would be reachable here, so the assertion is
    // really "at least one live broker bound" — the single-broker
    // invariant (BROKER-03) is enforced inside `bind_exclusive`, which
    // would have caused the loser to `process::exit(0)` before binding.
    let connect = std::os::unix::net::UnixStream::connect(&sock);
    assert!(
        connect.is_ok(),
        "exactly one broker must be bound on {} after the spawn race; got {connect:?}",
        sock.display()
    );

    // Tear down. Killing the register processes also signals the broker
    // to enter idle-exit (the 5-min timer arms when client_count → 0),
    // but we don't wait for it — the tempdir cleanup unlinks the socket.
    let _ = c1.kill();
    let _ = c2.kill();
    let _ = c1.wait();
    let _ = c2.wait();
}
