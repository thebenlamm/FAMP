//! `famp inspect messages` -- envelope metadata visibility over inspector RPC.

use clap::Args;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{
    InspectKind, InspectMessagesReply, InspectMessagesRequest, MessageListReply,
};
use std::fmt::Write as _;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectMessagesArgs {
    /// Filter to messages addressed to this identity.
    #[arg(long)]
    pub to: Option<String>,
    /// Limit to N most-recent envelopes.
    #[arg(long)]
    pub tail: Option<u64>,
    /// Emit JSON output instead of a fixed-width table.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: InspectMessagesArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();

    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&sock).await else {
        eprintln!("error: broker not running at {sock_str}");
        return Err(CliError::Exit(1));
    };

    let payload = call(
        &mut stream,
        InspectKind::Messages(InspectMessagesRequest {
            to: args.to.clone(),
            tail: args.tail,
        }),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect messages call failed: {e}")))?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| CliError::Generic(format!("json render: {e}")))?
        );
        return Ok(());
    }

    let reply: InspectMessagesReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("messages reply schema mismatch: {e}")))?;
    let rendered = render_reply(&reply)?;
    if !rendered.is_empty() {
        println!("{rendered}");
    }
    Ok(())
}

fn render_reply(reply: &InspectMessagesReply) -> Result<String, CliError> {
    match reply {
        InspectMessagesReply::List(list) => Ok(render_list(list)),
        InspectMessagesReply::BudgetExceeded { elapsed_ms } => {
            eprintln!("error: inspect timed out after {elapsed_ms}ms");
            Err(CliError::Exit(1))
        }
    }
}

fn render_list(list: &MessageListReply) -> String {
    const HEADERS: [&str; 8] = [
        "FROM",
        "TO",
        "TASK_ID",
        "CLASS",
        "STATE",
        "TIMESTAMP",
        "BODY_BYTES",
        "SHA256_PREFIX",
    ];

    let mut widths: [usize; 8] = HEADERS.map(str::len);
    let formatted: Vec<[String; 8]> = list
        .rows
        .iter()
        .map(|row| {
            [
                row.sender.clone(),
                row.recipient.clone(),
                row.task_id.clone(),
                row.class.clone(),
                row.state.clone(),
                row.timestamp.clone(),
                row.body_bytes.to_string(),
                row.body_sha256_prefix.clone(),
            ]
        })
        .collect();
    for row in &formatted {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = String::new();
    out.push_str(&format_row(&HEADERS.map(String::from), &widths));
    for row in &formatted {
        out.push('\n');
        out.push_str(&format_row(row, &widths));
    }
    out
}

fn format_row<const N: usize>(cells: &[String; N], widths: &[usize; N]) -> String {
    let mut out = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        let _ = write!(&mut out, "{cell:width$}", width = widths[i]);
    }
    out.trim_end().to_string()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use famp_inspect_proto::{InspectMessagesReply, MessageListReply, MessageRow};

    fn message() -> MessageRow {
        MessageRow {
            sender: "agent:local.bus/alice".into(),
            recipient: "agent:local.bus/bob".into(),
            task_id: "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7".into(),
            class: "deliver".into(),
            state: "COMMITTED".into(),
            timestamp: "2026-05-10T18:00:00Z".into(),
            body_bytes: 42,
            body_sha256_prefix: "a1b2c3d4e5f6".into(),
        }
    }

    #[test]
    fn message_table_has_metadata_headers_and_no_body_column() {
        let rendered = render_list(&MessageListReply {
            rows: vec![message()],
        });
        let header = rendered.lines().next().unwrap_or_default();
        assert!(header.contains("BODY_BYTES"));
        assert!(header.contains("SHA256_PREFIX"));
        assert!(!header.split_whitespace().any(|column| column == "BODY"));
    }

    #[test]
    fn empty_message_list_prints_only_header() {
        let rendered = render_list(&MessageListReply { rows: vec![] });
        assert_eq!(rendered.lines().count(), 1);
    }

    #[test]
    fn budget_exceeded_maps_to_exit_one() {
        let err = render_reply(&InspectMessagesReply::BudgetExceeded { elapsed_ms: 500 })
            .expect_err("budget must fail");
        assert!(matches!(err, CliError::Exit(1)));
    }
}
