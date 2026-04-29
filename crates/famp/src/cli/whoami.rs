//! `famp whoami [--as <name>]` — Phase 02 Plan 02-07 (CLI-07).
//!
//! Returns the effective identity for a D-10 proxy connection, plus the
//! list of channels the canonical holder is currently joined to. Always
//! opens a `Hello.bind_as` proxy connection — `whoami` without an
//! identity-bound connection is a useless echo.
//!
//! Per D-10, the broker's `Whoami` reply returns the effective identity
//! for THIS connection — which, on a proxy, is the bound name. The
//! `joined` list is the canonical holder's current channels (not the
//! proxy connection's, which can never be in any channel because Join
//! mutates the holder).
//!
//! ## Output
//!
//! ```text
//! {"active":"alice","joined":["#planning","#standup"]}
//! ```

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply};

use crate::bus_client::{resolve_sock_path, BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

/// CLI args for `famp whoami`.
#[derive(clap::Args, Debug)]
pub struct WhoamiArgs {
    /// Override identity (D-01); resolved value feeds into `Hello.bind_as`
    /// on the proxy connection (D-10). `--as` is the CLI surface; the Rust
    /// field is `act_as` because `as` is a reserved keyword.
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Structured outcome — same shape as [`BusReply::WhoamiOk`]. Used by the
/// MCP `famp_whoami` tool wrapper (plan 02-09).
#[derive(Debug, Clone)]
pub struct WhoamiOutcome {
    pub active: Option<String>,
    pub joined: Vec<String>,
}

/// Structured entry — opens a D-10 proxy connection, sends
/// `BusMessage::Whoami`, returns the typed outcome.
pub async fn run_at_structured(sock: &Path, args: WhoamiArgs) -> Result<WhoamiOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;

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
        .send_recv(BusMessage::Whoami {})
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    bus.shutdown().await;

    match reply {
        BusReply::WhoamiOk { active, joined } => Ok(WhoamiOutcome { active, joined }),
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Whoami: {other:?}"),
        }),
    }
}

/// Production entry — resolves the broker socket and prints a JSON-Line.
pub async fn run(args: WhoamiArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(&resolve_sock_path(), args).await?;
    println!(
        "{}",
        serde_json::json!({
            "active": outcome.active,
            "joined": outcome.joined,
        })
    );
    Ok(())
}
