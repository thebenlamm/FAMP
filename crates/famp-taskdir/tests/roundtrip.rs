#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silencers: these workspace deps are only used via famp-taskdir's public API,
// but unused_crate_dependencies still fires on integration test crates.
use serde as _;
use thiserror as _;
use toml as _;
use uuid as _;

use famp_taskdir::{TaskDir, TaskDirError, TaskRecord};
use tempfile::TempDir;

fn sample_uuid() -> String {
    // Stable UUIDv7 for tests.
    "01931d7a-1234-7abc-8def-abcdef012345".to_string()
}

fn sample_record() -> TaskRecord {
    TaskRecord::new_requested(
        sample_uuid(),
        "alice".to_string(),
        "2026-04-14T12:00:00Z".to_string(),
    )
}

#[test]
fn create_then_read_returns_same_record() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();
    let back = store.read(&rec.task_id).unwrap();
    assert_eq!(back, rec);
}

#[test]
fn create_rejects_duplicate() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();
    let err = store.create(&rec).unwrap_err();
    assert!(matches!(err, TaskDirError::AlreadyExists { .. }));
}

#[test]
fn create_rejects_invalid_uuid() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let mut rec = sample_record();
    rec.task_id = "not-a-uuid".to_string();
    let err = store.create(&rec).unwrap_err();
    assert!(matches!(err, TaskDirError::InvalidUuid { .. }));
}

#[test]
fn read_missing_returns_not_found() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let err = store.read(&sample_uuid()).unwrap_err();
    assert!(matches!(err, TaskDirError::NotFound { .. }));
}

#[test]
fn update_mutates_in_place() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();
    let updated = store
        .update(&rec.task_id, |mut r| {
            r.terminal = true;
            r.state = "COMPLETED".to_string();
            r
        })
        .unwrap();
    assert!(updated.terminal);
    assert_eq!(updated.state, "COMPLETED");
    let back = store.read(&rec.task_id).unwrap();
    assert_eq!(back, updated);
}

#[test]
fn list_returns_all_records() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let ids = [
        "01931d7a-0001-7abc-8def-abcdef012345",
        "01931d7a-0002-7abc-8def-abcdef012345",
        "01931d7a-0003-7abc-8def-abcdef012345",
    ];
    for id in ids {
        let rec = TaskRecord::new_requested(
            id.to_string(),
            "alice".to_string(),
            "2026-04-14T12:00:00Z".to_string(),
        );
        store.create(&rec).unwrap();
    }
    let mut all = store.list().unwrap();
    all.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].task_id, ids[0]);
    assert_eq!(all[2].task_id, ids[2]);
}

#[test]
fn list_skips_unparseable() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();
    // Drop a garbage file in the dir.
    std::fs::write(dir.path().join("garbage.toml"), b"this is not = = toml").unwrap();
    let all = store.list().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].task_id, rec.task_id);
}

#[test]
fn update_round_trip_byte_stable() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();
    let path = dir.path().join(format!("{}.toml", rec.task_id));
    let first = std::fs::read(&path).unwrap();
    // Update without changing anything — must produce the same file bytes.
    store.update(&rec.task_id, |r| r).unwrap();
    let second = std::fs::read(&path).unwrap();
    assert_eq!(first, second, "toml round-trip must be byte-stable");
}

#[test]
fn open_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let _ = TaskDir::open(dir.path()).unwrap();
    let _ = TaskDir::open(dir.path()).unwrap();
}

#[test]
fn update_rejects_changed_task_id_and_does_not_create_orphan() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let rec = sample_record();
    store.create(&rec).unwrap();

    let original_id = rec.task_id;
    let new_id = "01931d7a-9999-7abc-8def-abcdef012345".to_string();

    let err = store
        .update(&original_id, |mut r| {
            r.task_id = new_id.clone();
            r
        })
        .unwrap_err();

    match err {
        TaskDirError::TaskIdChanged { original, next } => {
            assert_eq!(original, original_id);
            assert_eq!(next, new_id);
        }
        other => panic!("expected TaskIdChanged, got {other:?}"),
    }

    // No file under the new id should exist (no orphan).
    let new_path = dir.path().join(format!("{new_id}.toml"));
    assert!(
        !new_path.exists(),
        "rejected update must not have created an orphan file at {}",
        new_path.display()
    );
    // Original file is still intact and unchanged.
    let original_path = dir.path().join(format!("{original_id}.toml"));
    assert!(original_path.exists(), "original file must remain on disk");
    let still = store.read(&original_id).unwrap();
    assert_eq!(still.task_id, original_id);
}
