//! Phase 11 Plan 08 Task 1 (F-1 / INV-H / T-11-23 / T-11-24 / T-11-26) —
//! adversarial coverage for the ingress destination-authority gates added
//! to `crates/famp-gateway/src/ingress.rs::inbox_handler`.
//!
//! Three cases, each asserting the mailbox state directly (not merely the
//! HTTP status), per this plan's explicit acceptance criterion:
//!
//! 1. An envelope whose `to` authority does not match this gateway's
//!    configured own-domain is rejected (403 `foreign_domain`) and no
//!    mailbox insertion occurs.
//! 2. An envelope whose `to` leaf differs from the URL-path recipient is
//!    rejected (400 `misaddressed_recipient`) and the path-named mailbox
//!    stays empty.
//! 3. A well-formed same-domain, correctly-addressed envelope still
//!    delivers end-to-end onto the local bus.
//!
//! Uses a real broker subprocess (ChildGuard convention) + a real
//! `GatewayRegistry` backing the SENDER's bare name (mirroring
//! `inbox_handler`'s `guard.get_mut(sender.name())` dispatch) — the
//! router itself is exercised in-process via `Router::oneshot`, not a
//! live TLS listener (`build_gateway_router` needs no TLS to test).

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use axum::body::to_bytes;
use axum::Router;
use famp::bus_client::BusClient;
use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, Timestamp, UnsignedEnvelope};
use famp_bus::{BusMessage, BusReply};
use famp_envelope::body::ack::{AckBody, AckDisposition};
use famp_gateway::ingress::build_gateway_router;
use famp_gateway::GatewayRegistry;
use famp_keyring::Keyring;
use tokio::sync::Mutex;
use tower::ServiceExt;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// See `tests/liveness.rs` for the full rationale: `Command::cargo_bin`
/// resolves the sibling `famp` binary via the shared-workspace
/// `target/debug/` fallback, not `CARGO_BIN_EXE_famp`.
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

/// A signed `ack`-class envelope (mirrors `ingress.rs`'s own private
/// `ack_bytes` unit-test helper — duplicated here since that helper is
/// private to the lib's `#[cfg(test)]` module).
fn ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal, id: MessageId) -> Vec<u8> {
    let ts = Timestamp("2026-07-28T00:00:00Z".to_string());
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let unsigned = UnsignedEnvelope::<AckBody>::new(
        id,
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts,
        body,
    );
    unsigned.sign(sk).unwrap().encode().unwrap()
}

/// Percent-encode `recipient_str` into a single URL path segment, mirroring
/// exactly what `HttpTransport::send` does — identical to `ingress.rs`'s
/// own private `inbox_uri` test helper.
fn inbox_uri(recipient_str: &str) -> String {
    let mut url = url::Url::parse("http://gateway.test/").unwrap();
    {
        let mut segs = url.path_segments_mut().unwrap();
        segs.pop_if_empty();
        segs.extend(["famp", "v0.5.1", "inbox"]);
        segs.push(recipient_str);
    }
    url.path().to_string()
}

async fn post_inbox(
    router: Router,
    recipient_str: &str,
    body: Vec<u8>,
) -> axum::response::Response {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(inbox_uri(recipient_str))
        .header("content-type", "application/famp+json")
        .body(axum::body::Body::from(body))
        .unwrap();
    router.oneshot(req).await.unwrap()
}

/// Register a REAL bus identity `name` (not a gateway stand-in) so its
/// mailbox can be inspected directly via `BusMessage::Await`. Uses
/// `connect_no_spawn` (not `connect`) — the broker is already up
/// (`spawn_broker_subprocess` + `wait_for_broker_socket` in
/// `build_harness`), and `connect`'s own spawn-if-absent path is racy
/// against an already-running broker on a non-default socket path.
async fn register_real(sock: &Path, name: &str) -> BusClient {
    let mut client = BusClient::connect_no_spawn(sock, None)
        .await
        .expect("connect_no_spawn");
    client
        .send_recv(BusMessage::Register {
            name: name.to_owned(),
            pid: std::process::id(),
            cwd: None,
            listen: false,
        })
        .await
        .expect("register");
    client
}

/// Assert `client`'s mailbox is currently empty (a short `Await` times
/// out) — the load-bearing "no mailbox insertion occurred" check this
/// plan requires beyond a bare HTTP status assertion.
async fn assert_mailbox_empty(client: &mut BusClient) {
    let reply = client
        .send_recv(BusMessage::Await {
            timeout_ms: 300,
            task: None,
        })
        .await
        .expect("send_recv await");
    assert!(
        matches!(reply, BusReply::AwaitTimeout {}),
        "expected an empty mailbox (AwaitTimeout), got {reply:?} — a reject path must \
         perform zero mailbox insertion"
    );
}

struct Harness {
    _tmp: tempfile::TempDir,
    _broker: ChildGuard,
    sock: std::path::PathBuf,
    router: Router,
}

/// Stand up a broker + a `GatewayRegistry` backing `alice` (the sender
/// stand-in `inbox_handler` dispatches through, D-05) + a router
/// configured with `own_domain`. `alice`'s pubkey is pinned in the
/// keyring passed to the router.
async fn build_harness(own_domain: Option<&str>, sk: &FampSigningKey) -> Harness {
    ensure_famp_bin_built();
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, Duration::from_secs(5));

    let mut registry = GatewayRegistry::default();
    registry
        .back(&sock, "alice".into())
        .await
        .expect("back alice");

    let alice: Principal = "agent:hosta.test/alice".parse().unwrap();
    let mut keyring = Keyring::new();
    keyring.pin_tofu(alice, sk.verifying_key()).unwrap();

    let router = build_gateway_router(
        Arc::new(Mutex::new(registry)),
        Arc::new(keyring),
        own_domain.map(Arc::from),
    );

    Harness {
        _tmp: tmp,
        _broker: broker,
        sock,
        router,
    }
}

#[tokio::test]
async fn envelope_addressed_to_foreign_domain_is_rejected_and_mailbox_untouched() {
    let sk = FampSigningKey::from_bytes([60u8; 32]);
    let harness = build_harness(Some("hostb.test"), &sk).await;

    // A REAL "bob" registration so we have a mailbox to assert stays
    // empty. Own-domain is "hostb.test"; the envelope's `to` targets a
    // DIFFERENT domain (`otherdomain.test`) while its leaf still matches
    // the URL-path recipient, so ONLY the foreign-domain check fires.
    let mut bob = register_real(&harness.sock, "bob").await;

    let from: Principal = "agent:hosta.test/alice".parse().unwrap();
    let to: Principal = "agent:otherdomain.test/bob".parse().unwrap();
    let bytes = ack_bytes(&sk, &from, &to, MessageId::new_v7());

    let resp = post_inbox(harness.router, &to.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "foreign_domain");

    assert_mailbox_empty(&mut bob).await;
}

#[tokio::test]
async fn envelope_to_differs_from_path_recipient_is_rejected_and_mailbox_untouched() {
    let sk = FampSigningKey::from_bytes([61u8; 32]);
    let harness = build_harness(Some("hostb.test"), &sk).await;

    let mut carol = register_real(&harness.sock, "carol").await;

    let from: Principal = "agent:hosta.test/alice".parse().unwrap();
    // Envelope is addressed to bob (same domain as own_domain, so the
    // foreign-domain check would pass) but the URL path names carol —
    // the sender signed for someone else; the path must not override it.
    let to: Principal = "agent:hostb.test/bob".parse().unwrap();
    let path_recipient: Principal = "agent:hostb.test/carol".parse().unwrap();
    let bytes = ack_bytes(&sk, &from, &to, MessageId::new_v7());

    let resp = post_inbox(harness.router, &path_recipient.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "misaddressed_recipient");

    assert_mailbox_empty(&mut carol).await;
}

#[tokio::test]
async fn well_formed_same_domain_envelope_still_delivers() {
    let sk = FampSigningKey::from_bytes([62u8; 32]);
    let harness = build_harness(Some("hostb.test"), &sk).await;

    let mut bob = register_real(&harness.sock, "bob").await;

    let from: Principal = "agent:hosta.test/alice".parse().unwrap();
    let to: Principal = "agent:hostb.test/bob".parse().unwrap();
    let id = MessageId::new_v7();
    let bytes = ack_bytes(&sk, &from, &to, id);

    let resp = post_inbox(harness.router, &to.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::ACCEPTED);

    let reply = bob
        .send_recv(BusMessage::Await {
            timeout_ms: 5_000,
            task: None,
        })
        .await
        .expect("send_recv await");
    match reply {
        BusReply::AwaitOk { envelopes, .. } => {
            assert!(
                envelopes
                    .iter()
                    .any(|e| e.get("id").and_then(|v| v.as_str()) == Some(id.to_string().as_str())),
                "delivered envelope not found in bob's mailbox: {envelopes:?}"
            );
        }
        other => panic!("expected AwaitOk, got {other:?}"),
    }
}
