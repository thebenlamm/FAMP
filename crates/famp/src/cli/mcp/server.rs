//! Stdio MCP JSON-RPC server — newline-delimited JSON.
//!
//! ## Wire format
//!
//! Claude Code's stdio transport uses newline-delimited JSON (NDJSON):
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
                    "body":    { "type": "string", "description": "Message content" }
                },
                "required": ["peer", "mode"]
            }
        },
        {
            "name": "famp_await",
            "description": "Wait for a new message to arrive. Use task_id to wait for a reply to a specific task you sent.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timeout_seconds": { "type": "integer", "description": "Timeout in seconds (default 30)" },
                    "task_id":         { "type": "string",  "description": "Wait for reply to this specific task" }
                }
            }
        },
        {
            "name": "famp_inbox",
            "description": "List received messages. Each entry has a 'task_id' — use that task_id with famp_send (mode=deliver or terminal) to reply.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "ack"], "description": "list=show messages, ack=mark as processed" },
                    "since":  { "type": "integer", "description": "Byte offset to start from (default 0)" },
                    "offset": { "type": "integer", "description": "Byte offset to ack up to" }
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

/// Read one newline-delimited JSON-RPC message from a buffered stdin.
/// Returns `None` on EOF.
async fn read_msg<R>(reader: &mut BufReader<R>) -> Option<serde_json::Value>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut line = String::new();
    let n = reader.read_line(&mut line).await.ok()?;
    if n == 0 {
        return None; // EOF
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        // Skip empty lines, try again
        return Box::pin(read_msg(reader)).await;
    }
    serde_json::from_str(trimmed).ok()
}

// ── main server loop ──────────────────────────────────────────────────────────

/// Run the stdio MCP server until stdin is closed.
pub async fn run(home: PathBuf) -> Result<(), CliError> {
    let home = Arc::new(home);
    let mut reader = BufReader::new(stdin());
    let mut out = stdout();

    while let Some(msg) = read_msg(&mut reader).await {
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
