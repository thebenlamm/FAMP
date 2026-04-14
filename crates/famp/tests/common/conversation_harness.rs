//! Phase 3 Plan 03-04 — shared helpers for the Phase 3 conversation
//! integration tests.
//!
//! This harness builds on top of [`listen_harness`](super::listen_harness)
//! and the `famp::cli::*` entry points exposed by earlier Phase 3 plans.
//! It exists to keep `conversation_full_lifecycle`,
//! `conversation_restart_safety`, and `conversation_inbox_lock` free of
//! boilerplate and to give Phase 4's MCP tests something reusable.
//!
//! Phase-2 keyring constraint (important):
//!
//!   The Phase 2 `famp listen` daemon builds a single-entry keyring
//!   pinning `agent:localhost/self` → the daemon's own verifying key. Any
//!   envelope whose `from` principal is anything else is rejected with
//!   `UnknownSender` before the handler runs. Phase 3's conversation
//!   tests therefore use ONE shared `FAMP_HOME` for both sender and
//!   receiver (`from == to == agent:localhost/self`). A genuine two-home
//!   flow is Phase 4 work (multi-entry keyring).

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp::cli::error::CliError;
use famp::cli::peer::add::run_add_at as peer_add_run_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};
use famp_taskdir::{TaskDir, TaskRecord};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::listen_harness::init_home_in_process;

/// Create a temporary `FAMP_HOME`, initialize it in-process, and return
/// the owning `TempDir`. Caller holds the guard for the test duration.
#[must_use]
pub fn setup_home() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    init_home_in_process(tmp.path());
    tmp
}

/// Read the self pubkey from disk and base64url-unpadded-encode it.
#[must_use]
pub fn pubkey_b64(home: &Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Bind an ephemeral port on `127.0.0.1`, spawn
/// `famp::cli::listen::run_on_listener` in-process against it, wait until
/// the daemon accepts TCP, and return the bound address + join handle +
/// shutdown sender.
///
/// The caller MUST drop `shutdown_tx` (or `send(())` it) and then
/// `.await` the join handle before the test returns, otherwise the
/// daemon task will outlive the test's tokio runtime.
pub async fn spawn_listener(
    home: &Path,
) -> (SocketAddr, JoinHandle<()>, oneshot::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    };
    let home_owned = home.to_path_buf();
    let handle = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_owned, listener, shutdown_signal)
            .await
            .expect("run_on_listener");
    });

    // Wait for TCP accept.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "daemon bind timed out at {addr}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    (addr, handle, shutdown_tx)
}

/// Signal shutdown to a daemon spawned via [`spawn_listener`] and await
/// its join handle with a bounded timeout.
pub async fn stop_listener(handle: JoinHandle<()>, shutdown_tx: oneshot::Sender<()>) {
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// Register `addr` in `peers.toml` as alias `alias` with the self pubkey
/// and principal `agent:localhost/self` — the only principal the Phase 2
/// keyring resolves.
pub fn add_self_peer(home: &Path, alias: &str, addr: SocketAddr) {
    peer_add_run_at(
        home,
        alias.to_string(),
        format!("https://{addr}"),
        pubkey_b64(home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");
}

/// Rewrite `peers.toml` to point `alias` at a new `SocketAddr`. Used by
/// the restart-safety test because Phase 3 has no `famp peer update`
/// subcommand and the daemon's ephemeral port changes across restarts.
pub fn update_peer_endpoint(home: &Path, alias: &str, addr: SocketAddr) {
    let path = famp::cli::paths::peers_toml_path(home);
    let mut peers = famp::cli::config::read_peers(&path).expect("read peers");
    let entry = peers.find_mut(alias).expect("alias in peers.toml");
    entry.endpoint = format!("https://{addr}");
    // Blank out the pinned fingerprint so TOFU recaptures on next contact.
    entry.tls_fingerprint_sha256 = None;
    famp::cli::config::write_peers_atomic(&path, &peers).expect("write peers");
}

/// Open a new task via `famp send --new-task` and return its `UUIDv7`
/// task-id by reading the one record that now exists in `<home>/tasks`.
///
/// Precondition: no prior task records in this home.
pub async fn new_task(home: &Path, alias: &str, summary: &str) -> String {
    send_run_at(
        home,
        SendArgs {
            to: alias.to_string(),
            new_task: Some(summary.to_string()),
            task: None,
            terminal: false,
            body: None,
        },
    )
    .await
    .expect("send new task");
    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    let records = tasks.list().unwrap();
    assert_eq!(records.len(), 1, "expected exactly one task record after new_task");
    records[0].task_id.clone()
}

/// Send a `--task <id>` deliver, terminal or not. Panics on any error.
pub async fn deliver(home: &Path, alias: &str, task_id: &str, terminal: bool, body: &str) {
    send_run_at(
        home,
        SendArgs {
            to: alias.to_string(),
            new_task: None,
            task: Some(task_id.to_string()),
            terminal,
            body: Some(body.to_string()),
        },
    )
    .await
    .expect("send deliver");
}

/// Same as [`deliver`] but returns the `Result` for negative assertions.
pub async fn try_deliver(
    home: &Path,
    alias: &str,
    task_id: &str,
    terminal: bool,
    body: &str,
) -> Result<(), CliError> {
    send_run_at(
        home,
        SendArgs {
            to: alias.to_string(),
            new_task: None,
            task: Some(task_id.to_string()),
            terminal,
            body: Some(body.to_string()),
        },
    )
    .await
}

/// Run `famp await --timeout <timeout>` once and return the parsed JSON
/// line the subcommand printed. Panics on timeout or write failure.
pub async fn await_once(home: &Path, timeout: &str) -> serde_json::Value {
    let mut buf: Vec<u8> = Vec::new();
    await_run_at(
        home,
        AwaitArgs {
            timeout: timeout.to_string(),
            task: None,
        },
        &mut buf,
    )
    .await
    .expect("await_once");
    let text = String::from_utf8(buf).expect("utf-8 output");
    let line = text.lines().next().expect("await printed no lines");
    serde_json::from_str(line).expect("await output parses as JSON")
}

/// Read a task record from `<home>/tasks/<task_id>.toml`.
pub fn read_task(home: &Path, task_id: &str) -> TaskRecord {
    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    tasks.read(task_id).expect("read task record")
}

/// Count lines currently in `<home>/inbox.jsonl`.
#[must_use]
pub fn inbox_line_count(home: &Path) -> usize {
    std::fs::read_to_string(home.join("inbox.jsonl"))
        .map(|s| s.lines().count())
        .unwrap_or(0)
}
