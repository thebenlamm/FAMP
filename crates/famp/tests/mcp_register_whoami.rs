//! Integration tests for `famp_register` and `famp_whoami` MCP tools —
//! Phase 02 plan 02-09 v0.9 reshape.
//!
//! Pre-02-09 these tests asserted the v0.8 surface
//! (`{ identity, source, home }`) and the filesystem-backed identity
//! resolver (`unknown_identity` for missing agent dirs). The v0.9 surface
//! talks to the local UDS broker:
//!
//! - `famp_register(identity)` returns `{ active, drained, peers }` and
//!   no longer validates against `$FAMP_LOCAL_ROOT/agents/<name>/`
//!   (the broker accepts any well-formed name; collisions surface as
//!   `name_taken`, not `unknown_identity`).
//! - `famp_whoami()` returns `{ active, joined }` from the broker. When
//!   the session is unregistered the bus client is opened with
//!   `bind_as: None` and the broker returns `active: null, joined: []`.

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::mcp_harness::Harness;

/// Run `test` with a fresh `$FAMP_BUS_SOCKET` pointing into a private
/// tempdir. WR-06: scoped via `temp_env::with_var` so save+restore
/// survives panics and the call site doesn't need an `unsafe` block
/// under Rust 2024. The TempDir outlives the closure (created here,
/// dropped on return), and the MCP child spawned by `Harness::with_agents`
/// inherits the env at spawn time — by the time the closure returns and
/// `with_var` restores the parent env, the child already has its own
/// snapshot.
fn with_fresh_socket<F: FnOnce()>(test: F) {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("bus.sock");
    let sock_str = sock.to_string_lossy().into_owned();
    temp_env::with_var("FAMP_BUS_SOCKET", Some(sock_str.as_str()), test);
}

#[test]
fn register_valid_identity_succeeds() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&["alice"]);
        let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
        let body = Harness::ok_content(&r);
        assert_eq!(body["active"], "alice", "register response: {body}");
        assert!(
            body["drained"].is_number(),
            "drained must be a count, got: {body}"
        );
        assert!(body["peers"].is_array(), "peers must be array: {body}");

        let w = h.tool_call("famp_whoami", &serde_json::json!({}));
        let wb = Harness::ok_content(&w);
        assert_eq!(wb["active"], "alice", "whoami response: {wb}");
        assert!(
            wb["joined"].is_array(),
            "joined must be array (post-register): {wb}"
        );
    });
}

#[test]
fn register_invalid_name_returns_envelope_invalid() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let r = h.tool_call(
            "famp_register",
            &serde_json::json!({ "identity": "foo bar" }),
        );
        // v0.9 maps invalid name to envelope_invalid (the bus-layer
        // "well-formed-name regex failed" discriminator). Pre-02-09 this was
        // the v0.8-only `invalid_identity_name`.
        assert_eq!(Harness::error_kind(&r), "envelope_invalid");
    });
}

#[test]
fn register_with_empty_string_returns_envelope_invalid() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "" }));
        assert_eq!(Harness::error_kind(&r), "envelope_invalid");
    });
}

#[test]
fn register_idempotent_same_identity() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&["alice"]);
        let r1 = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
        assert!(
            r1.get("result").is_some(),
            "first register must succeed: {r1}"
        );
        // Second register on the SAME process / SAME identity. The broker
        // sees a name_taken or RegisterOk depending on whether it dedups by
        // pid; in v0.9 the same pid re-registering as the same name is not
        // a collision. Either way `whoami` must continue to report alice.
        let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
        let w = h.tool_call("famp_whoami", &serde_json::json!({}));
        let wb = Harness::ok_content(&w);
        assert_eq!(wb["active"], "alice", "whoami after re-register: {wb}");
    });
}

#[test]
fn whoami_unregistered_returns_null_active() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let w = h.tool_call("famp_whoami", &serde_json::json!({}));
        let wb = Harness::ok_content(&w);
        // Pre-registration: broker reports no bound name on this connection.
        assert!(wb["active"].is_null(), "expected null active, got {wb}");
        assert!(wb["joined"].is_array(), "joined must be array: {wb}");
        assert_eq!(
            wb["joined"].as_array().unwrap().len(),
            0,
            "joined must be empty pre-register: {wb}"
        );
    });
}

#[test]
fn tools_list_returns_eight_tools() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let r = h.call("tools/list", &serde_json::json!({}));
        let tools = r["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names.len(), 8, "expected 8 tools, got: {names:?}");
        for expected in [
            "famp_send",
            "famp_await",
            "famp_inbox",
            "famp_peers",
            "famp_register",
            "famp_whoami",
            "famp_join",
            "famp_leave",
        ] {
            assert!(
                names.contains(&expected),
                "missing tool: {expected}; got {names:?}"
            );
        }
    });
}
