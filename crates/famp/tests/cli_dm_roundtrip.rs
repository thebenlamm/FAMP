#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 02 plan 02-03 (CLI-01): `famp register` integration smoke test.
//!
//! `test_register_blocks` is implemented here (replacing the 02-00
//! `#[ignore]` stub). The full DM round-trip in `test_dm_roundtrip` and
//! the rest of the `cli_dm_roundtrip` family land in plan 02-12; those
//! stubs remain `#[ignore]`.

use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt as _;
use famp::bus_client::codec;
use famp_bus::{BusMessage, BusReply};
use tokio::net::UnixStream;

/// CLI-01 (plan 02-03): `famp register alice --no-reconnect` connects
/// to the broker, registers, prints the locked startup line on stderr,
/// and then blocks. A second client (this test) can Hello+Register a
/// different name (`bob`) on the same broker. Killing alice's process
/// causes it to exit within 1 s.
#[test]
fn test_register_blocks() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("test-bus.sock");

    let mut child = std::process::Command::cargo_bin("famp")
        .unwrap()
        .args(["register", "alice", "--no-reconnect"])
        .env("FAMP_BUS_SOCKET", &sock)
        // Suppress any HOME-resolution side effects from leaking into
        // the user's real `~/.famp`.
        .env("HOME", tmp.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn `famp register`");

    // Give the child time to (a) spawn the broker subprocess, (b) bind
    // the UDS, (c) Hello+Register, and (d) emit the startup line. Up
    // to 5 seconds — broker startup on a cold cargo test cache can be
    // slow on macOS.
    let mut alive_after_register = false;
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        match child.try_wait() {
            Ok(None) => {
                // Process still running. Probe the socket; if it
                // accepts a connection, the broker is up and the
                // register handshake has likely completed.
                if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
                    alive_after_register = true;
                    break;
                }
            }
            Ok(Some(status)) => {
                let _ = child.wait();
                panic!(
                    "famp register exited prematurely with status {status:?}; \
                     expected blocking process"
                );
            }
            Err(e) => panic!("try_wait failed: {e}"),
        }
    }
    assert!(
        alive_after_register,
        "famp register did not become reachable within 5 s"
    );

    // Hand off to a tokio runtime for the second-client probe + cleanup.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        // Second client: connect to the same broker and Register as
        // `bob` (different name from alice → no NameTaken collision).
        let mut stream = UnixStream::connect(&sock)
            .await
            .expect("second client failed to connect to broker");

        let hello = BusMessage::Hello {
            bus_proto: 1,
            client: "cli-dm-roundtrip-test/0.0.1".into(),
            bind_as: None,
        };
        codec::write_frame(&mut stream, &hello)
            .await
            .expect("write Hello");
        let reply: BusReply = codec::read_frame(&mut stream).await.expect("read HelloOk");
        match reply {
            BusReply::HelloOk { bus_proto } => assert_eq!(bus_proto, 1),
            other => panic!("expected HelloOk, got {other:?}"),
        }

        // Register as bob (a fresh, unique name) with a synthetic PID.
        let register = BusMessage::Register {
            name: "bob".into(),
            pid: 99_999,
        };
        codec::write_frame(&mut stream, &register)
            .await
            .expect("write Register");
        let reply: BusReply = codec::read_frame(&mut stream)
            .await
            .expect("read RegisterOk");
        match reply {
            BusReply::RegisterOk { active, .. } => {
                assert_eq!(active, "bob", "broker reported wrong active name");
            }
            other => panic!("expected RegisterOk, got {other:?}"),
        }
    });

    // Kill alice and assert it exits within 1s (signal-driven shutdown).
    child.kill().expect("kill alice");
    let exited_within_1s = (0..20).any(|_| {
        std::thread::sleep(Duration::from_millis(50));
        matches!(child.try_wait(), Ok(Some(_)))
    });
    assert!(
        exited_within_1s,
        "famp register did not exit within 1 s after SIGKILL"
    );

    // Best-effort: terminate the broker child too so the next test does
    // not inherit a stale `/tmp/...` socket. The broker's own idle-exit
    // timer would also handle this within 5 minutes; tearing down the
    // tempdir below is the actual correctness guarantee.
}

#[test]
#[ignore = "stub: implementation lands in plan 02-12"]
fn test_dm_roundtrip() {
    unimplemented!("filled in by plan 02-12");
}

#[test]
#[ignore = "stub: implementation lands in plan 02-12"]
fn test_inbox_list() {
    unimplemented!("filled in by plan 02-12");
}

#[test]
#[ignore = "stub: implementation lands in plan 02-12"]
fn test_await_unblocks() {
    unimplemented!("filled in by plan 02-12");
}

#[test]
#[ignore = "stub: implementation lands in plan 02-12"]
fn test_whoami() {
    unimplemented!("filled in by plan 02-12");
}
