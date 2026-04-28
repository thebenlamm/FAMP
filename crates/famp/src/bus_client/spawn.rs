//! Portable broker spawn helper — Phase 02 D-Q1.
//!
//! Forks the current `famp` binary into a `famp broker` child whose
//! first action (between `fork()` and `exec()`) is `nix::unistd::setsid`,
//! detaching it from the controlling terminal. This pattern survives
//! `Cmd-Q` on macOS Terminal.app and is portable across macOS + Linux.
//! The macOS-only `posix_spawn` "set new session" flag is intentionally
//! **NOT** used here (the `nix 0.31` `PosixSpawnFlags` API does not
//! expose it). See `RESEARCH.md §"Resolved Open Questions" Q1`.
//!
//! The spawned child writes its stdout/stderr to `<bus_dir>/broker.log`
//! (mode `0o600`, append-only). The parent disowns the child by dropping
//! the `Child` handle — the new session means the child outlives its
//! parent process group and survives any terminal close.

#![allow(unsafe_code)] // Q1-locked broker-spawn pattern; see module doc.

use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("io error spawning broker")]
    Io(#[source] std::io::Error),
    #[error("failed to locate current executable")]
    CurrentExe(#[source] std::io::Error),
    #[error("broker did not start within 2s")]
    BrokerDidNotStart,
    #[error("socket path is not valid utf-8")]
    SocketPathNotUtf8,
}

/// Spawn the broker child process if no broker is currently listening
/// on `sock_path`. No-op (returns `Ok(())`) when an existing broker is
/// already accepting connections.
///
/// The detection short-circuit uses a synchronous `UnixStream::connect`
/// because this helper is called from non-async setup paths (e.g. the
/// MCP session bootstrap before tokio is fully running). The post-spawn
/// poll for socket-up is also synchronous — `BusClient::connect` is the
/// owner of the async retry loop on top of this primitive.
pub fn spawn_broker_if_absent(sock_path: &Path) -> Result<(), SpawnError> {
    // Fast path: broker already accepting.
    if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
        return Ok(());
    }

    // Make sure the bus directory exists; the broker child writes its
    // log file there, and the eventual UDS bind will need the parent.
    let bus_dir = sock_path.parent().ok_or_else(|| {
        SpawnError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "socket path has no parent",
        ))
    })?;
    std::fs::create_dir_all(bus_dir).map_err(SpawnError::Io)?;

    let log_path = bus_dir.join("broker.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)
        .map_err(SpawnError::Io)?;
    let log_clone = log.try_clone().map_err(SpawnError::Io)?;

    let exe = std::env::current_exe().map_err(SpawnError::CurrentExe)?;
    let sock_str = sock_path.to_str().ok_or(SpawnError::SocketPathNotUtf8)?;

    // SAFETY: `pre_exec` runs in the forked child before `exec()`. Only
    // async-signal-safe operations are permitted; `setsid()` is on the
    // POSIX async-signal-safe list. We do not access any of the parent's
    // heap state from inside the closure.
    let mut cmd = Command::new(&exe);
    cmd.args(["broker", "--socket", sock_str])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_clone));
    unsafe {
        cmd.pre_exec(|| {
            // First action in the child: detach from the controlling
            // terminal by creating a new session. `nix::Errno` has a
            // `From<Errno> for io::Error` impl that pre_exec expects.
            nix::unistd::setsid()
                .map(|_pgid| ())
                .map_err(std::io::Error::from)?;
            Ok(())
        });
    }
    let child = cmd.spawn().map_err(SpawnError::Io)?;
    // Disown: dropping the Child does NOT reap; the broker has its own
    // session. The kernel will reparent on parent exit.
    drop(child);

    // Poll for socket-up. 10 × 200ms = 2s wall clock.
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
            return Ok(());
        }
    }
    Err(SpawnError::BrokerDidNotStart)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// When a `UnixListener` is already bound to `sock_path`, the helper
    /// must return `Ok(())` without forking anything.
    #[test]
    fn returns_ok_when_socket_already_accepting() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        // Sanity: confirm a sync connect succeeds (the helper's fast path).
        assert!(std::os::unix::net::UnixStream::connect(&sock).is_ok());
        let res = spawn_broker_if_absent(&sock);
        assert!(res.is_ok(), "expected Ok, got {res:?}");
    }
}
