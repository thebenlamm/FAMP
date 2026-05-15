//! Integration coverage for `famp_channel_log`.

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fs::File;
use std::io::Write;

use common::mcp_harness::Harness;

#[test]
fn channel_log_reads_channel_mailbox_without_registration() {
    let local_root = tempfile::tempdir().unwrap();
    let root_path = local_root.path().to_path_buf();
    let mailboxes = root_path.join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();
    let path = mailboxes.join("#planning.jsonl");
    let mut file = File::create(&path).unwrap();

    let first = serde_json::json!({
        "id": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7",
        "from": "agent:local.bus/alice",
        "to": "chan:local.bus/#planning",
        "class": "new_task",
        "ts": "2026-05-15T17:00:00Z",
        "body": { "event": "famp.send.new_task", "text": "one" }
    });
    let second = serde_json::json!({
        "id": "019d9ba2-2d31-7ae2-ba77-9e55863ac7f7",
        "from": "agent:local.bus/bob",
        "to": "chan:local.bus/#planning",
        "class": "deliver",
        "ts": "2026-05-15T17:01:00Z",
        "body": { "event": "famp.send.deliver", "text": "two" }
    });
    let first_line = serde_json::to_string(&first).unwrap();
    let second_line = serde_json::to_string(&second).unwrap();
    writeln!(file, "{first_line}").unwrap();
    writeln!(file, "{second_line}").unwrap();
    let first_offset = u64::try_from(first_line.len() + 1).unwrap();

    let mut h = Harness::with_local_root(&root_path, Some(local_root));
    let resp = h.tool_call(
        "famp_channel_log",
        &serde_json::json!({ "channel": "planning", "limit": 1 }),
    );
    let body = Harness::ok_content(&resp);

    assert_eq!(body["channel"], "#planning");
    assert_eq!(body["envelopes"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["envelopes"][0]["id"],
        "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7"
    );
    assert_eq!(body["next_offset"], first_offset);

    let resp = h.tool_call(
        "famp_channel_log",
        &serde_json::json!({ "channel": "#planning", "since": first_offset }),
    );
    let body = Harness::ok_content(&resp);

    assert_eq!(body["envelopes"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["envelopes"][0]["id"],
        "019d9ba2-2d31-7ae2-ba77-9e55863ac7f7"
    );
    assert!(body["next_offset"].as_u64().unwrap() > first_offset);
}
