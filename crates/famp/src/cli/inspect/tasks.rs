//! `famp inspect tasks` -- task FSM visibility over the v0.10 inspector RPC.

use clap::Args;

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

pub async fn run(_args: InspectTasksArgs) -> Result<(), CliError> {
    todo!("implemented in GREEN phase")
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
        let err = render_reply(&InspectTasksReply::BudgetExceeded { elapsed_ms: 500 }, false)
            .expect_err("budget must fail");
        assert!(matches!(err, CliError::Exit(1)));
    }

    fn render_list(_list: &TaskListReply, _orphans_only: bool) -> String {
        todo!("implemented in GREEN phase")
    }

    fn render_reply(_reply: &InspectTasksReply, _orphans_only: bool) -> Result<String, CliError> {
        todo!("implemented in GREEN phase")
    }
}
