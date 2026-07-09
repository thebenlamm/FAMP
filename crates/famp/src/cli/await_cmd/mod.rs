//! `famp await` — block until an envelope arrives on the local bus.
//!
//! ## v0.9 rewire (Phase 02 plan 02-06, CLI-05)
//!
//! Replaces the v0.8 `inbox.jsonl` polling shape with a single round-trip
//! to the local-first UDS broker:
//!
//! 1. Resolve identity via the D-01 four-tier resolver
//!    (`--as` > `$FAMP_LOCAL_IDENTITY` > cwd → wires.tsv > error).
//! 2. Open a `BusClient` with `Hello { bind_as: Some(resolved_identity) }`
//!    (D-10 proxy binding). The broker validates that `resolved_identity`
//!    is held by a live `famp register <name>` process; refusal surfaces
//!    as `BusClientError::HelloFailed { kind: NotRegistered, .. }` which
//!    we translate to `CliError::NotRegisteredHint` (D-02 hard error).
//! 3. Send `BusMessage::Await { timeout_ms, task }` and wait for one reply.
//! 4. On `BusReply::AwaitOk { envelopes, .. }` print each typed envelope as
//!    one JSONL line on stdout and exit 0.
//! 5. On `BusReply::AwaitTimeout {}` print `{"timeout":true}` on stdout and
//!    exit 0 — timeout is NOT an error per D-02; only `BusReply::Err` is.
//! 6. On `BusReply::Err { kind: NotRegistered, .. }` (per-op liveness
//!    re-check failed — holder died between Hello and Await) surface the
//!    same `NotRegisteredHint`.
//!
//! ## Three-layer pattern
//!
//! - [`run`] — production entry point. Resolves the bus socket via
//!   `bus_client::resolve_sock_path()` and forwards to [`run_at`] writing
//!   to `stdout`.
//! - [`run_at`] — sock-explicit + writer-explicit wrapper used by
//!   integration tests (lets the harness redirect stdout into a buffer
//!   and override the socket via `$FAMP_BUS_SOCKET`).
//! - [`run_at_structured`] — typed return for MCP-tool reuse
//!   (`crates/famp/src/cli/mcp/tools/await_.rs` calls this so the MCP
//!   tool surface is identical to the CLI).
//!
//! ## Output shape (locked by D-02 + plan 02-06, updated by quick-260515-s3h)
//!
//! ```json
//! {"mailbox": {"kind": "channel"/"agent", "name": "..."}, "envelopes": [...], "next_offset": 42}
//! {"timeout": true}                                        // on AwaitTimeout
//! ```
//!
//! Identity binding is at the connection level via `Hello.bind_as`
//! (D-10) — the `Await` message itself carries no identity field.

use std::io::Write;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply, MailboxName};
use serde_json::Value;
use tokio::io::unix::AsyncFd;

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

/// Reserved module path.
///
/// The v0.9 await is a single-shot bus round-trip and no longer polls a
/// file, so the v0.8 `poll::find_match` helper is dead code under this
/// transport. Kept so a follow-up plan can re-introduce typed shaping
/// helpers without churning callers.
#[allow(dead_code)]
pub mod poll;

/// CLI flags for `famp await`.
///
/// `--timeout` accepts any `humantime` duration (`5s`, `30s`, `2m`,
/// `250ms`). Default `30s`.
///
/// `--task` accepts a UUID; when set, the broker filters its mailbox
/// stream and only returns envelopes whose task matches.
///
/// `--as` overrides the D-01 identity resolution chain. The resolved
/// value feeds into `Hello.bind_as` on the connection (D-10). The Rust
/// field is named `act_as` because `as` is a reserved keyword.
#[derive(clap::Args, Debug)]
pub struct AwaitArgs {
    /// Block timeout. Accepts `30s`, `5m`, `1h`, `250ms`, etc.
    #[arg(long, default_value = "30s")]
    pub timeout: humantime::Duration,
    /// Optional task-id filter (UUID).
    #[arg(long)]
    pub task: Option<uuid::Uuid>,
    /// Override identity (D-01); the resolved value feeds into
    /// `Hello.bind_as` on the connection (D-10).
    #[arg(long = "as")]
    pub act_as: Option<String>,
    /// Generic, host-neutral cancellation seam (issue #21). When set, the
    /// parked await races the bus reply against readability (bytes **or**
    /// EOF) on this already-open file descriptor. On abort the command
    /// prints `{"aborted":true}` and exits **3** (distinct from 0 =
    /// message/timeout, 1 = error). The fd must be `>= 3` (never stdio)
    /// and already open; an invalid fd is a hard error, not UB.
    ///
    /// This carries **no** knowledge of what writes the fd — a pipe, a
    /// FIFO, whatever. All host coupling (e.g. the Claude Code Stop hook's
    /// queue watcher) lives outside `famp`.
    #[arg(long = "abort-on-fd")]
    pub abort_on_fd: Option<i32>,
}

/// Structured outcome from [`run_at_structured`]. `envelopes` is empty
/// on `AwaitTimeout`; on `AwaitOk` it carries the broker-delivered batch.
///
/// `timed_out` is the orthogonal flag the MCP tool surfaces in the
/// JSON-RPC result.
#[derive(Debug, Clone)]
pub struct AwaitOutcome {
    /// Typed envelopes returned by the broker. Empty on timeout.
    pub envelopes: Vec<serde_json::Value>,
    /// Mailbox that produced this batch. None on timeout.
    pub mailbox: Option<MailboxName>,
    /// Resume offset for `mailbox`. None on timeout.
    pub next_offset: Option<u64>,
    /// `true` when the broker returned `BusReply::AwaitTimeout {}`.
    pub timed_out: bool,
    /// Optional human-readable diagnostic printed with timeout JSON.
    pub diagnostic: Option<String>,
    /// `true` when the await was cancelled via `--abort-on-fd` before a
    /// reply arrived (issue #21). Always `false` unless `abort_on_fd` was
    /// set; the MCP tool never sets it, so this stays `false` there.
    /// Drives the `{"aborted":true}` stdout line and the exit-code-3 return.
    pub aborted: bool,
}

/// Top-level entry point for `Commands::Await`.
///
/// Resolves the bus socket, performs the bus round-trip, then writes one
/// JSON line to stdout AFTER awaiting the bus future. Doing the write
/// outside the `.await` boundary keeps the returned future `Send` —
/// `std::io::stdout().lock()` returns a non-`Send` guard which would
/// otherwise leak across the suspension point.
pub async fn run(args: AwaitArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(&resolve_sock_path(), args).await?;
    write_outcome(&outcome, &mut std::io::stdout())?;
    // Exit 3 is the distinct abort code (issue #21): 0 = message/timeout,
    // 1 = real error, 3 = cancelled via --abort-on-fd. `main` maps
    // `CliError::Exit(code)` → `process::exit(code)` WITHOUT printing an
    // error line, so the `{"aborted":true}` stdout above is the only output.
    if outcome.aborted {
        return Err(CliError::Exit(3));
    }
    Ok(())
}

/// Render an [`AwaitOutcome`] to a writer as a single JSONL line.
///
/// Split out from [`run_at`] so callers (`run`, integration tests) can
/// drive the write side independently of the bus round-trip.
///
/// Output shape (quick-260515-s3h):
/// - On message: `{"mailbox": {"kind": "channel"/"agent", "name": "..."}, "envelopes": [...], "next_offset": N}`
/// - On timeout: `{"timeout": true}` (optionally with `"diagnostic": "..."`)
pub(crate) fn write_outcome(outcome: &AwaitOutcome, mut out: impl Write) -> Result<(), CliError> {
    if outcome.aborted {
        // Cancellation seam (issue #21). Peer/host bytes never reach this
        // line — it is a fixed sentinel. The caller (`run`/`run_at`) maps
        // this to exit code 3.
        writeln!(out, "{}", serde_json::json!({"aborted": true})).map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: e,
        })?;
        return Ok(());
    }
    if outcome.timed_out {
        let mut value = serde_json::json!({"timeout": true});
        if let Some(diagnostic) = outcome.diagnostic.as_deref() {
            value["diagnostic"] = serde_json::json!(diagnostic);
        }
        writeln!(out, "{value}").map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: e,
        })?;
    } else {
        let mailbox_value = match &outcome.mailbox {
            Some(MailboxName::Channel(name)) => {
                serde_json::json!({"kind": "channel", "name": name})
            }
            Some(MailboxName::Agent(name)) => {
                serde_json::json!({"kind": "agent", "name": name})
            }
            None => serde_json::Value::Null,
        };
        let wrapper = serde_json::json!({
            "mailbox": mailbox_value,
            "envelopes": outcome.envelopes,
            "next_offset": outcome.next_offset,
        });
        let line = serde_json::to_string(&wrapper).map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: e,
        })?;
    }
    Ok(())
}

/// Sock-explicit + writer-explicit wrapper.
///
/// Tests and the integration harness call this so they can redirect
/// stdout into a buffer and drive the bus over a tempdir socket via
/// `$FAMP_BUS_SOCKET`. The writer is consumed AFTER the bus round-trip
/// so the future stays `Send` regardless of the writer type.
pub async fn run_at(sock: &Path, args: AwaitArgs, out: impl Write) -> Result<(), CliError> {
    let outcome = run_at_structured(sock, args).await?;
    write_outcome(&outcome, out)?;
    if outcome.aborted {
        return Err(CliError::Exit(3));
    }
    Ok(())
}

/// Structured entry point.
///
/// Performs the bus round-trip and returns the typed [`AwaitOutcome`]
/// without printing. Used by the MCP tool wrapper
/// (`cli::mcp::tools::await_::call`) so the JSON-RPC result shape is
/// owned by the MCP layer, not by this CLI.
pub async fn run_at_structured(sock: &Path, args: AwaitArgs) -> Result<AwaitOutcome, CliError> {
    // 1. D-01 four-tier identity resolution.
    let identity = resolve_identity(args.act_as.as_deref())?;

    // 2. Convert humantime::Duration → u64 ms with a saturating cap.
    let timeout_ms: u64 = std::time::Duration::from(args.timeout)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);

    // 2b. Build the optional cancellation seam (issue #21) BEFORE connecting
    //     so an invalid fd is a fast, broker-independent hard error rather
    //     than surfacing only after a Hello round-trip.
    let abort_fd = match args.abort_on_fd {
        Some(n) => Some(build_abort_fd(n)?),
        None => None,
    };

    // 3. Open the bus connection with the D-10 proxy binding. The
    //    broker validates at Hello time that `identity` is held by a
    //    live `famp register` process; refusal surfaces as
    //    HelloFailed { NotRegistered }.
    let mut bus = connect_bound(sock, &identity).await?;

    // 4. Single round-trip: Await { timeout_ms, task }. When a
    //    cancellation fd is armed, race the reply against fd-readability;
    //    `None` means the abort won.
    let msg = BusMessage::Await {
        timeout_ms,
        task: args.task,
    };
    let reply = if let Some(async_fd) = &abort_fd {
        match bus
            .send_recv_abortable(msg, async_fd)
            .await
            .map_err(|e| CliError::BusClient {
                detail: format!("{e:?}"),
            })? {
            Some(reply) => reply,
            None => {
                return Ok(AwaitOutcome {
                    envelopes: Vec::new(),
                    mailbox: None,
                    next_offset: None,
                    timed_out: false,
                    diagnostic: None,
                    aborted: true,
                });
            }
        }
    } else {
        bus.send_recv(msg).await.map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?
    };

    // 5. Map the four expected reply variants. `BusErrorKind` is closed
    //    so the `_ =>` wildcard arm is on `BusReply` variants we never
    //    expect in response to an Await op (e.g. `SendOk`); per
    //    plan acceptance this is a `BusClient { source: ... }` error,
    //    NOT a `BusError`-with-kind, because there is no kind to carry.
    match reply {
        BusReply::AwaitOk {
            envelopes,
            mailbox,
            next_offset,
        } => Ok(AwaitOutcome {
            envelopes,
            mailbox: Some(mailbox),
            next_offset: Some(next_offset),
            timed_out: false,
            diagnostic: None,
            aborted: false,
        }),
        BusReply::AwaitTimeout {} => {
            let diagnostic = timeout_diagnostic(&mut bus, &identity, args.task).await;
            Ok(AwaitOutcome {
                envelopes: Vec::new(),
                mailbox: None,
                next_offset: None,
                timed_out: true,
                diagnostic: Some(diagnostic),
                aborted: false,
            })
        }
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Await: {other:?}"),
        }),
    }
}

/// Validate and adopt a caller-supplied file descriptor as an async
/// cancellation seam (issue #21).
///
/// The fd must be `>= 3` (refusing to steal stdio) and already open. We
/// verify openness with `F_GETFD` (EBADF on a closed fd → hard error) and
/// set `O_NONBLOCK` because [`AsyncFd`] requires a non-blocking fd. A
/// readable event on the returned `AsyncFd` (bytes OR EOF) is the abort
/// signal — this is generic over pipes and FIFOs and carries no
/// host-specific meaning.
///
/// # Ownership / unsafe
///
/// nix 0.31's `fcntl` takes `AsFd`, not `RawFd`, so we adopt the fd via
/// `OwnedFd::from_raw_fd` FIRST and validate immediately after. That is
/// the single narrowly-scoped `#[allow(unsafe_code)]` in this crate
/// outside `bus_client::spawn` — the crate posture is `unsafe_code =
/// "deny"` (not `forbid`) expressly to permit exactly this.
///
/// If the very first check (`F_GETFD`) shows the fd is closed, we
/// `mem::forget` the `OwnedFd` instead of dropping it: dropping an
/// `OwnedFd` that wraps an invalid fd makes `close(2)` return `EBADF`,
/// which Rust's IO-safety runtime treats as a fatal double-close and
/// `abort()`s the process. Forgetting the (invalid) fd leaks nothing —
/// there is no real resource behind it — and lets us return a clean
/// `CliError`. Later failure paths (`F_GETFL`/`F_SETFL`/reactor
/// registration) run only after `F_GETFD` proved the fd valid, so a
/// normal drop there is correct.
fn build_abort_fd(n: i32) -> Result<AsyncFd<OwnedFd>, CliError> {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};

    if n < 3 {
        return Err(CliError::Generic(format!(
            "--abort-on-fd must be >= 3 (refusing to take over stdio); got {n}"
        )));
    }

    // SAFETY: We assume ownership of the caller-supplied fd. Validity is
    // verified via F_GETFD immediately below before any use; an invalid fd
    // yields a hard CliError (after mem::forget to dodge the close-EBADF
    // abort). Single-unsafe adaptation required by nix 0.31's AsFd-based
    // fcntl API.
    #[allow(unsafe_code)]
    let owned = unsafe { OwnedFd::from_raw_fd(n) };

    // Verify the fd is actually open (EBADF → hard error, not UB). On
    // failure, forget the OwnedFd so its Drop does not close(2) an invalid
    // fd and trip the IO-safety runtime abort.
    if let Err(e) = fcntl(owned.as_fd(), FcntlArg::F_GETFD) {
        std::mem::forget(owned);
        return Err(CliError::Generic(format!(
            "--abort-on-fd {n} is not a valid open file descriptor: {e}"
        )));
    }

    // AsyncFd requires the fd be non-blocking.
    let cur = fcntl(owned.as_fd(), FcntlArg::F_GETFL).map_err(|e| {
        CliError::Generic(format!("failed to read flags on --abort-on-fd {n}: {e}"))
    })?;
    let newflags = OFlag::from_bits_truncate(cur) | OFlag::O_NONBLOCK;
    fcntl(owned.as_fd(), FcntlArg::F_SETFL(newflags)).map_err(|e| {
        CliError::Generic(format!(
            "failed to set O_NONBLOCK on --abort-on-fd {n}: {e}"
        ))
    })?;

    AsyncFd::new(owned).map_err(|e| {
        CliError::Generic(format!(
            "failed to register --abort-on-fd {n} with the async reactor: {e}"
        ))
    })
}

pub(crate) async fn connect_bound(sock: &Path, identity: &str) -> Result<BusClient, CliError> {
    BusClient::connect(sock, Some(identity.to_string()))
        .await
        .map_err(|e| match e {
            // D-10 typed Hello rejections we already classify specifically.
            BusClientError::HelloFailed {
                kind: BusErrorKind::NotRegistered,
                ..
            } => CliError::NotRegisteredHint {
                name: identity.to_string(),
            },
            BusClientError::HelloFailed {
                kind: BusErrorKind::BrokerProtoMismatch,
                message,
            } => CliError::BusError {
                kind: BusErrorKind::BrokerProtoMismatch,
                message: mixed_binary_hint(message),
            },
            // Any other typed Hello rejection (Internal, EnvelopeInvalid,
            // BrokerUnreachable-as-kind, etc.) — propagate the real
            // (kind, message) so operators see WHY the broker refused
            // instead of a generic "broker unreachable". Mapping this
            // family to BrokerUnreachable was the bug we just removed:
            // `inbox list` and `await` both bind the same identity via
            // D-10 proxy Hello, so when one refused and the other
            // succeeded the rejection reason was unrecoverable from the
            // CLI surface, making the Stop hook's "broker unreachable"
            // line uninvestigable.
            BusClientError::HelloFailed { kind, message } => CliError::BusError { kind, message },
            // Codec corruption usually means cross-build client/broker
            // mismatch — keep the existing hint.
            BusClientError::Frame(_) | BusClientError::Decode(_) => CliError::BusClient {
                detail: mixed_binary_hint(format!("{e:?}")),
            },
            // Genuine "the broker is not at this socket": NotFound /
            // ConnectionRefused on the underlying UnixStream. Anything
            // else (PermissionDenied, BrokenPipe, timeout, ...) deserves
            // its real error surface.
            BusClientError::Io(ref ioe)
                if matches!(
                    ioe.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                CliError::BrokerUnreachable
            }
            // Spawn failed, other IO error, or an unexpected non-Err
            // reply to Hello. Each has a real cause worth printing —
            // funnel through BusClient { detail } which preserves the
            // Debug chain (BrokerDidNotStart wraps SpawnError, Io wraps
            // io::Error with kind).
            // VER-01: ProtocolMismatch (bus_proto integer mismatch) — surface
            // the Display string which already names `famp daemon restart`.
            BusClientError::ProtocolMismatch { .. } => CliError::BusClient {
                detail: format!("{e}"),
            },
            BusClientError::Io(_)
            | BusClientError::BrokerDidNotStart(_)
            | BusClientError::UnexpectedReply(_) => CliError::BusClient {
                detail: format!("{e:?}"),
            },
        })
}

fn mixed_binary_hint(detail: impl AsRef<str>) -> String {
    let exe = std::env::current_exe().map_or_else(
        |_| "(unknown current executable)".to_string(),
        |p| p.display().to_string(),
    );
    format!(
        "{}. This can happen when a client from one FAMP build talks to a broker from another build. Current client: {exe}. Run `famp inspect broker` to see the broker pid/build/socket, then restart the broker if needed.",
        detail.as_ref()
    )
}

async fn timeout_diagnostic(
    bus: &mut BusClient,
    identity: &str,
    task: Option<uuid::Uuid>,
) -> String {
    let base = task.map_or_else(
        || "await timed out waiting for a new message".to_string(),
        |task| format!("await timed out waiting for new messages on task {task}"),
    );
    let hint = format!(
        "Run `famp inbox list --as {identity} --include-terminal` to inspect already-delivered messages, or use `famp wait-reply --as {identity} --task <task_id>` for reply waits that check the inbox before blocking."
    );

    if let Some(task) = task {
        if matches!(inbox_has_reply_for_task(bus, task).await, Ok(true)) {
            return format!(
                "{base}; a matching reply is already present in the inbox but was not new past the await cursor. {hint}"
            );
        }
    }

    format!("{base}. {hint}")
}

pub(crate) async fn inbox_has_reply_for_task(
    bus: &mut BusClient,
    task: uuid::Uuid,
) -> Result<bool, CliError> {
    let reply = bus
        .send_recv(BusMessage::Inbox {
            since: Some(0),
            include_terminal: Some(true),
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    match reply {
        BusReply::InboxOk { envelopes, .. } => Ok(envelopes
            .iter()
            .any(|envelope| is_reply_for_task(envelope, task))),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Inbox diagnostic: {other:?}"),
        }),
    }
}

pub(crate) fn is_reply_for_task(envelope: &Value, task: uuid::Uuid) -> bool {
    if envelope.get("class").and_then(Value::as_str) == Some("request") {
        return false;
    }
    envelope
        .get("causality")
        .and_then(|c| c.get("ref"))
        .and_then(Value::as_str)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
        .is_some_and(|candidate| candidate == task)
}
