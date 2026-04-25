//! `famp inbox list` — non-blocking dump via `read_from`.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use famp_taskdir::{TaskDir, TaskDirError};

use crate::cli::error::CliError;
use crate::cli::paths;

/// Derive the `task_id` a given inbox entry refers to.
///
/// - `class == "request"`: envelope's `id` field IS the `task_id`.
/// - Any other class: `causality.ref` carries the `task_id`.
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
///
/// # Filtering
///
/// By default (`include_terminal = false`) entries whose `task_id`
/// maps to a taskdir record with `terminal == true` are omitted.
///
/// - Missing taskdir record → **fail-open**: entry is surfaced.
/// - Corrupt taskdir record (TOML parse / IO error) → **fail-closed**:
///   entry is hidden; a diagnostic is written to stderr via
///   `eprintln!`. A corrupt record for a terminal task must not
///   resurrect its history into `list` forever; operator visibility
///   comes through stderr.
///
/// # Canonical completion signal
///
/// `list` is not the place to learn that a task just completed. Once
/// the daemon flips a task's taskdir record to `terminal = true`, the
/// closing deliver is hidden here. Agents that need real-time
/// completion notifications MUST use `famp await`, which is
/// deliberately unfiltered.
pub fn run_list(
    home: &Path,
    since: Option<u64>,
    include_terminal: bool,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries =
        famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0)).map_err(CliError::Inbox)?;

    // Only open the taskdir when filtering. If it fails to open
    // (e.g. fresh FAMP_HOME with no tasks dir), fall back to "filter
    // disabled for this call" — equivalent to include_terminal=true.
    // Opening a TaskDir mkdir -p's the root, so normal paths succeed.
    let taskdir: Option<TaskDir> = if include_terminal {
        None
    } else {
        match TaskDir::open(paths::tasks_dir(home)) {
            Ok(td) => Some(td),
            Err(err) => {
                eprintln!("famp inbox list: taskdir unavailable, filter disabled: {err}");
                None
            }
        }
    };
    let mut terminal_cache: HashMap<String, bool> = HashMap::new();

    for (value, end_offset) in entries {
        let task_id = extract_task_id(&value);
        if let Some(ref td) = taskdir {
            if task_id.is_empty() {
                // Nothing to look up — fail-open.
            } else if is_terminal_cached(td, task_id, &mut terminal_cache) {
                continue;
            }
        }
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let body = value.get("body").cloned().unwrap_or(Value::Null);
        // Quick-260425-pc7: hoist scope.more_coming to a top-level field
        // on request envelopes so callers don't have to dig through
        // body.scope to know whether the sender expects follow-up
        // briefing before this task is ready to commit. Default false
        // (key absent in legacy + non-flagging senders).
        let more_coming = if class == "request" {
            body.pointer("/scope/more_coming")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        } else {
            false
        };
        let shaped = json!({
            "offset": end_offset,
            "task_id": task_id,
            "from": from,
            "class": class,
            "more_coming": more_coming,
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

/// Cached taskdir lookup. Returns `true` if the entry should be hidden.
///
/// Caching the verdict also de-duplicates the fail-closed `eprintln!` —
/// a corrupt taskdir record logs once per `run_list` call, not once per
/// affected inbox entry.
///
/// Rules:
/// - `NotFound` / `InvalidUuid` → `false` (fail-open; surface entry).
/// - `Ok(rec)`                   → `rec.terminal`.
/// - any other error             → `true`  (fail-closed; hide entry + `eprintln`).
fn is_terminal_cached(td: &TaskDir, task_id: &str, cache: &mut HashMap<String, bool>) -> bool {
    if let Some(cached) = cache.get(task_id) {
        return *cached;
    }
    let verdict = match td.read(task_id) {
        Ok(rec) => rec.terminal,
        // Fail-open per spec edge-case table: an unparseable or absent
        // task_id is a property of the inbox entry, not evidence that a
        // task has completed. InvalidUuid here means the entry's
        // causality.ref (or id, for `request`) isn't a valid UUID —
        // surface it and move on.
        Err(TaskDirError::NotFound { .. } | TaskDirError::InvalidUuid { .. }) => false,
        Err(other) => {
            eprintln!("famp inbox list: hiding entry for task_id={task_id}: {other}",);
            true
        }
    };
    cache.insert(task_id.to_string(), verdict);
    verdict
}
