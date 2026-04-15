//! Integration tests locking the read-path boundary:
//! - Truncated trailing line (no final `\n`) is silently skipped
//! - Garbage final line (no final `\n`) is silently skipped
//! - Mid-file corruption (bad line with a good line after it) is a hard error
//!
//! These tests write raw bytes via `std::fs::write` so the read path is
//! verified independently of `Inbox::append`.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_inbox::{read::read_all, InboxError};
use serde_json::json;

#[test]
fn tail_tolerant_when_file_lacks_trailing_newline() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("inbox.jsonl");
    // `{"b":2` is both missing the final `\n` AND truncated mid-object.
    std::fs::write(&path, b"{\"a\":1}\n{\"b\":2").unwrap();

    let values = read_all(&path).unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], json!({ "a": 1 }));
}

#[test]
fn tail_tolerant_when_final_line_is_garbage_bytes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("inbox.jsonl");
    std::fs::write(&path, b"{\"a\":1}\n<<<not json>>>").unwrap();

    let values = read_all(&path).unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], json!({ "a": 1 }));
}

#[test]
fn mid_file_corruption_is_hard_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("inbox.jsonl");
    // Bad middle line, well-formed line after, trailing newline → mid-file.
    std::fs::write(&path, b"{\"a\":1}\nNOT JSON\n{\"b\":2}\n").unwrap();

    let err = read_all(&path).unwrap_err();
    match err {
        InboxError::CorruptLine { line_no, .. } => {
            assert_eq!(line_no, 2, "line 2 is the bad one");
        }
        other => panic!("expected CorruptLine, got {other:?}"),
    }
}
