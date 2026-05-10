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
//! 4. On `BusReply::AwaitOk { envelope }` print the typed envelope as one
//!    JSONL line on stdout and exit 0.
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
//! ## Output shape (locked by D-02 + plan 02-06)
//!
//! ```json
//! {"famp":"0.5.2", "id":"...", "from":"...", ... }    // envelope JSONL on AwaitOk
//! {"timeout":true}                                     // on AwaitTimeout
//! ```
//!
//! Identity binding is at the connection level via `Hello.bind_as`
//! (D-10) — the `Await` message itself carries no identity field.

use std::io::Write;
use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

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
}

/// Structured outcome from [`run_at_structured`]. `envelope` is `None`
/// on `AwaitTimeout`, `Some(value)` on `AwaitOk`. `timed_out` is the
/// orthogonal flag the MCP tool surfaces in the JSON-RPC result.
#[derive(Debug, Clone)]
pub struct AwaitOutcome {
    /// The typed envelope returned by the broker, or `None` on timeout.
    pub envelope: Option<serde_json::Value>,
    /// `true` when the broker returned `BusReply::AwaitTimeout {}`.
    pub timed_out: bool,
    /// Optional human-readable diagnostic printed with timeout JSON.
    pub diagnostic: Option<String>,
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
    write_outcome(&outcome, &mut std::io::stdout())
}

/// Render an [`AwaitOutcome`] to a writer as a single JSONL line.
///
/// Split out from [`run_at`] so callers (`run`, integration tests) can
/// drive the write side independently of the bus round-trip.
pub(crate) fn write_outcome(outcome: &AwaitOutcome, mut out: impl Write) -> Result<(), CliError> {
    if outcome.timed_out {
        let mut value = serde_json::json!({"timeout": true});
        if let Some(diagnostic) = outcome.diagnostic.as_deref() {
            value["diagnostic"] = serde_json::json!(diagnostic);
        }
        writeln!(out, "{value}").map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: e,
        })?;
    } else if let Some(env) = outcome.envelope.as_ref() {
        let line = serde_json::to_string(env).map_err(|e| CliError::Io {
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
    write_outcome(&outcome, out)
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

    // 3. Open the bus connection with the D-10 proxy binding. The
    //    broker validates at Hello time that `identity` is held by a
    //    live `famp register` process; refusal surfaces as
    //    HelloFailed { NotRegistered }.
    let mut bus = connect_bound(sock, &identity).await?;

    // 4. Single round-trip: Await { timeout_ms, task }.
    let reply = bus
        .send_recv(BusMessage::Await {
            timeout_ms,
            task: args.task,
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    // 5. Map the four expected reply variants. `BusErrorKind` is closed
    //    so the `_ =>` wildcard arm is on `BusReply` variants we never
    //    expect in response to an Await op (e.g. `SendOk`); per
    //    plan acceptance this is a `BusClient { source: ... }` error,
    //    NOT a `BusError`-with-kind, because there is no kind to carry.
    match reply {
        BusReply::AwaitOk { envelope } => Ok(AwaitOutcome {
            envelope: Some(envelope),
            timed_out: false,
            diagnostic: None,
        }),
        BusReply::AwaitTimeout {} => {
            let diagnostic = timeout_diagnostic(&mut bus, &identity, args.task).await;
            Ok(AwaitOutcome {
                envelope: None,
                timed_out: true,
                diagnostic: Some(diagnostic),
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

pub(crate) async fn connect_bound(sock: &Path, identity: &str) -> Result<BusClient, CliError> {
    BusClient::connect(sock, Some(identity.to_string()))
        .await
        .map_err(|e| match e {
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
            BusClientError::Frame(_) | BusClientError::Decode(_) => CliError::BusClient {
                detail: mixed_binary_hint(format!("{e:?}")),
            },
            _ => CliError::BrokerUnreachable,
        })
}

fn mixed_binary_hint(detail: impl AsRef<str>) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown current executable)".to_string());
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
    let base = match task {
        Some(task) => format!("await timed out waiting for new messages on task {task}"),
        None => "await timed out waiting for a new message".to_string(),
    };
    let hint = format!(
        "Run `famp inbox list --as {identity} --include-terminal` to inspect already-delivered messages, or use `famp wait-reply --as {identity} --task <task_id>` for reply waits that check the inbox before blocking."
    );

    if let Some(task) = task {
        if let Ok(true) = inbox_has_reply_for_task(bus, task).await {
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
