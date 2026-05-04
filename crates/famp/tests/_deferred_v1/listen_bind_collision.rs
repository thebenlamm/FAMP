//! Plan 02-03 Task 2 — `listen_bind_collision`: DAEMON-03 PortInUse gate.
//!
//! Two subprocesses on the same port. The second MUST exit non-zero with
//! the error string "already bound" (from `CliError::PortInUse`'s
//! `#[error("another famp listen is already bound to {addr}")]`).
//!
//! Port-selection strategy: bind-and-drop a scratch TCP listener on
//! `127.0.0.1:0` to grab a free port, note its port number, then drop
//! the listener so daemon A can re-use it. This has a known race window
//! (another process could grab the port between drop and respawn);
//! acceptable for a local test harness — the test is checking the OS's
//! `EADDRINUSE` mapping, not the port-selection timing.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    clippy::single_match_else,
    unused_crate_dependencies
)]

mod common;

use std::{io::Read, time::Duration};

use common::{init_home_in_process, spawn_listen, wait_for_bind, ChildGuard};

#[test]
fn second_listen_on_same_port_errors_port_in_use() {
    // Two separate homes so keys/certs don't collide on disk.
    let tmp_a = tempfile::TempDir::new().unwrap();
    let tmp_b = tempfile::TempDir::new().unwrap();
    let home_a = tmp_a.path().to_path_buf();
    let home_b = tmp_b.path().to_path_buf();
    init_home_in_process(&home_a);
    init_home_in_process(&home_b);

    // Grab a free port by binding + dropping a scratch listener.
    let scratch = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = scratch.local_addr().unwrap();
    drop(scratch);

    // Daemon A — should succeed and bind.
    let child_a = spawn_listen(&home_a, &addr.to_string());
    let mut guard_a = ChildGuard::new(child_a);
    // We deliberately don't read the stderr beacon here — we only care
    // that port is taken. Use wait_for_bind to sync: that also surfaces
    // a spurious A-failure (e.g., cert load) as a clean error.
    wait_for_bind(guard_a.as_mut().unwrap(), addr, Duration::from_secs(5))
        .expect("daemon A should bind the scratch port");

    // Daemon B — should fail fast with PortInUse on the same addr.
    let mut child_b = spawn_listen(&home_b, &addr.to_string());

    // Wait up to 5s for B to exit.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        match child_b.try_wait().expect("try_wait") {
            Some(status) => break status,
            None => {
                if std::time::Instant::now() >= deadline {
                    let _ = child_b.kill();
                    let _ = child_b.wait();
                    panic!("daemon B did not exit within 5s on same-port bind");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    assert!(
        !exit_status.success(),
        "daemon B must exit non-zero on PortInUse; got {exit_status:?}"
    );

    // Drain stderr and assert it contains the PortInUse display text.
    let mut stderr = String::new();
    if let Some(mut e) = child_b.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    assert!(
        stderr.contains("already bound"),
        "daemon B stderr must contain PortInUse text; got: {stderr:?}"
    );

    // guard_a's Drop will SIGKILL daemon A on scope exit.
}
