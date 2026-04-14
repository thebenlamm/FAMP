#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silencers for workspace deps consumed transitively via famp_inbox.
use serde_json as _;
use thiserror as _;

use famp_inbox::InboxCursor;
use tempfile::TempDir;

#[tokio::test]
async fn read_returns_zero_when_missing() {
    let dir = TempDir::new().unwrap();
    let cur = InboxCursor::at(dir.path().join("inbox.cursor"));
    assert_eq!(cur.read().await.unwrap(), 0);
}

#[tokio::test]
async fn advance_then_read_roundtrip() {
    let dir = TempDir::new().unwrap();
    let cur = InboxCursor::at(dir.path().join("inbox.cursor"));
    cur.advance(42).await.unwrap();
    assert_eq!(cur.read().await.unwrap(), 42);
    cur.advance(99).await.unwrap();
    assert_eq!(cur.read().await.unwrap(), 99);
}

#[cfg(unix)]
#[tokio::test]
async fn advance_creates_0600_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("c");
    InboxCursor::at(&path).advance(1).await.unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn read_garbage_returns_cursor_parse_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("c");
    std::fs::write(&path, b"not-a-number\n").unwrap();
    let err = InboxCursor::at(&path).read().await.unwrap_err();
    assert!(matches!(err, famp_inbox::InboxError::CursorParse { .. }));
}

#[tokio::test]
async fn advance_is_atomic_across_concurrent_writers() {
    use std::sync::Arc;
    let dir = TempDir::new().unwrap();
    let cur = Arc::new(InboxCursor::at(dir.path().join("c")));
    let mut handles = Vec::new();
    for i in 0..8u64 {
        let cur = cur.clone();
        handles.push(tokio::spawn(async move { cur.advance(i).await }));
    }
    for h in handles {
        h.await.unwrap().unwrap();
    }
    let final_offset = cur.read().await.unwrap();
    assert!(final_offset < 8);
}
