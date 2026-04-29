#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Plan 02-11 Task 3: D-10 proxy invariants at the OS+wire level.
//
// D-10 (CONTEXT.md): identity binding is a connection property promoted
// to `Hello.bind_as: Option<String>`. A proxy (`bind_as = Some(name)`)
// connection is read/write-through to the canonical live registered
// holder of `name`. The broker validates the holder is live at Hello
// time and on every identity-required op (per-op liveness re-check).
//
// These tests are wire-level (BusClient + BusMessage) rather than
// CLI-level because the canonical `famp join` / `famp sessions` /
// `famp whoami` subcommands are introduced by plan 02-07 (parallel
// wave) and are not yet on this branch base. The invariants under
// test live on the broker, not the CLI surface, so wire-level coverage
// is faithful to the D-10 contract.
//
// Three invariants:
//
//   1. test_proxy_join_persists_after_disconnect: a proxy that does
//      `Join(#x)` then disconnects MUST NOT remove the canonical
//      holder from #x. The broker mutates the canonical holder's
//      `joined` set, not the short-lived proxy's.
//
//   2. test_proxy_inbox_unregistered_fails: connecting with
//      `bind_as = Some("alice")` when no `famp register alice` is
//      live MUST fail at Hello with HelloErr{NotRegistered}.
//
//   3. test_proxy_send_after_holder_dies: if alice is registered, then
//      alice's holder process is SIGKILLed (no graceful disconnect
//      frame), a subsequent proxy connect with `bind_as = "alice"`
//      MUST fail. The broker's 1-second Tick + per-op liveness
//      re-check catches the dead holder.

use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::{BusClient, BusClientError};
use famp_bus::{BusErrorKind, BusMessage, BusReply, Target};

/// Helper: ensure a `famp register <name>` foreground process is
/// running and registered with the broker. Returns the spawned child
/// so the caller can kill/wait it.
fn spawn_register(sock: &std::path::Path, name: &str) -> std::process::Child {
    Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", sock)
        .args(["register", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Helper: spawn a `famp broker --socket <path>` daemon directly (not
/// via `spawn_broker_if_absent`, because that helper uses
/// `std::env::current_exe()` which inside an integration test resolves
/// to the test binary — which has no `broker` subcommand). Returns
/// the spawned child.
fn spawn_broker_subprocess(sock: &std::path::Path) -> std::process::Child {
    Command::cargo_bin("famp")
        .unwrap()
        .args(["broker", "--socket", sock.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// D-10 invariant 1: a proxy connection joining a channel mutates the
/// canonical holder's `joined` set, not the proxy's. The proxy
/// disconnect (one-shot CLI exits) MUST NOT remove the holder from
/// the channel.
#[test]
fn test_proxy_join_persists_after_disconnect() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");

        // 1. Start alice as canonical holder.
        let mut alice = spawn_register(&sock, "alice");
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 2. Open a one-shot proxy connection (bind_as: Some("alice"))
        //    — this is what `famp join --as alice #planning` would do.
        //    Send Join, observe JoinOk, then DROP the connection (the
        //    broker observes Disconnect on its end).
        {
            let mut proxy = BusClient::connect(&sock, Some("alice".into()))
                .await
                .expect("alice proxy connect");
            let reply = proxy
                .send_recv(BusMessage::Join {
                    channel: "#planning".into(),
                })
                .await
                .expect("join send_recv");
            match reply {
                BusReply::JoinOk { channel, .. } => assert_eq!(channel, "#planning"),
                other => panic!("expected JoinOk, got {other:?}"),
            }
            proxy.shutdown().await;
            // proxy drops here; broker_rx receives Disconnect.
        }

        // Yield enough so the broker observes the proxy disconnect
        // BEFORE we re-probe via a new proxy.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 3. Re-probe via a fresh proxy: alice MUST still be a member
        //    of #planning. We use Whoami (proxy → broker returns the
        //    canonical holder's joined set per D-10).
        let mut probe = BusClient::connect(&sock, Some("alice".into()))
            .await
            .expect("alice probe proxy connect");
        let reply = probe
            .send_recv(BusMessage::Whoami {})
            .await
            .expect("whoami send_recv");
        probe.shutdown().await;

        match reply {
            BusReply::WhoamiOk { active, joined } => {
                assert_eq!(
                    active.as_deref(),
                    Some("alice"),
                    "active identity must be alice"
                );
                assert!(
                    joined.iter().any(|c| c == "#planning"),
                    "alice MUST still be in #planning after proxy disconnect (D-10): joined={joined:?}"
                );
            }
            other => panic!("expected WhoamiOk, got {other:?}"),
        }

        let _ = alice.kill();
        let _ = alice.wait();
    });
}

/// D-10 invariant 2: connecting with `bind_as = Some("alice")` when no
/// `famp register alice` is live MUST fail at Hello with
/// HelloErr{NotRegistered}.
#[test]
fn test_proxy_inbox_unregistered_fails() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");

        // No `famp register alice` is running. Start a bare broker
        // explicitly (we cannot rely on spawn_broker_if_absent because
        // current_exe() inside the test binary is the test, not famp).
        let mut broker = spawn_broker_subprocess(&sock);
        // Poll for socket up.
        for _ in 0..20 {
            if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Connect with bind_as = Some("alice") — MUST fail at Hello
        // because alice is not held by any live registered process.
        let result = BusClient::connect(&sock, Some("alice".into())).await;

        match result {
            Err(BusClientError::HelloFailed { kind, .. }) => {
                assert!(
                    matches!(kind, BusErrorKind::NotRegistered),
                    "Hello must fail with NotRegistered (D-10), got: {kind:?}"
                );
            }
            Err(other) => panic!(
                "expected HelloFailed{{NotRegistered}}, got error: {other:?}"
            ),
            Ok(_) => panic!(
                "BusClient::connect MUST fail when bind_as references an unregistered holder (D-10)"
            ),
        }

        let _ = broker.kill();
        let _ = broker.wait();
    });
}

/// D-10 invariant 3: if alice is registered, then alice's holder
/// process is `SIGKILL`ed without a clean disconnect frame, a
/// subsequent proxy connect with `bind_as = "alice"` MUST fail. The
/// broker's 1-second Tick + per-op liveness re-check catches the dead
/// holder.
#[test]
fn test_proxy_send_after_holder_dies() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");

        // 1. Start alice (and bob, so the broker has another live client
        //    to keep it from idle-exiting during the wait below).
        let mut alice = spawn_register(&sock, "alice");
        let mut bob = spawn_register(&sock, "bob");
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 2. SIGKILL alice's register process. No graceful disconnect
        //    frame is sent; the broker only learns alice is gone via
        //    its periodic Tick → is_alive(pid) check.
        // PID is positive; cast is safe on every supported target.
        #[allow(clippy::cast_possible_wrap)]
        let alice_pid = alice.id() as i32;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(alice_pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();
        let _ = alice.wait();

        // 3. Wait for the broker to run at least one or two Tick
        //    iterations (1s interval). 3s is generous.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // 4. Proxy-connect with bind_as = "alice" — MUST fail at Hello.
        //    The broker's hello() handler probes for a live holder of
        //    "alice"; the Tick disconnect has already removed alice's
        //    ClientState OR the holder's pid no longer passes is_alive.
        //    Either way the result is HelloErr{NotRegistered}.
        let result = BusClient::connect(&sock, Some("alice".into())).await;
        match result {
            Err(BusClientError::HelloFailed { kind, .. }) => {
                assert!(
                    matches!(kind, BusErrorKind::NotRegistered),
                    "Hello must fail with NotRegistered after holder dies (D-10), got: {kind:?}"
                );
            }
            Err(other) => panic!(
                "expected HelloFailed{{NotRegistered}}, got error: {other:?}"
            ),
            Ok(mut client) => {
                // If we got past Hello, a per-op liveness re-check on
                // Send must catch it. Either path satisfies D-10.
                let reply = client
                    .send_recv(BusMessage::Send {
                        to: Target::Agent { name: "bob".into() },
                        envelope: serde_json::json!({"mode": "ghost"}),
                    })
                    .await
                    .expect("send_recv");
                match reply {
                    BusReply::Err {
                        kind: BusErrorKind::NotRegistered,
                        ..
                    } => {}
                    other => panic!(
                        "Hello unexpectedly succeeded; per-op liveness re-check did NOT \
                         reject Send with NotRegistered (D-10), got: {other:?}"
                    ),
                }
            }
        }

        let _ = bob.kill();
        let _ = bob.wait();
    });
}
