//! `famp wait-reply` — task reply wait with inbox-first semantics.
//!
//! Unlike `famp await --task`, this command first scans the existing
//! inbox, including terminal messages, for a non-request envelope whose
//! `causality.ref` matches the task id. Only when no existing reply is
//! found does it park an await for future messages.

use famp_bus::{BusErrorKind, BusMessage, BusReply, MailboxName};

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{
    connect_bound, is_reply_for_task, render_stamped_envelopes, write_outcome, AwaitOutcome,
};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

#[derive(clap::Args, Debug)]
pub struct WaitReplyArgs {
    /// Task id of the task whose reply you're waiting for. Matches
    /// envelopes via `causality.ref`; does NOT match envelopes whose
    /// own task id equals this value, and does NOT match unrelated
    /// new-task posts (e.g. fresh channel tasks while you're parked).
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
            // D-17: `is_reply_for_task` reads task-matching metadata from
            // the INNER envelope; the origin travels forward through
            // `render_stamped_envelopes` so `write_outcome` renders the
            // body (and tags it) per this specific envelope's real
            // origin, not an interim default.
            if let Some(stamped) = envelopes
                .into_iter()
                .find(|stamped| is_reply_for_task(&stamped.envelope, args.task))
            {
                return Ok(AwaitOutcome {
                    envelopes: render_stamped_envelopes(vec![stamped]),
                    mailbox: Some(MailboxName::Agent(identity.clone())),
                    next_offset: None,
                    timed_out: false,
                    diagnostic: None,
                    aborted: false,
                });
            }
        }
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => return Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => return Err(CliError::BusError { kind, message }),
        // T-14-08: `other` may be `RegisterOk`/`JoinOk`/`AwaitOk`, each
        // carrying attacker-authored `StampedEnvelope` content for a
        // Gateway/Unknown-origin sender. `{:?}` on the whole reply would
        // print that payload into an error string that reaches
        // stderr/MCP tool results — use the variant name only.
        other => {
            return Err(CliError::BusClient {
                detail: format!("unexpected reply to Inbox: {}", other.variant_name()),
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
        BusReply::AwaitOk {
            envelopes,
            mailbox,
            next_offset,
        } => Ok(AwaitOutcome {
            envelopes: render_stamped_envelopes(envelopes),
            mailbox: Some(mailbox),
            next_offset: Some(next_offset),
            timed_out: false,
            diagnostic: None,
            aborted: false,
        }),
        BusReply::AwaitTimeout {} => Ok(AwaitOutcome {
            envelopes: Vec::new(),
            mailbox: None,
            next_offset: None,
            timed_out: true,
            diagnostic: Some(format!(
                "wait-reply timed out for task {} after checking the existing inbox, including terminal messages",
                args.task
            )),
            aborted: false,
        }),
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        // T-14-08: variant name only, never the payload (see the fixed
        // `unexpected reply to Inbox` arm above for the same threat).
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Await: {}", other.variant_name()),
        }),
    }
}
