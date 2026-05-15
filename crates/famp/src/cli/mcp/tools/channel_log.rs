//! `famp_channel_log` MCP tool.
//!
//! Reads a channel mailbox directly from disk, without requiring MCP
//! registration. This is a recovery path for channel messages that arrived
//! while an agent was busy composing and therefore were not observed through
//! `famp_await`.

use std::path::Path;

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::{bus_dir, resolve_sock_path};
use crate::cli::mcp::tools::ToolError;
use crate::cli::util::normalize_channel;

const DEFAULT_LIMIT: u64 = 50;

/// Dispatch a `famp_channel_log` tool call.
#[allow(clippy::unused_async)]
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let sock = resolve_sock_path();
    call_at_bus_dir(input, bus_dir(&sock))
}

fn call_at_bus_dir(input: &Value, bus_dir: &Path) -> Result<Value, ToolError> {
    let channel_raw = input
        .get("channel")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: channel (string)",
            )
        })?;
    let channel = normalize_channel(channel_raw)
        .map_err(|e| ToolError::new(BusErrorKind::EnvelopeInvalid, e.to_string()))?;
    let since = optional_u64(input, "since")?.unwrap_or(0);
    let limit = optional_u64(input, "limit")?.unwrap_or(DEFAULT_LIMIT);

    let path = bus_dir.join("mailboxes").join(format!("{channel}.jsonl"));
    read_channel_mailbox(&channel, &path, since, limit)
}

fn optional_u64(input: &Value, field: &str) -> Result<Option<u64>, ToolError> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                format!("field {field} must be a non-negative integer"),
            )
        }),
        Some(_) => Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            format!("field {field} must be a non-negative integer"),
        )),
    }
}

fn read_channel_mailbox(
    channel: &str,
    path: &Path,
    since: u64,
    limit: u64,
) -> Result<Value, ToolError> {
    let entries = famp_inbox::read::read_from(path, since).map_err(|e| {
        ToolError::new(
            BusErrorKind::Internal,
            format!("cannot read channel mailbox {}: {e}", path.display()),
        )
    })?;
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut next_offset = since;
    let mut envelopes = Vec::new();

    for (envelope, end_offset) in entries.into_iter().take(take) {
        next_offset = end_offset;
        envelopes.push(envelope);
    }

    Ok(serde_json::json!({
        "channel": channel,
        "envelopes": envelopes,
        "next_offset": next_offset,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn write_mailbox(dir: &Path, channel: &str, envelopes: &[Value]) -> (u64, u64, u64) {
        let mailboxes = dir.join("mailboxes");
        std::fs::create_dir_all(&mailboxes).unwrap();
        let path = mailboxes.join(format!("{channel}.jsonl"));
        let mut file = File::create(path).unwrap();
        let mut offsets = Vec::new();
        let mut running = 0_u64;
        for envelope in envelopes {
            let line = serde_json::to_string(envelope).unwrap();
            writeln!(file, "{line}").unwrap();
            running += u64::try_from(line.len() + 1).unwrap();
            offsets.push(running);
        }
        (offsets[0], offsets[1], offsets[2])
    }

    #[test]
    fn reads_normalized_channel_with_limit_and_next_offset() {
        let tmp = tempfile::TempDir::new().unwrap();
        let envelopes = vec![
            serde_json::json!({ "id": "one", "to": "chan:local.bus/#planning" }),
            serde_json::json!({ "id": "two", "to": "chan:local.bus/#planning" }),
            serde_json::json!({ "id": "three", "to": "chan:local.bus/#planning" }),
        ];
        let (_first_offset, second_offset, _third_offset) =
            write_mailbox(tmp.path(), "#planning", &envelopes);

        let out = call_at_bus_dir(
            &serde_json::json!({ "channel": "planning", "limit": 2 }),
            tmp.path(),
        )
        .unwrap();

        assert_eq!(out["channel"], "#planning");
        assert_eq!(out["envelopes"].as_array().unwrap().len(), 2);
        assert_eq!(out["envelopes"][0]["id"], "one");
        assert_eq!(out["envelopes"][1]["id"], "two");
        assert_eq!(out["next_offset"], second_offset);
    }

    #[test]
    fn since_offset_pages_forward() {
        let tmp = tempfile::TempDir::new().unwrap();
        let envelopes = vec![
            serde_json::json!({ "id": "one" }),
            serde_json::json!({ "id": "two" }),
            serde_json::json!({ "id": "three" }),
        ];
        let (_first_offset, second_offset, third_offset) =
            write_mailbox(tmp.path(), "#ops", &envelopes);

        let out = call_at_bus_dir(
            &serde_json::json!({ "channel": "#ops", "since": second_offset }),
            tmp.path(),
        )
        .unwrap();

        assert_eq!(out["envelopes"].as_array().unwrap().len(), 1);
        assert_eq!(out["envelopes"][0]["id"], "three");
        assert_eq!(out["next_offset"], third_offset);
    }

    #[test]
    fn missing_mailbox_is_empty_log() {
        let tmp = tempfile::TempDir::new().unwrap();

        let out = call_at_bus_dir(
            &serde_json::json!({ "channel": "planning", "since": 12 }),
            tmp.path(),
        )
        .unwrap();

        assert_eq!(out["channel"], "#planning");
        assert!(out["envelopes"].as_array().unwrap().is_empty());
        assert_eq!(out["next_offset"], 12);
    }

    #[test]
    fn channel_is_required_string() {
        let tmp = tempfile::TempDir::new().unwrap();

        let err = call_at_bus_dir(&serde_json::json!({ "limit": 1 }), tmp.path()).unwrap_err();

        assert_eq!(err.kind, BusErrorKind::EnvelopeInvalid);
        assert!(err.message.contains("channel"));
    }

    #[test]
    fn limit_must_be_non_negative_integer() {
        let tmp = tempfile::TempDir::new().unwrap();

        let err = call_at_bus_dir(
            &serde_json::json!({ "channel": "planning", "limit": -1 }),
            tmp.path(),
        )
        .unwrap_err();

        assert_eq!(err.kind, BusErrorKind::EnvelopeInvalid);
        assert!(err.message.contains("limit"));
    }
}
