//! `famp sessions [--me]` — Phase 02 Plan 02-07 (CLI-08, CLI-11).
//!
//! Read-only listing of currently registered sessions held by live
//! `famp register <name>` processes. Reads from broker memory (NOT the
//! diagnostic `sessions.jsonl`) so the answer is always the canonical
//! truth.
//!
//! ## Identity
//!
//! Sessions is read-only and uses the `--me` flag (NOT `--as`) per
//! RESEARCH §6 (CLI table). When `--me` is set, the CLI:
//!
//! 1. Resolves identity through D-01.
//! 2. Opens a D-10 proxy connection with `Hello { bind_as: Some(name) }`
//!    so the broker validates liveness at Hello time.
//! 3. Filters [`BusReply::SessionsOk`] rows to just `row.name == name`.
//!
//! Without `--me`, the connection is unbound (`bind_as: None`) — an
//! observer-only proxy that does not attempt liveness validation.
//!
//! ## Output
//!
//! One JSONL line per [`SessionRow`]:
//!
//! ```text
//! {"name":"alice","pid":12345,"joined":["#planning","#standup"]}
//! {"name":"bob","pid":12346,"joined":[]}
//! ```

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply, SessionRow};

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

/// CLI args for `famp sessions`.
#[derive(clap::Args, Debug)]
pub struct SessionsArgs {
    /// Filter to the caller's resolved identity only. Mutually exclusive
    /// with `--as` semantics (sessions has no `--as`); resolves identity
    /// through D-01 and uses `Hello.bind_as` proxy validation.
    #[arg(long)]
    pub me: bool,
}

/// Structured outcome — the broker's full session table (or filtered
/// subset when `--me` is set). Used by the MCP `famp_sessions` tool
/// wrapper (plan 02-09).
#[derive(Debug, Clone)]
pub struct SessionsOutcome {
    pub rows: Vec<SessionRow>,
}

/// Structured entry — returns the (optionally filtered) `Vec<SessionRow>`
/// without printing.
pub async fn run_at_structured(
    sock: &Path,
    args: &SessionsArgs,
) -> Result<SessionsOutcome, CliError> {
    // If --me is set, resolve identity and use Hello.bind_as proxy so the
    // broker validates liveness. Without --me, connect with bind_as: None
    // — the connection becomes an unbound observer for the read-only op.
    let bind_as = if args.me {
        Some(resolve_identity(None)?)
    } else {
        None
    };

    let bind_as_for_err = bind_as.clone();
    let mut bus = BusClient::connect(sock, bind_as.clone())
        .await
        .map_err(|e| match &e {
            BusClientError::HelloFailed {
                kind: BusErrorKind::NotRegistered,
                ..
            } if bind_as_for_err.is_some() => CliError::NotRegisteredHint {
                name: bind_as_for_err.clone().unwrap_or_default(),
            },
            BusClientError::Io(_) | BusClientError::BrokerDidNotStart(_) => {
                CliError::BrokerUnreachable
            }
            _ => CliError::BusClient {
                detail: format!("{e:?}"),
            },
        })?;

    let reply = bus
        .send_recv(BusMessage::Sessions {})
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    bus.shutdown().await;

    match reply {
        BusReply::SessionsOk { rows } => {
            let filtered = match bind_as.as_deref() {
                Some(name) => rows.into_iter().filter(|r| r.name == name).collect(),
                None => rows,
            };
            Ok(SessionsOutcome { rows: filtered })
        }
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint {
            name: bind_as.unwrap_or_default(),
        }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Sessions: {other:?}"),
        }),
    }
}

/// Test-facing entry — accepts an explicit broker socket and writer.
pub async fn run_at(
    sock: &Path,
    args: SessionsArgs,
    out: &mut (dyn std::io::Write + Send),
) -> Result<(), CliError> {
    let outcome = run_at_structured(sock, &args).await?;
    for row in &outcome.rows {
        let line = serde_json::to_string(row).map_err(|e| CliError::Io {
            path: sock.to_path_buf(),
            source: std::io::Error::other(format!("serialize SessionRow: {e}")),
        })?;
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: sock.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

/// Production entry — resolves the broker socket and writes JSONL to stdout.
pub async fn run(args: SessionsArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let mut stdout = std::io::stdout();
    run_at(&sock, args, &mut stdout).await
}
