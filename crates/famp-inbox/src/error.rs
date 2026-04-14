//! Narrow error enum for `famp-inbox`.
//!
//! Three variants by design:
//! - [`InboxError::Io`] — any filesystem-level error, annotated with the path
//! - [`InboxError::CorruptLine`] — a non-terminal JSONL line failed to parse
//!   (mid-file corruption; hard error)
//! - [`InboxError::EmbeddedNewline`] — the caller tried to append bytes
//!   containing a raw `\n`, which would split one logical envelope across
//!   two JSONL lines

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("io error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("inbox line {line_no} is not valid JSON")]
    CorruptLine {
        line_no: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("inbox line contains embedded newline")]
    EmbeddedNewline,

    #[error("cursor parse error at {path:?}")]
    CursorParse { path: PathBuf },

    #[error("inbox lock held by pid {pid} at {path:?}")]
    LockHeld { path: PathBuf, pid: u32 },
}
