#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 14 plan 14-05: version-skew and proto-rejection integration
//! tests (D-03/D-11/D-14).
//!
//! `quarantine_tracer.rs` (14-01) already pins the unit/single-path
//! shapes of fail-closed stamping and proto-1 rejection. This file is
//! the integration-level counterpart D-11 asks for, over real sockets,
//! covering the D-03/D-14 fail-closed DISJUNCTION end to end:
//!
//! With `BUS_PROTO_VERSION` past 1 (it was 2 when this was written and
//! has since moved again — read the constant, not this comment), an
//! actual old gateway binary is
//! rejected at Hello and never delivers anything — it cannot produce an
//! unmarked record even in principle. The honest fail-closed assertion
//! is therefore a disjunction: an unstamped record either (a) never
//! arrives at all (proto-1 rejection, pinned by
//! `skew_proto_1_gateway_is_rejected_before_delivery`), or (b) arrives
//! marked (a stale on-disk writer bypassing the broker entirely, pinned
//! by `skew_unstamped_mailbox_record_renders_untrusted`). In no case
//! does it arrive unmarked or marked-local. Together the two tests cover
//! the disjunction; neither alone would. This is a framing correction
//! forced by the proto-2 decision (D-09), not a weakening of D-03's
//! original "old gateway binary" wording — see each test's own doc
//! comment and 14-05-SUMMARY.md for the framing note to famp-lead-730.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::{codec, BusClient, BusClientError};
use famp::cli::inbox::list::{run_at_structured, ListArgs};
use famp::cli::render::render_envelope_body;
use famp_bus::{BusErrorKind, BusMessage, BusReply, Origin, Target};

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

fn audit_log_envelope(sender: &str, recipient: &str, id: &str, msg: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": id,
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-07-30T12:00:00Z",
        "body": { "event": "famp.send.deliver", "details": { "msg": msg } },
    })
}

/// D-03/D-14, branch (b) of the fail-closed disjunction: a stale writer
/// that bypasses the broker's own stamping path entirely (the strongest
/// available simulation — it covers ANY unstamped writer, including a
/// foreign implementation or a pre-v1.1 mailbox file, not just one old
/// gateway binary) must still render marked, never local, never dropped.
#[test]
fn skew_unstamped_mailbox_record_renders_untrusted() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // Write a bare (unstamped) envelope line directly into bob's
        // on-disk mailbox, bypassing the broker/executor stamping path
        // entirely — the faithful simulation of a stale writer.
        let envelope = audit_log_envelope(
            "carol",
            "bob",
            "01890000-0000-7000-8000-0000000000a1",
            "unstamped record",
        );
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
            .expect("the unstamped record must NOT be dropped — it must arrive marked");
        assert_eq!(
            stamped.origin,
            Origin::Unknown,
            "an unstamped mailbox record must resolve to Origin::Unknown, never Local"
        );

        let raw_body = stamped.envelope.get("body").cloned().unwrap();
        let rendered = render_envelope_body(stamped.origin, &raw_body);
        assert_ne!(
            rendered, raw_body,
            "an unstamped record must render marked (wrapped), not verbatim as if local"
        );
    });
}

/// D-03/D-14, branch (a) of the fail-closed disjunction: a client
/// declaring an old `bus_proto` is rejected at Hello before it can
/// deliver anything, and no mailbox record is ever produced for the
/// intended recipient. The surfaced client-side error (the real
/// production `BusClientError::ProtocolMismatch` Display) names both
/// remedies.
///
/// `BusClient::connect` always declares the CURRENT `BUS_PROTO_VERSION`
/// on its own Hello frame (`bus_client/mod.rs`), so it cannot be used
/// directly to simulate an old client — a raw Hello frame is required,
/// exactly as `quarantine_tracer.rs`'s `tracer_proto_1_client_cannot_connect`
/// already does at the reply-shape level. This test additionally proves
/// the DELIVERY-SIDE consequence (no mailbox record) and reconstructs
/// the real client-side `BusClientError::ProtocolMismatch` from the raw
/// reply — `bus_client::classify_hello_reply` (the function that does
/// this mapping in production) is crate-private, not reachable from an
/// integration test, so the match arm is mirrored here byte-for-byte
/// against the same public `BusClientError` enum and its real `Display`
/// impl (thiserror-derived, defined once in `bus_client/mod.rs`) —
/// the assertion below is against real production error text, not a
/// hand-authored string.
#[test]
fn skew_proto_1_gateway_is_rejected_before_delivery() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let hello = BusMessage::Hello {
            bus_proto: 1,
            client: "proto-1-gateway/0.0.1".into(),
            bind_as: None,
        };
        codec::write_frame(&mut stream, &hello).await.unwrap();
        let reply: BusReply = codec::read_frame(&mut stream).await.unwrap();

        let client_error = match reply {
            BusReply::HelloErr {
                kind: BusErrorKind::BrokerProtoMismatch,
                message,
            } => BusClientError::ProtocolMismatch {
                broker_message: message,
            },
            other => panic!("expected HelloErr{{BrokerProtoMismatch}}, got {other:?}"),
        };
        let display = client_error.to_string();
        assert!(
            display.contains("just install"),
            "rejection error must name `just install`; got: {display}"
        );
        assert!(
            display.contains("famp daemon restart"),
            "rejection error must name `famp daemon restart`; got: {display}"
        );

        // Attempt one Send anyway on the same (never-handshaked)
        // connection — the broker's pre-dispatch gate rejects any
        // non-Hello frame on a connection that never completed a
        // successful Hello (handle_wire's `already_handshaked` check),
        // so this must also be rejected, and MUST NOT reach the mailbox.
        let envelope = audit_log_envelope(
            "eve",
            "bob",
            "01890000-0000-7000-8000-0000000000a2",
            "must never arrive",
        );
        let send = BusMessage::Send {
            to: Target::Agent { name: "bob".into() },
            envelope,
        };
        codec::write_frame(&mut stream, &send).await.unwrap();
        let send_reply: BusReply = codec::read_frame(&mut stream).await.unwrap();
        assert!(
            !matches!(send_reply, BusReply::SendOk { .. }),
            "a Send on a connection that never completed Hello must never succeed: {send_reply:?}"
        );

        let bus_dir = sock.parent().unwrap();
        let mailbox_path = bus_dir.join("mailboxes").join("bob.jsonl");
        let mailbox_contents =
            std::fs::read_to_string(&mailbox_path).unwrap_or_default();
        assert!(
            !mailbox_contents.contains("must never arrive"),
            "bob's mailbox must gain no record from a proto-1-rejected connection: {mailbox_contents}"
        );
    });
}

/// D-01/D-02 fail-closed pin re-proven over a real socket (not just
/// in-process serde): a canonical holder that registers WITHOUT
/// declaring `origin` (the field genuinely omitted from the wire frame,
/// per `Register`'s `#[serde(default, skip_serializing_if =
/// "Option::is_none")]`) produces mailbox records that render marked —
/// the same polarity 14-01 pinned in `origin.rs`'s unit tests, here
/// proven end-to-end so a serde default that only holds in-process
/// cannot pass while the real wire path silently fails.
#[test]
fn skew_register_without_origin_produces_marked_records() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // A canonical (non-proxy) connection that registers WITHOUT
        // declaring an origin — `origin: None` genuinely omits the key
        // on the wire (BUS-02 byte-exact precedent), simulating a client
        // implementation that has not been updated to declare
        // provenance (e.g. a not-yet-updated foreign implementation
        // that nonetheless negotiates the current bus_proto correctly).
        let mut eve = BusClient::connect(&sock, None).await.unwrap();
        let reply = eve
            .send_recv(BusMessage::Register {
                name: "eve".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: None,
            })
            .await
            .unwrap();
        assert!(
            matches!(reply, BusReply::RegisterOk { .. }),
            "origin-omitting register must still succeed (additive field, BUS-02): {reply:?}"
        );

        let envelope = audit_log_envelope(
            "eve",
            "bob",
            "01890000-0000-7000-8000-0000000000a3",
            "origin-omitted send",
        );
        let send_reply = eve
            .send_recv(BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: envelope.clone(),
            })
            .await
            .unwrap();
        assert!(
            matches!(send_reply, BusReply::SendOk { .. }),
            "origin-omitting send must succeed: {send_reply:?}"
        );
        eve.shutdown().await;

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
            .expect("bob's inbox must contain eve's message");
        assert_eq!(
            stamped.origin,
            Origin::Unknown,
            "a Register frame that omits origin must resolve to Origin::Unknown, never Local"
        );

        let raw_body = stamped.envelope.get("body").cloned().unwrap();
        let rendered = render_envelope_body(stamped.origin, &raw_body);
        assert_ne!(
            rendered, raw_body,
            "origin-omitted content must render marked, not verbatim"
        );
    });
}
