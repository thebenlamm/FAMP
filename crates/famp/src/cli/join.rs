//! `famp join <#channel> [--as <name>]` — Phase 02 Plan 02-07 (CLI-06).
//!
//! Joins a channel via the local UDS broker. Identity binding follows
//! D-10: connection-level via `Hello { bind_as: Some(resolved_identity) }`,
//! NOT a per-message field. The broker mutates the canonical live
//! registered holder's `joined` set (NOT the proxy connection's), so the
//! one-shot `famp join --as alice #planning` process can exit and `alice`
//! stays in `#planning` until the canonical `famp register alice` holder
//! exits or explicitly leaves.
//!
//! ## Output
//!
//! ```text
//! {"channel":"#planning","members":["alice","bob"],"drained":3}
//! ```
//!
//! `drained` is the *count* of typed envelopes drained on join — the full
//! envelopes are available structurally via [`run_at_structured`] for the
//! MCP `famp_join` tool wrapper (plan 02-09). The wire shape on
//! [`BusReply::JoinOk`] carries `drained: Vec<serde_json::Value>` (typed
//! envelopes per Phase-1 D-09); the CLI surfaces only the count for
//! ergonomics.

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply};

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;
use crate::cli::util::normalize_channel;

/// CLI args for `famp join`.
#[derive(clap::Args, Debug)]
pub struct JoinArgs {
    /// Channel name (with or without leading `#`).
    pub channel: String,
    /// Override identity (D-01); resolved value feeds into `Hello.bind_as`
    /// on the proxy connection (D-10). `--as` is the CLI surface; the Rust
    /// field is `act_as` because `as` is a reserved keyword.
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Structured outcome — same shape as [`BusReply::JoinOk`]. Used by the
/// MCP `famp_join` tool wrapper (plan 02-09) to surface the drained
/// envelopes verbatim instead of just their count.
#[derive(Debug, Clone)]
pub struct JoinOutcome {
    pub channel: String,
    pub members: Vec<String>,
    /// Typed envelopes drained on join (Phase-1 D-09 wire shape). The
    /// stdout JSONL form surfaces only the length; structured callers see
    /// the full envelopes.
    pub drained: Vec<serde_json::Value>,
}

/// Structured entry — opens a D-10 proxy connection, sends
/// `BusMessage::Join`, returns the typed outcome. Used by both the CLI
/// `run` wrapper (plan 02-07) and the MCP `famp_join` tool (plan 02-09).
pub async fn run_at_structured(sock: &Path, args: JoinArgs) -> Result<JoinOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;
    let channel = normalize_channel(&args.channel)?;

    // D-10 proxy connect. The broker validates `bind_as = Some(identity)`
    // maps to a live registered holder at Hello time. On Join, the broker
    // mutates the canonical holder's `joined` set, NOT this connection's,
    // so the one-shot CLI process exiting does NOT auto-leave the channel
    // (plan 02-02 broker logic; plan 02-11 verifies at integration level).
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
        .send_recv(BusMessage::Join {
            channel: channel.clone(),
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    // Best-effort shutdown so the broker observes Disconnect promptly.
    bus.shutdown().await;

    match reply {
        BusReply::JoinOk {
            channel: c,
            members,
            drained,
        } => Ok(JoinOutcome {
            channel: c,
            members,
            drained,
        }),
        // Per-op liveness re-check failed (the holder died between Hello
        // and Join). Same operator hint as the Hello-time refusal.
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Join: {other:?}"),
        }),
    }
}

/// Production entry — resolves the broker socket via
/// [`resolve_sock_path`] and prints a JSON-Line on success.
pub async fn run(args: JoinArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(&resolve_sock_path(), args).await?;
    let line = serde_json::json!({
        "channel": outcome.channel,
        "members": outcome.members,
        "drained": outcome.drained.len(),
    });
    println!("{line}");
    Ok(())
}
