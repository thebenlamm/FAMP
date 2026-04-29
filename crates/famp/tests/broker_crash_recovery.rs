#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Plan 02-11 Task 2: TEST-03 — kill -9 broker recovery without message
// loss. The mailbox is on-disk JSONL (durability path D-04/D-07), so a
// broker process death between AppendMailbox and Reply MUST NOT lose
// envelopes. Register processes reconnect with bounded exponential
// backoff (RECONNECT_INITIAL=1s, cap=30s) and the spawn helper revives
// the broker on the next BusClient::connect call.
//
// Implementation note: this test uses a valid `AnyBusEnvelope`-shaped
// JSON value (the `audit_log` class) because Phase-1 D-09 makes the
// broker's drain path call `AnyBusEnvelope::decode`. The Phase-2
// `famp send` CLI emits a wire-CLI-shaped value (`{mode, summary,
// body, ...}`) that does not yet survive that decode — that contract
// is owned by plan 02-12. Until 02-12 lands, the only durable shape we
// can roundtrip across a broker kill+respawn is a real envelope, so
// the test pushes one via `BusMessage::Send` directly.

use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

/// Build a minimal valid `audit_log` envelope JSON value. The shape
/// matches `famp_envelope::bus::tests::audit_log_value` so it survives
/// `AnyBusEnvelope::decode` at drain time.
fn audit_log_envelope(marker: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": "01890000-0000-7000-8000-000000000099",
        "from": "agent:example.test/alice",
        "to": "agent:example.test/bob",
        "authority": "advisory",
        "ts": "2026-04-27T12:00:00Z",
        "body": { "event": marker }
    })
}

#[test]
fn test_kill9_recovery() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let env_key = "FAMP_BUS_SOCKET";

        // 1. Start alice and bob register holders. Each one shells out via
        //    Command::cargo_bin("famp"); the first to reach bind() wins
        //    the broker race per BROKER-03.
        let mut alice = Command::cargo_bin("famp")
            .unwrap()
            .env(env_key, &sock)
            .args(["register", "alice"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut bob = Command::cargo_bin("famp")
            .unwrap()
            .env(env_key, &sock)
            .args(["register", "bob"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Allow both holders to register and the broker to settle.
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 2. Push a valid audit_log envelope into bob's mailbox via the
        //    proxy shape (`bind_as: Some("alice")`). The broker validates
        //    alice is held by a live registered holder at Hello time,
        //    encodes the envelope, and appends to bob's mailbox file.
        {
            let mut proxy = famp::bus_client::BusClient::connect(&sock, Some("alice".into()))
                .await
                .expect("alice proxy connect");
            let reply = proxy
                .send_recv(famp_bus::BusMessage::Send {
                    to: famp_bus::Target::Agent { name: "bob".into() },
                    envelope: audit_log_envelope("kill9-marker"),
                })
                .await
                .expect("send_recv");
            match reply {
                famp_bus::BusReply::SendOk { .. } => {}
                other => panic!("expected SendOk, got {other:?}"),
            }
            proxy.shutdown().await;
        }

        // 3. Find the broker pid via pgrep. The broker's argv contains
        //    `famp broker --socket <path>` per spawn::spawn_broker_if_absent.
        let pgrep = Command::new("pgrep")
            .args(["-f", &format!("famp broker --socket {}", sock.display())])
            .output()
            .unwrap();
        let pid_str = String::from_utf8_lossy(&pgrep.stdout);
        let broker_pid: i32 = pid_str
            .lines()
            .next()
            .and_then(|s| s.trim().parse().ok())
            .expect("broker pid must be findable via pgrep");

        // 4. SIGKILL the broker (no graceful shutdown — simulates `kill -9`).
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(broker_pid),
            nix::sys::signal::Signal::SIGKILL,
        )
        .unwrap();

        // 5. Wait for the alice/bob register processes to detect the
        //    disconnect (BusClient::wait_for_disconnect surfaces EOF on
        //    the held UnixStream), sleep RECONNECT_INITIAL (1s), and
        //    re-spawn the broker via spawn_broker_if_absent on the next
        //    BusClient::connect. Allow generous headroom (10s) for both
        //    holders to be reliably re-registered before we probe;
        //    macOS process spawn is slower than Linux.
        tokio::time::sleep(Duration::from_secs(10)).await;

        // 6. Verify the message survived: drain bob's mailbox via a fresh
        //    `bind_as: Some("bob")` proxy + `BusMessage::Inbox`. The
        //    mailbox JSONL persisted across the kill; the new broker
        //    reads it on Inbox-time drain (D-09) and surfaces the typed
        //    envelopes.
        let mut probe = famp::bus_client::BusClient::connect(&sock, Some("bob".into()))
            .await
            .expect("bob proxy connect after recovery");
        let reply = probe
            .send_recv(famp_bus::BusMessage::Inbox {
                since: None,
                include_terminal: None,
            })
            .await
            .expect("inbox send_recv after recovery");
        probe.shutdown().await;

        match reply {
            famp_bus::BusReply::InboxOk { envelopes, .. } => {
                let serialized = serde_json::to_string(&envelopes).unwrap();
                assert!(
                    serialized.contains("kill9-marker"),
                    "bob's inbox must contain kill9-marker after broker kill+recovery (TEST-03); \
                     got: {serialized}"
                );
            }
            other => panic!("expected InboxOk after recovery, got {other:?}"),
        }

        // Tear down. The register reconnect loop is still active; killing
        // the alice/bob processes terminates them cleanly.
        let _ = alice.kill();
        let _ = bob.kill();
        let _ = alice.wait();
        let _ = bob.wait();
    });
}
