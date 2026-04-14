//! Plan 02-03 Task 2 — `listen_truncated_tail`: INBOX-04 / INBOX-05
//! reinforcement at the famp-crate integration layer.
//!
//! Plan 02-01 ships a unit-level tail-tolerance test inside the
//! `famp-inbox` crate. This test reinforces the same contract at the
//! consumer layer — the same code path Phase 3's `famp await` will
//! take. It does NOT spawn a daemon: it synthesizes a
//! "daemon-crashed-mid-write" inbox by hand-writing a valid line
//! followed by a partial second line (no trailing newline), then
//! calls `famp_inbox::read::read_all` and asserts the read tolerates
//! the truncated tail.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

mod common;

use std::io::Write;

use common::init_home_in_process;

#[test]
fn read_all_tolerates_daemon_crash_truncated_tail() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Hand-craft inbox.jsonl with one complete line + one partial tail.
    let inbox_path = home.join("inbox.jsonl");
    {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&inbox_path)
            .unwrap();
        // Valid first line, terminated by newline.
        f.write_all(br#"{"class":"ack","n":1}"#).unwrap();
        f.write_all(b"\n").unwrap();
        // Partial second line — represents a crash mid-write: bytes
        // started hitting the file but the newline never got there.
        f.write_all(br#"{"class":"ack","n":2"#).unwrap();
        f.flush().unwrap();
    }

    // read_all must return the one complete line and silently drop the
    // truncated tail (INBOX-04 / INBOX-05).
    let values = famp_inbox::read::read_all(&inbox_path).expect("read_all tolerant");
    assert_eq!(
        values.len(),
        1,
        "expected exactly 1 complete line; partial tail must be dropped; got: {values:?}"
    );
    assert_eq!(
        values[0]
            .get("n")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        1,
        "first line must be the complete one"
    );
}
