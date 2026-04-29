//! Phase 02 plan 02-05 — `famp inbox ack` GREEN integration test.
//!
//! Scope: exercise the *local* atomic cursor-write path. `inbox ack` is
//! intentionally a no-broker-round-trip command (RESEARCH §6 — the client
//! is authoritative on its per-session cursor), so this test does NOT
//! spawn a broker, register an identity, or open a UDS socket. It just
//! shells `famp inbox ack --offset N --as alice` against an isolated
//! `FAMP_BUS_SOCKET` path and asserts the resulting cursor file.
//!
//! Closes CLI-04 (atomic ack) + CLI-10 (atomic write — single-shot
//! crash-safety on this path; full kill-9 recovery proof is owned by
//! plan 02-11).

#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn test_inbox_ack_cursor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bus_dir = tmp.path();
    let sock = bus_dir.join("test-bus.sock");

    // Pre-create the mailbox tree as the broker would, with three
    // fake JSONL lines for alice. `inbox ack` does not actually read
    // this file; the lines are present only to make the fixture
    // realistic.
    let mailboxes = bus_dir.join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();
    let mailbox = mailboxes.join("alice.jsonl");
    let body = b"{\"id\":\"01913000-0000-7000-8000-000000000001\"}\n\
                 {\"id\":\"01913000-0000-7000-8000-000000000002\"}\n\
                 {\"id\":\"01913000-0000-7000-8000-000000000003\"}\n";
    std::fs::write(&mailbox, body).unwrap();

    // Run `famp inbox ack --offset 99 --as alice`. Pure local — no
    // broker, no Hello, no BusMessage. Stdout MUST be the ack JSON.
    let out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["inbox", "ack", "--offset", "99", "--as", "alice"])
        .env("FAMP_BUS_SOCKET", &sock)
        .output()
        .expect("famp inbox ack");
    assert!(
        out.status.success(),
        "famp inbox ack failed: status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let trimmed = stdout.trim_end();
    assert_eq!(
        trimmed, "{\"acked\":true,\"offset\":99}",
        "stdout shape mismatch: {trimmed:?}",
    );

    // Cursor file MUST exist with mode 0o600 and contain `99\n`.
    let cursor = mailboxes.join(".alice.cursor");
    assert!(cursor.exists(), "cursor file not created at {cursor:?}");
    let mode = std::fs::metadata(&cursor).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "cursor file mode: expected 0o600, got {mode:o}"
    );
    let cursor_body = std::fs::read_to_string(&cursor).unwrap();
    assert_eq!(cursor_body, "99\n", "cursor body: {cursor_body:?}");
}
