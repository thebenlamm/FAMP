//! Plan 02-03 Task 2 — `listen_smoke`: DAEMON-01 + DAEMON-02 end-to-end gate.
//!
//! In-process spawn of `famp::cli::listen::run_on_listener` on an
//! ephemeral port. POST a signed AckBody envelope, assert 200 OK, assert
//! the inbox JSONL contains exactly one line with `"class": "ack"`.
//!
//! In-process (not subprocess) because the smoke test doesn't need an
//! OS-process boundary — it exercises the handler/router wiring, not
//! SIGKILL or SIGINT behavior. Faster and simpler than a subprocess.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{
    build_signed_ack_bytes, build_trusting_reqwest_client, init_home_in_process, post_bytes,
    read_inbox_lines, self_principal, wait_for_tls_listener_ready,
};
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn smoke_post_delivers_to_inbox() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Bind an ephemeral port in-process so we know the address before
    // spawning run_on_listener.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    // Oneshot shutdown future handed to run_on_listener.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    };

    let home_for_task = home.clone();
    let server_task = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_for_task, listener, shutdown_signal)
            .await
            .expect("run_on_listener");
    });

    wait_for_tls_listener_ready().await;

    // Build envelope bytes + post them.
    let bytes = build_signed_ack_bytes(&home);
    let client = build_trusting_reqwest_client(&home);
    let resp = post_bytes(&client, addr, &self_principal(), bytes)
        .await
        .expect("post");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "listen handler must return 200 OK on durable commit; got {}",
        resp.status()
    );

    // Inbox should now have exactly one line, parsable as an ack.
    let lines = read_inbox_lines(&home);
    assert_eq!(lines.len(), 1, "expected one inbox line");
    let class = lines[0]
        .get("class")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert_eq!(
        class, "ack",
        "expected class=ack; got value: {:?}",
        lines[0]
    );

    // Shutdown the daemon task.
    let _ = shutdown_tx.send(());
    // Give the task a moment to unwind; don't hang if it doesn't.
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}
