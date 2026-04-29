//! Executor for `Out::AdvanceCursor`.
//!
//! Writes `<bus_dir>/mailboxes/<name>.cursor` (or `.<#channel>.cursor`)
//! via the same atomic temp+rename pattern used by
//! `famp-inbox::InboxCursor::advance` (cursor.rs lines 58-91). Mode 0600
//! on Unix.
//!
//! Path layout (Phase-1 D-09):
//!   - agent mailbox: `mailboxes/alice.jsonl` + `mailboxes/.alice.cursor`
//!   - channel mailbox: `mailboxes/#planning.jsonl` +
//!     `mailboxes/.#planning.cursor`
//!
//! The cursor body is a single ASCII decimal followed by `\n`.

use std::io::Write as _;
use std::path::Path;

/// Atomically write `offset` to the cursor file for `display_name`.
///
/// `display_name` is the `MailboxName::Display` form — the agent name
/// (`"alice"`) or channel display (`"#planning"`). The cursor file lives
/// at `<bus_dir>/mailboxes/.<display_name>.cursor`.
pub async fn execute_advance_cursor(
    bus_dir: &Path,
    display_name: &str,
    offset: u64,
) -> Result<(), std::io::Error> {
    let mailboxes = bus_dir.join("mailboxes");
    let target = mailboxes.join(format!(".{display_name}.cursor"));
    let body = format!("{offset}\n");
    let res = tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&mailboxes)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&mailboxes)?;
        tmp.write_all(body.as_bytes())?;
        tmp.as_file_mut().sync_all()?;
        tmp.persist(&target).map_err(|e| e.error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    })
    .await;

    match res {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(join) => Err(std::io::Error::other(format!(
            "spawn_blocking join: {join}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_offset_atomically() {
        let tmp = tempfile::TempDir::new().unwrap();
        execute_advance_cursor(tmp.path(), "alice", 4096)
            .await
            .unwrap();
        let body = std::fs::read_to_string(tmp.path().join("mailboxes/.alice.cursor")).unwrap();
        assert_eq!(body, "4096\n");
    }

    #[tokio::test]
    async fn channel_cursor_path_uses_hash_prefix() {
        let tmp = tempfile::TempDir::new().unwrap();
        execute_advance_cursor(tmp.path(), "#planning", 12)
            .await
            .unwrap();
        let p = tmp.path().join("mailboxes/.#planning.cursor");
        let body = std::fs::read_to_string(&p).unwrap();
        assert_eq!(body, "12\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cursor_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt as _;
        let tmp = tempfile::TempDir::new().unwrap();
        execute_advance_cursor(tmp.path(), "bob", 1).await.unwrap();
        let mode = std::fs::metadata(tmp.path().join("mailboxes/.bob.cursor"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
