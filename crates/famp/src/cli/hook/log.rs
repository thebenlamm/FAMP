//! Fail-open logging for Stop-hook helpers.
//!
//! Writes to `$FAMP_HOOK_LOG` or `$XDG_STATE_HOME/famp/await-hook.log`.
//! Never panics; every I/O failure is swallowed.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Rotate to `<log>.1` once the live file exceeds this. One generation only;
/// the hook writes a handful of lines per turn per identity, forever.
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

fn log_path() -> &'static PathBuf {
    LOG_PATH.get_or_init(|| {
        if let Ok(p) = std::env::var("FAMP_HOOK_LOG") {
            return PathBuf::from(p);
        }
        let state = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        state.join("famp").join("await-hook.log")
    })
}

/// RFC3339 to the second, matching the shell adapter's `date -Iseconds`, so
/// anything grepping this log by date keeps working across the migration.
fn timestamp() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

/// Roll `<log>` to `<log>.1` when it grows past [`MAX_LOG_BYTES`].
/// Best-effort: a failed rename just means the live file keeps growing.
fn rotate_if_needed(path: &Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_LOG_BYTES {
        return;
    }
    let mut rolled = path.as_os_str().to_owned();
    rolled.push(".1");
    let _ = std::fs::rename(path, PathBuf::from(rolled));
}

/// Append one log line. Failures are ignored (fail-open).
pub fn log(msg: &str) {
    let path = log_path();
    if path.is_symlink() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    rotate_if_needed(path);
    let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(
        f,
        "[{ts} pid={pid}] {msg}",
        ts = timestamp(),
        pid = std::process::id(),
        msg = msg
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_rfc3339_seconds() {
        // Parity with the shell adapter's `date -Iseconds`; anything grepping
        // the hook log by date depends on this shape.
        let ts = timestamp();
        assert!(ts.ends_with('Z'), "{ts}");
        assert_eq!(ts.len(), "2026-07-23T00:00:00Z".len(), "{ts}");
        assert_eq!(&ts[4..5], "-", "{ts}");
        assert_eq!(&ts[10..11], "T", "{ts}");
    }

    #[test]
    fn rotates_once_past_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("await-hook.log");
        let rolled = dir.path().join("await-hook.log.1");

        std::fs::write(&path, vec![b'x'; 16]).unwrap();
        rotate_if_needed(&path);
        assert!(!rolled.exists(), "under cap must not rotate");

        let over = usize::try_from(MAX_LOG_BYTES).unwrap() + 1;
        std::fs::write(&path, vec![b'x'; over]).unwrap();
        rotate_if_needed(&path);
        assert!(rolled.exists(), "over cap must roll to .1");
        assert!(!path.exists(), "live file is renamed, not copied");
    }

    #[test]
    fn rotate_tolerates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        rotate_if_needed(&dir.path().join("absent.log"));
    }
}
