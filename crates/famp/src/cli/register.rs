//! `famp register <name>` — Phase 2 plan 02-03 (CLI-01).
//!
//! Long-lived foreground subcommand that holds an identity slot on the
//! local UDS broker for the lifetime of the process. Per D-10 this is the
//! canonical registered holder of `name`: the connection sends
//! `Hello { bind_as: None }` and then `Register { name, pid, cwd, listen }`. All other
//! one-shot CLI commands (`send`, `inbox`, `await`, `join`, `leave`,
//! `whoami`, `sessions --me`) ride on this process by sending
//! `Hello { bind_as = name }` instead (the proxy shape).
//!
//! Default mode (D-08): exactly one stderr startup line, then silent
//! block until Ctrl-C or broker disconnect. `--tail` adds a 1-second
//! poll loop that prints incoming envelopes per RESEARCH §2 item 5
//! format. `--no-reconnect` exits non-zero on the first broker
//! disconnect (deterministic for tests/CI).
//!
//! Without `--no-reconnect`, the run loop reconnects with bounded
//! exponential backoff `1s → 2s → 4s → 8s → 16s → 30s → 30s …` capped
//! at 30 s per RESEARCH §2 item 8 (the CONTEXT.md cap of 60 s is
//! tuned down to 30 s here — broker idle exit is 5 min, so a 30 s cap
//! gives 2-3 reconnect attempts inside a typical 60 s window).
//!
//! On every reconnect attempt the loop calls
//! `bus_client::spawn::spawn_broker_if_absent` first so a broker that
//! exited from 5-min idle is respawned at the next attempt.

use std::path::Path;
use std::time::Duration;

use famp_bus::{BusErrorKind, BusMessage, BusReply};

use crate::bus_client::{spawn, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::util::shutdown_signal;

/// Initial reconnect delay (RESEARCH §2 item 8).
const RECONNECT_INITIAL: Duration = Duration::from_secs(1);
/// Reconnect ceiling (RESEARCH §2 item 8 — tuned down from CONTEXT.md's
/// 60 s to 30 s; broker idle exit is 300 s so 30 s gives 2-3 attempts in
/// a typical 60 s window).
const RECONNECT_CAP: Duration = Duration::from_secs(30);
/// `--tail` poll cadence: send `BusMessage::Inbox` every second and
/// print any new envelopes.
const TAIL_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Body-truncation cap for the `--tail` line (RESEARCH §2 item 5).
const TAIL_BODY_TRUNCATE: usize = 80;

/// Args for `famp register`.
#[derive(clap::Args, Debug, Clone)]
pub struct RegisterArgs {
    /// Identity name to register as. Becomes the canonical holder of
    /// this slot on the broker for the lifetime of this process.
    pub name: String,

    /// Opt into a live event stream on stderr. Default per D-08 is a
    /// single startup line then silent block.
    #[arg(long)]
    pub tail: bool,

    /// Exit non-zero on first broker disconnect instead of reconnecting.
    /// Deterministic flag for tests and CI; humans should leave it off
    /// so the process rides through transient broker restarts.
    #[arg(long)]
    pub no_reconnect: bool,
}

/// Production entry point for `famp register`.
///
/// Loops forever (until Ctrl-C / SIGTERM / `--no-reconnect`-driven
/// disconnect). Each iteration: spawn-if-absent → connect with
/// `bind_as: None` → `Register { name, pid, cwd, listen }` → either
/// `block_until_disconnect` (default) or `tail_loop` (with `--tail`).
/// Reconnect backoff resets to 1 s after every successful run.
pub async fn run(args: RegisterArgs) -> Result<(), CliError> {
    let sock = crate::bus_client::resolve_sock_path();
    let mut delay = RECONNECT_INITIAL;
    loop {
        // Spawn the broker if the socket is absent. Best-effort —
        // `BusClient::connect` will surface the unreachable case as
        // `BusClientError::Io` (connect stage) or `BrokerDidNotStart`
        // (spawn stage) and we map each to a stage-aware
        // `CliError::BusClient { detail }` below.
        let _ = spawn::spawn_broker_if_absent(&sock);

        // bind_as: None — `famp register` IS the canonical holder
        // (D-10), NOT a proxy. Proxy semantics are reserved for the
        // one-shot CLI subcommands.
        let connect_result = BusClient::connect(&sock, None).await;
        let connection_outcome = match connect_result {
            Ok(client) => run_one_session(client, &args, &sock).await,
            Err(e) => Err(map_bus_client_err(e, &sock)),
        };

        match connection_outcome {
            // Session ended cleanly (Ctrl-C inside block/tail, or
            // broker EOF without `--no-reconnect`). Reset backoff and
            // loop. Ctrl-C is observed as a `signal_caught` outcome
            // below and short-circuits the loop.
            Ok(SessionOutcome::SignalCaught) => return Ok(()),
            Ok(SessionOutcome::Disconnected) => {
                if args.no_reconnect {
                    return Err(CliError::Disconnected);
                }
                // BL-01: do NOT reset `delay` here. Resetting on every
                // disconnect collapses the documented `1 → 2 → 4 → 8 →
                // 16 → 30` schedule into a flat 1 s wait when the broker
                // bounces repeatedly (the thundering-herd / busy-loop
                // case bounded backoff is supposed to prevent). Backoff
                // grows on every disconnect; only a long-running session
                // (handled in `run_one_session` via the success-tick
                // reset below) returns the schedule to its initial value.
                eprintln!("broker disconnected — reconnecting in {}s", delay.as_secs());
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, RECONNECT_CAP);
            }
            // NameTaken and BusError are terminal: NameTaken's locked
            // stderr line was already emitted by the RegisterOk arm
            // fallthrough; both propagate to the binary as non-zero
            // exit (the outer match here is exhaustive over CliError
            // because every other variant funnels through `other_err`
            // below).
            Err(e @ (CliError::NameTaken { .. } | CliError::BusError { .. })) => return Err(e),
            Err(other_err) => {
                if args.no_reconnect {
                    return Err(other_err);
                }
                // Connect-time failure (broker unreachable, hello
                // refused, IO error). Log and retry with backoff.
                eprintln!(
                    "broker connect failed ({}) — reconnecting in {}s",
                    other_err,
                    delay.as_secs()
                );
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, RECONNECT_CAP);
            }
        }
    }
}

/// Outcome of a single broker session (one Hello+Register round).
enum SessionOutcome {
    /// SIGINT/SIGTERM observed; the run loop should exit Ok.
    SignalCaught,
    /// Broker dropped the connection (read returned EOF or error
    /// after the Register handshake). The run loop should reconnect
    /// (or exit non-zero with `--no-reconnect`).
    Disconnected,
}

/// One full Hello+Register session against an already-connected
/// `BusClient`. On success runs either `block_until_disconnect` or
/// `tail_loop` depending on `--tail`; on failure returns a typed
/// `CliError` that the outer run loop classifies.
async fn run_one_session(
    mut client: BusClient,
    args: &RegisterArgs,
    sock: &Path,
) -> Result<SessionOutcome, CliError> {
    let pid = std::process::id();
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string());
    let reply = client
        .send_recv(BusMessage::Register {
            name: args.name.clone(),
            pid,
            cwd,
            listen: args.tail,
        })
        .await
        .map_err(|e| map_bus_client_err(e, sock))?;

    match reply {
        BusReply::RegisterOk {
            active,
            drained,
            peers,
        } => {
            // Locked startup line per RESEARCH §2 item 12 (stderr).
            // Format pinned by acceptance criteria grep — do NOT
            // reformat without updating the plan's truths block.
            eprintln!(
                "registered as {} (pid {}, joined: {:?}, peers: {:?}) — Ctrl-C to release",
                active,
                pid,
                Vec::<String>::new(),
                peers
            );

            if args.tail {
                // Drain backlog first so the user sees pre-register
                // state before live events. `next_offset` for the
                // poll loop starts at 0 because Register.drained is
                // the broker's full mailbox snapshot at register
                // time; subsequent Inbox polls advance from the
                // broker's reported `next_offset`.
                for env in &drained {
                    emit_tail_line(env);
                }
                tail_loop(&mut client, &args.name, sock).await
            } else {
                block_until_disconnect(&mut client).await
            }
        }
        BusReply::Err {
            kind: BusErrorKind::NameTaken,
            ..
        } => {
            // Locked text per acceptance-criteria truth block.
            eprintln!("{} is already registered by another process", args.name);
            Err(CliError::NameTaken {
                name: args.name.clone(),
            })
        }
        BusReply::Err { kind, message } | BusReply::HelloErr { kind, message } => {
            Err(CliError::BusError { kind, message })
        }
        other => Err(CliError::BusError {
            kind: BusErrorKind::Internal,
            message: format!("unexpected broker reply: {other:?}"),
        }),
    }
}

/// Block until either Ctrl-C (return `SignalCaught` so the run loop
/// exits Ok) or the broker connection drops (return `Disconnected` so
/// the outer reconnect-with-backoff loop fires; load-bearing for
/// TEST-03 kill-9 recovery). The `UnixStream` inside the client closes
/// via Drop when this function returns; the broker observes that as a
/// `Disconnect` per its run-loop's per-client `Disconnect` arm.
///
/// We do a 1-byte peek-style read on the wire so that broker death
/// (process kill, network reset) becomes a wakeable event rather than
/// requiring the next request/reply round-trip to surface it. The
/// Phase-1 broker contract forbids unsolicited frames, so any readable
/// event MUST be EOF/error, never a valid frame. If a future Phase-2
/// extension introduces server-pushed events the polling path lives in
/// `tail_loop` (`--tail`), not here.
async fn block_until_disconnect(client: &mut BusClient) -> Result<SessionOutcome, CliError> {
    tokio::select! {
        () = shutdown_signal() => Ok(SessionOutcome::SignalCaught),
        () = client.wait_for_disconnect() => Ok(SessionOutcome::Disconnected),
    }
}

/// 1-second poll loop that fetches new envelopes via
/// `BusMessage::Inbox` and prints them to stderr in the
/// RESEARCH §2 item 5 format. Cursor advance is written via
/// `cli::broker::cursor_exec` for parity with `famp inbox ack`.
async fn tail_loop(
    client: &mut BusClient,
    identity: &str,
    sock: &Path,
) -> Result<SessionOutcome, CliError> {
    // Local cursor: starts at 0, advances by `next_offset` each poll.
    let mut cursor: u64 = 0;
    let bus_dir = sock.parent().unwrap_or_else(|| Path::new("/"));
    loop {
        tokio::select! {
            biased;
            () = shutdown_signal() => return Ok(SessionOutcome::SignalCaught),
            res = client.send_recv(BusMessage::Inbox {
                since: Some(cursor),
                include_terminal: None,
            }) => {
                match res {
                    Ok(BusReply::InboxOk { envelopes, next_offset }) => {
                        for env in &envelopes {
                            emit_tail_line(env);
                        }
                        if next_offset > cursor {
                            // Persist cursor advance to disk (mirrors
                            // `famp inbox ack` so a follow-up
                            // `famp inbox list` does not re-emit lines
                            // already tailed). Best-effort — a write
                            // failure is logged but does not tear down
                            // the tail loop.
                            if let Err(e) = crate::cli::broker::cursor_exec::execute_advance_cursor(
                                bus_dir,
                                identity,
                                next_offset,
                            )
                            .await
                            {
                                eprintln!("warning: cursor advance failed: {e}");
                            }
                            cursor = next_offset;
                        }
                    }
                    Ok(BusReply::Err { kind, message }) => {
                        return Err(CliError::BusError { kind, message });
                    }
                    Ok(other) => {
                        return Err(CliError::BusError {
                            kind: BusErrorKind::Internal,
                            message: format!("unexpected broker reply on Inbox: {other:?}"),
                        });
                    }
                    Err(e) => {
                        // Most likely the broker closed the socket.
                        // Surface as a normal Disconnected outcome so
                        // the outer run loop can reconnect.
                        if matches!(e, BusClientError::Io(_)) {
                            return Ok(SessionOutcome::Disconnected);
                        }
                        return Err(map_bus_client_err(e, sock));
                    }
                }
            }
        }
        tokio::time::sleep(TAIL_POLL_INTERVAL).await;
    }
}

/// Emit one line per RESEARCH §2 item 5 format:
/// `< <ISO-8601Z> from=<name> to=<name|#chan> task=<uuid> body="<truncated-80>"`
///
/// Reads `from`, `to`, `task`, and the body envelope-shape-agnostically
/// from the canonical-JSON value. Missing fields fall back to a
/// placeholder so a malformed envelope still prints one tail line and
/// the loop keeps running.
fn emit_tail_line(envelope: &serde_json::Value) {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

    let from = envelope
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    // `to` may be a string (channel "#chan") or a structured target.
    // Canonical envelope uses string at top level for inbox tail
    // purposes; fall back to debug-quote if not a string.
    let to = envelope.get("to").map_or_else(
        || "?".to_string(),
        |t| t.as_str().map_or_else(|| t.to_string(), str::to_string),
    );
    let task = envelope
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("-")
        .to_string();

    // Body: prefer string body field, fall back to compact debug.
    let body_raw = envelope.get("body").map_or_else(String::new, |b| {
        b.as_str().map_or_else(|| b.to_string(), str::to_string)
    });

    let body = truncate_for_tail(&body_raw);
    eprintln!("< {now} from={from} to={to} task={task} body=\"{body}\"");
}

/// Truncate `s` to at most `TAIL_BODY_TRUNCATE` characters and escape
/// embedded double-quotes / control characters so the line stays
/// single-line and parseable.
fn truncate_for_tail(s: &str) -> String {
    let truncated: String = s.chars().take(TAIL_BODY_TRUNCATE).collect();
    truncated
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Map `BusClientError` to a typed `CliError`. Transport failures route
/// through `CliError::BusClient { detail }` with stage-aware, errno-bearing
/// messages so operators can distinguish "no broker at socket" (connect
/// stage) from "we forked a broker and the OS refused process creation"
/// (spawn stage). `HelloErr` surfaces as a `BusError` so the outer run
/// loop can decide on reconnect/exit semantics.
///
/// `BrokerUnreachable` is NOT used here — it is reserved for the ~30 other
/// call sites (send/await/join/leave/sessions/whoami/inbox + mcp tools)
/// that do not have the stage context available. The retry/backoff path in
/// `run` is unchanged: `BusClient { .. }` falls into the same
/// `Err(other_err)` catch-all arm that `BrokerUnreachable` used.
fn map_bus_client_err(e: BusClientError, sock: &Path) -> CliError {
    match e {
        BusClientError::Io(io) => CliError::BusClient {
            detail: format!(
                "could not connect to existing broker at {}: {io}",
                sock.display()
            ),
        },
        BusClientError::BrokerDidNotStart(spawn_err) => match spawn_err {
            spawn::SpawnError::Io(io) => CliError::BusClient {
                detail: format!(
                    "tried to spawn a broker and process creation failed (spawn io: {io}) — \
                     if running inside a sandbox, broker process creation (fork/setsid) may be blocked; \
                     start a broker outside the sandbox or set FAMP_BUS_SOCKET to a reachable broker"
                ),
            },
            spawn::SpawnError::CurrentExe(io) => CliError::BusClient {
                detail: format!(
                    "tried to spawn a broker but could not locate the famp executable (current-exe: {io})"
                ),
            },
            spawn::SpawnError::BrokerDidNotStart => CliError::BusClient {
                detail: format!(
                    "spawned a broker but it did not bind {} within 2s — check the broker log at {}",
                    sock.display(),
                    sock.parent()
                        .unwrap_or_else(|| Path::new("/"))
                        .join("broker.log")
                        .display()
                ),
            },
            spawn::SpawnError::SocketPathNotUtf8 => CliError::BusClient {
                detail: format!(
                    "broker socket path is not valid UTF-8: {}",
                    sock.display()
                ),
            },
        },
        BusClientError::HelloFailed { kind, message } => CliError::BusError { kind, message },
        BusClientError::Frame(err) => CliError::BusError {
            kind: BusErrorKind::Internal,
            message: format!("frame codec error: {err}"),
        },
        BusClientError::Decode(err) => CliError::BusError {
            kind: BusErrorKind::EnvelopeInvalid,
            message: format!("decode error: {err}"),
        },
        BusClientError::UnexpectedReply(msg) => CliError::BusError {
            kind: BusErrorKind::Internal,
            message: format!("unexpected reply: {msg}"),
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn truncate_for_tail_caps_at_80() {
        let long = "a".repeat(200);
        let out = truncate_for_tail(&long);
        assert_eq!(out.chars().count(), TAIL_BODY_TRUNCATE);
    }

    #[test]
    fn truncate_for_tail_escapes_quotes_and_newlines() {
        let s = "hello \"world\"\nnext";
        let out = truncate_for_tail(s);
        assert!(out.contains("\\\""));
        assert!(out.contains("\\n"));
    }

    #[test]
    fn emit_tail_line_handles_missing_fields() {
        // The function prints to stderr; we only verify it does not
        // panic on a degenerate envelope (e.g. missing every field).
        let env = serde_json::json!({});
        emit_tail_line(&env);
    }

    #[test]
    fn reconnect_backoff_schedule_matches_research_item_8() {
        // Mirrors the schedule documented in the module comment and
        // RESEARCH §2 item 8: 1 → 2 → 4 → 8 → 16 → 30 → 30 → ...
        let mut d = RECONNECT_INITIAL;
        let observed = (0..7)
            .map(|_| {
                let cur = d;
                d = std::cmp::min(d * 2, RECONNECT_CAP);
                cur.as_secs()
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, vec![1, 2, 4, 8, 16, 30, 30]);
    }

    /// `BusClientError::Io` maps to a connect-stage message containing the
    /// socket path AND the os-error text. ECONNREFUSED-ish (errno 111).
    #[test]
    fn map_bus_client_err_io_contains_socket_and_errno() {
        let io_err = std::io::Error::from_raw_os_error(111);
        let sock = Path::new("/tmp/famp-test-wj6.sock");
        let err = map_bus_client_err(BusClientError::Io(io_err), sock);
        let CliError::BusClient { detail } = err else {
            panic!("expected CliError::BusClient, got {err:?}");
        };
        assert!(
            detail.contains("os error"),
            "expected 'os error' in detail, got: {detail}"
        );
        assert!(
            detail.contains("famp-test-wj6.sock"),
            "expected socket path in detail, got: {detail}"
        );
    }

    /// `BusClientError::BrokerDidNotStart(SpawnError::Io)` — fork/setsid blocked
    /// by a sandbox — maps to a spawn-stage message containing the os-error
    /// text, "sandbox", and "spawn".
    #[test]
    fn map_bus_client_err_broker_did_not_start_spawn_io_contains_errno_and_sandbox() {
        let io_err = std::io::Error::from_raw_os_error(1); // EPERM — fork/setsid class
        let sock = Path::new("/tmp/famp-test-wj6.sock");
        let err = map_bus_client_err(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::Io(io_err)),
            sock,
        );
        let CliError::BusClient { detail } = err else {
            panic!("expected CliError::BusClient, got {err:?}");
        };
        assert!(
            detail.contains("os error"),
            "expected 'os error' in detail (errno must not be swallowed), got: {detail}"
        );
        assert!(
            detail.contains("sandbox"),
            "expected 'sandbox' hint in detail, got: {detail}"
        );
        assert!(
            detail.contains("spawn"),
            "expected 'spawn' in detail, got: {detail}"
        );
    }

    /// `BusClientError::BrokerDidNotStart(SpawnError::BrokerDidNotStart)` —
    /// genuine 2s timeout, no errno — maps to a message mentioning the timeout
    /// and pointing at the broker log. Must NOT claim an os error.
    #[test]
    fn map_bus_client_err_broker_did_not_start_timeout_points_at_log() {
        let sock = Path::new("/tmp/famp-test-wj6.sock");
        let err = map_bus_client_err(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::BrokerDidNotStart),
            sock,
        );
        let CliError::BusClient { detail } = err else {
            panic!("expected CliError::BusClient, got {err:?}");
        };
        assert!(
            detail.contains("2s") || detail.contains("2 s"),
            "expected 2s timeout mention in detail, got: {detail}"
        );
        assert!(
            detail.contains("broker.log"),
            "expected broker log pointer in detail, got: {detail}"
        );
        assert!(
            !detail.contains("os error"),
            "genuine timeout has no os error, but detail claims one: {detail}"
        );
    }
}
