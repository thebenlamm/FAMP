//! `famp inspect identities` -- registered session identities.
//!
//! INSP-IDENT-01: name, listen-mode, registered-at, last-activity, cwd.
//! INSP-IDENT-02: mailbox unread, total, last-sender, last-received-at.
//! INSP-IDENT-03: no `surfaced` / `double_print` / `received_count` fields.
//! INSP-CLI-02: --json.
//! INSP-CLI-03: column-aligned table with explicit headers.
//! INSP-CLI-04: dead-broker fast-fail with empty stdout.

use clap::Args;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{
    IdentityRow, InspectIdentitiesReply, InspectIdentitiesRequest, InspectKind,
};
use std::fmt::Write as _;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectIdentitiesArgs {
    /// Emit JSON output instead of a fixed-width table.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: InspectIdentitiesArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();

    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&sock).await else {
        eprintln!("error: broker not running at {sock_str}");
        return Err(CliError::Exit(1));
    };

    let payload = call(
        &mut stream,
        InspectKind::Identities(InspectIdentitiesRequest::default()),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect identities call failed: {e}")))?;
    let reply: InspectIdentitiesReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("identities reply schema mismatch: {e}")))?;

    if args.json {
        let s = serde_json::to_string_pretty(&reply)
            .map_err(|e| CliError::Generic(format!("json serialize: {e}")))?;
        println!("{s}");
    } else {
        print_table(&reply.rows);
    }
    Ok(())
}

fn print_table(rows: &[IdentityRow]) {
    // Header order: NAME LISTEN CWD REGISTERED UNREAD TOTAL LAST_SENDER LAST_RECEIVED.
    const HEADERS: [&str; 8] = [
        "NAME",
        "LISTEN",
        "CWD",
        "REGISTERED",
        "UNREAD",
        "TOTAL",
        "LAST_SENDER",
        "LAST_RECEIVED",
    ];

    let mut widths: [usize; 8] = HEADERS.map(str::len);
    let formatted: Vec<[String; 8]> = rows
        .iter()
        .map(|r| {
            [
                r.name.clone(),
                if r.listen_mode { "true" } else { "false" }.to_string(),
                r.cwd.clone().unwrap_or_else(|| "-".to_string()),
                format_unix(r.registered_at_unix_seconds),
                r.mailbox_unread.to_string(),
                r.mailbox_total.to_string(),
                r.last_sender.clone(),
                r.last_received_at_unix_seconds
                    .map_or_else(|| "-".to_string(), format_unix),
            ]
        })
        .collect();

    for row in &formatted {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    println!("{}", format_row(&HEADERS.map(String::from), &widths));
    for row in &formatted {
        println!("{}", format_row(row, &widths));
    }
}

fn format_row(cells: &[String; 8], widths: &[usize; 8]) -> String {
    let mut out = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        let _ = write!(&mut out, "{cell:width$}", width = widths[i]);
    }
    out.trim_end().to_string()
}

fn format_unix(secs: u64) -> String {
    let Ok(secs_i64) = i64::try_from(secs) else {
        return secs.to_string();
    };
    time::OffsetDateTime::from_unix_timestamp(secs_i64).map_or_else(
        |_| secs.to_string(),
        |t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| secs.to_string())
        },
    )
}
