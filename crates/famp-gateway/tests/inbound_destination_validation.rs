//! Phase 11 Plan 08 Task 1 (F-1 / INV-H / T-11-23 / T-11-24 / T-11-26) —
//! adversarial coverage for the ingress destination-authority gates added
//! to `crates/famp-gateway/src/ingress.rs::inbox_handler`.
//!
//! Six cases (1-3 from Phase 11; 4-6 added 17-03 Task 2, INGR-04/INGR-01),
//! each asserting the mailbox state directly (not merely the HTTP status),
//! per this plan's explicit acceptance criterion:
//!
//! 1. An envelope whose `to` authority does not match this gateway's
//!    configured own-domain is rejected (403 `foreign_domain`) and no
//!    mailbox insertion occurs.
//! 2. An envelope whose `to` leaf differs from the URL-path recipient is
//!    rejected (400 `misaddressed_recipient`) and the path-named mailbox
//!    stays empty.
//! 3. A well-formed same-domain, correctly-addressed envelope still
//!    delivers end-to-end onto the local bus.
//! 4. (17-03, INGR-04/INGR-05) A foreign-domain envelope ALSO signed with
//!    the WRONG key is rejected `foreign_domain`, never `invalid_signature`
//!    — proves at the INTEGRATION level (real broker, real router) that
//!    `audience_check`'s domain half runs pre-verify, not merely at the
//!    `ingress_guard.rs` unit level.
//! 5. (17-03, INGR-04/INGR-05) An envelope from a sender this gateway does
//!    NOT back is rejected `sender_not_backed` pre-verify, before
//!    signature verification runs, with the target mailbox untouched.
//! 6. (17-03, INGR-01/INGR-05) A stale-timestamp envelope from a REAL
//!    backed, correctly-pinned sender is rejected `stale_timestamp`
//!    pre-verify, with the target mailbox untouched — the same freshness
//!    property `ingress.rs`'s own unit tests pin, reconfirmed here against
//!    a live broker.
//!
//! Uses a real broker subprocess (ChildGuard convention) + a real
//! `GatewayRegistry` backing the SENDER's bare name (mirroring
//! `inbox_handler`'s `guard.get_mut(sender.name())` dispatch) — the
//! router itself is exercised in-process via `Router::oneshot`, not a
//! live TLS listener (`build_gateway_router` needs no TLS to test).

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

/// Startup deadline for broker socket, relaxed from 5s to accommodate parallel
/// test execution where multiple harnesses spawn brokers simultaneously.
/// See crates/famp-gateway/tests/common/gateway_harness.rs::STARTUP_DEADLINE.
const STARTUP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

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
///
/// `ts` is live, not fixed: these envelopes go through the real
/// `inbox_handler` → `ingest_inbound` → `ingress_guard::run_cheap_gates`
/// path, which now runs Phase 17's freshness gate (INGR-01) against the
/// real wall clock before anything else. A fixed literal here goes stale
/// the moment it ages past `CLOCK_SKEW_WINDOW_SECS` (it did — this file's
/// 2026-07-28 literal broke all three tests days later without either the
/// foreign-domain, misaddressed-recipient, or delivery behavior under test
/// ever running).
///
/// 17-02 (INGR-02) deviation: `ingest_inbound` also now requires a
/// non-empty peeked `nonce` before signature verification runs. This
/// helper stamps a fresh, unique `.with_nonce(...)` on every call (the
/// same federation-wrapper field `egress.rs::sign_federation_fields`
/// adds on the way out) so these three pre-existing tests reject on the
/// destination-authority gate under test, never `missing_nonce`.
fn ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal, id: MessageId) -> Vec<u8> {
    let ts = Timestamp(famp_gateway::now_canonical_utc());
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
    )
    .with_nonce(uuid::Uuid::now_v7().to_string());
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
            origin: None,
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
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

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
                    .any(|e| e.envelope.get("id").and_then(|v| v.as_str())
                        == Some(id.to_string().as_str())),
                "delivered envelope not found in bob's mailbox: {envelopes:?}"
            );
        }
        other => panic!("expected AwaitOk, got {other:?}"),
    }
}

/// Case 4 (17-03, INGR-04/INGR-05): a foreign-domain envelope ALSO signed
/// with the WRONG key is rejected `foreign_domain`, never
/// `invalid_signature` — proves at the INTEGRATION level (real broker,
/// real router) that `audience_check`'s domain half runs pre-verify.
#[tokio::test]
async fn foreign_domain_envelope_signed_with_wrong_key_is_rejected_foreign_domain_not_invalid_signature(
) {
    let sk = FampSigningKey::from_bytes([63u8; 32]);
    let wrong_sk = FampSigningKey::from_bytes([64u8; 32]);
    let harness = build_harness(Some("hostb.test"), &wrong_sk).await;

    let mut bob = register_real(&harness.sock, "bob").await;

    let from: Principal = "agent:hosta.test/alice".parse().unwrap();
    // Same shape as the existing foreign-domain case: `to` targets a
    // DIFFERENT domain than own-domain ("hostb.test"), leaf still matches
    // the URL-path recipient. Signed with `sk`, but the harness's keyring
    // pins `wrong_sk` -- verification WOULD fail if it were ever reached.
    let to: Principal = "agent:otherdomain.test/bob".parse().unwrap();
    let bytes = ack_bytes(&sk, &from, &to, MessageId::new_v7());

    let resp = post_inbox(harness.router, &to.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        v["error"], "foreign_domain",
        "foreign-domain-AND-wrong-key case must reject on the CHEAP gate, never invalid_signature"
    );

    assert_mailbox_empty(&mut bob).await;
}

/// Case 5 (17-03, INGR-04/INGR-05): an envelope from a sender this
/// gateway does NOT back is rejected `sender_not_backed` pre-verify,
/// before signature verification runs, with the target mailbox untouched.
#[tokio::test]
async fn envelope_from_unbacked_sender_is_rejected_sender_not_backed_pre_verify() {
    let sk = FampSigningKey::from_bytes([65u8; 32]);
    // `build_harness` only ever backs "alice" in its `GatewayRegistry` --
    // this envelope claims a DIFFERENT sender ("eve") that the registry
    // has never heard of.
    let harness = build_harness(Some("hostb.test"), &sk).await;

    let mut bob = register_real(&harness.sock, "bob").await;

    let from: Principal = "agent:hosta.test/eve".parse().unwrap();
    let to: Principal = "agent:hostb.test/bob".parse().unwrap();
    let bytes = ack_bytes(&sk, &from, &to, MessageId::new_v7());

    let resp = post_inbox(harness.router, &to.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_GATEWAY);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "sender_not_backed");

    assert_mailbox_empty(&mut bob).await;
}

/// Case 6 (17-03, INGR-01/INGR-05): a stale-timestamp envelope from a
/// REAL backed, correctly-pinned sender is rejected `stale_timestamp`
/// pre-verify, with the target mailbox untouched -- reconfirms
/// `ingress.rs`'s own freshness-gate unit tests against a live broker.
#[tokio::test]
async fn stale_timestamp_envelope_is_rejected_pre_verify_with_mailbox_untouched() {
    let sk = FampSigningKey::from_bytes([66u8; 32]);
    let harness = build_harness(Some("hostb.test"), &sk).await;

    let mut bob = register_real(&harness.sock, "bob").await;

    let from: Principal = "agent:hosta.test/alice".parse().unwrap();
    let to: Principal = "agent:hostb.test/bob".parse().unwrap();

    let one_hour_ago = time::OffsetDateTime::now_utc() - time::Duration::hours(1);
    // `strip_subseconds` is `pub(crate)` inside `famp-gateway` and not
    // reachable from this external integration-test crate; `freshness_check`
    // parses full RFC 3339 (fractional seconds are legal) and rejects on
    // the whole-second delta regardless, so the un-stripped form is fine
    // here.
    let stale_ts = one_hour_ago
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a73".parse().unwrap();
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let unsigned = UnsignedEnvelope::<AckBody>::new(
        id,
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        Timestamp(stale_ts),
        body,
    )
    .with_nonce(uuid::Uuid::now_v7().to_string());
    let bytes = unsigned.sign(&sk).unwrap().encode().unwrap();

    let resp = post_inbox(harness.router, &to.to_string(), bytes).await;
    assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "stale_timestamp");

    assert_mailbox_empty(&mut bob).await;
}
