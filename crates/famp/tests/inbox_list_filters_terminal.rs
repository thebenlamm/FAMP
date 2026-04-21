//! Tests for `famp_inbox` filter semantics (spec 2026-04-20).
//!
//! Task 1 covers the `extract_task_id` helper; Tasks 2-4 extend with
//! filter, fail-open/fail-closed, cache, and MCP round-trips.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::cli::inbox::list::extract_task_id_for_test;
use famp_core::MessageClass;
use serde_json::json;

/// Every `MessageClass` variant must either yield a non-empty `task_id`
/// or be explicitly handled. A new variant that lands its id outside
/// the currently-understood envelope shape fails this test.
#[test]
fn extract_task_id_covers_every_message_class() {
    let cases: &[(MessageClass, &str)] = &[
        (MessageClass::Request, "01913000-0000-7000-8000-00000000000a"),
        (MessageClass::Commit, "01913000-0000-7000-8000-00000000000b"),
        (MessageClass::Deliver, "01913000-0000-7000-8000-00000000000c"),
        (MessageClass::Ack, "01913000-0000-7000-8000-00000000000d"),
        (MessageClass::Control, "01913000-0000-7000-8000-00000000000e"),
    ];

    for (class, expected_tid) in cases {
        let value = match class {
            // `request`: envelope's own `id` IS the task_id.
            MessageClass::Request => json!({
                "id": expected_tid,
                "class": class.to_string(),
            }),
            // Every other class: task_id lives in `causality.ref`.
            _ => json!({
                "id": "01913000-0000-7000-8000-0000000000ff",
                "class": class.to_string(),
                "causality": { "ref": expected_tid },
            }),
        };
        let extracted = extract_task_id_for_test(&value);
        assert_eq!(
            extracted,
            *expected_tid,
            "class={class} extracted={extracted:?} expected={expected_tid:?}",
        );
    }
}

use famp_taskdir::{TaskDir, TaskRecord};
use std::path::Path;

fn write_inbox(home: &Path, lines: &[serde_json::Value]) {
    let mut body = Vec::<u8>::new();
    for line in lines {
        body.extend_from_slice(serde_json::to_string(line).unwrap().as_bytes());
        body.push(b'\n');
    }
    std::fs::write(home.join("inbox.jsonl"), body).unwrap();
}

fn seed_taskdir(home: &Path, task_id: &str, peer: &str, terminal: bool) {
    let tasks = home.join("tasks");
    let dir = TaskDir::open(&tasks).unwrap();
    let mut rec = TaskRecord::new_requested(
        task_id.to_string(),
        peer.to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    );
    if terminal {
        rec.state = "COMPLETED".to_string();
        rec.terminal = true;
    }
    dir.create(&rec).unwrap();
}

const TID_ACTIVE: &str = "01913000-0000-7000-8000-0000000000a1";
const TID_DONE: &str = "01913000-0000-7000-8000-0000000000a2";

fn fixture_entries() -> [serde_json::Value; 4] {
    [
        json!({
            "id": TID_ACTIVE,
            "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE },
            "body": { "text": "active-request" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000b1",
            "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE },
            "body": { "text": "active-deliver" },
        }),
        json!({
            "id": TID_DONE,
            "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "done-request" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000b2",
            "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "done-deliver" },
        }),
    ]
}

#[test]
fn list_hides_entries_for_terminal_tasks_by_default() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, /* include_terminal */ false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "only active task entries visible: {text}");
    for l in &lines {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["task_id"].as_str().unwrap(), TID_ACTIVE);
    }
}

#[test]
fn list_include_terminal_returns_all_entries() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, /* include_terminal */ true, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert_eq!(
        text.lines().count(),
        4,
        "all four entries returned with override"
    );
}

#[test]
fn list_fail_open_on_missing_taskdir_record() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    // Seed inbox but NO taskdir records at all.
    write_inbox(&home, &fixture_entries());

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert_eq!(
        text.lines().count(),
        4,
        "missing taskdir records fail-open (surface entry): {text}"
    );
}

#[test]
fn list_fail_closed_on_corrupt_taskdir_record() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    // Seed one valid (active) and one *corrupt* record for TID_DONE.
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    let corrupt_path = home.join("tasks").join(format!("{TID_DONE}.toml"));
    std::fs::write(&corrupt_path, "this is not valid toml ===").unwrap();

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "corrupt record → fail-closed → entry hidden: {text}"
    );
    for l in &lines {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["task_id"].as_str().unwrap(), TID_ACTIVE);
    }
}

#[test]
fn list_caches_taskdir_reads_within_one_call() {
    // Three entries all referencing the same terminal task.
    // Behavioural assertion: all three entries are hidden uniformly —
    // same answer every time (cache returns consistent verdict).
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    let entries = vec![
        json!({
            "id": TID_DONE, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e1" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000c1", "class": "commit",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e2" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000c2", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e3" },
        }),
    ];
    write_inbox(&home, &entries);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(
        text.lines().next().is_none(),
        "all three entries for the same terminal task are hidden: {text}"
    );
}

// Silencers — match the convention in inbox_list_respects_cursor.rs.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
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
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tokio as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
