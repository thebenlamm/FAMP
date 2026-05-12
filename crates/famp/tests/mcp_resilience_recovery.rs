//! Resilience hooks for the Claude Code "[Tool result missing due to
//! internal error]" failure mode.
//!
//! Claude Code's stdio MCP transport occasionally drops the JSON-RPC
//! response on its way back to the model even though the broker
//! processed the request. For `famp_send` this means the recipient's
//! mailbox has the message but the sender has no `task_id` to thread a
//! reply against and no idea whether to retry.
//!
//! Three tools cooperate to make recovery possible:
//!
//! 1. `famp_send` records `LastSend { task_id, to_peer|to_channel, ts }`
//!    on the per-process session state every time the broker confirms
//!    `SendOk`.
//! 2. `famp_whoami` surfaces that `last_send` record in its output. An
//!    agent that received a dropped `famp_send` response calls
//!    `famp_whoami` to recover the assigned `task_id`.
//! 3. `famp_verify` takes a `task_id` and looks it up in the inspector
//!    socket's envelope metadata, so the agent can confirm delivery
//!    BEFORE deciding to retry. `famp_verify` is FREE-PASS (no
//!    registration required) so it works even after a session restart.
//!
//! These tests exercise the round-trip through a single `famp mcp`
//! subprocess against an ephemeral broker. The broker's `send_agent`
//! handler appends to the recipient's mailbox unconditionally (the
//! mailbox is identity-keyed on disk, not gated on the recipient being
//! online), so sending to a never-registered peer still produces a
//! verifiable envelope — exactly the shape needed to assert the
//! resilience contract without spinning up two MCP processes.

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::mcp_harness::Harness;

/// Run `test` with a fresh `$FAMP_BUS_SOCKET` pointing into a private
/// tempdir. WR-06: scoped via `temp_env::with_var` so save+restore
/// survives panics and the call site doesn't need an `unsafe` block
/// under Rust 2024.
fn with_fresh_socket<F: FnOnce()>(test: F) {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("bus.sock");
    let sock_str = sock.to_string_lossy().into_owned();
    temp_env::with_var("FAMP_BUS_SOCKET", Some(sock_str.as_str()), test);
}

#[test]
fn whoami_carries_last_send_after_successful_send() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&["alice"]);

        // Pre-send: whoami exposes no last_send field.
        let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
        let pre = Harness::ok_content(&h.tool_call("famp_whoami", &serde_json::json!({})));
        assert!(
            pre.get("last_send").is_none(),
            "fresh session must not surface last_send: {pre}"
        );

        // Send alice→bob. bob is not registered; the broker still
        // appends to bob's mailbox (mailboxes are identity-keyed on
        // disk, not gated on the recipient being online), so we get a
        // real task_id back.
        let send = h.tool_call(
            "famp_send",
            &serde_json::json!({
                "peer": "bob",
                "mode": "open",
                "title": "hello",
                "body":  "round-trip resilience probe",
            }),
        );
        let sb = Harness::ok_content(&send);
        let send_task_id = sb["task_id"]
            .as_str()
            .unwrap_or_else(|| panic!("send missing task_id: {sb}"))
            .to_string();
        assert!(!send_task_id.is_empty(), "task_id must be non-empty: {sb}");

        // whoami now surfaces last_send mirroring what the dropped
        // response would have carried.
        let post = Harness::ok_content(&h.tool_call("famp_whoami", &serde_json::json!({})));
        let ls = post
            .get("last_send")
            .unwrap_or_else(|| panic!("post-send whoami missing last_send: {post}"));
        assert_eq!(
            ls["task_id"].as_str().unwrap_or(""),
            send_task_id,
            "last_send.task_id must equal the send's task_id: {post}"
        );
        assert_eq!(
            ls["to_peer"].as_str().unwrap_or(""),
            "bob",
            "last_send.to_peer must echo the recipient: {post}"
        );
        assert!(
            ls.get("to_channel").is_none(),
            "to_channel must be absent on agent-targeted send: {post}"
        );
        let ts = ls["ts"].as_str().unwrap_or("");
        assert!(
            ts.contains('T') && ts.ends_with('Z'),
            "last_send.ts must look like RFC 3339 UTC, got: {ts}"
        );
    });
}

#[test]
fn verify_returns_delivered_true_for_known_task_id() {
    // Historical context (pre-2026-05-12): the inspector's
    // `read_message_snapshot` only walked mailboxes for identities in
    // `BrokerStateView.clients`, which forced this test to register
    // both alice AND bob. After adversarial-review finding 2, verify
    // reads mailbox files directly and no longer cares whether the
    // recipient is registered. Bob's registration here is now redundant
    // for correctness but kept for parity with the typical wire shape
    // (registered → registered DM).
    //
    // The companion test `verify_finds_offline_recipient_envelope`
    // below covers the original gap (recipient never registers).
    let shared_root = tempfile::tempdir().unwrap();
    let root_path = shared_root.path().to_path_buf();
    let mut alice = Harness::with_local_root(&root_path, None);
    let mut bob = Harness::with_local_root(&root_path, None);

    let _ = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let _ = bob.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));

    let send = alice.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer":  "bob",
            "mode":  "open",
            "title": "verify-me",
        }),
    );
    let sb = Harness::ok_content(&send);
    let task_id = sb["task_id"]
        .as_str()
        .unwrap_or_else(|| panic!("send missing task_id: {sb}"))
        .to_string();

    // The recovery path: agent calls famp_verify with the task_id it
    // recovered from famp_whoami.last_send (or remembered out of band)
    // plus the recipient identity. Inspector RPC walks the recipient's
    // mailbox metadata and finds a match.
    let verify = alice.tool_call(
        "famp_verify",
        &serde_json::json!({
            "task_id": task_id,
            "peer":    "bob",
        }),
    );
    let vb = Harness::ok_content(&verify);
    assert_eq!(
        vb["delivered"].as_bool(),
        Some(true),
        "delivered must be true for an envelope that landed: {vb}"
    );
    assert_eq!(vb["task_id"].as_str().unwrap_or(""), task_id);
    // Row shape mirrors `famp-inspect-proto::MessageRow`.
    let row = &vb["row"];
    assert!(row.is_object(), "row must be present on hit: {vb}");
    assert_eq!(row["task_id"].as_str().unwrap_or(""), task_id);
    // sender / recipient are Principal-shaped strings like
    // `agent:local.bus/<name>` (cli::send::build_envelope_value).
    assert!(
        row["sender"].as_str().unwrap_or("").contains("alice"),
        "row.sender should mention alice: {row}"
    );
    assert!(
        row["recipient"].as_str().unwrap_or("").contains("bob"),
        "row.recipient should mention bob: {row}"
    );
    drop(alice);
    drop(bob);
    drop(shared_root);
}

#[test]
fn verify_finds_offline_recipient_envelope() {
    // Adversarial-review finding 2 (high): the prior verify
    // implementation bounced through `InspectKind::Messages`, which
    // only scans mailboxes for currently-registered canonical holders.
    // A recipient that crashed, exited, or has never registered has
    // its mailbox on disk (the broker writes envelopes unconditionally)
    // but was invisible to the inspector — leaving the agent with a
    // false `delivered: false` despite a durable write.
    //
    // After the fix, verify reads the mailbox JSONL file directly so
    // offline / never-registered recipients are covered. This test
    // exercises the offline case by NOT registering the recipient.
    let shared_root = tempfile::tempdir().unwrap();
    let root_path = shared_root.path().to_path_buf();
    let mut alice = Harness::with_local_root(&root_path, None);

    let _ = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    // Bob is NEVER registered. The broker still writes his mailbox.
    let send = alice.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer":  "bob",
            "mode":  "open",
            "title": "offline-verify",
        }),
    );
    let sb = Harness::ok_content(&send);
    let task_id = sb["task_id"].as_str().unwrap().to_string();

    let verify = alice.tool_call(
        "famp_verify",
        &serde_json::json!({ "task_id": task_id, "peer": "bob" }),
    );
    let vb = Harness::ok_content(&verify);
    assert_eq!(
        vb["delivered"].as_bool(),
        Some(true),
        "verify must find envelopes destined for never-registered recipients: {vb}"
    );
    assert_eq!(vb["task_id"].as_str().unwrap_or(""), task_id);
    let row = &vb["row"];
    assert!(row.is_object(), "row must be present on hit: {vb}");
    assert!(
        row["sender"].as_str().unwrap_or("").contains("alice"),
        "row.sender should mention alice: {row}"
    );
    drop(alice);
    drop(shared_root);
}

#[test]
fn verify_envelope_id_distinguishes_specific_reply_on_thread() {
    // Adversarial-review finding 4 (medium): matching only on
    // `causality.ref == thread_id` proves "some envelope on this
    // thread landed" but not "MY specific reply landed." When the
    // caller supplies envelope_id, verify additionally requires the
    // envelope's own id to match — closing the false-positive window.
    let shared_root = tempfile::tempdir().unwrap();
    let root_path = shared_root.path().to_path_buf();
    let mut alice = Harness::with_local_root(&root_path, None);
    let mut bob = Harness::with_local_root(&root_path, None);

    let _ = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let _ = bob.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));

    // Alice opens a thread.
    let open = alice.tool_call(
        "famp_send",
        &serde_json::json!({ "peer": "bob", "mode": "open", "title": "ping" }),
    );
    let ob = Harness::ok_content(&open);
    let thread_id = ob["task_id"].as_str().unwrap().to_string();

    // Bob replies twice on the same thread (both replies share
    // causality.ref = thread_id; each has a distinct envelope id).
    let reply_a = bob.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "alice",
            "mode": "reply",
            "task_id": thread_id,
            "body":  "pong-1",
            "expect_reply": true,
        }),
    );
    let ra = Harness::ok_content(&reply_a);
    let reply_a_id = ra["task_id"].as_str().unwrap().to_string();

    let reply_b = bob.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "alice",
            "mode": "reply",
            "task_id": thread_id,
            "body":  "pong-2",
            "expect_reply": true,
        }),
    );
    let rb = Harness::ok_content(&reply_b);
    let reply_b_id = rb["task_id"].as_str().unwrap().to_string();
    assert_ne!(reply_a_id, reply_b_id);

    // Thread-only verify hits (some reply on the thread landed).
    let v_thread = Harness::ok_content(&bob.tool_call(
        "famp_verify",
        &serde_json::json!({ "task_id": thread_id, "peer": "alice" }),
    ));
    assert_eq!(v_thread["delivered"].as_bool(), Some(true));

    // Verify with envelope_id = reply_b_id MUST find it (proves the
    // SECOND reply specifically landed, not just "something on thread").
    let v_specific = Harness::ok_content(&bob.tool_call(
        "famp_verify",
        &serde_json::json!({
            "task_id": thread_id,
            "peer": "alice",
            "envelope_id": reply_b_id,
        }),
    ));
    assert_eq!(
        v_specific["delivered"].as_bool(),
        Some(true),
        "verify with envelope_id must match the specific reply: {v_specific}"
    );

    // Verify with an envelope_id that's well-formed but not present
    // must miss, EVEN though a thread match exists.
    let bogus = "0193abcd-ef01-7fff-8fff-ffffffffffff";
    let v_bogus = Harness::ok_content(&bob.tool_call(
        "famp_verify",
        &serde_json::json!({
            "task_id": thread_id,
            "peer": "alice",
            "envelope_id": bogus,
        }),
    ));
    assert_eq!(
        v_bogus["delivered"].as_bool(),
        Some(false),
        "envelope_id mismatch must produce delivered=false even when thread matches: {v_bogus}"
    );

    drop(alice);
    drop(bob);
    drop(shared_root);
}

#[test]
fn whoami_last_send_captures_thread_task_id_on_reply_mode() {
    // Regression guard for the open-vs-reply task_id divergence
    // documented in `session::LastSend`: the inspector keys reply
    // envelopes by `causality.ref`, not by the reply envelope's own
    // id, so a recovery flow that verifies a reply landed MUST pass
    // the thread id, not the SendOk task_id of the reply itself.
    //
    // This test asserts:
    //   1. `mode="open"` records `task_id` (=== envelope id) and NO
    //      `thread_task_id` (it would be redundant).
    //   2. `mode="reply"` records BOTH:
    //        - `task_id` = the reply's own envelope id (from SendOk)
    //        - `thread_task_id` = the originating thread's task id
    //      and `thread_task_id` is what `famp_verify` accepts to
    //      confirm a reply landed (it matches the inspector's
    //      `MessageRow.task_id`).
    let shared_root = tempfile::tempdir().unwrap();
    let root_path = shared_root.path().to_path_buf();
    let mut alice = Harness::with_local_root(&root_path, None);
    let mut bob = Harness::with_local_root(&root_path, None);
    let _ = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let _ = bob.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));

    // Alice opens a thread.
    let open = alice.tool_call(
        "famp_send",
        &serde_json::json!({ "peer": "bob", "mode": "open", "title": "ping" }),
    );
    let ob = Harness::ok_content(&open);
    let thread_id = ob["task_id"].as_str().unwrap().to_string();

    // Open-mode whoami: thread_task_id is absent (None serializes as
    // omitted via skip_serializing_if).
    let w_open = Harness::ok_content(&alice.tool_call("famp_whoami", &serde_json::json!({})));
    let ls_open = w_open.get("last_send").expect("last_send present");
    assert_eq!(ls_open["task_id"].as_str().unwrap(), thread_id);
    assert!(
        ls_open.get("thread_task_id").is_none(),
        "open-mode must NOT record thread_task_id (it would dup task_id): {ls_open}"
    );

    // Bob replies (keep thread open so we can verify mid-thread).
    let reply = bob.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "alice",
            "mode": "reply",
            "task_id": thread_id,
            "body":  "pong",
            "expect_reply": true,
        }),
    );
    let rb = Harness::ok_content(&reply);
    let reply_envelope_id = rb["task_id"].as_str().unwrap().to_string();
    assert_ne!(
        reply_envelope_id, thread_id,
        "reply's SendOk.task_id is the new envelope id, distinct from the thread id"
    );

    // Reply-mode whoami: BOTH task_id and thread_task_id are present.
    let w_reply = Harness::ok_content(&bob.tool_call("famp_whoami", &serde_json::json!({})));
    let ls_reply = w_reply
        .get("last_send")
        .expect("bob's last_send present after reply");
    assert_eq!(
        ls_reply["task_id"].as_str().unwrap(),
        reply_envelope_id,
        "reply-mode task_id must equal SendOk.task_id (the new envelope id): {ls_reply}"
    );
    assert_eq!(
        ls_reply["thread_task_id"].as_str().unwrap_or(""),
        thread_id,
        "reply-mode thread_task_id must equal the originating thread id: {ls_reply}"
    );

    // famp_verify against thread_task_id finds the row in alice's
    // mailbox — this is the recovery path for a dropped reply send.
    let v = Harness::ok_content(&bob.tool_call(
        "famp_verify",
        &serde_json::json!({ "task_id": thread_id, "peer": "alice" }),
    ));
    assert_eq!(
        v["delivered"].as_bool(),
        Some(true),
        "thread_task_id must verify-true for an envelope-on-thread that landed: {v}"
    );

    drop(alice);
    drop(bob);
    drop(shared_root);
}

#[test]
fn verify_returns_delivered_false_for_unknown_task_id() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&["alice"]);
        let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
        // Send something so the broker is alive and inspector has rows
        // to scan — but verify a *different* task_id so we exercise the
        // miss path.
        let _ = h.tool_call(
            "famp_send",
            &serde_json::json!({ "peer": "bob", "mode": "open", "title": "x" }),
        );

        let verify = h.tool_call(
            "famp_verify",
            &serde_json::json!({
                // A well-formed but fabricated UUIDv7 the broker has never seen.
                "task_id": "0193abcd-ef01-7fff-8fff-ffffffffffff",
                "peer":    "bob",
            }),
        );
        let vb = Harness::ok_content(&verify);
        assert_eq!(
            vb["delivered"].as_bool(),
            Some(false),
            "delivered must be false for an absent task_id: {vb}"
        );
        assert!(vb.get("row").is_none(), "row must be omitted on miss: {vb}");
    });
}

#[test]
fn verify_is_free_pass_no_registration_required() {
    // The recovery hook MUST work after a session restart, when the
    // process has no active_identity. Verify the dispatcher does NOT
    // route this tool through the NotRegistered gate.
    //
    // We do call `famp_whoami` first — not to register, but to spawn
    // the broker via `session::ensure_bus`. `famp_whoami` is also
    // FREE-PASS so the session is still in the "no active_identity"
    // state when famp_verify runs. Without this priming step the test
    // would hit `broker_unreachable` (the inspector RPC does not start
    // brokers; only `bus_client::ensure_bus` does), which would mask
    // the actual registration-gate assertion.
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let w = h.tool_call("famp_whoami", &serde_json::json!({}));
        let wb = Harness::ok_content(&w);
        assert!(
            wb["active"].is_null(),
            "session must be unregistered for this test: {wb}"
        );

        // famp_verify must not return not_registered. The broker has
        // never seen this task_id so delivered=false is expected — what
        // we're asserting is that we DO NOT get a NotRegistered error.
        let verify = h.tool_call(
            "famp_verify",
            &serde_json::json!({
                "task_id": "0193abcd-ef01-7fff-8fff-ffffffffffff",
            }),
        );
        assert!(
            verify.get("error").is_none(),
            "famp_verify must be FREE-PASS (no registration gate); got error: {verify}"
        );
        let vb = Harness::ok_content(&verify);
        assert_eq!(vb["delivered"].as_bool(), Some(false));
    });
}

#[test]
fn verify_rejects_missing_task_id() {
    with_fresh_socket(|| {
        let mut h = Harness::with_agents(&[]);
        let verify = h.tool_call("famp_verify", &serde_json::json!({}));
        assert_eq!(
            Harness::error_kind(&verify),
            "envelope_invalid",
            "missing task_id must surface envelope_invalid: {verify}"
        );
    });
}
