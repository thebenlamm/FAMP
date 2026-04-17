//! Narrow error enum for `famp-taskdir`.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TaskDirError {
    #[error("io error at {path:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("toml parse failed at {path:?}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("toml serialize failed for task {task_id}")]
    TomlSerialize {
        task_id: String,
        #[source]
        source: toml::ser::Error,
    },

    #[error("task record not found: {task_id}")]
    NotFound { task_id: String },

    #[error("task record already exists: {task_id}")]
    AlreadyExists { task_id: String },

    #[error("invalid task_id (not a UUID): {value}")]
    InvalidUuid { value: String },

    /// The closure passed to [`crate::TaskDir::update`] returned a record
    /// whose `task_id` differs from the one being updated. The mutation is
    /// rejected to prevent orphan/duplicate files: writing under the new id
    /// without removing the old file would leave two records on disk for
    /// what callers treat as a single task.
    #[error("update mutated task_id from {original} to {next}; identity must be stable")]
    TaskIdChanged { original: String, next: String },
}
