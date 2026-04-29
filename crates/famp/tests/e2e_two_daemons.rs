//! Phase 4 Plan 04-03 — E2E-01 automated two-daemon integration test.
//!
//! Drives the full `request → auto-commit → deliver × 4 → terminal → COMPLETED`
//! lifecycle across TWO independent `FAMP_HOME`s (Alice and Bob) with distinct
//! principals, distinct ephemeral ports, and mutual peer registration.
//!
//! This is the ROADMAP Phase 4 success criterion #3 gate:
//! ≥4 non-terminal deliver messages + 1 terminal deliver, automated.
//!
//! ## Flow
//!
//! 1. `spawn_two_daemons()` — Alice and Bob daemons up, peers registered.
//! 2. Alice sends `new_task` to Bob → `task_id` captured.
//! 3. Alice `await --task <id>` → receives auto-commit → A's record = COMMITTED.
//! 4. Seed a COMMITTED task record on Bob's side (one-sided task ownership
//!    is documented in 03-02-SUMMARY.md; the receiver does not auto-create
//!    a local record when it receives a request — only the sender does).
//!    This seed lets Bob call `send_structured(--task X)` in step 5.
//! 5. Four non-terminal delivers (A→B, B→A, A→B, B→A); each side awaits after
//!    receiving so the cursor advances.
//! 6. Alice sends terminal deliver → A's record = COMPLETED.
//! 7. Bob awaits the terminal deliver (consume it from B's inbox).
//! 8. Teardown + assert inbox counts.
//!
//! Phase 02 Plan 02-04: gated off — v0.8 HTTPS shape incompatible with
//! v0.9 bus path. The two-daemon harness still exercises the federation
//! HTTPS path through `cli::listen` directly, but `cli::send` no longer
//! drives it; Phase 4 will either delete this file or migrate it onto
//! a federation gateway harness once that lands.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use common::conversation_harness::{inbox_line_count, read_task};
use common::two_daemon_harness::spawn_two_daemons;

use famp::cli::await_cmd::{run_at_structured as await_structured, AwaitArgs};
use famp::cli::send::{run_at_structured as send_structured, SendArgs};
use famp_taskdir::{TaskDir, TaskRecord};

/// Helper: send a new-task request from `home` to peer `to_alias` with the
/// given summary. Returns the `task_id`.
async fn send_new_task(home: &std::path::Path, to_alias: &str, summary: &str) -> String {
    let outcome = send_structured(
        home,
        SendArgs {
            to: Some(to_alias.to_string()),
            channel: None,
            new_task: Some(summary.to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
        },
    )
    .await
    .expect("send new_task");
    outcome.task_id
}

/// Helper: send a non-terminal deliver from `home` to peer `to_alias`.
async fn send_deliver(home: &std::path::Path, to_alias: &str, task_id: &str, body: &str) {
    send_structured(
        home,
        SendArgs {
            to: Some(to_alias.to_string()),
            channel: None,
            new_task: None,
            task: Some(task_id.to_string()),
            terminal: false,
            body: Some(body.to_string()),
            more_coming: false,
            act_as: None,
        },
    )
    .await
    .expect("send deliver (non-terminal)");
}

/// Helper: send a terminal deliver from `home` to peer `to_alias`.
async fn send_terminal(home: &std::path::Path, to_alias: &str, task_id: &str) {
    send_structured(
        home,
        SendArgs {
            to: Some(to_alias.to_string()),
            channel: None,
            new_task: None,
            task: Some(task_id.to_string()),
            terminal: true,
            body: Some("done".to_string()),
            more_coming: false,
            act_as: None,
        },
    )
    .await
    .expect("send terminal deliver");
}

/// Helper: await one inbox entry at `home` with a 10s timeout and an optional
/// task filter.
async fn await_one(
    home: &std::path::Path,
    task_id: Option<&str>,
) -> famp::cli::await_cmd::AwaitOutcome {
    await_structured(
        home,
        AwaitArgs {
            timeout: "10s".to_string(),
            task: task_id.map(str::to_string),
        },
    )
    .await
    .expect("await_one")
}

/// Assert `run_list` filter behavior on the originator after a task has
/// reached COMPLETED.
///
/// - Default filter (`include_terminal=false`): **no** inbox entries for
///   `task_id` are surfaced (the taskdir record is terminal, so `run_list`
///   must hide them).
/// - Override (`include_terminal=true`): entries for `task_id` remain
///   visible. In this E2E the originator's inbound-only inbox holds the
///   commit reply + two non-terminal delivers from the peer (3 entries),
///   so we only need to assert ≥2.
fn assert_list_filters_completed_task(home: &std::path::Path, task_id: &str) {
    use famp::cli::inbox::list::run_list;

    let mut filtered = Vec::<u8>::new();
    run_list(home, None, /* include_terminal */ false, &mut filtered).unwrap();
    let filtered_text = String::from_utf8(filtered).unwrap();
    for line in filtered_text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_ne!(
            v["task_id"].as_str().unwrap(),
            task_id,
            "default filter must hide completed task entries: {line}",
        );
    }

    let mut unfiltered = Vec::<u8>::new();
    run_list(
        home,
        None,
        /* include_terminal */ true,
        &mut unfiltered,
    )
    .unwrap();
    let unfiltered_text = String::from_utf8(unfiltered).unwrap();
    let matching = unfiltered_text
        .lines()
        .filter(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["task_id"]
                .as_str()
                .unwrap()
                == task_id
        })
        .count();
    assert!(
        matching >= 2,
        "include_terminal=true surfaces the completed task's prior inbox entries: count={matching}",
    );
}

/// Seed a COMMITTED task record on the receiver side.
///
/// Phase 3/4 only creates local task records on the SENDER side (one-sided
/// task ownership — documented in 03-02-SUMMARY.md). The receiver (Bob) does
/// not auto-create a record when it receives a request. To allow Bob to send
/// delivers back (which requires a local task record), we manually seed a
/// COMMITTED record in Bob's tasks dir.
///
/// This represents what a future "receive-side task tracking" feature would do
/// automatically; for now it is an explicit test setup step.
fn seed_committed_record(home: &std::path::Path, task_id: &str, peer_alias: &str) {
    let tasks = TaskDir::open(home.join("tasks")).expect("open tasks dir");
    let mut rec = TaskRecord::new_requested(
        task_id.to_string(),
        peer_alias.to_string(),
        "2026-04-15T00:00:00Z".to_string(),
    );
    // The receiver auto-committed, so from Bob's perspective the task is COMMITTED.
    rec.state = "COMMITTED".to_string();
    tasks.create(&rec).expect("seed committed record");
}

/// Full lifecycle across two independent daemons:
///
/// ```text
/// Alice                Bob
///   |-- new_task ------->|   (request envelope)
///   |<-- auto-commit ----|   (daemon auto-reply)
///   Alice awaits commit; record REQUESTED → COMMITTED
///
///   [test seeds COMMITTED record on Bob's side for deliver path]
///
///   |-- deliver-1 ------>|
///   Bob awaits deliver-1
///   |<-- deliver-2 ------|
///   Alice awaits deliver-2
///   |-- deliver-3 ------>|
///   Bob awaits deliver-3
///   |<-- deliver-4 ------|
///   Alice awaits deliver-4
///   (4 non-terminal delivers total, 2 from each side)
///
///   |-- terminal ------->|
///   Alice: record COMPLETED
///   Bob awaits terminal
/// ```
#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; \
revisit / migrate in Phase 4 federation gateway"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn e2e_two_daemons_full_lifecycle() {
    famp::cli::send::client::allow_tofu_bootstrap_for_tests();

    // ── Setup ────────────────────────────────────────────────────────────────
    let daemons = spawn_two_daemons().await;
    let a_home = daemons.a_home.path().to_path_buf();
    let b_home = daemons.b_home.path().to_path_buf();

    // ── Step 1: Alice opens a new task to Bob ────────────────────────────────
    let task_id = send_new_task(&a_home, "bob", "hello from alice").await;

    // A's record should exist in REQUESTED state.
    let rec = read_task(&a_home, &task_id);
    assert_eq!(rec.state, "REQUESTED", "A: after new_task → REQUESTED");
    assert!(!rec.terminal);

    // B's inbox should have received the request.
    // Allow a brief moment for the auto-commit goroutine to fire.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(
        inbox_line_count(&b_home) >= 1,
        "B: inbox must have at least the request envelope"
    );

    // ── Step 2: Alice awaits the auto-commit reply ───────────────────────────
    // Bob's daemon auto-commits back to Alice; Alice awaits that commit.
    let commit_outcome = await_one(&a_home, Some(&task_id)).await;
    assert_eq!(
        commit_outcome.class, "commit",
        "A: first awaited entry must be the auto-commit reply"
    );

    // A's task record should now be COMMITTED.
    let rec = read_task(&a_home, &task_id);
    assert_eq!(
        rec.state, "COMMITTED",
        "A: after receiving commit → COMMITTED"
    );
    assert!(!rec.terminal);

    // A's inbox at this point: the commit reply from B (the auto-commit).
    // In a two-daemon setup A's outgoing request goes to B's inbox, NOT A's.
    // Only inbound envelopes appear in A's inbox.
    assert_eq!(
        inbox_line_count(&a_home),
        1,
        "A inbox: 1 line (commit reply from B)"
    );

    // ── Step 2b: Seed COMMITTED record on Bob's side ─────────────────────────
    // One-sided task ownership: Bob's daemon auto-committed but did NOT create
    // a local task record (that is the sender's responsibility). We seed one
    // so Bob can call famp send --task <id> for the deliver phase.
    seed_committed_record(&b_home, &task_id, "alice");

    // ── Step 3: Four non-terminal delivers (2 each side, interleaved) ────────
    // deliver-1: A → B
    send_deliver(&a_home, "bob", &task_id, "deliver 1 from alice").await;

    // Bob awaits deliver-1 (consume from B's inbox with task filter).
    let b_d1 = await_one(&b_home, Some(&task_id)).await;
    assert_eq!(b_d1.class, "deliver", "B: deliver-1 class");
    assert_eq!(
        b_d1.from, "agent:localhost/alice",
        "B: deliver-1 from Alice"
    );

    // deliver-2: B → A
    send_deliver(&b_home, "alice", &task_id, "deliver 2 from bob").await;

    // Alice awaits deliver-2.
    let a_d2 = await_one(&a_home, Some(&task_id)).await;
    assert_eq!(a_d2.class, "deliver", "A: deliver-2 class");
    assert_eq!(a_d2.from, "agent:localhost/bob", "A: deliver-2 from Bob");

    // deliver-3: A → B
    send_deliver(&a_home, "bob", &task_id, "deliver 3 from alice").await;

    // Bob awaits deliver-3.
    let b_d3 = await_one(&b_home, Some(&task_id)).await;
    assert_eq!(b_d3.class, "deliver", "B: deliver-3 class");

    // deliver-4: B → A
    send_deliver(&b_home, "alice", &task_id, "deliver 4 from bob").await;

    // Alice awaits deliver-4.
    let a_d4 = await_one(&a_home, Some(&task_id)).await;
    assert_eq!(a_d4.class, "deliver", "A: deliver-4 class");

    // 4 non-terminal delivers have been exchanged — ROADMAP criterion #3 met.
    // Suppress unused variable warnings for intermediate outcomes.
    let _ = (b_d3, a_d4);

    // ── Step 4: Assert inbox counts ─────────────────────────────────────────
    // A inbox (inbound only): commit(from B) + deliver-2(from B) + deliver-4(from B) = 3
    assert!(
        inbox_line_count(&a_home) >= 3,
        "A inbox: commit + 2 delivers from B (≥3 lines)"
    );
    // B inbox (inbound only): request(from A) + deliver-1(from A) + deliver-3(from A) = 3
    assert!(
        inbox_line_count(&b_home) >= 3,
        "B inbox: request + 2 delivers from A (≥3 lines)"
    );

    // ── Step 5: Terminal deliver A → B ───────────────────────────────────────
    send_terminal(&a_home, "bob", &task_id).await;

    // A's record should now be COMPLETED.
    let rec = read_task(&a_home, &task_id);
    assert_eq!(
        rec.state, "COMPLETED",
        "A: after terminal deliver → COMPLETED"
    );
    assert!(rec.terminal, "A: record marked terminal");

    // ── Step 6: Bob consumes the terminal deliver ─────────────────────────────
    let b_term = await_one(&b_home, Some(&task_id)).await;
    assert_eq!(b_term.class, "deliver", "B: terminal deliver class");
    // Verify it is a terminal deliver: `body.interim` must be false.
    let interim = b_term
        .body
        .get("interim")
        .and_then(serde_json::Value::as_bool);
    assert_eq!(
        interim,
        Some(false),
        "B: terminal deliver has interim=false"
    );

    // Bob's local record was seeded as COMMITTED (step 2b). The test does
    // not auto-advance Bob's record to COMPLETED (one-sided task ownership
    // is documented in 03-02-SUMMARY.md — this is acceptable for v0.8).

    // ── Step 7: Post-completion `inbox list` filter assertions ───────────────
    // After task is COMPLETED on the originator (A), calling `run_list` with
    // the default filter must return zero entries for that task_id. With
    // include_terminal=true, prior inbox entries for the task remain visible.
    // The taskdir terminal flip is confirmed synchronously above
    // (`rec.terminal` check after `send_terminal`), so no additional wait
    // is needed before `run_list` observes the filter.
    assert_list_filters_completed_task(&a_home, &task_id);

    // ── Teardown ─────────────────────────────────────────────────────────────
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
