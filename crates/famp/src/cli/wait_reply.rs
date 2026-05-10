//! `famp wait-reply` — task reply wait with inbox-first semantics.
//!
//! Unlike `famp await --task`, this command first scans the existing
//! inbox, including terminal messages, for a non-request envelope whose
//! `causality.ref` matches the task id. Only when no existing reply is
//! found does it park an await for future messages.

use famp_bus::{BusErrorKind, BusMessage, BusReply};

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{connect_bound, is_reply_for_task, write_outcome, AwaitOutcome};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

#[derive(clap::Args, Debug)]
pub struct WaitReplyArgs {
    /// Task id to wait for.
    #[arg(long)]
    pub task: uuid::Uuid,
    /// Block timeout after the inbox-first check. Accepts `30s`, `5m`, `250ms`, etc.
    #[arg(long, default_value = "30s")]
    pub timeout: humantime::Duration,
    /// Override identity; resolved value feeds into `Hello.bind_as`.
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

pub async fn run(args: WaitReplyArgs) -> Result<(), CliError> {
    let outcome = run_structured(args).await?;
    write_outcome(&outcome, &mut std::io::stdout())
}

pub async fn run_structured(args: WaitReplyArgs) -> Result<AwaitOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;
    let sock = resolve_sock_path();
    let mut bus = connect_bound(&sock, &identity).await?;

    let inbox_reply = bus
        .send_recv(BusMessage::Inbox {
            since: Some(0),
            include_terminal: Some(true),
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    match inbox_reply {
        BusReply::InboxOk { envelopes, .. } => {
            if let Some(envelope) = envelopes
                .into_iter()
                .find(|envelope| is_reply_for_task(envelope, args.task))
            {
                return Ok(AwaitOutcome {
                    envelope: Some(envelope),
                    timed_out: false,
                    diagnostic: None,
                });
            }
        }
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => return Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => return Err(CliError::BusError { kind, message }),
        other => {
            return Err(CliError::BusClient {
                detail: format!("unexpected reply to Inbox: {other:?}"),
            });
        }
    }

    let timeout_ms: u64 = std::time::Duration::from(args.timeout)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let await_reply = bus
        .send_recv(BusMessage::Await {
            timeout_ms,
            task: Some(args.task),
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    match await_reply {
        BusReply::AwaitOk { envelope } => Ok(AwaitOutcome {
            envelope: Some(envelope),
            timed_out: false,
            diagnostic: None,
        }),
        BusReply::AwaitTimeout {} => Ok(AwaitOutcome {
            envelope: None,
            timed_out: true,
            diagnostic: Some(format!(
                "wait-reply timed out for task {} after checking the existing inbox, including terminal messages",
                args.task
            )),
        }),
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
