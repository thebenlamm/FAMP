//! `InboxCursor` — sidecar byte-offset tracker for `inbox.jsonl`.
//!
//! Lives next to `Inbox` per CONTEXT D-Cursor: the cursor and the
//! JSONL file are tightly coupled (the offset is meaningful only
//! against the file the cursor was advanced against), so they share
//! a crate.
//!
//! Wire format: a single line of ASCII decimal followed by `\n`.
//! Atomic replace via `tempfile::NamedTempFile` in the same directory.
//! Mode 0600 on Unix.
//!
//! MIRROR: `crates/famp-taskdir/src/atomic.rs` keeps a near-identical
//! `write_atomic_file` helper. Keep them in sync if you touch the fsync
//! or permissions logic.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::InboxError;

pub struct InboxCursor {
    path: PathBuf,
}

impl InboxCursor {
    /// Construct a cursor handle at `path`. Does not touch disk.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the next unread byte offset. Returns 0 if the cursor
    /// file does not yet exist (first-run case).
    pub async fn read(&self) -> Result<u64, InboxError> {
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(source) => {
                return Err(InboxError::Io {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| InboxError::CursorParse {
                path: self.path.clone(),
            })?
            .trim_end_matches('\n');
        text.parse::<u64>().map_err(|_| InboxError::CursorParse {
            path: self.path.clone(),
        })
    }

    /// Atomically write `offset` to the cursor path. Mode 0600 on Unix.
    pub async fn advance(&self, offset: u64) -> Result<(), InboxError> {
        let path = self.path.clone();
        let body = format!("{offset}\n");
        let res = tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            let parent = path.parent().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent")
            })?;
            std::fs::create_dir_all(parent)?;
            let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
            tmp.write_all(body.as_bytes())?;
            tmp.as_file_mut().sync_all()?;
            tmp.persist(&path).map_err(|e| e.error)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok(())
        })
        .await;

        match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(source)) => Err(InboxError::Io {
                path: self.path.clone(),
                source,
            }),
            Err(join) => Err(InboxError::Io {
                path: self.path.clone(),
                source: std::io::Error::other(format!("spawn_blocking join: {join}")),
            }),
        }
    }
}
