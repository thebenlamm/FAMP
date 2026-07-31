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
// consumed only by Task 3's fetch-route additions to this same file (or
// by other test binaries) are not yet referenced here. Remove each line
// as this file's own Task 3 edit starts using it.
use base64 as _;
use famp_transport_http as _;
use reqwest as _;
use serde as _;
use serde_json as _;
use thiserror as _;
use tower_http as _;
use uuid as _;

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::Response;
use axum::Router;
use famp_crypto::TrustedVerifyingKey;
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
