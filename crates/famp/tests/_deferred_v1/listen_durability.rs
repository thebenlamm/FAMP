//! Plan 02-03 Task 2 — `listen_durability`: INBOX-02 fsync-before-200 gate.
//!
//! MUST use a real subprocess (SIGKILL cannot be meaningfully sent to an
//! in-process tokio task). Sequence:
//!
//! 1. spawn `famp listen --listen 127.0.0.1:0` with stderr piped
//! 2. read the beacon line `listening on https://127.0.0.1:<port>` and
//!    parse the bound addr (no pre-bind race — we discover the port
//!    AFTER the daemon announces it)
//! 3. POST a signed AckBody via reqwest; assert 200 OK
//! 4. IMMEDIATELY SIGKILL the child (via `child.kill()`)
//! 5. wait() the child to fully exit
//! 6. open inbox.jsonl via `famp_inbox::read::read_all` — assert exactly
//!    one value present
//!
//! A passing test proves the daemon fsynced the inbox file BEFORE
//! returning 200 (otherwise the kill between 200 and fsync would race
//! and the line would be missing from a fresh fs view).

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

mod common;

use std::time::Duration;

use common::{
    build_signed_ack_bytes, build_trusting_reqwest_client, init_home_in_process, post_bytes,
    read_inbox_lines, read_stderr_bound_addr, self_principal, spawn_listen, ChildGuard,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigkill_after_200_leaves_line_intact() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // --listen 127.0.0.1:0 tells the daemon to pick an ephemeral port.
    // We read it back from the beacon line on stderr — no race window.
    let child = spawn_listen(&home, "127.0.0.1:0");
    let mut guard = ChildGuard::new(child);
    let addr = {
        let child = guard.as_mut().expect("child present");
        read_stderr_bound_addr(child, Duration::from_secs(5))
            .expect("read beacon line from daemon stderr")
    };

    // POST a signed envelope — must succeed with 200.
    let bytes = build_signed_ack_bytes(&home);
    let client = build_trusting_reqwest_client(&home);
    let resp = post_bytes(&client, addr, &self_principal(), bytes)
        .await
        .expect("post");
    assert_eq!(resp.status().as_u16(), 200, "expected 200 OK from listen");

    // IMMEDIATELY after the 200: SIGKILL the child. The fsync contract
    // says the handler only returned 200 AFTER inbox.append completed,
    // and inbox.append only returns Ok AFTER sync_data — so even a
    // SIGKILL now must leave the line on disk.
    let mut child = guard.take().expect("child present");
    child.kill().expect("kill");
    let _ = child.wait().expect("wait");

    // Read the inbox fresh; must contain the one line.
    let lines = read_inbox_lines(&home);
    assert_eq!(
        lines.len(),
        1,
        "INBOX-02 fsync contract violated: expected 1 line on disk after \
         SIGKILL-after-200; got {}. Lines: {lines:?}",
        lines.len()
    );
    let class = lines[0]
        .get("class")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert_eq!(class, "ack");
}
