//! Integration tests for `famp_register` and `famp_whoami` MCP tools.
//! Spec: docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md
//! Phase plan: .planning/phases/01-session-bound-mcp-identity/01-03-PLAN.md

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::mcp_harness::Harness;

#[test]
fn register_valid_identity_succeeds() {
    let mut h = Harness::with_agents(&["alice"]);
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let body = Harness::ok_content(&r);
    assert_eq!(body["identity"], "alice");
    assert_eq!(body["source"], "explicit");
    assert!(
        body["home"].as_str().unwrap().ends_with("/agents/alice"),
        "home path should end with /agents/alice, got: {}",
        body["home"]
    );

    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert_eq!(wb["identity"], "alice");
    assert_eq!(wb["source"], "explicit");
}

#[test]
fn register_invalid_name_returns_invalid_identity_name() {
    let mut h = Harness::with_agents(&[]);
    let r = h.tool_call(
        "famp_register",
        &serde_json::json!({ "identity": "foo bar" }),
    );
    assert_eq!(Harness::error_kind(&r), "invalid_identity_name");
}

#[test]
fn register_with_empty_string_returns_invalid_identity_name() {
    let mut h = Harness::with_agents(&[]);
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "" }));
    assert_eq!(Harness::error_kind(&r), "invalid_identity_name");
}

#[test]
fn register_unknown_identity_returns_unknown_identity() {
    let mut h = Harness::with_agents(&["alice"]); // bob is NOT initialized
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));
    assert_eq!(Harness::error_kind(&r), "unknown_identity");
}

#[test]
fn register_idempotent_same_identity() {
    let mut h = Harness::with_agents(&["alice"]);
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(
        r.get("result").is_some(),
        "second register must succeed: {r}"
    );
    let body = Harness::ok_content(&r);
    assert_eq!(body["identity"], "alice");
}

#[test]
fn register_replaces_with_different_identity() {
    let mut h = Harness::with_agents(&["alice", "bob"]);
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));
    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert_eq!(wb["identity"], "bob");
}

#[test]
fn whoami_unregistered_returns_null() {
    let mut h = Harness::with_agents(&[]);
    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert!(wb["identity"].is_null(), "expected null, got {wb}");
    assert_eq!(wb["source"], "unregistered");
}

#[test]
fn tools_list_returns_six_tools() {
    let mut h = Harness::with_agents(&[]);
    let r = h.call("tools/list", &serde_json::json!({}));
    let tools = r["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names.len(), 6, "expected 6 tools, got: {names:?}");
    for expected in [
        "famp_send",
        "famp_await",
        "famp_inbox",
        "famp_peers",
        "famp_register",
        "famp_whoami",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool: {expected}; got {names:?}"
        );
    }
}
