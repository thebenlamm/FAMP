//! Pre-registration tool gating — every messaging tool must refuse
//! with `not_registered` before `famp_register` succeeds in the
//! current session. Spec lines 165–174 (the explicit "Failure
//! behavior" list); CONTEXT.md "Pre-registration error".

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

fn send_msg(stdin: &mut ChildStdin, msg: &serde_json::Value) {
    let mut body = serde_json::to_string(msg).unwrap();
    body.push('\n');
    stdin.write_all(body.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

fn recv_msg<R: std::io::Read>(reader: &mut BufReader<R>, timeout: Duration) -> serde_json::Value {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for MCP response"
        );
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap();
        assert!(n > 0, "stdout closed unexpectedly");
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return serde_json::from_str(trimmed).unwrap();
        }
    }
}

fn spawn_unbound() -> (
    Child,
    ChildStdin,
    BufReader<std::process::ChildStdout>,
    tempfile::TempDir,
) {
    // After 01-03 the server starts unbound by default; no env-var seam needed.
    // An empty local_root (no agents sub-dir) means no identity is registrable,
    // but the server starts fine — the gating test never calls famp_register.
    let local_root = tempfile::tempdir().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["mcp"])
        .env("FAMP_LOCAL_ROOT", local_root.path())
        .env_remove("FAMP_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn famp mcp");
    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    (child, stdin, stdout, local_root)
}

fn assert_not_registered(resp: &serde_json::Value, tool_name: &str) {
    let err = resp.get("error").unwrap_or_else(|| {
        panic!("{tool_name}: expected error response, got: {resp}")
    });
    assert_eq!(err["code"].as_i64().unwrap(), -32000, "{tool_name}: code");
    let kind = err["data"]["famp_error_kind"].as_str().unwrap_or("");
    assert_eq!(kind, "not_registered", "{tool_name}: famp_error_kind");
    let hint = err["data"]["details"]["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains("famp_register"),
        "{tool_name}: hint mentions famp_register, got: {hint}"
    );
    // Pin the exact hint to catch silent drift.
    assert_eq!(
        hint,
        "Call famp_register with an identity name first. Use famp_whoami to inspect current binding.",
        "{tool_name}: hint string drift",
    );
}

#[test]
fn messaging_tools_refuse_before_register() {
    let (mut child, mut stdin, mut stdout, _home) = spawn_unbound();

    // initialize handshake
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
        }),
    );
    let init_resp = recv_msg(&mut stdout, Duration::from_secs(5));
    assert!(init_resp.get("result").is_some(), "initialize: {init_resp}");

    // famp_send
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "famp_send", "arguments": { "peer": "x", "mode": "new_task" } }
        }),
    );
    let r = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_not_registered(&r, "famp_send");

    // famp_await
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "famp_await", "arguments": { "timeout_seconds": 1 } }
        }),
    );
    let r = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_not_registered(&r, "famp_await");

    // famp_inbox
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "famp_inbox", "arguments": { "action": "list" } }
        }),
    );
    let r = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_not_registered(&r, "famp_inbox");

    // famp_peers
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "famp_peers", "arguments": { "action": "list" } }
        }),
    );
    let r = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_not_registered(&r, "famp_peers");

    drop(stdin);
    let _ = child.wait();
}
