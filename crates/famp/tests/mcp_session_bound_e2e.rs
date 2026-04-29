//! Phase 1 (session-bound MCP identity) — two-window E2E.
//!
//! Spawns TWO `famp mcp` subprocesses against a shared `FAMP_LOCAL_ROOT`
//! containing two pre-initialized agents. Each MCP server registers as
//! a different identity. Drives a full `request → commit → deliver →
//! terminal` cycle through real signed envelopes between the two daemons.
//!
//! Spec: docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md
//!   (acceptance criteria #1, #3, #4, #5)
//! Plan: .planning/phases/01-session-bound-mcp-identity/01-05-PLAN.md

#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::mcp_harness::Harness;
use common::two_daemon_harness::spawn_two_daemons_under_local_root;

use famp_taskdir::{TaskDir, TaskRecord};

/// Seed a COMMITTED task record on the receiver side (bob).
///
/// One-sided task ownership: Bob's daemon auto-committed but did NOT create a
/// local task record. This seed lets Bob call `famp_send --task <id>` for the
/// deliver phase. Same pattern as `e2e_two_daemons.rs`.
fn seed_committed_record(home: &std::path::Path, task_id: &str, peer_alias: &str) {
    let tasks = TaskDir::open(home.join("tasks")).expect("open tasks dir");
    let mut rec = TaskRecord::new_requested(
        task_id.to_string(),
        peer_alias.to_string(),
        "2026-04-26T00:00:00Z".to_string(),
    );
    rec.state = "COMMITTED".to_string();
    tasks.create(&rec).expect("seed committed record");
}

/// Read a task record from `<home>/tasks/<task_id>.toml`.
fn read_task(home: &std::path::Path, task_id: &str) -> TaskRecord {
    let tasks = TaskDir::open(home.join("tasks")).expect("open tasks dir");
    tasks.read(task_id).expect("read task record")
}

/// Assert bob's inbox contains the given `task_id`. `include_terminal=true` so
/// the request envelope (`class=request`) is visible even after bob's daemon
/// auto-committed it.
fn assert_bob_inbox_has_task(win_b: &mut Harness, task_id: &str) {
    let resp = win_b.tool_call(
        "famp_inbox",
        &serde_json::json!({ "action": "list", "include_terminal": true }),
    );
    let body = Harness::ok_content(&resp);
    let entries = body["entries"].as_array().expect("entries array");
    assert!(
        entries
            .iter()
            .any(|e| e["task_id"].as_str() == Some(task_id)),
        "bob inbox missing task_id {task_id}; entries: {entries:?}"
    );
}

/// Full two-window lifecycle:
///
/// ```text
/// Window-A (alice)               Window-B (bob)
///   |-- famp_register(alice) --|
///                               |-- famp_register(bob) --|
///   |-- famp_whoami --> "alice" |
///                               |-- famp_whoami --> "bob" |
///   |-- famp_send new_task -----|----> daemon-bob --------|
///   |<-- famp_await commit -----|<---- auto-commit -------|
///   [test seeds COMMITTED on bob's filesystem]
///                               |-- famp_inbox list → sees task |
///                               |-- famp_send deliver ---|----> daemon-alice --|
///   |<-- famp_await deliver ----|<--------------------------|
///                               |-- famp_send terminal --|----> daemon-alice --|
///   |<-- famp_await terminal ---|<----------------------------|
///   [assert alice's task record == COMPLETED + terminal=true]
/// ```
#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; \
revisit / migrate in Phase 4 federation gateway"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_windows_register_as_different_identities_and_full_lifecycle() {
    famp::cli::send::client::allow_tofu_bootstrap_for_tests();

    let local_root = tempfile::tempdir().expect("tempdir");
    let daemons = spawn_two_daemons_under_local_root(local_root.path()).await;
    let a_home = daemons.a_home.clone();
    let b_home = daemons.b_home.clone();

    // Two MCP server subprocesses sharing the same local_root —
    // each represents one Claude Code / Codex window in the spec UX.
    let mut win_a = Harness::with_local_root(local_root.path(), None);
    let mut win_b = Harness::with_local_root(local_root.path(), None);

    // ── Step 1: each window registers as a different identity ──────────────────
    let r_a = win_a.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(r_a.get("result").is_some(), "win_a register failed: {r_a}");
    let r_b = win_b.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));
    assert!(r_b.get("result").is_some(), "win_b register failed: {r_b}");

    // Cross-process isolation gate.
    let w_a = Harness::ok_content(&win_a.tool_call("famp_whoami", &serde_json::json!({})));
    assert_eq!(w_a["identity"], "alice", "win_a whoami: {w_a}");
    let w_b = Harness::ok_content(&win_b.tool_call("famp_whoami", &serde_json::json!({})));
    assert_eq!(w_b["identity"], "bob", "win_b whoami: {w_b}");

    // ── Step 2: alice opens a new task to bob ──────────────────────────────────
    let send_resp = win_a.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "bob", "mode": "new_task", "title": "hello", "body": "hello bob"
        }),
    );
    let task_id = Harness::ok_content(&send_resp)["task_id"]
        .as_str()
        .expect("task_id")
        .to_string();

    // ── Step 3: alice awaits auto-commit ───────────────────────────────────────
    let aw = Harness::ok_content(&win_a.tool_call(
        "famp_await",
        &serde_json::json!({
            "timeout_seconds": 10, "task_id": task_id
        }),
    ));
    assert_eq!(aw["class"], "commit", "alice expected commit, got: {aw}");

    // ── Step 4: seed COMMITTED on bob's side (one-sided task ownership) ────────
    seed_committed_record(&b_home, &task_id, "alice");

    // ── Step 5: bob verifies inbox contains the request ────────────────────────
    assert_bob_inbox_has_task(&mut win_b, &task_id);

    // ── Step 6: bob sends a non-terminal deliver to alice ─────────────────────
    let r = win_b.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "alice", "mode": "deliver", "task_id": task_id, "body": "ack"
        }),
    );
    assert!(r.get("result").is_some(), "win_b deliver failed: {r}");

    // ── Step 7: alice awaits deliver ──────────────────────────────────────────
    let ad = Harness::ok_content(&win_a.tool_call(
        "famp_await",
        &serde_json::json!({
            "timeout_seconds": 10, "task_id": task_id
        }),
    ));
    assert_eq!(ad["class"], "deliver", "alice expected deliver, got: {ad}");

    // ── Step 8: bob sends terminal ────────────────────────────────────────────
    let r = win_b.tool_call(
        "famp_send",
        &serde_json::json!({
            "peer": "alice", "mode": "terminal", "task_id": task_id, "body": "done"
        }),
    );
    assert!(r.get("result").is_some(), "win_b terminal failed: {r}");

    // ── Step 9: alice awaits terminal deliver ─────────────────────────────────
    let at = Harness::ok_content(&win_a.tool_call(
        "famp_await",
        &serde_json::json!({
            "timeout_seconds": 10, "task_id": task_id
        }),
    ));
    assert_eq!(
        at["class"], "deliver",
        "alice expected terminal deliver: {at}"
    );
    assert_eq!(
        at["body"]["interim"].as_bool(),
        Some(false),
        "terminal deliver must have interim=false, got: {}",
        at["body"]
    );

    // ── Step 10: alice's task record must be COMPLETED ────────────────────────
    let rec = read_task(&a_home, &task_id);
    assert_eq!(rec.state, "COMPLETED", "alice record: {rec:?}");
    assert!(rec.terminal, "alice record must be marked terminal");

    drop(win_a);
    drop(win_b);
    daemons.teardown().await;
}

// Unused-crate-dependency silencers (nextest requires all workspace deps to be
// referenced even if only used transitively).
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use humantime as _;
use rand as _;
use rcgen as _;
use reqwest as _;
use rustls as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use tempfile as _;
use thiserror as _;
use time as _;
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
