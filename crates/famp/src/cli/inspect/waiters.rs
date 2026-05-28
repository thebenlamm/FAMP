//! `famp inspect waiters` -- clients currently parked in `famp_await`.
//!
//! Shows each parked await fan-out as (name, mailbox, cursor, `deadline_ms`).
//! A single awaiting client yields one row for its agent mailbox plus one
//! row per joined channel it subscribes to.
//!
//! Works without registration (read-only, same as `famp inspect broker`).

use clap::Args;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectKind, InspectWaitersReply, InspectWaitersRequest, WaiterRow};
use std::fmt::Write as _;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectWaitersArgs {
    /// Emit JSON output instead of a fixed-width table.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: InspectWaitersArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();

    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&sock).await else {
        eprintln!("error: broker not running at {sock_str}");
        return Err(CliError::Exit(1));
    };

    let payload = call(
        &mut stream,
        InspectKind::Waiters(InspectWaitersRequest::default()),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect waiters call failed: {e}")))?;
    let reply: InspectWaitersReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("waiters reply schema mismatch: {e}")))?;

    if args.json {
        let s = serde_json::to_string_pretty(&reply)
            .map_err(|e| CliError::Generic(format!("json serialize: {e}")))?;
        println!("{s}");
    } else {
        print_table(&reply.rows);
    }
    Ok(())
}

fn print_table(rows: &[WaiterRow]) {
    const HEADERS: [&str; 4] = ["NAME", "MAILBOX", "CURSOR", "DEADLINE_MS"];

    let mut widths: [usize; 4] = HEADERS.map(str::len);
    let formatted: Vec<[String; 4]> = rows
        .iter()
        .map(|r| {
            [
                r.name.clone(),
                r.mailbox.clone(),
                r.cursor.to_string(),
                r.deadline_ms.to_string(),
            ]
        })
        .collect();

    for row in &formatted {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    println!("{}", format_row(&HEADERS.map(String::from), &widths));
    if rows.is_empty() {
        println!("(no active waiters)");
    } else {
        for row in &formatted {
            println!("{}", format_row(row, &widths));
        }
    }
}

fn format_row(cells: &[String; 4], widths: &[usize; 4]) -> String {
    let mut out = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        let _ = write!(&mut out, "{cell:width$}", width = widths[i]);
    }
    out.trim_end().to_string()
}
