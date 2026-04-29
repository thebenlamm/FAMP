//! `famp inbox list` — bus-backed listing of typed envelopes.
//!
//! Phase 02 plan 02-05 rewires this from the v0.8 file-reader shape
//! (`run_list(home, since, include_terminal, out)`) to a `BusClient`-driven
//! request/reply against the local UDS broker.
//!
//! Identity binding is connection-level per D-10: the CLI opens a
//! `BusClient` with `Hello { bind_as: Some(resolved_identity) }`, and
//! the broker reads the inbox of the bound identity, NOT the proxy
//! connection's name.
//!
//! Cursor management is intentionally CLIENT-SIDE — `inbox list` does
//! NOT advance any cursor (RESEARCH §6: "the broker does not track
//! per-session Inbox cursors — the client is authoritative"). Use
//! `famp inbox ack --offset N` to advance after consuming.
//!
//! Output framing: one JSONL line per typed envelope (raw
//! `serde_json::Value` straight from the wire), followed by a footer
//! line `{"next_offset":N}` so the user can pipe to
//! `famp inbox ack --offset $(... | tail -1 | jq .next_offset)`.

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

/// CLI args for `famp inbox list`.
#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Override starting offset (default: 0; broker treats `None` as 0).
    #[arg(long)]
    pub since: Option<u64>,
    /// Include envelopes for tasks already in a terminal state (default false per v0.8).
    #[arg(long)]
    pub include_terminal: bool,
    /// Override identity (D-01); resolved value feeds into `Hello.bind_as` (D-10).
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Structured outcome — same shape as `BusReply::InboxOk`. Used by
/// the MCP `famp_inbox` tool wrapper (plan 02-09).
pub struct ListOutcome {
    pub envelopes: Vec<Value>,
    pub next_offset: u64,
}

/// Run `famp inbox list` against the broker at `sock`.
///
/// Writes one JSONL line per envelope to `out`, followed by a
/// `{"next_offset":N}` footer. `out` is `Send` so the future composes
/// inside multi-threaded runtimes (D-clippy `future_not_send`).
pub async fn run_at(
    sock: &Path,
    args: ListArgs,
    out: &mut (dyn std::io::Write + Send),
) -> Result<(), CliError> {
    let outcome = run_at_structured(sock, args).await?;
    for env in &outcome.envelopes {
        let line = serde_json::to_string(env).map_err(|e| CliError::Io {
            path: sock.to_path_buf(),
            source: std::io::Error::other(format!("serialize envelope: {e}")),
        })?;
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: sock.to_path_buf(),
            source: e,
        })?;
    }
    writeln!(out, "{{\"next_offset\":{}}}", outcome.next_offset).map_err(|e| CliError::Io {
        path: sock.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Structured-outcome entry point — preserved for the MCP `famp_inbox` tool.
///
/// Plan 02-09 calls into this. Performs the same `Hello.bind_as` proxy
/// connect + `BusMessage::Inbox` round-trip but returns the typed
/// envelopes instead of writing JSONL.
pub async fn run_at_structured(sock: &Path, args: ListArgs) -> Result<ListOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;

    // D-10 proxy connect. The broker validates `bind_as = Some(identity)`
    // maps to a live registered holder at Hello time and rejects with
    // `HelloErr { kind: NotRegistered }` if not.
    let mut bus = BusClient::connect(sock, Some(identity.clone()))
        .await
        .map_err(|e| match e {
            BusClientError::HelloFailed {
                kind: BusErrorKind::NotRegistered,
                ..
            } => CliError::NotRegisteredHint {
                name: identity.clone(),
            },
            _ => CliError::BrokerUnreachable,
        })?;

    let reply = bus
        .send_recv(BusMessage::Inbox {
            since: args.since,
            include_terminal: Some(args.include_terminal),
        })
        .await
        .map_err(|_| CliError::BrokerUnreachable)?;

    match reply {
        BusReply::InboxOk {
            envelopes,
            next_offset,
        } => Ok(ListOutcome {
            envelopes,
            next_offset,
        }),
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::Io {
            path: sock.to_path_buf(),
            source: std::io::Error::other(format!("unexpected broker reply: {other:?}")),
        }),
    }
}

/// Top-level CLI entry — resolves the bus socket and writes JSONL to
/// stdout.
pub async fn run(args: ListArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let mut stdout = std::io::stdout();
    run_at(&sock, args, &mut stdout).await
}
