//! `famp inbox list` — non-blocking dump via `read_from`.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::cli::error::CliError;
use crate::cli::paths;

/// Derive the `task_id` a given inbox entry refers to.
///
/// - `class == "request"`: envelope's `id` field IS the task_id.
/// - Any other class: `causality.ref` carries the task_id.
///
/// Exhaustively covered by `tests/inbox_list_filters_terminal.rs` —
/// adding a new `MessageClass` variant without updating this function
/// will fail that test.
fn extract_task_id(value: &Value) -> &str {
    let class = value.get("class").and_then(Value::as_str).unwrap_or("");
    match class {
        "request" => value.get("id").and_then(Value::as_str).unwrap_or(""),
        _ => value
            .get("causality")
            .and_then(|c| c.get("ref"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    }
}

/// Test-only re-export so integration tests can call `extract_task_id`
/// without widening the module's public surface.
#[doc(hidden)]
pub fn extract_task_id_for_test(value: &Value) -> &str {
    extract_task_id(value)
}

/// Read every inbox entry at or past `since` and write one JSON line
/// per entry to `out`, in the same locked shape as `famp await`.
/// Does NOT advance the cursor.
pub fn run_list(home: &Path, since: Option<u64>, out: &mut dyn Write) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries =
        famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0)).map_err(CliError::Inbox)?;

    for (value, end_offset) in entries {
        let task_id = extract_task_id(&value);
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
