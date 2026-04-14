//! `famp await` match helper — extract the first entry that matches
//! the optional task filter and project it into the locked JSON shape.

use serde_json::{json, Value};

/// Walk `entries` in order. Return the first entry whose envelope
/// `task_id` matches `task_filter` (or the first entry period, if
/// `task_filter` is `None`), shaped into the locked output:
///
/// ```json
/// { "offset": <end_offset>,
///   "task_id": <str>,
///   "from": <str>,
///   "class": <str>,
///   "body": <any> }
/// ```
///
/// The second tuple element is the cursor value to advance to if the
/// caller consumes this one entry.
pub fn find_match(
    entries: &[(Value, u64)],
    task_filter: &Option<String>,
) -> Option<(Value, u64)> {
    for (value, end_offset) in entries {
        let task_id = value.get("task_id").and_then(Value::as_str);
        if let Some(filter) = task_filter {
            if task_id != Some(filter.as_str()) {
                continue;
            }
        }
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let body = value.get("body").cloned().unwrap_or(Value::Null);
        let out = json!({
            "offset": end_offset,
            "task_id": task_id.unwrap_or(""),
            "from": from,
            "class": class,
            "body": body,
        });
        return Some((out, *end_offset));
    }
    None
}
