//! Stdio MCP JSON-RPC server — newline-delimited JSON.
//!
//! ## Wire format
//!
//! The MCP client's stdio transport uses newline-delimited JSON (NDJSON):
//! one JSON object per line, terminated by `\n`.
//!
//! ## Handled methods
//!
//! | Method                  | Handler                              |
//! |-------------------------|--------------------------------------|
//! | `initialize`            | Returns server info + tool capability |
//! | `notifications/initialized` | No-op notification (no response) |
//! | `tools/list`            | Returns the four tool descriptors    |
//! | `tools/call`            | Dispatches to the right tool handler |
//! | anything else           | JSON-RPC `-32601 Method not found`   |

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{stdin, stdout};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::cli::error::CliError;
use crate::cli::mcp::tools;

// ── constants ─────────────────────────────────────────────────────────────────

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "famp-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── tool descriptors (sent in tools/list) ─────────────────────────────────────

fn tool_descriptors() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "famp_send",
            "description": "Send a FAMP message. Use 'new_task' to start a conversation. Use 'deliver' or 'terminal' to REPLY to an existing task (you MUST include the task_id from the inbox entry you're responding to).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "peer":    { "type": "string", "description": "Peer alias (e.g. 'alice' or 'bob')" },
                    "mode":    { "type": "string", "enum": ["new_task", "deliver", "terminal"], "description": "new_task=start conversation, deliver=interim reply, terminal=final reply" },
                    "task_id": { "type": "string", "description": "The task_id from the inbox entry you're replying to. REQUIRED for deliver/terminal modes." },
                    "title":   { "type": "string", "description": "Summary (for new_task mode)" },
                    "body":    { "type": "string", "description": "Task body content (the actual instructions). REQUIRED for new_task to carry content; the title field is only a short summary. For deliver/terminal modes, this is the reply text." },
                    "more_coming": { "type": "boolean", "description": "OPTIONAL, new_task mode only. Set true when this is the FIRST of multiple envelopes briefing the same task — the receiver will hold the task as 'pending follow-up' instead of treating it as ready to commit on the first envelope. Send subsequent context via famp_send mode=deliver; the briefing is complete when you send a deliver envelope without more_coming (or mode=terminal for a final reply). Default false (the task is fully briefed in this single envelope). Mirrors the body.interim flag on deliver envelopes. Ignored outside new_task mode." }
                },
                "required": ["peer", "mode"]
            }
        },
        {
            "name": "famp_await",
            "description": "Block until a new inbox message arrives. This is the canonical real-time signal — unlike famp_inbox list (which hides entries for tasks that have reached a terminal FSM state), famp_await delivers every message including the closing 'terminal' reply that finishes a task. USE THIS to detect when a task you sent via famp_send completes. Pass task_id to wait only for a reply to that specific task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timeout_seconds": { "type": "integer", "description": "Timeout in seconds (default 30)" },
                    "task_id":         { "type": "string",  "description": "Wait for reply to this specific task. Recommended when you know which task you're waiting on." }
                }
            }
        },
        {
            "name": "famp_inbox",
            "description": "List received messages (active work only) or advance the read cursor. Each list entry has a 'task_id' — use that with famp_send (mode=deliver or terminal) to reply. IMPORTANT: by default, list hides entries for tasks that have reached a terminal FSM state (COMPLETED, FAILED, CANCELLED) — it is the 'what's still on my plate' view. To observe task completion in real time, use famp_await instead. To see the full unfiltered log (e.g. for debugging), pass include_terminal=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action":           { "type": "string",  "enum": ["list", "ack"], "description": "list=show messages, ack=mark as processed" },
                    "since":            { "type": "integer", "description": "Byte offset to start from (default 0)" },
                    "offset":           { "type": "integer", "description": "Byte offset to ack up to (required for action=ack)" },
                    "include_terminal": { "type": "boolean", "description": "When action=list, include entries for tasks in a terminal FSM state. Default false. Use famp_await, not this flag, to observe completion in real time — this override is for full-history inspection." }
                },
                "required": ["action"]
            }
        },
        {
            "name": "famp_peers",
            "description": "List or add peers in peers.toml.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action":    { "type": "string", "enum": ["list", "add"] },
                    "alias":     { "type": "string" },
                    "endpoint":  { "type": "string" },
                    "pubkey":    { "type": "string", "description": "base64url-unpadded Ed25519 pubkey" },
                    "principal": { "type": "string" }
                },
                "required": ["action"]
            }
        }
    ])
}

/// Truncate a malformed input line to a small preview suitable for an
/// error payload — keeps `data` from carrying an arbitrarily large body
/// back to the peer.
fn preview(line: &str) -> String {
    const MAX: usize = 120;
    if line.len() <= MAX {
        line.to_string()
    } else {
        let mut s: String = line.chars().take(MAX).collect();
        s.push('…');
        s
    }
}

// ── framing ───────────────────────────────────────────────────────────────────

/// Write one newline-delimited JSON message to stdout.
async fn write_msg<W>(out: &mut W, msg: &serde_json::Value) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let mut body = serde_json::to_string(msg).map_err(std::io::Error::other)?;
    body.push('\n');
    out.write_all(body.as_bytes()).await?;
    out.flush().await
}

// ── response builders ─────────────────────────────────────────────────────────

fn ok_response(id: &serde_json::Value, result: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(
    id: &serde_json::Value,
    code: i64,
    message: &str,
    data: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code":    code,
            "message": message,
            "data":    data
        }
    })
}

fn cli_error_response(id: &serde_json::Value, err: &CliError) -> serde_json::Value {
    let data = serde_json::json!({
        "famp_error_kind": err.mcp_error_kind(),
        "details": {}
    });
    error_response(id, -32_000, &err.to_string(), &data)
}

fn tool_result(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "isError": false
    })
}

// ── message reader ────────────────────────────────────────────────────────────

/// Outcome of one read of the MCP NDJSON stream.
#[derive(Debug)]
enum ReadOutcome {
    /// Successfully parsed a JSON-RPC message.
    Message(serde_json::Value),
    /// The peer closed stdin cleanly — server should exit its loop.
    Eof,
    /// Underlying IO failure on stdin — surface via `-32700` so the peer
    /// gets a deterministic response rather than a silent hang.
    IoError(std::io::Error),
    /// Line arrived but is not valid JSON — emit JSON-RPC `-32700 Parse error`.
    ParseError {
        line: String,
        source: serde_json::Error,
    },
}

/// Read one newline-delimited JSON-RPC message from a buffered stdin.
///
/// Distinguishes EOF, IO failure, and JSON parse failure so the server
/// loop can report parse errors as `-32700` instead of silently treating
/// them as EOF (which made misbehaving clients hang waiting for a response).
async fn read_msg<R>(reader: &mut BufReader<R>) -> ReadOutcome
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let mut line = String::new();
        let n = match reader.read_line(&mut line).await {
            Ok(n) => n,
            Err(e) => return ReadOutcome::IoError(e),
        };
        if n == 0 {
            return ReadOutcome::Eof;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Skip empty/whitespace-only lines, continue loop.
            continue;
        }
        match serde_json::from_str(trimmed) {
            Ok(v) => return ReadOutcome::Message(v),
            Err(source) => {
                return ReadOutcome::ParseError {
                    line: trimmed.to_string(),
                    source,
                };
            }
        }
    }
}

// ── main server loop ──────────────────────────────────────────────────────────

/// Run the stdio MCP server until stdin is closed.
pub async fn run(home: PathBuf) -> Result<(), CliError> {
    let home = Arc::new(home);
    let mut reader = BufReader::new(stdin());
    let mut out = stdout();

    loop {
        let msg = match read_msg(&mut reader).await {
            ReadOutcome::Message(m) => m,
            ReadOutcome::Eof => break,
            ReadOutcome::IoError(e) => {
                // Emit a JSON-RPC parse error with id=null so the peer is not
                // left hanging, then exit the loop — stdin is unrecoverable.
                let data = serde_json::json!({ "io_error": e.to_string() });
                let resp = error_response(&serde_json::Value::Null, -32_700, "Parse error", &data);
                let _ = write_msg(&mut out, &resp).await;
                break;
            }
            ReadOutcome::ParseError { line, source } => {
                // The spec says id MUST be null when the request id can't be
                // determined (which is exactly the case for a non-JSON line).
                let data = serde_json::json!({
                    "parse_error": source.to_string(),
                    "line_preview": preview(&line),
                });
                let resp = error_response(&serde_json::Value::Null, -32_700, "Parse error", &data);
                let _ = write_msg(&mut out, &resp).await;
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = msg
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Notifications have no "id" — send no response.
        let is_notification = msg.get("id").is_none();

        let response = match method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name":    SERVER_NAME,
                        "version": SERVER_VERSION
                    }
                });
                ok_response(&id, &result)
            }

            "notifications/initialized" | "notifications/cancelled" => {
                // Notifications: consume and skip.
                continue;
            }

            "tools/list" => {
                let result = serde_json::json!({ "tools": tool_descriptors() });
                ok_response(&id, &result)
            }

            "tools/call" => {
                let params = msg.get("params").cloned().unwrap_or_default();
                let name = params["name"].as_str().unwrap_or("").to_string();
                let input = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));

                let call_result = dispatch_tool(&home, &name, &input).await;
                match call_result {
                    Ok(ref value) => ok_response(&id, &tool_result(value)),
                    Err(ref e) => cli_error_response(&id, e),
                }
            }

            "ping" => {
                let empty = serde_json::json!({});
                ok_response(&id, &empty)
            }

            _ => {
                if is_notification {
                    continue;
                }
                let data = serde_json::json!({ "method": method });
                error_response(&id, -32_601, "Method not found", &data)
            }
        };

        if !is_notification {
            let _ = write_msg(&mut out, &response).await;
        }
    }

    Ok(())
}

// ── tool dispatcher ───────────────────────────────────────────────────────────

async fn dispatch_tool(
    home: &std::path::Path,
    name: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, CliError> {
    match name {
        "famp_send" => tools::send::call(home, input).await,
        "famp_await" => tools::await_::call(home, input).await,
        "famp_inbox" => tools::inbox::call(home, input).await,
        "famp_peers" => tools::peers::call(home, input),
        other => Err(CliError::SendArgsInvalid {
            reason: format!("unknown tool '{other}'"),
        }),
    }
}
