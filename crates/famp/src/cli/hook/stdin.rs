//! Parse host Stop-hook JSON from stdin.

use serde_json::Value;
use std::io::Read;

/// Fields extracted from a Stop-hook stdin payload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopHookInput {
    pub transcript_path: Option<String>,
    pub session_id: Option<String>,
    /// Host-provided stop reason (Grok uses `end_turn` / `channel_closed` / …).
    pub reason: Option<String>,
}

/// Read stdin fully and parse as Stop-hook JSON. Fail-open: empty/malformed → default.
pub fn read_stop_hook_input() -> StopHookInput {
    let mut buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut buf);
    parse_stop_hook_json(&buf)
}

pub fn parse_stop_hook_json(raw: &str) -> StopHookInput {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return StopHookInput::default();
    };
    let Some(obj) = v.as_object() else {
        return StopHookInput::default();
    };
    let str_field = |snake: &str, camel: &str| -> Option<String> {
        obj.get(snake)
            .or_else(|| obj.get(camel))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    StopHookInput {
        transcript_path: str_field("transcript_path", "transcriptPath"),
        session_id: str_field("session_id", "sessionId"),
        reason: str_field("reason", "reason"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_snake_case() {
        let i = parse_stop_hook_json(
            r#"{"transcript_path":"/t.jsonl","session_id":"abc","reason":"end_turn"}"#,
        );
        assert_eq!(i.transcript_path.as_deref(), Some("/t.jsonl"));
        assert_eq!(i.session_id.as_deref(), Some("abc"));
        assert_eq!(i.reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn parses_camel_case() {
        let i = parse_stop_hook_json(r#"{"transcriptPath":"/t.jsonl","sessionId":"xyz"}"#);
        assert_eq!(i.transcript_path.as_deref(), Some("/t.jsonl"));
        assert_eq!(i.session_id.as_deref(), Some("xyz"));
    }

    #[test]
    fn malformed_is_empty() {
        assert_eq!(parse_stop_hook_json("not-json"), StopHookInput::default());
    }
}
