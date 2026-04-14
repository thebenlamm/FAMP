//! `famp inbox list` — non-blocking dump via `read_from`.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::cli::error::CliError;
use crate::cli::paths;

/// Read every inbox entry at or past `since` and write one JSON line
/// per entry to `out`, in the same locked shape as `famp await`.
/// Does NOT advance the cursor.
pub fn run_list(home: &Path, since: Option<u64>, out: &mut dyn Write) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries = famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0))
        .map_err(CliError::Inbox)?;

    for (value, end_offset) in entries {
        let task_id = value.get("task_id").and_then(Value::as_str).unwrap_or("");
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let body = value.get("body").cloned().unwrap_or(Value::Null);
        let shaped = json!({
            "offset": end_offset,
            "task_id": task_id,
            "from": from,
            "class": class,
            "body": body,
        });
        let line = serde_json::to_string(&shaped).unwrap_or_default();
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: inbox_path.clone(),
            source: e,
        })?;
    }
    Ok(())
}
