//! Shared Stop-hook entry filter used by install and uninstall.

use std::path::Path;

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
/// matches when its `command` field:
///   - equals a shim exactly, OR
///   - starts with `"<shim> "` (legacy entries with trailing arguments), OR
///   - contains a whitespace-separated token whose basename equals a shim's
///     filename (matches wrapped forms like `bash <path>`, `/bin/bash <path>`,
///     `sh <path>`).
pub fn remove_famp_hook_from_stop_entry(entry: &Value, shims: &[String]) -> Option<Value> {
    if entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| is_famp_command(command, shims))
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
                .is_some_and(|command| is_famp_command(command, shims))
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

/// Returns `true` when `command` is a famp-owned shim invocation.
///
/// Recognizes:
/// - exact match: `command == shim`
/// - prefix match: `command` starts with `"<shim> "` (legacy trailing args)
/// - wrapped form: any whitespace-separated token in `command` whose
///   basename equals the filename of a shim (e.g. `bash <shim>`,
///   `/bin/bash <shim>`, `sh <shim>`).
fn is_famp_command(command: &str, shims: &[String]) -> bool {
    // Existing forms: exact match or "<shim> " prefix.
    for shim in shims {
        if command == shim.as_str() || command.starts_with(&format!("{shim} ")) {
            return true;
        }
    }
    // NEW: wrapped forms like `bash <path>`, `/bin/bash <path>`, `sh <path>`.
    // Tokenize on whitespace; if any token's basename matches a shim's filename, match.
    let shim_filenames: Vec<&str> = shims
        .iter()
        .filter_map(|s| Path::new(s).file_name().and_then(|f| f.to_str()))
        .collect();
    if shim_filenames.is_empty() {
        return false;
    }
    for token in command.split_whitespace() {
        if let Some(basename) = Path::new(token).file_name().and_then(|f| f.to_str()) {
            if shim_filenames.contains(&basename) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    const SHIM: &str = "/home/u/.claude/hooks/famp-await.sh";

    fn shims() -> Vec<String> {
        vec![SHIM.to_string()]
    }

    fn bare_entry(command: &str) -> serde_json::Value {
        json!({ "type": "command", "command": command, "timeout": 30 })
    }

    fn wrapped_entry(command: &str) -> serde_json::Value {
        json!({
            "matcher": "",
            "hooks": [{ "type": "command", "command": command, "timeout": 86400 }]
        })
    }

    // ---- Positive cases (entry IS famp-owned → should be dropped → result is None) ----

    // Test 1: bare-command entry, exact shim path → dropped
    #[test]
    fn exact_match_bare_entry_is_dropped() {
        let entry = bare_entry(SHIM);
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "exact shim in bare entry must be dropped"
        );
    }

    // Test 2: exact shim with trailing arg → dropped
    #[test]
    fn prefix_match_with_args_is_dropped() {
        let entry = bare_entry(&format!("{SHIM} --arg"));
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "shim-prefixed command must be dropped"
        );
    }

    // Test 3: `bash <shim>` wrapped form → dropped
    #[test]
    fn bash_wrapped_form_is_dropped() {
        let entry = bare_entry(&format!("bash {SHIM}"));
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "`bash <shim>` entry must be dropped"
        );
    }

    // Test 4: `/bin/bash <shim> --foo` wrapped form with trailing args → dropped
    #[test]
    fn bin_bash_wrapped_with_args_is_dropped() {
        let entry = bare_entry(&format!("/bin/bash {SHIM} --foo"));
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "`/bin/bash <shim> --foo` entry must be dropped"
        );
    }

    // Test 5: `sh <shim>` wrapped form → dropped
    #[test]
    fn sh_wrapped_form_is_dropped() {
        let entry = bare_entry(&format!("sh {SHIM}"));
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "`sh <shim>` entry must be dropped"
        );
    }

    // Test 3 (inner-hooks variant): `bash <shim>` inside a hooks array → inner hook filtered
    #[test]
    fn bash_wrapped_form_in_inner_hooks_is_filtered() {
        let entry = wrapped_entry(&format!("bash {SHIM}"));
        assert!(
            remove_famp_hook_from_stop_entry(&entry, &shims()).is_none(),
            "`bash <shim>` in inner hooks-only entry must be dropped"
        );
    }

    // ---- Negative cases (entry is NOT famp-owned → should be preserved → result is Some(entry)) ----

    // Test 6: non-famp script wrapped with bash → preserved
    #[test]
    fn non_famp_bash_wrapped_is_preserved() {
        let entry = bare_entry("bash /other/hook.sh");
        let result = remove_famp_hook_from_stop_entry(&entry, &shims());
        assert_eq!(
            result,
            Some(entry),
            "non-famp wrapped command must not be reaped"
        );
    }

    // Test 7: empty command string → preserved
    #[test]
    fn empty_command_is_preserved() {
        let entry = bare_entry("");
        let result = remove_famp_hook_from_stop_entry(&entry, &shims());
        assert_eq!(result, Some(entry), "empty command must not be reaped");
    }

    // Test 8: empty shims slice → preserved
    #[test]
    fn empty_shims_slice_preserves_any_entry() {
        let entry = bare_entry(SHIM);
        let result = remove_famp_hook_from_stop_entry(&entry, &[]);
        assert_eq!(
            result,
            Some(entry),
            "empty shims slice must not reap anything"
        );
    }
}
