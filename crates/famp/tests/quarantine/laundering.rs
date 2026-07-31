//! QUAR-11 / D-15: the one-hop provenance-laundering limitation.
//!
//! FAMP's origin stamp tracks how content ENTERED this local bus — the
//! broker stamps `origin: gateway` on the mailbox append that receives a
//! federation-relayed message (D-02), and every rendering surface marks
//! that content as untrusted DATA (D-07/D-09). It does NOT — and cannot,
//! without content provenance far beyond this phase's scope — track
//! where content originally came from once a LOCAL agent re-authors it.
//!
//! Concretely: if remote-tagged content reaches local agent A, and A
//! then quotes or paraphrases that content into a NEW message A sends to
//! local agent B, B's copy is a message from A — a local sender — so it
//! arrives at B stamped `origin: local` and renders verbatim, unmarked.
//! The content survived the hop; the provenance did not. An agent that
//! re-emits remote text launders its provenance.
//!
//! This is a known, accepted, and documented limitation (T-14-18 in
//! 14-04-PLAN.md's threat model), not a defect this phase fixes.
//! Tracking content (not just transport) provenance across an agent's
//! own re-authoring is out of scope; QUAR-08's phase documentation
//! states this plainly rather than omitting or hiding it (D-15). The
//! test below asserts this outcome and PASSES on it — it is pinning the
//! limitation, not asserting its absence.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::BusClient;
use famp::cli::inbox::list::{run_at_structured, ListArgs};
use famp::cli::render::render_envelope_body;
use famp_bus::{BusMessage, Origin, Target};

#[path = "../common/child_guard.rs"]
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

fn valid_envelope(sender: &str, recipient: &str, body_marker: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": uuid::Uuid::now_v7().to_string(),
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-07-30T12:00:00Z",
        "body": {"event": "famp.send.deliver", "details": {"msg": body_marker}},
    })
}

fn rendered_body_text(v: &serde_json::Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// A remote gateway-origin sender addresses A directly, simulating an
/// inbound federation-relayed message (mirrors the pattern 14-02's
/// `quarantine_surfaces.rs` uses to simulate a gateway sender without
/// standing up a real `famp-gateway`).
async fn deliver_gateway_message_to_a(sock: &Path, attacker_marker: &str) {
    let mut remote = BusClient::connect(sock, None).await.unwrap();
    remote
        .send_recv(BusMessage::Register {
            name: "remote-peer".into(),
            pid: std::process::id(),
            cwd: None,
            listen: false,
            origin: Some(Origin::Gateway),
        })
        .await
        .unwrap();

    let inbound = valid_envelope("remote-peer", "a", attacker_marker);
    let send_reply = remote
        .send_recv(BusMessage::Send {
            to: Target::Agent { name: "a".into() },
            envelope: inbound,
        })
        .await
        .unwrap();
    assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));
    remote.shutdown().await;
}

/// Assertion 1: A's read of the gateway-origin message carries
/// `Origin::Gateway` and a marked body.
async fn assert_a_receives_marked_gateway_message(sock: &Path, attacker_marker: &str) {
    let a_read = run_at_structured(
        sock,
        ListArgs {
            since: None,
            include_terminal: false,
            act_as: Some("a".into()),
        },
    )
    .await
    .expect("a's inbox list");
    assert_eq!(
        a_read.envelopes.len(),
        1,
        "A must receive the remote message"
    );
    let stamped = &a_read.envelopes[0];
    assert_eq!(
        stamped.origin,
        Origin::Gateway,
        "A's copy must carry Origin::Gateway"
    );
    let raw_body = famp_envelope::EnvelopeView::new(&stamped.envelope)
        .body()
        .cloned()
        .expect("envelope has a body");
    let a_rendered = render_envelope_body(stamped.origin, &raw_body);
    let a_rendered_text = rendered_body_text(&a_rendered);
    assert!(
        a_rendered_text.contains("FAMP-QUARANTINE"),
        "A's rendered body must be quarantine-marked: {a_rendered_text}"
    );
    assert!(
        a_rendered_text.contains(attacker_marker),
        "A's rendered body must still contain the original attacker text: {a_rendered_text}"
    );
}

/// A quotes the text it read into a NEW message to local agent B. This
/// simulates an agent extracting content it observed and re-authoring it
/// into an outbound message — the laundering step (T-14-18).
async fn a_forwards_quoting_text_to_b(sock: &Path, quoted_text: &str) {
    let mut a_proxy = BusClient::connect(sock, Some("a".into())).await.unwrap();
    let outbound = valid_envelope("a", "b", quoted_text);
    let send_reply = a_proxy
        .send_recv(BusMessage::Send {
            to: Target::Agent { name: "b".into() },
            envelope: outbound,
        })
        .await
        .unwrap();
    assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));
    a_proxy.shutdown().await;
}

/// Assertions 2-4: B's read carries `Origin::Local`, renders
/// byte-identical (unmarked), and still contains the laundered attacker
/// text — proving the CONTENT survived the hop while the PROVENANCE did
/// not. A test that only asserted `Origin::Local` here would pass
/// trivially even if the body had been dropped.
async fn assert_b_receives_unmarked_laundered_message(sock: &Path, attacker_marker: &str) {
    let b_read = run_at_structured(
        sock,
        ListArgs {
            since: None,
            include_terminal: false,
            act_as: Some("b".into()),
        },
    )
    .await
    .expect("b's inbox list");
    assert_eq!(
        b_read.envelopes.len(),
        1,
        "B must receive A's forwarded message"
    );
    let b_stamped = &b_read.envelopes[0];

    assert_eq!(
        b_stamped.origin,
        Origin::Local,
        "B's copy of A's forwarded message must carry Origin::Local — A is a local sender"
    );

    let b_raw_body = famp_envelope::EnvelopeView::new(&b_stamped.envelope)
        .body()
        .cloned()
        .expect("envelope has a body");
    let b_rendered = render_envelope_body(b_stamped.origin, &b_raw_body);

    assert_eq!(
        b_rendered, b_raw_body,
        "B's local-origin body must render byte-identical (unmarked)"
    );
    let b_rendered_text = rendered_body_text(&b_rendered);
    assert!(
        !b_rendered_text.contains("FAMP-QUARANTINE"),
        "B's rendered body must carry NO quarantine mark: {b_rendered_text}"
    );
    assert!(
        b_rendered_text.contains(attacker_marker),
        "B's rendered body must contain the laundered attacker text: {b_rendered_text}"
    );
}

#[test]
fn laundered_remote_content_arrives_at_second_hop_untagged() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        // A and B are both LOCAL agents (real `famp register` CLI path,
        // no origin override -> Origin::Local).
        let _a = spawn_register(&sock, "a");
        wait_for_registration(&sock, "a").await;
        let _b = spawn_register(&sock, "b");
        wait_for_registration(&sock, "b").await;

        let attacker_marker = format!("ATTACKER-{}", uuid::Uuid::now_v7().simple());
        deliver_gateway_message_to_a(&sock, &attacker_marker).await;
        assert_a_receives_marked_gateway_message(&sock, &attacker_marker).await;

        let quoted_text = format!("FYI from remote, quoting: {attacker_marker}");
        a_forwards_quoting_text_to_b(&sock, &quoted_text).await;
        assert_b_receives_unmarked_laundered_message(&sock, &attacker_marker).await;
    });
}
