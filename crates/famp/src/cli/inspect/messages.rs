//! `famp inspect messages` -- envelope metadata visibility over inspector RPC.

use clap::Args;

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

pub async fn run(_args: InspectMessagesArgs) -> Result<(), CliError> {
    todo!("implemented in GREEN phase")
}

#[cfg(test)]
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

    fn render_list(_list: &MessageListReply) -> String {
        todo!("implemented in GREEN phase")
    }

    fn render_reply(_reply: &InspectMessagesReply) -> Result<String, CliError> {
        todo!("implemented in GREEN phase")
    }
}
