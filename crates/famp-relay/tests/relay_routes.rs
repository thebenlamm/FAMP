//! In-process route tests for `famp-relay`'s axum router, driven via
//! `tower::ServiceExt::oneshot` — mirrors the `post_inbox_raw` helper
//! shape in `famp-gateway`'s `ingress.rs` tests.
//!
//! Deliberately in `tests/`, not an inline `src/http.rs` module: it
//! keeps `src/` free of test-only body parsing so the crate's opacity
//! gate (`grep -c 'serde_json::from' crates/famp-relay/src/*.rs` == 0) is
//! a statement about PRODUCTION code and stays mechanically checkable.

#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silencer: this integration test binary is its own compilation unit,
// separate from the lib target's `#[cfg(test)]` unit tests — dependencies
// consumed only by other test binaries are not referenced here.
use assert_cmd as _;
use famp_transport_http as _;
use reqwest as _;
use serde as _;
use thiserror as _;
use tower_http as _;
use uuid as _;

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::Response;
use axum::Router;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_relay::fetch_auth::{sign_fetch_auth, RELAY_FETCH_ROUTE};
use famp_relay::http::{build_relay_router, RelayState};
use famp_relay::queue::RELAY_FETCH_MAX_BATCH;
use time::OffsetDateTime;
use tower::ServiceExt;

fn state_with_domain(domain: &str) -> RelayState {
    let mut domains: HashMap<String, Vec<TrustedVerifyingKey>> = HashMap::new();
    domains.insert(domain.to_owned(), Vec::new());
    RelayState::new(domains, Arc::from("https://relay.test"))
}

fn inbox_uri(principal: &str) -> String {
    let mut url = url::Url::parse("http://relay.test/").expect("base url");
    {
        let mut segs = url.path_segments_mut().expect("cannot-be-base url");
        segs.pop_if_empty();
        segs.extend(["famp", "v0.5.1", "inbox"]);
        segs.push(principal);
    }
    url.path().to_string()
}

async fn post_raw(router: Router, uri: &str, body: Vec<u8>) -> Response {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .body(axum::body::Body::from(body))
        .expect("build request");
    router.oneshot(req).await.expect("router call")
}

#[tokio::test]
async fn post_to_served_domain_returns_202_and_increases_depth() {
    let state = state_with_domain("hosta.test");
    let queues = state.queues();
    let router = build_relay_router(state);

    let resp = post_raw(
        router,
        &inbox_uri("agent:hosta.test/alice"),
        b"hello".to_vec(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert_eq!(queues.lock().await.depth("hosta.test"), 1);
}

#[tokio::test]
async fn post_to_unserved_domain_returns_404_and_queues_nothing() {
    let state = state_with_domain("hosta.test");
    let queues = state.queues();
    let router = build_relay_router(state);

    let resp = post_raw(
        router,
        &inbox_uri("agent:hostb.test/bob"),
        b"hello".to_vec(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(queues.lock().await.depth("hostb.test"), 0);
    assert_eq!(queues.lock().await.depth("hosta.test"), 0);
}

#[tokio::test]
async fn post_with_unparseable_principal_path_returns_400_and_queues_nothing() {
    let state = state_with_domain("hosta.test");
    let queues = state.queues();
    let router = build_relay_router(state);

    let resp = post_raw(
        router,
        "/famp/v0.5.1/inbox/not-a-principal",
        b"hello".to_vec(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(queues.lock().await.depth("hosta.test"), 0);
}

#[tokio::test]
async fn recipient_and_bytes_round_trip_verbatim_including_invalid_utf8() {
    let state = state_with_domain("hosta.test");
    let queues = state.queues();
    let router = build_relay_router(state);

    let invalid_utf8 = vec![0xFFu8, 0xFE, 0x00, 0x80];
    let resp = post_raw(
        router,
        &inbox_uri("agent:hosta.test/alice"),
        invalid_utf8.clone(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let drained = queues.lock().await.drain(
        "hosta.test",
        OffsetDateTime::now_utc(),
        RELAY_FETCH_MAX_BATCH,
    );
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].recipient, "agent:hosta.test/alice");
    assert_eq!(drained[0].bytes, invalid_utf8);
}

#[tokio::test]
async fn accepted_response_has_empty_body() {
    let state = state_with_domain("hosta.test");
    let router = build_relay_router(state);
    let resp = post_raw(router, &inbox_uri("agent:hosta.test/alice"), vec![1]).await;
    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    assert!(body.is_empty());
}

// ---------------------------------------------------------------------
// Fetch route (GET RELAY_FETCH_ROUTE) — signed-fetch authorization.
// ---------------------------------------------------------------------

const AUDIENCE: &str = "https://relay.test";

fn state_with_key(domain: &str, vk: TrustedVerifyingKey) -> RelayState {
    let mut domains: HashMap<String, Vec<TrustedVerifyingKey>> = HashMap::new();
    domains.insert(domain.to_owned(), vec![vk]);
    RelayState::new(domains, Arc::from(AUDIENCE))
}

// Not a format string — `RELAY_FETCH_ROUTE` is a literal axum route
// pattern (`/famp/relay/v1/fetch/{domain}`) whose `{domain}` placeholder
// is being substituted for a concrete test value, unrelated to
// `format!`/`println!`.
#[allow(clippy::literal_string_with_formatting_args)]
fn fetch_uri(domain: &str) -> String {
    RELAY_FETCH_ROUTE.replace("{domain}", domain)
}

async fn get_with_headers(router: Router, uri: &str, headers: &[(&str, &str)]) -> Response {
    let mut builder = axum::http::Request::builder().method("GET").uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let req = builder
        .body(axum::body::Body::empty())
        .expect("build request");
    router.oneshot(req).await.expect("router call")
}

async fn response_json(resp: Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("response body must be JSON")
}

#[tokio::test]
async fn fetch_with_valid_signed_auth_returns_envelopes_and_empties_queue() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let state = state_with_key("hosta.test", vk);
    let queues = state.queues();
    let router = build_relay_router(state);

    // Seed the queue via a real POST first.
    let router_for_post = router.clone();
    let resp = post_raw(
        router_for_post,
        &inbox_uri("agent:hosta.test/alice"),
        b"hello".to_vec(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let now = OffsetDateTime::now_utc();
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", now).expect("sign");
    let resp = get_with_headers(router, &fetch_uri("hosta.test"), &signed.header_pairs()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp).await;
    let envelopes = body["envelopes"].as_array().expect("envelopes array");
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0]["to"], "agent:hosta.test/alice");
    let decoded = URL_SAFE_NO_PAD
        .decode(envelopes[0]["body"].as_str().expect("body string"))
        .expect("valid base64url");
    assert_eq!(decoded, b"hello");
    assert_eq!(queues.lock().await.depth("hosta.test"), 0);
}

#[tokio::test]
async fn fetch_with_no_authorization_headers_returns_400_and_drains_nothing() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let state = state_with_key("hosta.test", vk);
    let queues = state.queues();
    let router = build_relay_router(state);

    post_raw(
        router.clone(),
        &inbox_uri("agent:hosta.test/alice"),
        b"hello".to_vec(),
    )
    .await;

    let resp = get_with_headers(router, &fetch_uri("hosta.test"), &[]).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(queues.lock().await.depth("hosta.test"), 1);
}

#[tokio::test]
async fn fetch_signed_for_a_different_domain_returns_403_and_drains_neither_queue() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let mut domains: HashMap<String, Vec<TrustedVerifyingKey>> = HashMap::new();
    domains.insert("hosta.test".to_owned(), vec![vk.clone()]);
    domains.insert("hostb.test".to_owned(), vec![vk]);
    let state = RelayState::new(domains, Arc::from(AUDIENCE));
    let queues = state.queues();
    let router = build_relay_router(state);

    post_raw(
        router.clone(),
        &inbox_uri("agent:hostb.test/bob"),
        b"hello".to_vec(),
    )
    .await;

    let now = OffsetDateTime::now_utc();
    // Signed for domain A, presented against domain B.
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", now).expect("sign");
    let resp = get_with_headers(router, &fetch_uri("hostb.test"), &signed.header_pairs()).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        queues.lock().await.depth("hostb.test"),
        1,
        "domain B must not have drained"
    );
    assert_eq!(queues.lock().await.depth("hosta.test"), 0);
}

#[tokio::test]
async fn fetch_signed_by_a_key_not_configured_for_domain_returns_403() {
    let sk = FampSigningKey::generate();
    let (_other_sk, other_vk) = {
        let s = FampSigningKey::generate();
        let vk = s.verifying_key();
        (s, vk)
    };
    let state = state_with_key("hosta.test", other_vk);
    let router = build_relay_router(state);

    let now = OffsetDateTime::now_utc();
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", now).expect("sign");
    let resp = get_with_headers(router, &fetch_uri("hosta.test"), &signed.header_pairs()).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn fetch_with_stale_timestamp_returns_401_and_drains_nothing() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let state = state_with_key("hosta.test", vk);
    let queues = state.queues();
    let router = build_relay_router(state);

    post_raw(
        router.clone(),
        &inbox_uri("agent:hosta.test/alice"),
        b"hello".to_vec(),
    )
    .await;

    let stale = OffsetDateTime::now_utc() - time::Duration::hours(1);
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", stale).expect("sign");
    let resp = get_with_headers(router, &fetch_uri("hosta.test"), &signed.header_pairs()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(queues.lock().await.depth("hosta.test"), 1);
}

#[tokio::test]
async fn replaying_a_successful_authorization_verbatim_returns_403_and_drains_nothing() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let state = state_with_key("hosta.test", vk);
    let queues = state.queues();
    let router = build_relay_router(state);

    post_raw(
        router.clone(),
        &inbox_uri("agent:hosta.test/alice"),
        b"first".to_vec(),
    )
    .await;

    let now = OffsetDateTime::now_utc();
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", now).expect("sign");

    let first = get_with_headers(
        router.clone(),
        &fetch_uri("hosta.test"),
        &signed.header_pairs(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(queues.lock().await.depth("hosta.test"), 0);

    post_raw(
        router.clone(),
        &inbox_uri("agent:hosta.test/alice"),
        b"second".to_vec(),
    )
    .await;
    assert_eq!(queues.lock().await.depth("hosta.test"), 1);

    let replay = get_with_headers(router, &fetch_uri("hosta.test"), &signed.header_pairs()).await;
    assert_eq!(replay.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        queues.lock().await.depth("hosta.test"),
        1,
        "replay must not drain the second, legitimately-queued envelope"
    );
}

#[tokio::test]
async fn fetch_for_never_configured_domain_returns_404() {
    let state = state_with_domain("hosta.test");
    let router = build_relay_router(state);

    let resp = get_with_headers(router, &fetch_uri("nobody.test"), &[]).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn each_fetch_failure_has_a_distinct_slug() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let state = state_with_key("hosta.test", vk);
    let router = build_relay_router(state);

    let resp = get_with_headers(router.clone(), &fetch_uri("nobody.test"), &[]).await;
    let body = response_json(resp).await;
    assert_eq!(body["error"], "unknown_domain");

    let resp = get_with_headers(router.clone(), &fetch_uri("hosta.test"), &[]).await;
    let body = response_json(resp).await;
    assert_eq!(body["error"], "missing_fetch_auth");

    let now = OffsetDateTime::now_utc();
    let signed =
        sign_fetch_auth(&sk, "https://wrong-audience.test", "hosta.test", now).expect("sign");
    let resp = get_with_headers(
        router.clone(),
        &fetch_uri("hosta.test"),
        &signed.header_pairs(),
    )
    .await;
    let body = response_json(resp).await;
    assert_eq!(body["error"], "fetch_audience_mismatch");

    let stale = now - time::Duration::hours(1);
    let signed = sign_fetch_auth(&sk, AUDIENCE, "hosta.test", stale).expect("sign");
    let resp = get_with_headers(
        router.clone(),
        &fetch_uri("hosta.test"),
        &signed.header_pairs(),
    )
    .await;
    let body = response_json(resp).await;
    assert_eq!(body["error"], "stale_fetch");
}
