//! `TaskDir` — directory of per-task TOML records.

use std::path::{Path, PathBuf};

use crate::atomic::write_atomic_file;
use crate::error::TaskDirError;
use crate::record::TaskRecord;

/// Directory-backed task record store.
pub struct TaskDir {
    root: PathBuf,
}

impl TaskDir {
    /// Open or create `root`. Creates the directory idempotently. On Unix
    /// sets the directory mode to 0700.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, TaskDirError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).map_err(|source| TaskDirError::Io {
            path: root.clone(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).map_err(
                |source| TaskDirError::Io {
                    path: root.clone(),
                    source,
                },
            )?;
        }
        Ok(Self { root })
    }

    /// Root directory of this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, task_id: &str) -> Result<PathBuf, TaskDirError> {
        uuid::Uuid::parse_str(task_id).map_err(|_| TaskDirError::InvalidUuid {
            value: task_id.to_string(),
        })?;
        Ok(self.root.join(format!("{task_id}.toml")))
    }

    /// Atomically create a new task record file. Returns `AlreadyExists`
    /// if the file already exists on disk.
    pub fn create(&self, record: &TaskRecord) -> Result<(), TaskDirError> {
        let path = self.path_for(&record.task_id)?;
        if path.exists() {
            return Err(TaskDirError::AlreadyExists {
                task_id: record.task_id.clone(),
            });
        }
        let body = toml::to_string(record).map_err(|source| TaskDirError::TomlSerialize {
            task_id: record.task_id.clone(),
            source,
        })?;
        write_atomic_file(&path, body.as_bytes()).map_err(|source| TaskDirError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(())
    }

    /// Read and parse the record for `task_id`.
    pub fn read(&self, task_id: &str) -> Result<TaskRecord, TaskDirError> {
        let path = self.path_for(task_id)?;
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(TaskDirError::NotFound {
                    task_id: task_id.to_string(),
                });
            }
            Err(source) => {
                return Err(TaskDirError::Io { path, source });
            }
        };
        let text = std::str::from_utf8(&bytes).map_err(|err| TaskDirError::Io {
            path: path.clone(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, err),
        })?;
        toml::from_str(text).map_err(|source| TaskDirError::TomlParse { path, source })
    }

    /// Read → mutate via closure → atomic write. Returns the new record.
    ///
    /// The mutation closure must NOT change `task_id`. The original id is
    /// the on-disk file key; allowing it to change here would silently
    /// create a second file under the new id while leaving the old one
    /// intact (orphan record + duplicate identity). Callers that genuinely
    /// need to rename a task should delete the old record and `create` a
    /// new one explicitly.
    pub fn update<F>(&self, task_id: &str, mutate: F) -> Result<TaskRecord, TaskDirError>
    where
        F: FnOnce(TaskRecord) -> TaskRecord,
    {
        let current = self.read(task_id)?;
        let next = mutate(current);
        if next.task_id != task_id {
            return Err(TaskDirError::TaskIdChanged {
                original: task_id.to_string(),
                next: next.task_id,
            });
        }
        // Use the validated original task_id, not next.task_id, so the
        // path is anchored to the file we read above.
        let path = self.path_for(task_id)?;
        let body = toml::to_string(&next).map_err(|source| TaskDirError::TomlSerialize {
            task_id: next.task_id.clone(),
            source,
        })?;
        write_atomic_file(&path, body.as_bytes()).map_err(|source| TaskDirError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(next)
    }

    /// Enumerate every `*.toml` record under the root. Parse failures
    /// are logged to stderr and skipped — a corrupted single file must
    /// not poison the iterator.
    pub fn list(&self) -> Result<Vec<TaskRecord>, TaskDirError> {
        let entries = std::fs::read_dir(&self.root).map_err(|source| TaskDirError::Io {
            path: self.root.clone(),
            source,
        })?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| TaskDirError::Io {
                path: self.root.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(err) => {
                    eprintln!(
                        "famp-taskdir: skipping unreadable {}: {err}",
                        path.display()
                    );
                    continue;
                }
            };
            let text = match std::str::from_utf8(&bytes) {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("famp-taskdir: skipping non-utf8 {}: {err}", path.display());
                    continue;
                }
            };
            match toml::from_str::<TaskRecord>(text) {
                Ok(rec) => out.push(rec),
                Err(err) => {
                    eprintln!(
                        "famp-taskdir: skipping unparseable {}: {err}",
                        path.display()
                    );
                }
            }
        }
        Ok(out)
    }
}
