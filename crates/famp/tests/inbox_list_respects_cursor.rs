//! Phase 3 Plan 03-03 Task 2 — `famp inbox list` + `famp inbox ack`.
//!
//! Hand-writes a three-line fixture into `inbox.jsonl` (no daemon needed —
//! list is a pure file reader), asserts `inbox list` prints all three in
//! order with increasing offsets, asserts `inbox ack <mid_offset>`
//! advances the cursor without printing, and asserts
//! `inbox list --since <mid>` prints only the trailing entry. Also
//! verifies `list` itself does NOT advance the cursor.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use famp::cli::inbox::{ack::run_ack, list::run_list};
use famp_inbox::InboxCursor;

use common::init_home_in_process;

const LINE1: &str = "{\"task_id\":\"01913000-0000-7000-8000-000000000001\",\"from\":\"agent:localhost/self\",\"class\":\"request\",\"body\":{\"text\":\"one\"}}\n";
const LINE2: &str = "{\"task_id\":\"01913000-0000-7000-8000-000000000002\",\"from\":\"agent:localhost/self\",\"class\":\"deliver\",\"body\":{\"text\":\"two\"}}\n";
const LINE3: &str = "{\"task_id\":\"01913000-0000-7000-8000-000000000003\",\"from\":\"agent:localhost/self\",\"class\":\"deliver\",\"body\":{\"text\":\"three\"}}\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_list_and_ack_honor_cursor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Handcrafted fixture — three complete JSONL lines.
    let mut body = Vec::new();
    body.extend_from_slice(LINE1.as_bytes());
    body.extend_from_slice(LINE2.as_bytes());
    body.extend_from_slice(LINE3.as_bytes());
    std::fs::write(home.join("inbox.jsonl"), &body).unwrap();

    // list (no --since) prints all three, in order, with strictly
    // increasing offsets that match byte boundaries.
    let mut buf = Vec::<u8>::new();
    run_list(&home, None, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "list prints all three: {text}");

    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let off1 = parsed[0]["offset"].as_u64().unwrap();
    let off2 = parsed[1]["offset"].as_u64().unwrap();
    let off3 = parsed[2]["offset"].as_u64().unwrap();
    assert_eq!(off1, LINE1.len() as u64);
    assert_eq!(off2, (LINE1.len() + LINE2.len()) as u64);
    assert_eq!(off3, body.len() as u64);
    assert_eq!(
        parsed[0]["body"]["text"].as_str().unwrap(),
        "one"
    );
    assert_eq!(parsed[2]["body"]["text"].as_str().unwrap(), "three");

    // list does NOT advance the cursor.
    let cursor = InboxCursor::at(home.join("inbox.cursor"));
    assert_eq!(cursor.read().await.unwrap(), 0, "list must not touch cursor");

    // ack(off2) advances the cursor without printing.
    run_ack(&home, off2).await.unwrap();
    assert_eq!(cursor.read().await.unwrap(), off2);

    // list --since off2 prints exactly one entry (the third line).
    let mut buf = Vec::<u8>::new();
    run_list(&home, Some(off2), &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1, "expected one line past off2: {text}");
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["offset"].as_u64().unwrap(), off3);
    assert_eq!(v["body"]["text"].as_str().unwrap(), "three");

    // list-since did NOT advance the cursor past what ack set.
    assert_eq!(cursor.read().await.unwrap(), off2);
}

// Silencers.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_keyring as _;
use famp_taskdir as _;
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
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
