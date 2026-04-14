//! Phase 3 Plan 03-03 Task 1 — `read_from` slice reader.
//!
//! Contract locked here:
//! - returns `Vec<(Value, end_offset)>` — the byte offset AFTER each line
//! - `start_offset = 0` yields every complete line
//! - `start_offset >= file_len` yields empty
//! - `start_offset` past EOF is clamped (no error)
//! - `start_offset` mid-line snaps forward to the next `\n + 1` boundary
//! - final partial line at EOF is silently dropped (tail tolerance)
//! - a non-terminal corrupt line is a hard `CorruptLine` error

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_inbox::read::read_from;
use serde_json as _;
use tempfile::TempDir;
use thiserror as _;

fn write_inbox(tmp: &TempDir, body: &[u8]) -> std::path::PathBuf {
    let path = tmp.path().join("inbox.jsonl");
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn read_from_zero_matches_read_all() {
    let tmp = TempDir::new().unwrap();
    let body = b"{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n";
    let path = write_inbox(&tmp, body);

    let entries = read_from(&path, 0).unwrap();
    let all = famp_inbox::read::read_all(&path).unwrap();
    assert_eq!(entries.len(), all.len());
    for ((v, _), w) in entries.iter().zip(all.iter()) {
        assert_eq!(v, w);
    }
    // Last end_offset equals file length (all three lines consumed).
    assert_eq!(entries.last().unwrap().1, body.len() as u64);
    // End offsets are strictly increasing.
    assert_eq!(entries[0].1, 8); // "{\"a\":1}\n" = 8 bytes
    assert_eq!(entries[1].1, 16);
    assert_eq!(entries[2].1, 24);
}

#[test]
fn read_from_past_eof_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let body = b"{\"a\":1}\n";
    let path = write_inbox(&tmp, body);

    let entries = read_from(&path, body.len() as u64).unwrap();
    assert!(entries.is_empty());

    // Clamped: past EOF still empty, no error.
    let entries = read_from(&path, body.len() as u64 + 100).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn read_from_advances_offset_per_line() {
    let tmp = TempDir::new().unwrap();
    // Three lines; start at the boundary AFTER the first one (offset 8).
    let body = b"{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n";
    let path = write_inbox(&tmp, body);

    let entries = read_from(&path, 8).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0.get("b").unwrap().as_i64().unwrap(), 2);
    assert_eq!(entries[0].1, 16);
    assert_eq!(entries[1].1, 24);
}

#[test]
fn read_from_snaps_mid_line_to_next_boundary() {
    let tmp = TempDir::new().unwrap();
    let body = b"{\"a\":1}\n{\"b\":2}\n";
    let path = write_inbox(&tmp, body);

    // Offset 3 is mid first-line. Should snap to byte 8 (after first \n)
    // and return only the second line.
    let entries = read_from(&path, 3).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0.get("b").unwrap().as_i64().unwrap(), 2);
    assert_eq!(entries[0].1, 16);
}

#[test]
fn read_from_tail_tolerant_partial_line() {
    let tmp = TempDir::new().unwrap();
    // Second line is partial (no terminating newline) — crash mid-write.
    let body = b"{\"a\":1}\n{\"b\":2";
    let path = write_inbox(&tmp, body);

    let entries = read_from(&path, 0).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0.get("a").unwrap().as_i64().unwrap(), 1);
    // End offset stops at the end of the complete line (byte 8),
    // not at file length.
    assert_eq!(entries[0].1, 8);
}

#[test]
fn read_from_corrupt_mid_file_line_errors() {
    let tmp = TempDir::new().unwrap();
    let body = b"{\"a\":1}\nnot-json\n{\"c\":3}\n";
    let path = write_inbox(&tmp, body);

    let err = read_from(&path, 0).unwrap_err();
    match err {
        famp_inbox::InboxError::CorruptLine { line_no, .. } => {
            assert_eq!(line_no, 2);
        }
        other => panic!("expected CorruptLine, got {other:?}"),
    }
}
