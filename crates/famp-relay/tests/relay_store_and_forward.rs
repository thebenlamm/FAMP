//! Live two-sided store-and-forward test: a real `famp-relay` subprocess
//! under TLS, driven by a TLS client that trusts the shared fixture
//! certs — proves the byte-identical round-trip end to end, not just
//! through the in-process router (`relay_routes.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silencer: this integration test binary is its own compilation unit;
// these dependencies back `famp-relay`'s HTTP surface, not this test's
// own client-side driving code.
use axum as _;
use serde as _;
use thiserror as _;
use tower as _;
use tower_http as _;
use uuid as _;

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use famp_crypto::FampSigningKey;
use famp_relay::fetch_auth::{normalize_audience, sign_fetch_auth, RELAY_FETCH_ROUTE};
use time::OffsetDateTime;
use url::Url;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

const STARTUP_DEADLINE: Duration = Duration::from_secs(30);

/// `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` —
/// the shared TLS fixture cert pair every cross-host relay/gateway test
/// reuses (09-PATTERNS.md). TLS here is channel encryption only; the
/// fetch-authorization signature is the real trust boundary.
fn cross_machine_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("famp")
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}

fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn wait_for_tcp(addr: SocketAddr, deadline: Duration) {
    let start = Instant::now();
    loop {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "relay listener at {addr} never came up within {deadline:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Spawn a real `famp-relay` subprocess, ChildGuard-wrapped, serving
/// `hosta.test` and `hostb.test` each with one configured key.
fn spawn_relay(port: u16, sk_a: &FampSigningKey, sk_b: &FampSigningKey) -> ChildGuard {
    let fixtures = cross_machine_fixture_dir();
    let public_url = format!("https://127.0.0.1:{port}");
    ChildGuard::new(
        Command::cargo_bin("famp-relay")
            .unwrap()
            .arg("--listen")
            .arg(format!("127.0.0.1:{port}"))
            .arg("--tls-cert")
            .arg(fixtures.join("alice.crt"))
            .arg("--tls-key")
            .arg(fixtures.join("alice.key"))
            .arg("--public-url")
            .arg(&public_url)
            .arg("--domain")
            .arg(format!("hosta.test={}", sk_a.verifying_key().to_b64url()))
            .arg("--domain")
            .arg(format!("hostb.test={}", sk_b.verifying_key().to_b64url()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn tls_client(fixtures: &std::path::Path) -> reqwest::Client {
    let tls = famp_transport_http::tls::build_client_config(Some(&fixtures.join("alice.crt")))
        .expect("client tls config");
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("reqwest client")
}

// Not a format string — see `relay_routes.rs::fetch_uri`'s identical
// note on `RELAY_FETCH_ROUTE`'s literal `{domain}` placeholder.
#[allow(clippy::literal_string_with_formatting_args)]
fn fetch_url(base: &str, domain: &str) -> String {
    format!("{base}{}", RELAY_FETCH_ROUTE.replace("{domain}", domain))
}

#[tokio::test]
async fn live_relay_accepts_post_and_returns_same_bytes_on_signed_fetch() {
    let port = pick_free_port();
    let sk_a = FampSigningKey::generate();
    let sk_b = FampSigningKey::generate();
    let _relay = spawn_relay(port, &sk_a, &sk_b);
    wait_for_tcp(
        format!("127.0.0.1:{port}").parse().unwrap(),
        STARTUP_DEADLINE,
    );

    let fixtures = cross_machine_fixture_dir();
    let client = tls_client(&fixtures);
    let base = format!("https://127.0.0.1:{port}");
    let public_url = Url::parse(&base).unwrap();
    let audience = normalize_audience(&public_url);

    // POST for a served domain.
    let invalid_utf8 = vec![0xFFu8, 0xFE, 0x00, 0x80, b'h', b'i'];
    let post_resp = client
        .post(format!(
            "{base}/famp/v0.5.1/inbox/{}",
            "agent%3Ahosta.test%2Falice"
        ))
        .body(invalid_utf8.clone())
        .send()
        .await
        .expect("post");
    assert_eq!(post_resp.status(), 202);

    // GET, signed for the same domain.
    let now = OffsetDateTime::now_utc();
    let signed = sign_fetch_auth(&sk_a, &audience, "hosta.test", now).expect("sign");
    let mut req = client.get(fetch_url(&base, "hosta.test"));
    for (name, value) in signed.header_pairs() {
        req = req.header(name, value);
    }
    let fetch_resp = req.send().await.expect("fetch");
    assert_eq!(fetch_resp.status(), 200);
    let body: serde_json::Value =
        serde_json::from_str(&fetch_resp.text().await.expect("read body")).expect("json body");
    let envelopes = body["envelopes"].as_array().expect("envelopes array");
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0]["to"], "agent:hosta.test/alice");
    let decoded = URL_SAFE_NO_PAD
        .decode(envelopes[0]["body"].as_str().expect("body string"))
        .expect("valid base64url");
    assert_eq!(
        decoded, invalid_utf8,
        "fetched bytes must be byte-identical to what was POSTed, including a non-UTF-8 body"
    );
}

#[tokio::test]
async fn live_relay_cross_domain_fetch_drains_nothing_then_correct_fetch_still_works() {
    let port = pick_free_port();
    let sk_a = FampSigningKey::generate();
    let sk_b = FampSigningKey::generate();
    let _relay = spawn_relay(port, &sk_a, &sk_b);
    wait_for_tcp(
        format!("127.0.0.1:{port}").parse().unwrap(),
        STARTUP_DEADLINE,
    );

    let fixtures = cross_machine_fixture_dir();
    let client = tls_client(&fixtures);
    let base = format!("https://127.0.0.1:{port}");
    let audience = normalize_audience(&Url::parse(&base).unwrap());

    let post_resp = client
        .post(format!("{base}/famp/v0.5.1/inbox/agent%3Ahostb.test%2Fbob"))
        .body(b"for-bob".to_vec())
        .send()
        .await
        .expect("post");
    assert_eq!(post_resp.status(), 202);

    // Fetch signed for the OTHER served domain (A), presented against B.
    let now = OffsetDateTime::now_utc();
    let wrong_signed = sign_fetch_auth(&sk_a, &audience, "hosta.test", now).expect("sign");
    let mut wrong_req = client.get(fetch_url(&base, "hostb.test"));
    for (name, value) in wrong_signed.header_pairs() {
        wrong_req = wrong_req.header(name, value);
    }
    let wrong_resp = wrong_req.send().await.expect("wrong fetch");
    assert_eq!(wrong_resp.status(), 403);

    // A subsequent CORRECTLY-signed fetch still returns the envelope —
    // proves the failed cross-domain attempt drained nothing.
    let correct_signed = sign_fetch_auth(&sk_b, &audience, "hostb.test", now).expect("sign");
    let mut correct_req = client.get(fetch_url(&base, "hostb.test"));
    for (name, value) in correct_signed.header_pairs() {
        correct_req = correct_req.header(name, value);
    }
    let correct_resp = correct_req.send().await.expect("correct fetch");
    assert_eq!(correct_resp.status(), 200);
    let body: serde_json::Value =
        serde_json::from_str(&correct_resp.text().await.expect("read body")).expect("json body");
    let envelopes = body["envelopes"].as_array().expect("envelopes array");
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0]["to"], "agent:hostb.test/bob");
}
