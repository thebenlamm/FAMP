//! Review round 2, finding D (quick task `260810-hac`).
//!
//! Re-registration must be AUTHORITATIVE over the stored wake address.
//!
//! Two facts compose into a stale-address bug. The broker's idempotent
//! `Register` handler never touches `wake_addr` (it sets name/pid/cwd/listen/
//! origin and leaves the rest), and `famp_register` reuses the session's
//! cached `BusClient` — so the second registration of a window lands on the
//! SAME `ClientState`. Before the fix, `record_wake_addr` returned early and
//! sent no frame at all when no cc-socks socket was found, so a socket that
//! disappeared between two registrations left the broker still handing out
//! the old address: pings to a dead socket, or to a reused one belonging to a
//! different session.
//!
//! The observable used here is the production one — the `wake_ping` object on
//! a peer's `famp_send` result — not broker internals, so the test cannot pass
//! by agreeing with the implementation about what "stored" means.
//!
//! ## Why writing into the real `/tmp/cc-socks` is safe here
//!
//! `record_wake_addr` resolves `parent_id()` of the `famp mcp` child, which is
//! THIS test binary, and `CC_SOCKS_DIR` is a hardcoded const with no seam. So
//! the only path the test can touch is `/tmp/cc-socks/<our own pid>.sock`. No
//! live Claude Code session can own that name, because we own that pid. Any
//! file already there is a leftover from a dead process and is replaced; the
//! socket is removed at the end and the shared directory is left in place.

#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use common::mcp_harness::Harness;

/// RAII cc-socks socket for this process, mirroring what Claude Code exposes.
struct CcSock {
    path: PathBuf,
    listener: Option<UnixListener>,
}

impl CcSock {
    fn bind_for_self() -> Self {
        let dir = PathBuf::from("/tmp/cc-socks");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.sock", std::process::id()));
        // Safe per the module doc: this name is derived from OUR pid.
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        Self {
            path,
            listener: Some(listener),
        }
    }

    /// Remove the socket, simulating the session's socket going away between
    /// two registrations of the same window.
    fn remove(&mut self) {
        drop(self.listener.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for CcSock {
    fn drop(&mut self) {
        self.remove();
    }
}

/// The `wake_ping.to` on a `famp_send` result, or `None` when the broker
/// handed back no address.
fn wake_ping_to(send_result: &serde_json::Value) -> Option<String> {
    let body = Harness::ok_content(send_result);
    body.get("wake_ping")
        .and_then(|p| p.get("to"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

#[test]
fn re_registering_without_a_socket_clears_the_stored_wake_address() {
    let dir = tempfile::tempdir().unwrap();
    for agent in ["alice", "bob"] {
        std::fs::create_dir_all(dir.path().join("agents").join(agent)).unwrap();
    }
    let mut sock = CcSock::bind_for_self();
    let expected_addr = format!("uds:{}", sock.path.display());

    // Two MCP windows on one bus: alice is the listening recipient, bob is
    // the Local sender whose tool result carries the ping.
    let mut alice = Harness::with_local_root(dir.path(), None);
    let mut bob = Harness::with_local_root(dir.path(), None);

    let reg = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let body = Harness::ok_content(&reg);
    assert_eq!(body["listen_mode"].as_bool(), Some(true), "{body}");
    let _ = bob.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));

    // PRECONDITION, not the assertion: with the socket present, alice's
    // address really is stored and reachable. Without this the clear below
    // would be trivially satisfied by nothing ever having been stored.
    let first = bob.tool_call(
        "famp_send",
        &serde_json::json!({ "peer": "alice", "mode": "open", "body": "before" }),
    );
    assert_eq!(
        wake_ping_to(&first),
        Some(expected_addr),
        "precondition: alice's wake address must be stored while the socket exists"
    );

    // The socket goes away — session restarted, socket replaced, host
    // cleaned /tmp. Alice's window re-registers on its SAME cached bus
    // connection, so the broker sees the same ClientState.
    sock.remove();
    let reg2 = alice.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(
        reg2.get("error").is_none(),
        "the re-register must still succeed: {reg2}"
    );

    // THE ASSERTION. Before the fix the MCP path sent no frame at all when
    // detection returned None, and the broker's idempotent Register does not
    // clear `wake_addr` on its own — so the dead address survived and the
    // sender was still told to ping it.
    let second = bob.tool_call(
        "famp_send",
        &serde_json::json!({ "peer": "alice", "mode": "open", "body": "after" }),
    );
    assert_eq!(
        wake_ping_to(&second),
        None,
        "re-registering with no socket must CLEAR the stored address, not \
         leave the broker handing out a dead or reused one"
    );
}
