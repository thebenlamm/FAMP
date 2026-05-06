//! Shared Stop-hook entry filter used by install and uninstall.

use serde_json::Value;

/// Remove famp-owned commands from a single Stop-hook array element.
///
/// Returns `None` (drop the entry entirely) if the entry itself is a
/// bare famp command, or if after filtering its inner `hooks` array
/// becomes empty. Returns `Some(entry)` unchanged when no famp commands
/// are found. Returns `Some(updated)` with famp inner hooks stripped
/// when the entry also contains non-famp hooks.
///
/// `shims` is a slice of canonical famp-owned path strings. An entry
/// matches when its `command` field equals a shim exactly OR starts with
/// `"<shim> "` (legacy entries that carried trailing arguments).
pub fn remove_famp_hook_from_stop_entry(entry: &Value, shims: &[String]) -> Option<Value> {
    if entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| shims.iter().any(|s| command == s.as_str() || command.starts_with(&format!("{s} "))))
    {
        return None;
    }

    let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
        return Some(entry.clone());
    };
    let filtered_hooks: Vec<Value> = hooks
        .iter()
        .filter(|hook| {
            !hook
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| shims.iter().any(|s| command == s.as_str() || command.starts_with(&format!("{s} "))))
        })
        .cloned()
        .collect();

    if filtered_hooks.len() == hooks.len() {
        return Some(entry.clone());
    }
    if filtered_hooks.is_empty() {
        return None;
    }

    let mut updated = entry.clone();
    let obj = updated.as_object_mut()?;
    obj.insert("hooks".to_string(), Value::Array(filtered_hooks));
    Some(updated)
}
