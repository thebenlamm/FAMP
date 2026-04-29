//! `famp leave <#channel> [--as <name>]` — Phase 02 Plan 02-07 (CLI-06).
//!
//! Leaves a channel via the local UDS broker. Identity binding follows
//! D-10: connection-level via `Hello { bind_as: Some(resolved_identity) }`.
//! Like Join, the broker mutates the canonical live registered holder's
//! `joined` set on Leave (NOT the proxy connection's), so a one-shot
//! `famp leave --as alice #planning` durably removes alice from the
//! channel until she rejoins.
//!
//! ## Output
//!
//! ```text
//! {"channel":"#planning"}
//! ```

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply};

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;
use crate::cli::util::normalize_channel;

/// CLI args for `famp leave`.
#[derive(clap::Args, Debug)]
pub struct LeaveArgs {
    /// Channel name (with or without leading `#`).
    pub channel: String,
    /// Override identity (D-01); resolved value feeds into `Hello.bind_as`
    /// on the proxy connection (D-10). `--as` is the CLI surface; the Rust
    /// field is `act_as` because `as` is a reserved keyword.
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Structured outcome — mirrors [`BusReply::LeaveOk`]. Used by the MCP
/// `famp_leave` tool wrapper (plan 02-09).
#[derive(Debug, Clone)]
pub struct LeaveOutcome {
    pub channel: String,
}

/// Structured entry — opens a D-10 proxy connection, sends
/// `BusMessage::Leave`, returns the typed outcome.
pub async fn run_at_structured(sock: &Path, args: LeaveArgs) -> Result<LeaveOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;
    let channel = normalize_channel(&args.channel)?;

    // D-10 proxy connect. Broker validates the canonical holder is live;
    // mutation lands on the holder, not on this proxy connection.
    let mut bus = BusClient::connect(sock, Some(identity.clone()))
        .await
        .map_err(|e| match &e {
            BusClientError::HelloFailed {
                kind: BusErrorKind::NotRegistered,
                ..
            } => CliError::NotRegisteredHint {
                name: identity.clone(),
            },
            BusClientError::Io(_) | BusClientError::BrokerDidNotStart(_) => {
                CliError::BrokerUnreachable
            }
            _ => CliError::BusClient {
                detail: format!("{e:?}"),
            },
        })?;

    let reply = bus
        .send_recv(BusMessage::Leave {
            channel: channel.clone(),
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    bus.shutdown().await;

    match reply {
        BusReply::LeaveOk { channel: c } => Ok(LeaveOutcome { channel: c }),
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Leave: {other:?}"),
        }),
    }
}

/// Production entry — resolves the broker socket and prints a JSON-Line.
pub async fn run(args: LeaveArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(&resolve_sock_path(), args).await?;
    println!("{}", serde_json::json!({"channel": outcome.channel}));
    Ok(())
}
