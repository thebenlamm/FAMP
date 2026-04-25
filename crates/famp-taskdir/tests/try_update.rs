#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silencers: workspace deps only used via famp-taskdir's public API.
use serde as _;
use thiserror as _;
use toml as _;
use uuid as _;

use famp_taskdir::{TaskDir, TaskDirError, TaskRecord, TryUpdateError};
use tempfile::TempDir;

fn fresh_task_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn requested_record(task_id: &str) -> TaskRecord {
    TaskRecord::new_requested(
        task_id.to_string(),
        "peer".to_string(),
        "2026-04-25T00:00:00Z".to_string(),
    )
}

/// Happy path: closure returns Ok(record) → persists atomically, returns new record.
#[test]
fn try_update_happy_path_persists_closure_result() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let task_id = fresh_task_id();
    let rec = requested_record(&task_id);
    store.create(&rec).unwrap();

    // Closure flips state to COMMITTED.
    let result = store.try_update::<std::io::Error, _>(&task_id, |mut r| {
        r.state = "COMMITTED".to_string();
        Ok(r)
    });

    let returned = result.expect("try_update should succeed");
    assert_eq!(returned.state, "COMMITTED");
    assert_eq!(returned.task_id, task_id);

    // Verify the disk state matches.
    let on_disk = store.read(&task_id).unwrap();
    assert_eq!(on_disk.state, "COMMITTED");
}

/// Closure-Err path: closure returns Err → NO disk write; file bytes unchanged.
#[test]
fn try_update_closure_err_does_not_write() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let task_id = fresh_task_id();
    let rec = requested_record(&task_id);
    store.create(&rec).unwrap();

    let task_file = dir.path().join(format!("{task_id}.toml"));
    let bytes_before = std::fs::read(&task_file).expect("read before try_update");

    let result = store.try_update(&task_id, |_r| Err::<TaskRecord, &str>("boom"));

    assert!(
        matches!(result, Err(TryUpdateError::Closure("boom"))),
        "expected Closure(\"boom\"), got: {result:?}"
    );

    let bytes_after = std::fs::read(&task_file).expect("read after try_update");
    assert_eq!(
        bytes_before, bytes_after,
        "file bytes must be byte-identical after a closure-Err try_update"
    );
}

/// `task_id`-stability: closure mutates `task_id` → `TaskIdChanged`, no write.
#[test]
fn try_update_task_id_changed_returns_taskidchanged_no_write() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let task_id = fresh_task_id();
    let rec = requested_record(&task_id);
    store.create(&rec).unwrap();

    let task_file = dir.path().join(format!("{task_id}.toml"));
    let bytes_before = std::fs::read(&task_file).expect("read before try_update");

    let different_id = fresh_task_id();
    let result = store.try_update::<std::io::Error, _>(&task_id, |mut r| {
        r.task_id = different_id.clone();
        Ok(r)
    });

    assert!(
        matches!(
            result,
            Err(TryUpdateError::Store(TaskDirError::TaskIdChanged { .. }))
        ),
        "expected TaskIdChanged, got: {result:?}"
    );

    let bytes_after = std::fs::read(&task_file).expect("read after try_update");
    assert_eq!(
        bytes_before, bytes_after,
        "file bytes must be unchanged when TaskIdChanged is returned"
    );
}

/// `NotFound`: closure is never invoked when the initial read fails.
#[test]
fn try_update_not_found_skips_closure() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();
    let task_id = fresh_task_id(); // valid UUID but no file on disk

    let result = store.try_update(
        &task_id,
        |_: TaskRecord| -> Result<TaskRecord, std::io::Error> {
            panic!("closure must not run when read fails (NotFound)");
        },
    );

    assert!(
        matches!(
            result,
            Err(TryUpdateError::Store(TaskDirError::NotFound { .. }))
        ),
        "expected NotFound, got: {result:?}"
    );
}

/// `InvalidUuid`: closure is never invoked when UUID parse fails.
#[test]
fn try_update_invalid_uuid_skips_closure() {
    let dir = TempDir::new().unwrap();
    let store = TaskDir::open(dir.path()).unwrap();

    let result = store.try_update(
        "not-a-uuid",
        |_: TaskRecord| -> Result<TaskRecord, std::io::Error> {
            panic!("closure must not run when UUID is invalid");
        },
    );

    assert!(
        matches!(
            result,
            Err(TryUpdateError::Store(TaskDirError::InvalidUuid { .. }))
        ),
        "expected InvalidUuid, got: {result:?}"
    );
}
