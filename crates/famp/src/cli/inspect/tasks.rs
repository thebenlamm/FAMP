//! `famp inspect tasks` -- task FSM visibility over the v0.10 inspector RPC.

use clap::Args;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{
    InspectKind, InspectTasksReply, InspectTasksRequest, TaskDetailFullReply, TaskDetailReply,
    TaskListReply, TaskRow,
};
use std::fmt::Write as _;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectTasksArgs {
    /// Filter to a specific task_id.
    #[arg(long)]
    pub id: Option<uuid::Uuid>,
    /// Emit each envelope in canonical JCS form. Requires `--id`.
    #[arg(long)]
    pub full: bool,
    /// Show only orphan tasks.
    #[arg(long)]
    pub orphans: bool,
    /// Emit JSON output instead of a fixed-width table.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: InspectTasksArgs) -> Result<(), CliError> {
    if args.full && args.id.is_none() {
        eprintln!("error: --full requires --id <task_id>");
        return Err(CliError::Exit(2));
    }

    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();

    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&sock).await else {
        eprintln!("error: broker not running at {sock_str}");
        return Err(CliError::Exit(1));
    };

    let payload = call(
        &mut stream,
        InspectKind::Tasks(InspectTasksRequest {
            id: args.id,
            full: args.full,
        }),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect tasks call failed: {e}")))?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| CliError::Generic(format!("json render: {e}")))?
        );
        return Ok(());
    }

    let reply: InspectTasksReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("tasks reply schema mismatch: {e}")))?;
    let rendered = render_reply(&reply, args.orphans)?;
    if !rendered.is_empty() {
        println!("{rendered}");
    }
    Ok(())
}

fn render_reply(reply: &InspectTasksReply, orphans_only: bool) -> Result<String, CliError> {
    match reply {
        InspectTasksReply::List(list) => Ok(render_list(list, orphans_only)),
        InspectTasksReply::Detail(detail) => Ok(render_detail(detail)),
        InspectTasksReply::DetailFull(full) => render_detail_full(full),
        InspectTasksReply::BudgetExceeded { elapsed_ms } => {
            eprintln!("error: inspect timed out after {elapsed_ms}ms");
            Err(CliError::Exit(1))
        }
    }
}

fn render_list(list: &TaskListReply, orphans_only: bool) -> String {
    const HEADERS: [&str; 8] = [
        "TASK_ID",
        "STATE",
        "PEER",
        "OPENED",
        "LAST_TRANSITION_AGE",
        "ENVELOPES",
        "TERMINAL",
        "ORPHAN",
    ];

    let rows: Vec<&TaskRow> = if orphans_only {
        list.rows.iter().filter(|row| row.orphan).collect()
    } else {
        list.rows.iter().collect()
    };

    let mut widths: [usize; 8] = HEADERS.map(str::len);
    let formatted: Vec<[String; 8]> = rows
        .iter()
        .map(|row| {
            [
                row.task_id.clone(),
                row.state.clone(),
                row.peer.clone(),
                format_unix(row.opened_at_unix_seconds),
                format!("{}s", row.last_transition_age_seconds),
                row.envelope_count.to_string(),
                row.terminal.to_string(),
                row.orphan.to_string(),
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

fn render_detail(detail: &TaskDetailReply) -> String {
    const HEADERS: [&str; 6] = [
        "ENVELOPE_ID",
        "SENDER",
        "RECIPIENT",
        "FSM_TRANSITION",
        "TIMESTAMP",
        "SIG_VERIFIED",
    ];
    let mut widths: [usize; 6] = HEADERS.map(str::len);
    let formatted: Vec<[String; 6]> = detail
        .envelopes
        .iter()
        .map(|envelope| {
            [
                envelope.envelope_id.clone(),
                envelope.sender.clone(),
                envelope.recipient.clone(),
                envelope.fsm_transition.clone(),
                envelope.timestamp.clone(),
                envelope.sig_verified.to_string(),
            ]
        })
        .collect();
    for row in &formatted {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = format!("task_id: {}\n\n", detail.task_id);
    out.push_str(&format_row(&HEADERS.map(String::from), &widths));
    for row in &formatted {
        out.push('\n');
        out.push_str(&format_row(row, &widths));
    }
    out
}

fn render_detail_full(full: &TaskDetailFullReply) -> Result<String, CliError> {
    serde_json::to_string_pretty(full)
        .map_err(|e| CliError::Generic(format!("full detail json render: {e}")))
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

fn format_unix(secs: u64) -> String {
    let Ok(secs_i64) = i64::try_from(secs) else {
        return secs.to_string();
    };
    time::OffsetDateTime::from_unix_timestamp(secs_i64).map_or_else(
        |_| secs.to_string(),
        |time| {
            time.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| secs.to_string())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use famp_inspect_proto::{InspectTasksReply, TaskListReply, TaskRow};

    fn row(task_id: &str, state: &str, orphan: bool) -> TaskRow {
        TaskRow {
            task_id: task_id.into(),
            state: state.into(),
            peer: "agent:local.bus/peer".into(),
            opened_at_unix_seconds: 1_700_000_000,
            last_send_at_unix_seconds: None,
            last_recv_at_unix_seconds: None,
            terminal: false,
            envelope_count: 1,
            last_transition_age_seconds: 5,
            orphan,
        }
    }

    #[test]
    fn list_table_has_required_headers_and_two_rows() {
        let list = TaskListReply {
            rows: vec![
                row("019d9ba2-2d30-7ae2-ba77-9e55863ac7f7", "COMMITTED", false),
                row("00000000-0000-0000-0000-000000000000", "REQUESTED", true),
            ],
        };

        let rendered = render_list(&list, false);
        assert!(rendered.contains("TASK_ID"));
        assert!(rendered.contains("STATE"));
        assert!(rendered.contains("PEER"));
        assert!(rendered.contains("ENVELOPES"));
        assert!(rendered.contains("LAST_TRANSITION_AGE"));
        assert!(rendered.contains("ORPHAN"));
        assert_eq!(rendered.lines().count(), 3);
    }

    #[test]
    fn budget_exceeded_maps_to_exit_one() {
        let err = render_reply(
            &InspectTasksReply::BudgetExceeded { elapsed_ms: 500 },
            false,
        )
        .expect_err("budget must fail");
        assert!(matches!(err, CliError::Exit(1)));
    }

    #[test]
    fn orphan_column_value_renders_true_for_orphan_row() {
        // An orphan row (nil UUID) must render "true" in the ORPHAN column,
        // not just expose the ORPHAN header. Checks render_list produces the
        // literal string "true" in addition to the column header.
        let list = TaskListReply {
            rows: vec![row(
                "00000000-0000-0000-0000-000000000000",
                "REQUESTED",
                true,
            )],
        };
        let rendered = render_list(&list, false);
        // The ORPHAN column header must be present.
        assert!(
            rendered.contains("ORPHAN"),
            "ORPHAN header missing: {rendered}"
        );
        // The boolean value "true" must appear as a cell in the data row.
        // Split off the header line so we only search data rows.
        let data_rows: Vec<&str> = rendered.lines().skip(1).collect();
        assert!(
            !data_rows.is_empty(),
            "expected at least one data row: {rendered}"
        );
        let data_section = data_rows.join("\n");
        assert!(
            data_section.contains("true"),
            "orphan column value 'true' missing from data rows: {rendered}"
        );
    }

    #[test]
    fn orphans_only_filter_excludes_non_orphan_rows() {
        // render_list(&list, true) must show header + 1 orphan row only.
        // render_list(&list, false) must show header + both rows.
        let list = TaskListReply {
            rows: vec![
                row("019d9ba2-2d30-7ae2-ba77-9e55863ac7f7", "COMMITTED", false),
                row("00000000-0000-0000-0000-000000000000", "REQUESTED", true),
            ],
        };

        let unfiltered = render_list(&list, false);
        let filtered = render_list(&list, true);

        // Unfiltered: header + 2 data rows = 3 lines.
        assert_eq!(
            unfiltered.lines().count(),
            3,
            "unfiltered should have header + 2 rows: {unfiltered}"
        );

        // Filtered (orphans_only=true): header + 1 orphan row = 2 lines.
        assert_eq!(
            filtered.lines().count(),
            2,
            "orphans_only filter should leave header + 1 row: {filtered}"
        );

        // The orphan task ID must appear in the filtered output.
        assert!(
            filtered.contains("00000000-0000-0000-0000-000000000000"),
            "orphan task ID missing from filtered output: {filtered}"
        );

        // The non-orphan task ID must NOT appear in the filtered output.
        assert!(
            !filtered.contains("019d9ba2-2d30-7ae2-ba77-9e55863ac7f7"),
            "non-orphan task ID should be excluded by filter: {filtered}"
        );
    }
}
