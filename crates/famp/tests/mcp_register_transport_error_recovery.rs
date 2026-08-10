//! Review round 2, finding C (quick task `260810-hac`).
//!
//! A `Register` round-trip that fails at the transport layer must not leave
//! the dead `BusClient` cached on the MCP session.
//!
//! `session::ensure_bus` returns early whenever `guard.bus.is_some()`, and
//! `send_recv` never reconnects. So before the fix, one failed `famp_register`
//! wedged the window permanently: every subsequent call reused the same dead
//! stream and the only recovery was restarting the whole `famp mcp` process.
//!
//! This is not a hypothetical path. `BUS_PROTO_VERSION` 2 → 3 (this same task)
//! FORCES every live window to re-register after a daemon restart, and
//! CLAUDE.md's deploy sequence asserts "Every live window then re-registers" —
//! a sentence that depends on this call being retryable.
//!
//! The test drives the real `famp mcp` subprocess against an ephemeral broker
//! and kills the broker between calls, rather than faking the transport, so it
//! exercises the actual cached-connection lifetime.

#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::process::Command;
use std::time::{Duration, Instant};

use common::mcp_harness::Harness;

/// Broker pids matching this harness's ephemeral socket, newest first.
///
/// Matches `spawn::spawn_broker_if_absent`'s argv (`famp broker --socket
/// <path>`), the same discriminator `broker_crash_recovery.rs` uses. The
/// socket path is per-test (a private tempdir), so this can never match a
/// sibling test's broker or the developer's real daemon.
fn broker_pids(sock: &std::path::Path) -> Vec<i32> {
    let out = Command::new("pgrep")
        .args(["-f", &format!("famp broker --socket {}", sock.display())])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}

/// Poll until `sock` has at least one live broker, or fail.
fn wait_for_broker(sock: &std::path::Path) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let pids = broker_pids(sock);
        if !pids.is_empty() {
            return pids;
        }
        assert!(
            Instant::now() < deadline,
            "no broker ever appeared for {}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_failed_register_round_trip_does_not_wedge_the_session() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents").join("alice")).unwrap();
    let sock = dir.path().join("bus.sock");
    let mut h = Harness::with_local_root(dir.path(), None);

    // 1. First register succeeds. This is what caches the `BusClient` on the
    //    session — the object whose lifetime the whole finding is about.
    let first = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let body = Harness::ok_content(&first);
    assert_eq!(
        body["active"].as_str(),
        Some("alice"),
        "precondition: the first register must succeed: {body}"
    );

    // 2. Kill the broker out from under the cached connection — the daemon
    //    restart this task's own proto bump makes mandatory.
    let pids = wait_for_broker(&sock);
    for pid in &pids {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !broker_pids(&sock).is_empty() {
        assert!(
            Instant::now() < deadline,
            "broker {pids:?} never died after kill -9"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // A `kill -9` leaves the socket FILE behind; the spawn helper handles a
    // stale path on the next connect. Nothing to clean up here.

    // 3. The next register fails — the cached stream's peer is gone. This is
    //    the expected, honest outcome and is NOT what the finding is about.
    let second = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(
        second.get("error").is_some(),
        "precondition: with the broker dead the round-trip must fail, \
         otherwise step 4 proves nothing: {second}"
    );

    // 4. THE ASSERTION. A retry must be able to recover, because the failed
    //    call discarded the dead connection and `ensure_bus` therefore
    //    reopens (respawning the broker). Before the fix `guard.bus` was
    //    still `Some(<dead stream>)`, `ensure_bus` returned early, and this
    //    call failed exactly like the last one — forever.
    let third = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(
        third.get("error").is_none(),
        "a retry after a failed Register round-trip must reconnect, not \
         reuse the discarded connection; got: {third}"
    );
    let body = Harness::ok_content(&third);
    assert_eq!(
        body["active"].as_str(),
        Some("alice"),
        "the retry must produce a real RegisterOk: {body}"
    );

    // Reap the broker this test respawned so it does not outlive the tempdir.
    for pid in broker_pids(&sock) {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}
