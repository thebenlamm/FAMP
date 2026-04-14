//! Plan 02-03 Task 2 — `listen_shutdown`: DAEMON-04 SIGINT graceful exit gate.
//!
//! Spawn `famp listen` as a subprocess, read its beacon line to confirm
//! bind, send SIGINT via `/bin/kill -INT <pid>`, wait up to 5s for the
//! child to exit, assert exit status is success (exit 0).
//!
//! We invoke `/bin/kill` as a child process rather than calling
//! `libc::kill` in an `unsafe` block so the test source stays fully
//! safe-rust (workspace `#![forbid(unsafe_code)]` stays clean in
//! production code; this test file also avoids any unsafe).

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    clippy::single_match_else,
    unused_crate_dependencies
)]

mod common;

use std::{process::Command, time::Duration};

use common::{
    init_home_in_process, read_stderr_bound_addr, spawn_listen, wait_for_bind, ChildGuard,
};

#[test]
fn sigint_causes_exit_0_within_5s() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    let child = spawn_listen(&home, "127.0.0.1:0");
    let mut guard = ChildGuard::new(child);

    // Consume stderr through the beacon line so we know the daemon is
    // fully up, AND wait for TCP accept so we know run_on_listener has
    // progressed past its spawn point and the tokio select! has
    // registered the SIGINT handler. Without the TCP-wait step, SIGINT
    // can race the handler-registration window and the child gets
    // killed by the default-disposition SIGINT (exit-by-signal 2)
    // instead of handled gracefully.
    let addr = {
        let child = guard.as_mut().unwrap();
        read_stderr_bound_addr(child, Duration::from_secs(5))
            .expect("read beacon line")
    };
    wait_for_bind(
        guard.as_mut().unwrap(),
        addr,
        Duration::from_secs(5),
    )
    .expect("daemon must accept before we SIGINT");
    // Extra settle window: the tokio select! arming `ctrl_c()` runs
    // slightly after tls_server::serve_std_listener spawns. Giving it
    // ~100ms is well under the test's 5s budget and eliminates flake.
    std::thread::sleep(Duration::from_millis(150));

    let pid = guard.as_mut().unwrap().id();
    let kill_status = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("/bin/kill");
    assert!(kill_status.success(), "/bin/kill -INT failed: {kill_status:?}");

    // Poll try_wait() for up to 5s.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let child = guard.as_mut().unwrap();
    let exit_status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None => {
                if std::time::Instant::now() >= deadline {
                    // Escalate: SIGKILL so we don't leak the daemon, then fail.
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("daemon did not exit within 5s of SIGINT");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };
    assert!(
        exit_status.success(),
        "SIGINT must cause exit 0; got {exit_status:?}"
    );

    // Guard has already seen the child exit — consume it to avoid the
    // drop handler redundantly wait()ing an already-reaped child.
    let _ = guard.take();
}
