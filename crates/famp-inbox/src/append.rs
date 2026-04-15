//! Append-side of the inbox: open + durable append with fsync-before-return.
//!
//! The durability receipt contract: when [`Inbox::append`] returns `Ok(())`,
//! both `write_all` AND `sync_data` have completed. Callers may return HTTP
//! 200 after observing `Ok`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::InboxError;

/// Durable append-only JSONL inbox.
///
/// A single `tokio::sync::Mutex<File>` shared via `Arc` serializes concurrent
/// appends. The file is opened once at construction with `append(true)` so
/// every write lands at the current EOF atomically at the OS level.
pub struct Inbox {
    path: PathBuf,
    file: Arc<Mutex<File>>,
}

impl Inbox {
    /// Open (creating if absent, mode 0600 on unix) a JSONL inbox at `path`.
    ///
    /// On unix the create step goes through `OpenOptionsExt::mode(0o600)` so
    /// the file is unreadable by other local users from the moment it exists.
    /// On non-unix the mode bits are not applied; `famp` is unix-only for
    /// v0.8 but the non-unix fallback keeps the crate compilable.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, InboxError> {
        let path = path.as_ref().to_path_buf();

        // Create-if-absent with 0600 on unix. This is a separate step from
        // the writer-handle open below because tokio's OpenOptions doesn't
        // expose `mode()` directly — but a create-then-reopen is fine since
        // no other process is contending for the file.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut create = std::fs::OpenOptions::new();
            create
                .create(true)
                .append(true)
                .mode(0o600)
                .open(&path)
                .map_err(|source| InboxError::Io {
                    path: path.clone(),
                    source,
                })?;
        }
        #[cfg(not(unix))]
        {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|source| InboxError::Io {
                    path: path.clone(),
                    source,
                })?;
        }

        let file = OpenOptions::new()
            .append(true)
            .write(true)
            .open(&path)
            .await
            .map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;

        Ok(Self {
            path,
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Append one already-serialized envelope as a single JSONL line.
    ///
    /// Rejects `bytes` containing any raw `\n` with
    /// [`InboxError::EmbeddedNewline`] **before** touching the file, so a
    /// malformed caller can never split one logical envelope into two
    /// JSONL lines.
    ///
    /// On `Ok(())`, `write_all(bytes)`, `write_all(b"\n")`, and
    /// `sync_data()` have all completed.
    pub async fn append(&self, envelope_bytes: &[u8]) -> Result<(), InboxError> {
        if envelope_bytes.contains(&b'\n') {
            return Err(InboxError::EmbeddedNewline);
        }

        let mut guard = self.file.lock().await;
        let result = async {
            guard
                .write_all(envelope_bytes)
                .await
                .map_err(|source| InboxError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            guard
                .write_all(b"\n")
                .await
                .map_err(|source| InboxError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            guard.sync_data().await.map_err(|source| InboxError::Io {
                path: self.path.clone(),
                source,
            })?;
            Ok::<(), InboxError>(())
        }
        .await;
        drop(guard);
        result
    }

    /// Path of the underlying JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::read::read_all;

    #[tokio::test]
    async fn append_then_read_back_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("inbox.jsonl");

        let inbox = Inbox::open(&path).await.unwrap();
        let a = serde_json::json!({ "n": 1, "kind": "request" }).to_string();
        let b = serde_json::json!({ "n": 2, "kind": "deliver" }).to_string();
        inbox.append(a.as_bytes()).await.unwrap();
        inbox.append(b.as_bytes()).await.unwrap();

        let values = read_all(&path).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["n"], 1);
        assert_eq!(values[0]["kind"], "request");
        assert_eq!(values[1]["n"], 2);
        assert_eq!(values[1]["kind"], "deliver");
    }

    #[tokio::test]
    async fn embedded_newline_rejected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("inbox.jsonl");
        let inbox = Inbox::open(&path).await.unwrap();

        let size_before = std::fs::metadata(&path).unwrap().len();
        let bad = b"{\"a\":1}\n{\"b\":2}";
        let err = inbox.append(bad).await.unwrap_err();
        assert!(matches!(err, InboxError::EmbeddedNewline));

        let size_after = std::fs::metadata(&path).unwrap().len();
        assert_eq!(
            size_before, size_after,
            "rejected append must not touch the file"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_appends_serialize() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("inbox.jsonl");
        let inbox = Arc::new(Inbox::open(&path).await.unwrap());

        // 16 tasks, each writing a distinct ~1 KB payload
        let mut handles = Vec::new();
        for i in 0..16u32 {
            let inbox = Arc::clone(&inbox);
            handles.push(tokio::spawn(async move {
                let filler: String = "x".repeat(900);
                let payload = serde_json::json!({ "i": i, "pad": filler }).to_string();
                inbox.append(payload.as_bytes()).await.unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        let values = read_all(&path).unwrap();
        assert_eq!(
            values.len(),
            16,
            "all 16 lines must be present and parseable"
        );

        let mut seen = std::collections::BTreeSet::new();
        for v in values {
            let i = v["i"].as_u64().unwrap();
            assert!(
                seen.insert(i),
                "duplicate index {i} — interleaving detected"
            );
            assert_eq!(v["pad"].as_str().unwrap().len(), 900);
        }
        assert_eq!(seen.len(), 16);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn open_creates_file_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("inbox.jsonl");

        let _inbox = Inbox::open(&path).await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "inbox file must be created with mode 0600");
    }
}
