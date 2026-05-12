//! `famp broker` subcommand — Phase 02 plan 02-02.
//!
//! Wraps the frozen Phase-1 `famp_bus::Broker` in a tokio UDS daemon.
//! `famp broker --socket <path>` binds the listener, runs the accept
//! loop, and exits cleanly on SIGINT/SIGTERM (or after 5 minutes of
//! idle per BROKER-04).
//!
//! The run loop is a single-threaded tokio `select!` over five arms:
//!
//! 1. `listener.accept()` → spawn a `client_task` per connection
//! 2. `broker_rx.recv()` → drive `Broker::handle(BrokerInput::Wire/Disconnect, now)`,
//!    execute the returned `Vec<Out>` IN ORDER (D-04 invariant)
//! 3. `tick_interval.tick()` → drive `BrokerInput::Tick` for await-timeouts
//! 4. `wait_or_never(&mut idle)` → after 5min idle, fsync + unlink + exit
//! 5. `shutdown_signal` → SIGINT/SIGTERM clean shutdown
//!
//! Module layout:
//!   - `accept`: per-client read/write task using `UnixStream::into_split`
//!   - `cursor_exec`: `Out::AdvanceCursor` executor (atomic temp+rename)
//!   - `idle`: `wait_or_never` helper for the 5-min idle-timer arm
//!   - `mailbox_env`: `DiskMailboxEnv` (`BrokerEnv` impl backed by famp-inbox)
//!   - `nfs_check`: best-effort NFS-mount detector (BROKER-05)
//!   - `sessions_log`: append-only `~/.famp/sessions.jsonl` writer (CLI-11)

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use famp_bus::{Broker, BrokerInput, BusReply, ClientId, MailboxName, Out, SessionRow};
use famp_inspect_server::{BrokerCtx, MailboxMeta};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, Semaphore};

use crate::bus_client;
use crate::cli::error::CliError;

pub mod accept;
pub mod cursor_exec;
pub mod idle;
pub mod mailbox_env;
pub mod nfs_check;
pub mod sessions_log;

use accept::{client_task, BrokerMsg};
use mailbox_env::{BrokerEnvHandle, DiskMailboxEnv};

/// 5-minute idle exit per BROKER-04.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// 1-second tick driving `BrokerInput::Tick` for await-timeout sweeps.
const TICK_INTERVAL: Duration = Duration::from_secs(1);
/// `mpsc::channel` capacity for broker-bound frames. 1024 is generous
/// for v0.9 (single host, ~10 agents); back-pressure here means the
/// broker actor task is starved, which is a load anomaly, not a hot
/// path.
const BROKER_INBOX_CAPACITY: usize = 1024;
/// Per-client reply channel capacity. 64 lets a slow client absorb
/// burst replies (channel fan-out + drain) without blocking the broker.
const REPLY_CHANNEL_CAPACITY: usize = 64;
/// GAP-03-01: bound on the number of in-flight inspect dispatch tasks the
/// broker will run concurrently. Inspect snapshot work is moved off the
/// main `execute_outs` path into a `tokio::spawn`'d task that holds an
/// owned semaphore permit; when no permit is available the request is
/// rejected immediately with the existing budget-exceeded payload so
/// saturated inspect floods cannot create an unbounded queue of
/// blocking-pool tasks AND saturated inspect FS reads (taskdir walk +
/// mailbox JSONL pre-read) do not starve concurrent sender mailbox
/// writes. Sized intentionally LOW: the broker chooses to shed inspect
/// load (returning the existing `budget_exceeded` payload) rather than
/// degrade live bus throughput. Inspector callers re-issue trivially
/// since `connect_and_call` is one round-trip.
const MAX_CONCURRENT_INSPECT_REQUESTS: usize = 1;

/// Args for `famp broker`.
#[derive(clap::Args, Debug, Clone)]
pub struct BrokerArgs {
    /// Override the broker socket path. Defaults to
    /// `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock` per
    /// `bus_client::resolve_sock_path`.
    #[arg(long)]
    pub socket: Option<PathBuf>,
}

/// Production entry point for `famp broker`.
pub async fn run(args: BrokerArgs) -> Result<(), CliError> {
    let sock_path = args.socket.unwrap_or_else(bus_client::resolve_sock_path);
    let bus_dir = sock_path
        .parent()
        .ok_or_else(|| CliError::Io {
            path: sock_path.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "socket path has no parent",
            ),
        })?
        .to_path_buf();
    std::fs::create_dir_all(&bus_dir).map_err(|e| CliError::Io {
        path: bus_dir.clone(),
        source: e,
    })?;
    if nfs_check::is_nfs(&bus_dir) {
        eprintln!(
            "WARNING: ~/.famp/ appears to be on an NFS mount. Unix domain socket semantics \
             depend on the local kernel; bind() may fail or behave unexpectedly. Move \
             ~/.famp/ to a local filesystem for reliable operation."
        );
    }
    let listener = match bind_exclusive(&sock_path)? {
        BindOutcome::Bound(l) => l,
        // BL-02: another live broker holds the socket. Return Ok(())
        // from `run` so destructors run in scope and the spawning client
        // sees a clean "broker present" outcome — instead of calling
        // `std::process::exit(0)` from a non-`main` helper.
        BindOutcome::Existing => return Ok(()),
    };
    eprintln!("broker started, socket: {}", sock_path.display());
    run_on_listener(
        &sock_path,
        &bus_dir,
        listener,
        crate::cli::util::shutdown_signal(),
    )
    .await
}

/// Result of a `bind_exclusive` attempt.
///
/// `Bound` carries a freshly-bound listener for a clean or stale-socket
/// path. `Existing` signals that another live broker already holds the
/// socket; the caller should treat this as success and `return Ok(())`
/// from the surrounding `run`.
enum BindOutcome {
    Bound(UnixListener),
    Existing,
}

/// Bind the UDS listener with single-broker exclusion (BROKER-03).
///
/// Algorithm:
///   1. `tokio::net::UnixListener::bind(sock_path)`:
///      - `Ok` → return.
///      - `EADDRINUSE` (Linux) or `EEXIST` (macOS — `bind(2)` over an
///        existing socket file returns `EEXIST` rather than
///        `EADDRINUSE` on Darwin) → another broker (or stale socket
///        file) holds the path. Probe by `connect()`-ing:
///          - connect succeeds → live broker; `process::exit(0)`.
///          - connect fails (`ECONNREFUSED` typically; `ENOENT` if the
///            file was unlinked between our `bind` and the probe;
///            `EACCES` on cross-user inode) → treat as stale; `unlink`
///            + retry `bind` once.
///      - other errors → `CliError::Io`.
fn bind_exclusive(sock_path: &Path) -> Result<BindOutcome, CliError> {
    match UnixListener::bind(sock_path) {
        Ok(l) => Ok(BindOutcome::Bound(l)),
        Err(e)
            if matches!(
                e.raw_os_error(),
                Some(c) if c == nix::libc::EADDRINUSE || c == nix::libc::EEXIST
            ) =>
        {
            // Probe: is there a live broker on the other end?
            if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
                // BL-02: another broker is live; defer to it via a typed
                // outcome (NOT `std::process::exit(0)` from a non-`main`
                // helper — that skips destructors and is untestable).
                return Ok(BindOutcome::Existing);
            }
            // Stale socket → unlink + retry once.
            std::fs::remove_file(sock_path).map_err(|src| CliError::Io {
                path: sock_path.to_path_buf(),
                source: src,
            })?;
            UnixListener::bind(sock_path)
                .map(BindOutcome::Bound)
                .map_err(|src| CliError::Io {
                    path: sock_path.to_path_buf(),
                    source: src,
                })
        }
        Err(e) => Err(CliError::Io {
            path: sock_path.to_path_buf(),
            source: e,
        }),
    }
}

/// Test-facing entry point. Takes a pre-bound listener so integration
/// tests can use `tempfile::TempDir` socket paths and an in-process
/// shutdown future.
pub async fn run_on_listener(
    sock_path: &Path,
    bus_dir: &Path,
    listener: UnixListener,
    shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), CliError> {
    // Build the env. One Arc → two `BrokerEnvHandle` clones: one
    // hands ownership to `Broker::new` (read path); the executor
    // keeps the other for `Out::AppendMailbox` writes.
    let env = Arc::new(DiskMailboxEnv::new(bus_dir).map_err(|e| CliError::Io {
        path: bus_dir.to_path_buf(),
        source: e,
    })?);
    let env_handle = BrokerEnvHandle::new(Arc::clone(&env));
    let mut broker = Broker::new(env_handle.clone());

    let (broker_tx, mut broker_rx) = mpsc::channel::<BrokerMsg>(BROKER_INBOX_CAPACITY);
    let mut reply_senders: HashMap<ClientId, mpsc::Sender<BusReply>> = HashMap::new();
    // GAP-03-01: bounded concurrency for non-blocking inspect dispatch.
    // Shared across all `Out::InspectRequest` handlers; each in-flight
    // inspect snapshot task holds one owned permit until reply is sent.
    let inspect_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_INSPECT_REQUESTS));
    // WR-07: SessionRow data now flows through Out::SessionEnded
    // (broker snapshots `joined` before clearing state). The previous
    // executor-side `session_meta` mirror is gone.
    let mut client_count: u32 = 0;
    let mut idle: Option<Pin<Box<tokio::time::Sleep>>> = None;
    let mut next_id: u64 = 0;
    let mut tick_interval = tokio::time::interval(TICK_INTERVAL);
    // Skip the immediate first tick so we don't spin Broker::handle
    // before the broker has any clients.
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    tokio::pin!(shutdown_signal);

    loop {
        tokio::select! {
            // Arm 1: new connection.
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        client_count += 1;
                        idle = None;
                        let id = ClientId::from(next_id);
                        next_id += 1;
                        let (reply_tx, reply_rx) = mpsc::channel(REPLY_CHANNEL_CAPACITY);
                        reply_senders.insert(id, reply_tx);
                        tokio::spawn(client_task(id, stream, broker_tx.clone(), reply_rx));
                    }
                    Err(e) => {
                        // accept failures are usually transient (EMFILE,
                        // EPROTO). Log + continue rather than crash the
                        // broker, which would forfeit every other live
                        // connection.
                        eprintln!("accept error: {e}");
                    }
                }
            }
            // Arm 2: client wire frame or disconnect.
            Some(msg) = broker_rx.recv() => {
                let outs = match msg {
                    BrokerMsg::Frame(client, wire) => {
                        broker.handle(
                            BrokerInput::Wire { client, msg: wire },
                            Instant::now(),
                        )
                    }
                    BrokerMsg::Disconnect(client) => {
                        let outs = broker.handle(BrokerInput::Disconnect(client), Instant::now());
                        // Drop reply sender so the per-client write loop
                        // exits cleanly.
                        reply_senders.remove(&client);
                        client_count = client_count.saturating_sub(1);
                        if client_count == 0 {
                            idle = Some(Box::pin(tokio::time::sleep(IDLE_TIMEOUT)));
                        }
                        outs
                    }
                };
                execute_outs(
                    outs,
                    &mut reply_senders,
                    &env_handle,
                    &broker,
                    sock_path,
                    bus_dir,
                    &inspect_semaphore,
                )
                .await;
            }
            // Arm 3: 1-second tick for await-timeout sweep.
            _ = tick_interval.tick() => {
                let outs = broker.handle(BrokerInput::Tick, Instant::now());
                execute_outs(
                    outs,
                    &mut reply_senders,
                    &env_handle,
                    &broker,
                    sock_path,
                    bus_dir,
                    &inspect_semaphore,
                )
                .await;
            }
            // Arm 4: 5-minute idle exit.
            () = idle::wait_or_never(&mut idle) => {
                eprintln!("broker idle for {}s; shutting down", IDLE_TIMEOUT.as_secs());
                let _ = std::fs::remove_file(sock_path);
                return Ok(());
            }
            // Arm 5: SIGINT / SIGTERM.
            () = &mut shutdown_signal => {
                eprintln!("shutdown signal received, exiting");
                let _ = std::fs::remove_file(sock_path);
                return Ok(());
            }
        }
    }
}

/// Execute a `Vec<Out>` from the broker in declared order (D-04
/// invariant: `AppendMailbox` BEFORE `Reply(SendOk)`; `AdvanceCursor`
/// AFTER `Reply(RegisterOk)`).
///
/// The match below is EXHAUSTIVE — adding a new `Out` variant in
/// `famp_bus::Broker` MUST fail to compile here until handled. Do NOT
/// add a `_ =>` wildcard arm.
async fn execute_outs(
    outs: Vec<Out>,
    reply_senders: &mut HashMap<ClientId, mpsc::Sender<BusReply>>,
    env: &BrokerEnvHandle,
    broker: &Broker<BrokerEnvHandle>,
    sock_path: &Path,
    bus_dir: &Path,
    inspect_semaphore: &Arc<Semaphore>,
) {
    for out in outs {
        match out {
            Out::Reply(id, reply) => {
                if let Some(tx) = reply_senders.get(&id) {
                    let _ = tx.send(reply).await;
                }
            }
            Out::AppendMailbox { target, line } => {
                if let Err(e) = env.append(&target, line).await {
                    // Mailbox append failure on the durability path is
                    // a hard error — but we cannot return from a
                    // single Out execution because the broker still
                    // wants the rest of the vec executed in order.
                    // Log loudly; future work (Phase 4) may convert
                    // this into a broker-internal Err reply.
                    eprintln!("AppendMailbox failure: {e}");
                }
            }
            Out::AdvanceCursor { name, offset } => {
                let display = match name {
                    MailboxName::Agent(n) | MailboxName::Channel(n) => n,
                };
                if let Err(e) = cursor_exec::execute_advance_cursor(bus_dir, &display, offset).await
                {
                    eprintln!("AdvanceCursor failure: {e}");
                }
            }
            Out::ParkAwait { .. } | Out::UnparkAwait { .. } => {
                // Pure broker state; no executor side-effect.
            }
            Out::ReleaseClient(id) => {
                // The broker has cleared its internal state for `id`;
                // drop the wire-side reply sender so the per-client
                // write loop notices the channel close and exits.
                reply_senders.remove(&id);
            }
            Out::InspectRequest { client, kind } => {
                // GAP-03-01: dispatch inspect snapshot work off the main
                // `execute_outs` loop so saturated direct inspect RPC
                // pressure does not starve ordinary bus send/receive
                // traffic. Capture an immutable snapshot here (broker
                // state/cursor offsets are NOT Send; the broker handle
                // itself is borrowed through this fn) and hand it to a
                // spawned task that owns a bounded inspect permit.

                // Clone the requesting client's reply sender BEFORE
                // any snapshot work. The spawned task must not touch
                // the broker's reply_senders map (owned by this loop).
                let Some(reply_tx) = reply_senders.get(&client).cloned() else {
                    // Client disconnected between request and dispatch;
                    // nothing to reply to.
                    continue;
                };

                // GAP-03-01: try to acquire a permit FIRST so the
                // fast-shed path skips the broker.view()/cursor walk
                // when we are already at the inspect concurrency cap.
                // try_acquire_owned never awaits.
                let permit = match Arc::clone(inspect_semaphore).try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        // No permit; reply with budget_exceeded
                        // (elapsed_ms=0 since no walk/dispatch was
                        // attempted). Spawn the reply send so the main
                        // execute_outs loop is not blocked on a
                        // possibly-full per-client reply channel under
                        // saturated inspect pressure.
                        tokio::spawn(async move {
                            let payload = inspect_budget_exceeded_payload(0);
                            let _ = reply_tx.send(BusReply::InspectOk { payload }).await;
                        });
                        continue;
                    }
                };

                // state snapshot is captured before spawn because Broker is not Send.
                let state_snapshot = broker.view();
                let bus_dir_owned = bus_dir.to_path_buf();
                let sock_path_owned = sock_path.to_path_buf();
                let kind_for_blocking = kind.clone();

                // Permit acquired; spawn the snapshot + dispatch + reply
                // pipeline. The outer loop returns immediately to
                // processing other broker outputs.
                tokio::spawn(async move {
                    // The permit must outlive the spawned work; binding
                    // it to a local keeps it alive for the task body.
                    let _permit = permit;
                    // D-03/D-05: 500ms budget wraps the ENTIRE walk + dispatch.
                    let started = Instant::now();
                    let result = tokio::time::timeout(
                        Duration::from_millis(500),
                        tokio::task::spawn_blocking(move || {
                            let ctx = build_inspect_ctx_blocking(
                                &state_snapshot,
                                &sock_path_owned,
                                &bus_dir_owned,
                                &kind_for_blocking,
                            );
                            famp_inspect_server::dispatch(&state_snapshot, &ctx, &kind_for_blocking)
                        }),
                    )
                    .await;

                    let payload = match result {
                        Ok(Ok(payload)) => payload,
                        Ok(Err(join_err)) => {
                            // Blocking thread panicked. Surface as BudgetExceeded
                            // with elapsed_ms = 0 to keep the codec path single.
                            eprintln!("inspect spawn_blocking panicked: {join_err}");
                            inspect_budget_exceeded_payload(0)
                        }
                        Err(_elapsed) => {
                            // The blocking thread may continue briefly, but all
                            // file handles are stack-local and drop on thread exit.
                            let elapsed_ms = started.elapsed().as_millis() as u64;
                            inspect_budget_exceeded_payload(elapsed_ms)
                        }
                    };

                    let _ = reply_tx.send(BusReply::InspectOk { payload }).await;
                });
            }
            Out::SessionEnded { name, pid, joined } => {
                // WR-07: append the diagnostic SessionRow with the
                // broker's pre-disconnect snapshot of `joined`. Best-
                // effort; failure is logged but not fatal (sessions.jsonl
                // is diagnostic-only).
                let row = SessionRow { name, pid, joined };
                if let Err(e) = sessions_log::append_session_row(bus_dir, &row) {
                    eprintln!("sessions.jsonl write error: {e}");
                }
            }
        }
    }
}

/// Builds `BrokerCtx` from owned data -- runs inside `spawn_blocking`
/// (D-05/D-07). Lazy pre-read per D-06: only walks taskdir for
/// `InspectKind::Tasks(_)`; only walks mailbox JSONL for
/// `InspectKind::Tasks(_)` or `InspectKind::Messages(_)`.
fn build_inspect_ctx_blocking(
    view: &famp_bus::BrokerStateView,
    sock_path: &Path,
    bus_dir: &Path,
    kind: &famp_inspect_proto::InspectKind,
) -> BrokerCtx {
    let mailbox_metadata = view
        .clients
        .iter()
        .map(|client| {
            (
                client.name.clone(),
                read_mailbox_meta_for(bus_dir, &client.name),
            )
        })
        .collect::<BTreeMap<_, _>>();

    // D-06: lazy taskdir walk; only for Tasks requests.
    let task_data = if matches!(kind, famp_inspect_proto::InspectKind::Tasks(_)) {
        walk_taskdir(bus_dir)
    } else {
        None
    };

    // D-06: lazy mailbox JSONL pre-read; for Tasks (envelope chain
    // summaries) and Messages (the metadata surface itself).
    let message_data = match kind {
        famp_inspect_proto::InspectKind::Tasks(_)
        | famp_inspect_proto::InspectKind::Messages(_) => {
            Some(read_message_snapshot(bus_dir, view))
        }
        _ => None,
    };

    BrokerCtx {
        pid: std::process::id(),
        socket_path: sock_path.display().to_string(),
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        mailbox_metadata,
        task_data,
        message_data,
    }
}

/// D-07: taskdir walk runs inside `spawn_blocking`.
fn walk_taskdir(bus_dir: &Path) -> Option<famp_inspect_server::TaskSnapshot> {
    let dir = famp_taskdir::TaskDir::open(bus_dir.join("tasks")).ok()?;
    let records = dir.list().unwrap_or_default();
    Some(famp_inspect_server::TaskSnapshot {
        records: records
            .into_iter()
            .map(|r| famp_inspect_server::TaskSnapshotRow {
                task_id: r.task_id,
                state: r.state,
                peer: r.peer,
                opened_at: r.opened_at,
                last_send_at: r.last_send_at,
                last_recv_at: r.last_recv_at,
                terminal: r.terminal,
            })
            .collect(),
    })
}

/// Reads each registered identity's mailbox JSONL into a `MessageSnapshot`.
/// Missing or unreadable mailboxes contribute an empty `Vec` for that identity.
fn read_message_snapshot(
    bus_dir: &Path,
    view: &famp_bus::BrokerStateView,
) -> famp_inspect_server::MessageSnapshot {
    let mut by_recipient = BTreeMap::new();
    for client in &view.clients {
        let path = bus_dir
            .join("mailboxes")
            .join(format!("{}.jsonl", client.name));
        let entries = famp_inbox::read::read_all(&path).unwrap_or_default();
        by_recipient.insert(client.name.clone(), entries);
    }
    famp_inspect_server::MessageSnapshot { by_recipient }
}

fn inspect_budget_exceeded_payload(elapsed_ms: u64) -> serde_json::Value {
    serde_json::json!({ "kind": "budget_exceeded", "elapsed_ms": elapsed_ms })
}

fn read_mailbox_meta_for(bus_dir: &Path, name: &str) -> MailboxMeta {
    // fix 260512-jdv: cursor truth lives on disk at
    // `<bus_dir>/mailboxes/.<name>.cursor` written by
    // `cursor_exec::execute_advance_cursor`. The in-memory `BrokerState.cursors`
    // map was never populated and has been deleted.
    let cursor_path = bus_dir.join("mailboxes").join(format!(".{name}.cursor"));
    let cursor_offset = std::fs::read_to_string(&cursor_path)
        .ok()
        .and_then(|s| s.trim_end_matches('\n').parse::<u64>().ok())
        .unwrap_or(0);

    let path = bus_dir.join("mailboxes").join(format!("{name}.jsonl"));
    let Ok(entries) = famp_inbox::read::read_all(&path) else {
        return MailboxMeta::default();
    };
    let total = entries.len() as u64;
    let unread = famp_inbox::read::read_from(&path, cursor_offset)
        .map_or(0, |entries| entries.len().try_into().unwrap_or(u64::MAX));
    let last_sender = entries.last().and_then(|value| {
        value
            .get("from")
            .and_then(|from| from.as_str().map(String::from))
    });
    let last_received_at_unix_seconds = entries
        .last()
        .and_then(|value| value.get("ts").and_then(serde_json::Value::as_str))
        .and_then(|ts| {
            time::OffsetDateTime::parse(ts, &time::format_description::well_known::Rfc3339).ok()
        })
        .and_then(|dt| u64::try_from(dt.unix_timestamp()).ok());

    MailboxMeta {
        unread,
        total,
        last_sender,
        last_received_at_unix_seconds,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp_bus::{BrokerStateView, ClientStateView};
    use std::os::unix::net::UnixListener as StdUnixListener;
    use std::time::SystemTime;

    #[tokio::test]
    async fn test_bind_exclusive_returns_listener_on_clean_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let outcome = bind_exclusive(&sock).expect("clean bind");
        match outcome {
            BindOutcome::Bound(listener) => drop(listener),
            BindOutcome::Existing => panic!("expected fresh bind, got Existing"),
        }
        // The path should still exist (UDS leaves it on disk until
        // unlinked) — we let TempDir clean up.
        assert!(sock.exists());
    }

    #[tokio::test]
    async fn test_bind_exclusive_unlinks_stale_socket() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        // Simulate a stale socket: bind a UDS, then drop the listener
        // and the underlying fd without unlinking. UDS files persist
        // on the filesystem; a connect() to them returns ECONNREFUSED
        // because no process is listening.
        let stale = StdUnixListener::bind(&sock).unwrap();
        drop(stale);
        // The socket file is still present.
        assert!(sock.exists());
        // bind_exclusive sees EADDRINUSE, probes (connect refused),
        // unlinks, and re-binds.
        let outcome = bind_exclusive(&sock).expect("stale-unlink path");
        match outcome {
            BindOutcome::Bound(listener) => drop(listener),
            BindOutcome::Existing => panic!("expected re-bind, got Existing"),
        }
        assert!(sock.exists());
    }

    #[tokio::test]
    async fn test_bind_exclusive_returns_existing_when_live_broker_present() {
        // BL-02 regression: when another listener is live on the same
        // path, bind_exclusive must return BindOutcome::Existing rather
        // than calling std::process::exit(0).
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        // Hold the listener open so the connect() probe in bind_exclusive
        // succeeds.
        let live = StdUnixListener::bind(&sock).unwrap();
        let outcome = bind_exclusive(&sock).expect("existing-broker outcome");
        match outcome {
            BindOutcome::Existing => {}
            BindOutcome::Bound(_) => panic!("expected Existing, got Bound"),
        }
        drop(live);
    }

    mod broker_inspect_tests {
        use super::*;

        fn view_with_clients(names: &[&str]) -> BrokerStateView {
            BrokerStateView {
                started_at: SystemTime::now(),
                clients: names
                    .iter()
                    .map(|name| ClientStateView {
                        name: (*name).to_string(),
                        pid: None,
                        bind_as: None,
                        cwd: None,
                        listen_mode: false,
                        registered_at: SystemTime::now(),
                        last_activity: SystemTime::now(),
                        joined: vec![],
                    })
                    .collect(),
            }
        }

        #[test]
        fn taskdir_missing_returns_empty_snapshot() {
            let tmp = tempfile::TempDir::new().unwrap();
            let snap = walk_taskdir(tmp.path()).expect("fresh taskdir snapshot");
            assert!(snap.records.is_empty());
        }

        #[test]
        fn taskdir_with_one_record_returns_one_row() {
            let tmp = tempfile::TempDir::new().unwrap();
            let task_id = "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7";
            let dir = famp_taskdir::TaskDir::open(tmp.path().join("tasks")).unwrap();
            dir.create(&famp_taskdir::TaskRecord {
                task_id: task_id.to_string(),
                state: "COMMITTED".to_string(),
                peer: "agent:local.bus/x".to_string(),
                opened_at: "2026-05-10T18:00:00Z".to_string(),
                last_send_at: Some("2026-05-10T18:01:00Z".to_string()),
                last_recv_at: None,
                terminal: false,
            })
            .unwrap();

            let snap = walk_taskdir(tmp.path()).expect("taskdir snapshot");
            assert_eq!(snap.records.len(), 1, "expected one row, got {snap:?}");
            assert_eq!(snap.records[0].task_id, task_id);
            assert_eq!(snap.records[0].state, "COMMITTED");
            assert_eq!(snap.records[0].peer, "agent:local.bus/x");
            assert_eq!(
                snap.records[0].last_send_at.as_deref(),
                Some("2026-05-10T18:01:00Z")
            );
            assert!(!snap.records[0].terminal);
        }

        #[test]
        fn message_snapshot_missing_mailbox_is_empty_for_registered_client() {
            let tmp = tempfile::TempDir::new().unwrap();
            let view = view_with_clients(&["alice"]);
            let snap = read_message_snapshot(tmp.path(), &view);
            assert_eq!(
                snap.by_recipient.get("alice").map(Vec::len),
                Some(0),
                "missing mailbox should produce an empty vector"
            );
        }

        #[test]
        fn message_snapshot_populated_mailbox_returns_line_count() {
            let tmp = tempfile::TempDir::new().unwrap();
            let mailboxes = tmp.path().join("mailboxes");
            std::fs::create_dir_all(&mailboxes).unwrap();
            std::fs::write(
            mailboxes.join("alice.jsonl"),
            b"{\"from\":\"bob\",\"to\":\"alice\",\"body\":{\"n\":1}}\n{\"from\":\"carol\",\"to\":\"alice\",\"body\":{\"n\":2}}\n",
        )
        .unwrap();

            let view = view_with_clients(&["alice"]);
            let snap = read_message_snapshot(tmp.path(), &view);
            assert_eq!(snap.by_recipient["alice"].len(), 2);
        }

        #[test]
        fn build_inspect_ctx_for_broker_kind_does_not_walk_taskdir_or_mailboxes() {
            let tmp = tempfile::TempDir::new().unwrap();
            let view = view_with_clients(&["alice"]);
            let ctx = build_inspect_ctx_blocking(
                &view,
                &tmp.path().join("bus.sock"),
                tmp.path(),
                &famp_inspect_proto::InspectKind::Broker(
                    famp_inspect_proto::InspectBrokerRequest::default(),
                ),
            );
            assert!(ctx.task_data.is_none());
            assert!(ctx.message_data.is_none());
        }

        #[test]
        fn budget_exceeded_payload_is_kind_tagged_json() {
            let payload = inspect_budget_exceeded_payload(501);
            assert_eq!(payload["kind"], "budget_exceeded");
            assert_eq!(payload["elapsed_ms"], 501);
        }
    }
}
