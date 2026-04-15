//! Integration smoke test for Plan 02-02's daemon-restart read path.
//!
//! Open an inbox via the public API, append three payloads, drop the inbox
//! so the file handle releases, re-read via `read_all`, assert exact order.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_inbox::{read::read_all, Inbox};
use serde_json::json;

#[tokio::test]
async fn inbox_append_roundtrip_via_public_api() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("inbox.jsonl");

    let inbox = Inbox::open(&path).await.unwrap();
    let payloads = [
        json!({ "seq": 1, "body": "request" }),
        json!({ "seq": 2, "body": "commit" }),
        json!({ "seq": 3, "body": "deliver" }),
    ];
    for p in &payloads {
        inbox.append(p.to_string().as_bytes()).await.unwrap();
    }
    drop(inbox);

    let values = read_all(&path).unwrap();
    assert_eq!(values.len(), 3);
    for (got, want) in values.iter().zip(payloads.iter()) {
        assert_eq!(got, want);
    }
}
