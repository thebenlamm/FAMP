//! Transcript / rollout listen-state replay.
//!
//! Port of the Python identity extractor embedded in `famp-await.sh`.
//! Scans the last 2 MB of a Claude Code transcript or Codex rollout JSONL
//! and replays successful `famp_register` / `famp_set_listen` control actions.

use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_BYTES: u64 = 2_000_000;

#[derive(Debug, Clone)]
enum ActionKind {
    Register,
    SetListen,
}

#[derive(Debug, Clone)]
struct Action {
    tool_use_id: String,
    kind: ActionKind,
    identity: String,
    /// `None` means absent/null → treat as listen ON (matches MCP default).
    listen: Option<bool>,
}

/// Replayed listen-mode state for a transcript/rollout path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenState {
    /// A successful register/set-listen action left listen mode ON for `String`.
    Active(String),
    /// At least one successful register/set-listen action was replayed, but
    /// listen mode ended OFF (explicit `listen:false` or `famp_set_listen(false)`).
    ExplicitlyOff,
    /// No successful register/set-listen control action was found (or the
    /// file could not be scanned at all) — caller should fail open.
    Unresolved,
}

/// Extract the active listen-mode identity from a transcript/rollout path.
/// Returns `None` on any uncertainty (fail-open) OR an explicit opt-out.
///
/// Thin wrapper over [`extract_listen_state`]; kept for existing callers/tests.
pub fn extract_listen_identity(path: &Path) -> Option<String> {
    match extract_listen_state(path) {
        ListenState::Active(name) => Some(name),
        ListenState::ExplicitlyOff | ListenState::Unresolved => None,
    }
}

/// Extract the listen-mode state from a transcript/rollout path, distinguishing
/// an explicit listen opt-out from an unresolved/absent registration.
pub fn extract_listen_state(path: &Path) -> ListenState {
    let Ok(actions) = scan_transcript(path) else {
        return ListenState::Unresolved;
    };
    replay_actions(&actions)
}

fn scan_transcript(path: &Path) -> std::io::Result<(Vec<Action>, HashMap<String, bool>)> {
    let mut file = File::open(path)?;
    let fsize = file.metadata()?.len();
    let offset = fsize.saturating_sub(MAX_BYTES);
    if offset > 0 {
        file.seek(SeekFrom::Start(offset))?;
    }
    // Read raw bytes rather than `read_to_string`: the seek offset can land
    // mid-multibyte-character, and a transcript may contain invalid UTF-8
    // elsewhere. `from_utf8_lossy` mirrors the shell adapter's Python
    // `errors='replace'` behavior instead of hard-failing identity resolution.
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let mut tail = String::from_utf8_lossy(&buf).into_owned();
    // Discard partial line at the seek boundary.
    if offset > 0 {
        if let Some(nl) = tail.find('\n') {
            tail = tail[nl + 1..].to_string();
        }
    }

    let mut actions = Vec::new();
    let mut results: HashMap<String, bool> = HashMap::new();
    let mut pos: u64 = 0;

    for line in tail.lines() {
        pos += 1;
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        parse_claude_format(&ev, pos, &mut actions, &mut results);
        parse_codex_format(&ev, pos, &mut actions, &mut results);
    }

    Ok((actions, results))
}

fn parse_args(raw: &Value) -> Value {
    match raw {
        Value::Object(_) => raw.clone(),
        Value::String(s) => {
            serde_json::from_str(s).unwrap_or(Value::Object(serde_json::Map::new()))
        }
        _ => Value::Object(serde_json::Map::new()),
    }
}

fn listen_flag(args: &Value) -> Option<bool> {
    match args.get("listen") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(other) => other.as_bool(),
    }
}

fn identity_from_args(args: &Value) -> Option<String> {
    args.get("identity")
        .or_else(|| args.get("name"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn function_output_success(payload: &Value) -> bool {
    let output = payload.get("output").and_then(Value::as_str).unwrap_or("");
    let lowered = output.to_lowercase().replace(' ', "");
    !lowered.contains("\"iserror\":true") && !lowered.contains("\"is_error\":true")
}

fn parse_claude_format(
    ev: &Value,
    pos: u64,
    actions: &mut Vec<Action>,
    results: &mut HashMap<String, bool>,
) {
    let msg = ev.get("message").filter(|m| m.is_object()).unwrap_or(ev);
    let content = match msg.get("content") {
        Some(Value::Array(a)) => a.as_slice(),
        _ => return,
    };
    for block in content {
        let Some(obj) = block.as_object() else {
            continue;
        };
        let t = obj.get("type").and_then(Value::as_str).unwrap_or("");
        let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
        if t == "tool_use" {
            let inp = parse_args(obj.get("input").unwrap_or(&Value::Null));
            let uid = obj
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if name.ends_with("famp_register") {
                if let Some(ident) = identity_from_args(&inp) {
                    if !uid.is_empty() {
                        actions.push(Action {
                            tool_use_id: uid,
                            kind: ActionKind::Register,
                            identity: ident,
                            listen: listen_flag(&inp),
                        });
                    }
                }
            } else if name.ends_with("famp_set_listen") && !uid.is_empty() {
                actions.push(Action {
                    tool_use_id: uid,
                    kind: ActionKind::SetListen,
                    identity: String::new(),
                    listen: listen_flag(&inp),
                });
            }
        } else if t == "tool_result" {
            if let Some(uid) = obj.get("tool_use_id").and_then(Value::as_str) {
                // Strict: only JSON true counts as error.
                let ok = obj.get("is_error") != Some(&Value::Bool(true));
                results.insert(uid.to_string(), ok);
            }
        }
    }
    let _ = pos;
}

fn parse_codex_format(
    ev: &Value,
    pos: u64,
    actions: &mut Vec<Action>,
    results: &mut HashMap<String, bool>,
) {
    let payload = match ev.get("payload") {
        Some(p) if p.is_object() => p,
        _ => return,
    };
    let ptype = payload.get("type").and_then(Value::as_str).unwrap_or("");

    if ptype == "function_call" {
        let mut tool = payload
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let namespace = payload
            .get("namespace")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !namespace.is_empty() && namespace != "mcp__famp" {
            tool.clear();
        }
        let args = parse_args(payload.get("arguments").unwrap_or(&Value::Null));
        let uid = payload
            .get("call_id")
            .and_then(Value::as_str)
            .map_or_else(|| format!("codex-fc:{pos}"), str::to_owned);
        if tool.ends_with("famp_register") {
            if let Some(ident) = identity_from_args(&args) {
                actions.push(Action {
                    tool_use_id: uid,
                    kind: ActionKind::Register,
                    identity: ident,
                    listen: listen_flag(&args),
                });
            }
        } else if tool.ends_with("famp_set_listen") {
            actions.push(Action {
                tool_use_id: uid,
                kind: ActionKind::SetListen,
                identity: String::new(),
                listen: listen_flag(&args),
            });
        }
    } else if ptype == "function_call_output" {
        if let Some(uid) = payload.get("call_id").and_then(Value::as_str) {
            if !results.contains_key(uid) {
                results.insert(uid.to_string(), function_output_success(payload));
            }
        }
    }

    if ptype == "mcp_tool_call_end" {
        let inv = payload
            .get("invocation")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));
        let mut tool = inv
            .get("tool")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // Anti-hijack, mirroring the `namespace` check on the `function_call`
        // branch: only the `famp` MCP server may set a listen identity. Without
        // this, any server exposing a tool whose name merely *ends with*
        // `famp_register` could arm listen mode for an arbitrary identity.
        //
        // Absent/empty `server` is treated as allow, deliberately and in parity
        // with the sibling branch: real Codex `mcp_tool_call_end` events always
        // populate `invocation.server` (MCP routing depends on it), so an empty
        // value never accompanies a genuine foreign call. See the
        // `codex_*_mcp_server_*` fixtures pinning both the foreign-blocked and
        // empty-allowed cases.
        let server = inv.get("server").and_then(Value::as_str).unwrap_or("");
        if !server.is_empty() && server != "famp" {
            tool.clear();
        }
        let tool = tool.as_str();
        let args = parse_args(inv.get("arguments").unwrap_or(&Value::Null));
        let result = payload
            .get("result")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));
        let ok_payload = result.get("Ok").filter(|v| v.is_object());
        let ok = ok_payload.is_some_and(|o| o.get("isError") != Some(&Value::Bool(true)));
        let uid = payload
            .get("call_id")
            .and_then(Value::as_str)
            .map_or_else(|| format!("codex:{pos}"), str::to_owned);
        if tool.ends_with("famp_register") {
            if let Some(ident) = identity_from_args(&args) {
                actions.push(Action {
                    tool_use_id: uid.clone(),
                    kind: ActionKind::Register,
                    identity: ident,
                    listen: listen_flag(&args),
                });
                results.insert(uid, ok);
            }
        } else if tool.ends_with("famp_set_listen") {
            actions.push(Action {
                tool_use_id: uid.clone(),
                kind: ActionKind::SetListen,
                identity: String::new(),
                listen: listen_flag(&args),
            });
            results.insert(uid, ok);
        }
    }
}

fn replay_actions(data: &(Vec<Action>, HashMap<String, bool>)) -> ListenState {
    let (actions, results) = data;
    let mut active = String::new();
    let mut last_identity = String::new();
    // Tracks an actual off-signal (explicit `listen:false`), NOT mere
    // emptiness of `active`. A successful `set_listen(true)` replayed with
    // no resolvable identity (e.g. the original `famp_register` scrolled
    // out of the 2 MB transcript tail post-compaction) must classify as
    // `Unresolved` — not `ExplicitlyOff` — so the pid-correlated fallback
    // (which exists precisely for that compaction case) still runs.
    let mut explicit_off = false;
    for action in actions {
        if !results.get(&action.tool_use_id).copied().unwrap_or(false) {
            continue;
        }
        match action.kind {
            ActionKind::Register => {
                if !action.identity.is_empty() {
                    last_identity.clone_from(&action.identity);
                }
                // listen defaults ON unless explicit JSON false.
                if action.listen == Some(false) {
                    active.clear();
                    explicit_off = true;
                } else {
                    active.clone_from(&action.identity);
                    explicit_off = false;
                }
            }
            ActionKind::SetListen => {
                if action.listen == Some(false) {
                    active.clear();
                    explicit_off = true;
                } else if !last_identity.is_empty() {
                    active.clone_from(&last_identity);
                    explicit_off = false;
                }
                // else: listen turned back on but no identity is resolvable
                // from this replay window — leave `explicit_off` as-is;
                // there is nothing to activate yet.
            }
        }
    }
    if !active.is_empty() {
        ListenState::Active(active)
    } else if explicit_off {
        ListenState::ExplicitlyOff
    } else {
        ListenState::Unresolved
    }
}

/// Validate identity: `^[A-Za-z0-9._-]{1,64}$`, no newlines.
pub fn validate_identity(identity: &str) -> bool {
    if identity.is_empty() || identity.len() > 64 || identity.contains('\n') {
        return false;
    }
    identity
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Validate sender for notification: `^[A-Za-z0-9@._:/-]{1,128}$`.
pub fn validate_sender(sender: &str) -> bool {
    if sender.is_empty() || sender.len() > 128 {
        return false;
    }
    sender
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | ':' | '/' | '-'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(body: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f
    }

    #[test]
    fn claude_listen_true_register() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()).as_deref(), Some("dk"));
    }

    #[test]
    fn claude_listen_false_is_none() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":false}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()), None);
    }

    #[test]
    fn claude_set_listen_false_cancels() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"mcp__famp__famp_set_listen","input":{"listen":false}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()), None);
    }

    #[test]
    fn listen_absent_defaults_on() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()).as_deref(), Some("dk"));
    }

    #[test]
    fn codex_mcp_tool_call_end() {
        let body = r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"ok"}],"isError":false}}}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()).as_deref(), Some("codex"));
    }

    #[test]
    fn codex_foreign_mcp_server_cannot_arm_listen() {
        // A non-famp MCP server exposing a tool whose name ends in
        // "famp_register" must not resolve to a listen identity.
        let body = r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_evil","invocation":{"server":"evil","tool":"evil__famp_register","arguments":{"identity":"attacker","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()), None);
        assert!(matches!(
            extract_listen_state(f.path()),
            ListenState::Unresolved
        ));
    }

    #[test]
    fn codex_empty_mcp_server_still_resolves() {
        // Companion to the foreign-blocked case: an absent `server` on a
        // genuine famp event must still arm listen mode. This pins the
        // deliberate "empty means allow" half of the anti-hijack rule so a
        // future tightening to reject empty is a conscious, tested choice.
        let body = r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()).as_deref(), Some("codex"));
    }

    #[test]
    fn codex_set_listen_false_cancels() {
        let body = r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_set","invocation":{"server":"famp","tool":"famp_set_listen","arguments":{"listen":false}},"result":{"Ok":{"content":[],"isError":false}}}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_identity(f.path()), None);
    }

    #[test]
    fn validate_identity_rules() {
        assert!(validate_identity("dk"));
        assert!(validate_identity("a.b_c-1"));
        assert!(!validate_identity(""));
        assert!(!validate_identity("has space"));
        assert!(!validate_identity("has\nnewline"));
    }

    #[test]
    fn invalid_utf8_early_line_still_resolves_identity() {
        // First line: an early "line" containing an invalid UTF-8 byte
        // sequence (a lone continuation byte). Second line: a valid JSONL
        // register line. `scan_transcript` must not hard-fail on the
        // invalid bytes (fixed via `read_to_end` + `from_utf8_lossy`).
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(b"garbage-line-with-invalid-utf8:");
        bytes.push(0xFF); // invalid standalone byte in UTF-8
        bytes.push(b'\n');
        bytes.extend_from_slice(
            br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}"#,
        );
        bytes.push(b'\n');
        bytes.extend_from_slice(
            br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}"#,
        );
        bytes.push(b'\n');

        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(&bytes).unwrap();
        assert_eq!(extract_listen_identity(f.path()).as_deref(), Some("dk"));
    }

    #[test]
    fn listen_state_active_when_registered_on() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(
            extract_listen_state(f.path()),
            ListenState::Active("dk".to_string())
        );
    }

    #[test]
    fn listen_state_explicitly_off_when_register_listen_false() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":false}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_state(f.path()), ListenState::ExplicitlyOff);
    }

    /// Regression guard: a post-compaction transcript tail can contain a
    /// successful `famp_set_listen(true)` with the original `famp_register`
    /// scrolled out of the 2 MB window. This must classify as `Unresolved`
    /// (fallback allowed) — NOT `ExplicitlyOff` (which would wrongly
    /// suppress the pid-correlated fallback that exists for exactly this
    /// compaction case).
    #[test]
    fn listen_state_unresolved_when_set_listen_true_without_register_compaction_guard() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_set_listen","input":{"listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(extract_listen_state(f.path()), ListenState::Unresolved);
    }

    #[test]
    fn listen_state_active_when_set_listen_true_reactivates_after_register_listen_false() {
        let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":false}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":"ok"}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"mcp__famp__famp_set_listen","input":{"listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","is_error":false,"content":"ok"}]}}
"#;
        let f = write_temp(body);
        assert_eq!(
            extract_listen_state(f.path()),
            ListenState::Active("dk".to_string())
        );
    }

    #[test]
    fn listen_state_unresolved_when_no_control_action() {
        let body = "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[]}}\n";
        let f = write_temp(body);
        assert_eq!(extract_listen_state(f.path()), ListenState::Unresolved);
    }

    #[test]
    fn listen_state_unresolved_when_file_missing() {
        let missing = Path::new("/nonexistent/path/does-not-exist.jsonl");
        assert_eq!(extract_listen_state(missing), ListenState::Unresolved);
    }
}
