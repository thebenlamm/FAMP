//! `famp await` match helper — extract the first entry that matches
//! the optional task filter and project it into the locked JSON shape.
//!
//! ## Task-ID extraction from raw envelope JSON
//!
//! The inbox stores raw signed-envelope bytes. The envelope JSON structure is:
//!
//! ```json
//! { "famp": "0.5.1", "id": "<uuid>", "from": "...", "to": "...",
//!   "class": "request|commit|deliver|ack|control", ..., "body": {...} }
//! ```
//!
//! There is NO top-level `task_id` field. The task ID is derived as:
//!
//! - `class == "request"`: `task_id` = `id` (the request IS the task opener)
//! - all other classes: `task_id` = `causality["ref"]` (links back to the
//!   original request whose `id` became the `task_id`; the field serializes
//!   as `"ref"` due to the `#[serde(rename = "ref")]` on `Causality`)
//!
//! The shaped output JSON uses `task_id` as the key so callers get a
//! uniform field regardless of which class they received.

use serde_json::{json, Value};

/// Extract the task ID from a raw envelope JSON value.
///
/// Returns `None` if the envelope is not task-scoped or the relevant field
/// is absent (e.g. an Ack without causality).
fn extract_task_id(value: &Value) -> Option<&str> {
    let class = value.get("class").and_then(Value::as_str)?;
    match class {
        "request" => {
            // The request id IS the task id.
            value.get("id").and_then(Value::as_str)
        }
        _ => {
            // For commit, deliver, ack, control: causality["ref"] is the
            // task id (the original request's id).
            // Note: Causality serializes the field as "ref" (serde rename),
            // NOT "referenced".
            value
                .get("causality")
                .and_then(|c| c.get("ref"))
                .and_then(Value::as_str)
        }
    }
}

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
///
/// ## Task-filter skip rule
///
/// When `task_filter` is `Some`, `request`-class entries are skipped even
/// when their `id` matches. The originator calls `await --task X` to wait
/// for a reply (commit/deliver/ack), never to receive their own outgoing
/// request back. Skipped entries are NOT consumed — the caller's
/// consume-and-discard logic in `mod.rs` handles advancing past them.
pub fn find_match(entries: &[(Value, u64)], task_filter: &Option<String>) -> Option<(Value, u64)> {
    for (value, end_offset) in entries {
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let task_id = extract_task_id(value);
        if let Some(filter) = task_filter {
            // Skip request-class entries when filtering by task: the originator
            // is waiting for a reply, not their own outgoing request envelope.
            if class == "request" {
                continue;
            }
            if task_id != Some(filter.as_str()) {
                continue;
            }
        }
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
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
