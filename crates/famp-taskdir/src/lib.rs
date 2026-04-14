//! `famp-taskdir` — TOML-per-task storage primitive for FAMP v0.8 Phase 3.
//!
//! One file per task at `<root>/<task_id>.toml`. Atomic replace via
//! `tempfile::NamedTempFile` in the same directory + fsync + rename.
//! No network, no signing, no FSM logic — pure storage.

#![forbid(unsafe_code)]

pub mod atomic;
pub mod error;
pub mod record;
pub mod store;

pub use error::TaskDirError;
pub use record::TaskRecord;
pub use store::TaskDir;
