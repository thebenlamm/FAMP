//! 999.11 acceptance test (HANDOFF.md §6): `famp inspect identities`' `unread`
//! for an identity must equal the number of envelopes a subsequent
//! `famp_inbox` call actually returns for it.
//!
//! **This is EXPECTED TO FAIL on current HEAD.** It pins the exact divergence
//! diagnosed in `.planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md`
//! §2: `unread` is computed from the on-disk `.{name}.cursor` file (advanced
//! only by `register`/`join`/`famp inbox ack`), while `famp_inbox` (MCP) tracks
//! delivery via an entirely separate, never-synchronized in-session cursor
//! (`SessionState.inbox_offset`, see `cli/mcp/session.rs` and fix #13). Two
//! authorities for the same fact, per the SPEC's root complaint. Not
//! `#[ignore]`d: this repo's convention (see `multi_task_interleave_filtered_await.rs`)
//! is to let known-bug reproducers stay RED and visible in `cargo test` output.
//!
//! Mechanism (per the Explore-agent survey that produced this file): a single
//! send-then-read cycle does NOT expose the divergence, because both cursors
//! start at the same baseline. It takes two send-then-read cycles: after the
//! first `famp_inbox` call consumes message A, the on-disk `.cursor` is still
//! untouched (still 0), so once message B arrives, `unread` counts BOTH A and
//! B (2), while a second `famp_inbox` call with no explicit `since` returns
//! only B (1) — the MCP session already saw A and will not replay it.

#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use serde_json::json;
use std::process::Command;
use std::time::Duration;

mod common;
use common::mcp_harness::Harness;

fn mailbox_unread_for(sock: &std::path::Path, name: &str) -> u64 {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", sock)
        .args(["inspect", "identities", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "inspect identities failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == name)
        .unwrap_or_else(|| panic!("missing {name} row: {value}"));
    row["mailbox_unread"]
        .as_u64()
        .unwrap_or_else(|| panic!("mailbox_unread missing/non-numeric: {row}"))
}

fn inbox_entry_count(harness: &mut Harness, since: Option<u64>) -> usize {
    let args = match since {
        Some(s) => json!({ "since": s }),
        None => json!({}),
    };
    let resp = harness.tool_call("famp_inbox", &args);
    let body = Harness::ok_content(&resp);
    body["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("entries not an array: {body}"))
        .len()
}

#[test]
fn inspect_identities_unread_equals_subsequent_famp_inbox_delivered_count() {
    let local_root = tempfile::tempdir().unwrap();
    let local_root_path = local_root.path().to_path_buf();
    let sock = local_root_path.join("bus.sock");

    let mut receiver = Harness::with_local_root(&local_root_path, Some(local_root));
    let mut sender = Harness::with_local_root(&local_root_path, None);

    let reg_r = receiver.tool_call("famp_register", &json!({ "name": "receiver" }));
    Harness::ok_content(&reg_r);
    let reg_s = sender.tool_call("famp_register", &json!({ "name": "sender" }));
    Harness::ok_content(&reg_s);

    // Round 1: send A, drain it via famp_inbox. This alone does NOT expose
    // the bug — both authorities agree at this point (sanity check).
    let send_a = sender.tool_call(
        "famp_send",
        &json!({ "peer": "receiver", "mode": "new_task", "title": "message A" }),
    );
    Harness::ok_content(&send_a);
    std::thread::sleep(Duration::from_millis(200));

    let unread_after_a = mailbox_unread_for(&sock, "receiver");
    assert_eq!(
        unread_after_a, 1,
        "sanity: unread should be 1 after message A"
    );
    let delivered_1 = inbox_entry_count(&mut receiver, None);
    assert_eq!(
        delivered_1, 1,
        "sanity: first famp_inbox call should return message A"
    );

    // Round 2: send B. `unread` (disk-cursor-based) now counts A+B, but
    // `famp_inbox` (MCP session-cursor-based) has already "seen" A and will
    // only return B on a `since`-less call.
    let send_b = sender.tool_call(
        "famp_send",
        &json!({ "peer": "receiver", "mode": "new_task", "title": "message B" }),
    );
    Harness::ok_content(&send_b);
    std::thread::sleep(Duration::from_millis(200));

    let unread_after_b = mailbox_unread_for(&sock, "receiver");
    let delivered_2 = inbox_entry_count(&mut receiver, None);

    assert_eq!(
        unread_after_b as usize, delivered_2,
        "999.11: `famp inspect identities` unread ({unread_after_b}) must equal \
         the number of envelopes a subsequent famp_inbox call actually returns \
         ({delivered_2}) — see HANDOFF.md §2/§6. Two uncoordinated cursor \
         authorities (disk .cursor vs MCP session inbox_offset) for the same fact."
    );
}
