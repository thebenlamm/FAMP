#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 14 plan 14-01 tracer: the fail-closed provenance spine, proven
//! end-to-end on one path — a gateway-origin sender to `famp_inbox`
//! (via `cli::inbox::list::run_at_structured`), a local-origin sender,
//! and a legacy unstamped on-disk record.
//!
//! Task 2 truths pinned here:
//!   - A gateway-registered sender's message renders through
//!     `famp_inbox` carrying an untrusted mark
//!     (`tracer_gateway_origin_renders_untrusted_through_inbox`).
//!   - A local sender's message does not
//!     (`tracer_local_origin_renders_verbatim`).
//!   - An unstamped legacy on-disk record resolves to `Origin::Unknown`
//!     (`tracer_legacy_unstamped_line_resolves_unknown`).
//!
//! Task 3 adds `tracer_proto_1_client_cannot_connect`.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::{codec, BusClient};
use famp::cli::inbox::list::{run_at_structured, ListArgs};
use famp::cli::render::render_envelope_body;
use famp_bus::{BusMessage, Origin};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// Spawn a bare `famp broker --socket <path>` daemon directly (mirrors
/// `broker_proxy_semantics.rs`'s `spawn_broker_subprocess`: we cannot use
/// `spawn_broker_if_absent` inside an integration test binary because
/// `current_exe()` there resolves to the TEST binary, not `famp`).
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

/// Spawn a real `famp register <name>` foreground process (the CLI path
/// that declares `origin: Some(Origin::Local)`, Phase 14 Task 2 point E).
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

fn audit_log_envelope(sender: &str, recipient: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": "01890000-0000-7000-8000-000000000001",
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-07-30T12:00:00Z",
        "body": {
            "event": "famp.send.deliver",
            "details": { "msg": "hello from the tracer" }
        }
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Task 2 truth: a cross-host message (simulated by a canonical
/// connection declaring `origin: Some(Origin::Gateway)`, matching
/// `ProxiedPrincipal::register`'s wire shape without needing a live
/// `famp-gateway` process) renders through `famp_inbox` carrying an
/// untrusted mark.
#[test]
fn tracer_gateway_origin_renders_untrusted_through_inbox() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        // bob: ordinary local recipient, kept alive via `famp register`.
        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // alice: a canonical (non-proxy) connection declaring
        // `origin: Some(Origin::Gateway)` — the exact wire shape
        // `ProxiedPrincipal::register` sends, held open on this same
        // connection for the life of the test (matching the gateway's
        // "one connection backs the principal" invariant).
        let mut alice = BusClient::connect(&sock, None).await.unwrap();
        let reply = alice
            .send_recv(BusMessage::Register {
                name: "alice".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: Some(Origin::Gateway),
            })
            .await
            .unwrap();
        assert!(
            matches!(reply, famp_bus::BusReply::RegisterOk { .. }),
            "alice (gateway-origin) register must succeed: {reply:?}"
        );

        let envelope = audit_log_envelope("alice", "bob");
        let send_reply = alice
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope: envelope.clone(),
            })
            .await
            .unwrap();
        assert!(
            matches!(send_reply, famp_bus::BusReply::SendOk { .. }),
            "gateway-origin send must succeed: {send_reply:?}"
        );

        let outcome = run_at_structured(
            &sock,
            ListArgs {
                since: None,
                include_terminal: false,
                act_as: Some("bob".into()),
            },
        )
        .await
        .expect("bob inbox list");

        let stamped = outcome
            .envelopes
            .iter()
            .find(|s| {
                s.envelope.get("id").and_then(|v| v.as_str())
                    == envelope.get("id").and_then(|v| v.as_str())
            })
            .expect("bob's inbox must contain alice's message");
        assert_eq!(
            stamped.origin,
            Origin::Gateway,
            "message from a gateway-registered sender must be stamped Origin::Gateway"
        );

        let raw_body = stamped.envelope.get("body").cloned().unwrap();
        let rendered = render_envelope_body(stamped.origin, &raw_body);
        assert_ne!(
            rendered, raw_body,
            "gateway-origin body must render wrapped, not verbatim"
        );

        alice.shutdown().await;
    });
}

/// Task 2 truth: a local sender's message does NOT carry the untrusted
/// mark — `render_envelope_body(Origin::Local, body)` returns `body`
/// unchanged.
#[test]
fn tracer_local_origin_renders_verbatim() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        // bob: recipient.
        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // alice: `famp register` — the real CLI path, which declares
        // `origin: Some(Origin::Local)` (Phase 14 Task 2 point E).
        let _alice = spawn_register(&sock, "alice");
        wait_for_registration(&sock, "alice").await;

        // Send as alice via a one-shot proxy connection (mirrors how
        // `famp send --as alice` operates).
        let mut proxy = BusClient::connect(&sock, Some("alice".into()))
            .await
            .expect("alice proxy connect");
        let envelope = audit_log_envelope("alice", "bob");
        let send_reply = proxy
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope: envelope.clone(),
            })
            .await
            .unwrap();
        assert!(
            matches!(send_reply, famp_bus::BusReply::SendOk { .. }),
            "local-origin send must succeed: {send_reply:?}"
        );
        proxy.shutdown().await;

        let outcome = run_at_structured(
            &sock,
            ListArgs {
                since: None,
                include_terminal: false,
                act_as: Some("bob".into()),
            },
        )
        .await
        .expect("bob inbox list");

        let stamped = outcome
            .envelopes
            .iter()
            .find(|s| {
                s.envelope.get("id").and_then(|v| v.as_str())
                    == envelope.get("id").and_then(|v| v.as_str())
            })
            .expect("bob's inbox must contain alice's message");
        assert_eq!(
            stamped.origin,
            Origin::Local,
            "message from `famp register`'s canonical local holder must be stamped Origin::Local"
        );

        let raw_body = stamped.envelope.get("body").cloned().unwrap();
        let rendered = render_envelope_body(stamped.origin, &raw_body);
        assert_eq!(
            rendered, raw_body,
            "local-origin body must render byte-identical to the raw body"
        );
    });
}

/// Task 2 truth: a mailbox JSONL line written before this phase (bare
/// envelope, no stamp) renders as `Origin::Unknown` — never `Local`.
#[test]
fn tracer_legacy_unstamped_line_resolves_unknown() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // Write a bare (unstamped) envelope line directly into bob's
        // on-disk mailbox — the pre-Phase-14 format, bypassing the
        // broker/executor stamping path entirely.
        let envelope = audit_log_envelope("carol", "bob");
        let line = famp_canonical::canonicalize(&envelope).unwrap();
        let bus_dir = sock.parent().unwrap();
        let mailbox_path = bus_dir.join("mailboxes").join("bob.jsonl");
        std::fs::create_dir_all(mailbox_path.parent().unwrap()).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&mailbox_path)
            .unwrap();
        file.write_all(&line).unwrap();
        file.write_all(b"\n").unwrap();
        drop(file);

        let outcome = run_at_structured(
            &sock,
            ListArgs {
                since: None,
                include_terminal: false,
                act_as: Some("bob".into()),
            },
        )
        .await
        .expect("bob inbox list");

        let stamped = outcome
            .envelopes
            .iter()
            .find(|s| {
                s.envelope.get("id").and_then(|v| v.as_str())
                    == envelope.get("id").and_then(|v| v.as_str())
            })
            .expect("bob's inbox must contain the legacy line");
        assert_eq!(
            stamped.origin,
            Origin::Unknown,
            "a pre-Phase-14 unstamped mailbox line must resolve to Origin::Unknown, never Local"
        );
    });
}

/// Task 3: a raw Hello frame declaring `bus_proto: 1` against a live
/// (v1.1+) broker is refused with a reply naming the expected proto
/// version — the hard `BUS_PROTO_VERSION` bump actually blocks old
/// clients rather than silently degrading (D-09/D-09a).
#[test]
fn tracer_proto_1_client_cannot_connect() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let hello = BusMessage::Hello {
            bus_proto: 1,
            client: "proto-1-tracer/0.0.1".into(),
            bind_as: None,
        };
        codec::write_frame(&mut stream, &hello).await.unwrap();
        let reply: famp_bus::BusReply = codec::read_frame(&mut stream).await.unwrap();

        match reply {
            famp_bus::BusReply::HelloErr { kind, message } => {
                assert_eq!(
                    kind,
                    famp_bus::BusErrorKind::BrokerProtoMismatch,
                    "proto-1 Hello must be rejected with BrokerProtoMismatch, got kind={kind:?}"
                );
                let expected = famp_bus::BUS_PROTO_VERSION.to_string();
                assert!(
                    message.contains(&expected),
                    "rejection message must name the expected proto version ({expected}); got: {message}"
                );
            }
            other => panic!("expected HelloErr{{BrokerProtoMismatch}}, got {other:?}"),
        }
    });
}
