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
}
