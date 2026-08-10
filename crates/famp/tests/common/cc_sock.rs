//! RAII cc-socks socket for the current process (quick task `260810-hac`).
//!
//! `mcp/tools/register.rs::record_wake_addr` resolves `parent_id()` of the
//! `famp mcp` child — which, under an `mcp_harness::Harness`, is the test
//! binary — and joins it onto the hardcoded `CC_SOCKS_DIR` const. There is no
//! seam to redirect, so a test that needs a window to have a wake address must
//! bind a real unix socket at the real path.
//!
//! ## Why writing into the shared `/tmp/cc-socks` is safe
//!
//! The only path this can touch is `/tmp/cc-socks/<our own pid>.sock`. No live
//! Claude Code session can own that name, because we own that pid. Anything
//! already at that path is a leftover from a dead process. The socket is
//! removed on drop; the shared directory is created if missing and never
//! removed.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;

pub struct CcSock {
    path: PathBuf,
    listener: Option<UnixListener>,
}

impl CcSock {
    /// Bind `/tmp/cc-socks/<pid>.sock` for this process.
    ///
    /// # Panics
    /// Panics if the directory cannot be created or the socket cannot be bound.
    #[must_use]
    pub fn bind_for_self() -> Self {
        let dir = PathBuf::from("/tmp/cc-socks");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        Self {
            path,
            listener: Some(listener),
        }
    }

    /// The wake address the MCP register path will derive from this socket.
    #[must_use]
    pub fn wake_addr(&self) -> String {
        format!("uds:{}", self.path.display())
    }

    /// Remove the socket, simulating the session's socket going away between
    /// two registrations of the same window. Idempotent.
    pub fn remove(&mut self) {
        drop(self.listener.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for CcSock {
    fn drop(&mut self) {
        self.remove();
    }
}
