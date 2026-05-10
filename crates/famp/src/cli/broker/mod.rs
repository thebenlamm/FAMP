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
use tokio::sync::mpsc;

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
                let ctx = build_inspect_ctx(broker, sock_path, bus_dir);
                let payload = famp_inspect_server::dispatch(&broker.view(), &ctx, &kind);
                if let Some(tx) = reply_senders.get(&client) {
                    let _ = tx.send(BusReply::InspectOk { payload }).await;
                }
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

fn build_inspect_ctx(
    broker: &Broker<BrokerEnvHandle>,
    sock_path: &Path,
    bus_dir: &Path,
) -> BrokerCtx {
    let view = broker.view();
    let mailbox_metadata = view
        .clients
        .iter()
        .map(|client| {
            let mailbox = MailboxName::Agent(client.name.clone());
            let cursor_offset = broker.cursor_offset(&mailbox);
            (
                client.name.clone(),
                read_mailbox_meta_for(bus_dir, &client.name, cursor_offset),
            )
        })
        .collect::<BTreeMap<_, _>>();

    BrokerCtx {
        pid: std::process::id(),
        socket_path: sock_path.display().to_string(),
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        mailbox_metadata,
    }
}

fn read_mailbox_meta_for(bus_dir: &Path, name: &str, cursor_offset: u64) -> MailboxMeta {
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
    use std::os::unix::net::UnixListener as StdUnixListener;

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
}
