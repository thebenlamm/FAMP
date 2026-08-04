#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 19 plan 19-02: real-socket proof that only Local-origin records
//! satisfy a parked Await while held remote records remain explicitly readable.

use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::BusClient;
use famp::cli::await_cmd::{run_at_structured as await_run_at_structured, AwaitArgs};
use famp::cli::inbox::list::{run_at_structured as inbox_run_at_structured, ListArgs};
use famp_bus::{BusMessage, BusReply, Origin, Target};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

fn spawn_broker(sock: &std::path::Path) -> ChildGuard {
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

fn spawn_register(sock: &std::path::Path, name: &str) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", sock)
            .args(["register", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

async fn wait_for_registration(sock: &std::path::Path, name: &str) {
    for _ in 0..50 {
        if let Ok(mut client) = BusClient::connect(sock, Some(name.into())).await {
            client.shutdown().await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{name} failed to register within 5s");
}

async fn wait_for_socket(sock: &std::path::Path) {
    for _ in 0..50 {
        if std::os::unix::net::UnixStream::connect(sock).is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("broker socket {} never came up", sock.display());
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn marker() -> String {
    format!("MARK-{}", uuid::Uuid::now_v7().simple())
}

fn valid_envelope(sender: &str, recipient: &str, body_marker: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": uuid::Uuid::now_v7().to_string(),
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-08-04T12:00:00Z",
        "body": {"event": "famp.send.deliver", "details": {"msg": body_marker}},
    })
}

fn assert_agent_delivery(reply: BusReply, expected_woken: bool) {
    let BusReply::SendOk { delivered, .. } = reply else {
        panic!("expected SendOk reply")
    };
    assert_eq!(delivered.len(), 1, "DM send must have one delivery row");
    let row = &delivered[0];
    assert_eq!(
        row.to,
        Target::Agent { name: "bob".into() },
        "delivery row must describe Bob's mailbox"
    );
    assert!(row.ok, "broker must persist the record successfully");
    assert_eq!(
        row.woken, expected_woken,
        "broker-reported wake state must match origin eligibility"
    );
}

#[test]
fn remote_is_held_until_local_wakes_and_remains_in_inbox() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        let sock_for_await = sock.clone();
        let await_task = tokio::spawn(async move {
            await_run_at_structured(
                &sock_for_await,
                AwaitArgs {
                    timeout: humantime::Duration::from(Duration::from_secs(10)),
                    task: None,
                    act_as: Some("bob".into()),
                    abort_on_fd: None,
                },
            )
            .await
            .expect("bob await")
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        let mut remote = BusClient::connect(&sock, None).await.unwrap();
        let register_reply = remote
            .send_recv(BusMessage::Register {
                name: "remote".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: Some(Origin::Gateway),
            })
            .await
            .unwrap();
        assert!(matches!(register_reply, BusReply::RegisterOk { .. }));

        let remote_marker = marker();
        let remote_envelope = valid_envelope("remote", "bob", &remote_marker);
        let remote_id = remote_envelope["id"].as_str().unwrap().to_owned();
        let remote_reply = remote
            .send_recv(BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: remote_envelope,
            })
            .await
            .unwrap();

        // This broker-owned signal precedes all Await rendering and prevents
        // a client-side result filter from satisfying the proof.
        assert_agent_delivery(remote_reply, false);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !await_task.is_finished(),
            "Gateway traffic must leave the already-parked Await pending"
        );

        // The normal CLI registration path declares the sender Local; its
        // proxy connection is the production one-shot send shape.
        let _local = spawn_register(&sock, "local");
        wait_for_registration(&sock, "local").await;
        let mut local_proxy = BusClient::connect(&sock, Some("local".into()))
            .await
            .unwrap();
        let local_marker = marker();
        let local_envelope = valid_envelope("local", "bob", &local_marker);
        let local_id = local_envelope["id"].as_str().unwrap().to_owned();
        let local_reply = local_proxy
            .send_recv(BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: local_envelope.clone(),
            })
            .await
            .unwrap();
        assert_agent_delivery(local_reply, true);

        let outcome = tokio::time::timeout(Duration::from_secs(10), await_task)
            .await
            .expect("Local send must finish Await within 10s")
            .expect("Await task must not panic");
        assert_eq!(
            outcome.envelopes.len(),
            1,
            "Await must return exactly the eligible Local record"
        );
        let received = &outcome.envelopes[0];
        assert_eq!(received["id"], local_id);
        assert_eq!(received["origin"], "local");
        assert_eq!(received["body"], local_envelope["body"]);
        assert!(received.to_string().contains(&local_marker));
        assert!(
            !received.to_string().contains(&remote_marker),
            "held Gateway record must not leak into the Await result"
        );

        let inbox = inbox_run_at_structured(
            &sock,
            ListArgs {
                since: None,
                include_terminal: false,
                act_as: Some("bob".into()),
            },
        )
        .await
        .expect("explicit Inbox read from zero");
        let held = inbox
            .envelopes
            .iter()
            .find(|stamped| stamped.envelope["id"] == remote_id)
            .expect("held Gateway record must remain in Bob's mailbox");
        assert_eq!(held.origin, Origin::Gateway);
        assert!(
            held.envelope.to_string().contains(&remote_marker),
            "explicit Inbox must return the original remote marker"
        );

        local_proxy.shutdown().await;
        remote.shutdown().await;
    });
}
