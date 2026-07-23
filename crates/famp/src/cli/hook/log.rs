//! Fail-open logging for Stop-hook helpers.
//!
//! Writes to `$FAMP_HOOK_LOG` or `$XDG_STATE_HOME/famp/await-hook.log`.
//! Never panics; every I/O failure is swallowed.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

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

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // ISO-ish without pulling chrono; good enough for hook logs.
    format!("{secs}")
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
