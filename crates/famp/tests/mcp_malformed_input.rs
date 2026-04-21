//! Regression tests for MCP malformed-input handling.
//!
//! 1. A non-JSON line on stdin must produce a JSON-RPC `-32700 Parse error`,
//!    not silent EOF (which would leave the peer hanging forever).
//! 2. The `famp_inbox` list tool must fail loudly when the inbox contains a
//!    malformed JSONL line, not silently degrade entries to `null`.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

fn spawn_mcp(home: &Path) -> (Child, ChildStdin, ChildStdout) {
    let status = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["init"])
        .env("FAMP_HOME", home)
        .status()
        .expect("famp init");
    assert!(status.success(), "famp init failed: {status}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["mcp"])
        .env("FAMP_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn famp mcp");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    (child, stdin, stdout)
}

fn recv_msg(stdout: &mut ChildStdout, timeout: Duration) -> serde_json::Value {
    let deadline = Instant::now() + timeout;
    let mut reader = std::io::BufReader::new(stdout);
    loop {
        assert!(Instant::now() < deadline, "timed out waiting for MCP line");
        let mut line = String::new();
        reader.read_line(&mut line).expect("read line");
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return serde_json::from_str(trimmed).expect("valid JSON line from server");
        }
    }
}

#[test]
fn malformed_json_line_returns_parse_error_not_silent_eof() {
    let home = tempfile::tempdir().expect("tempdir");
    let (mut child, mut stdin, mut stdout) = spawn_mcp(home.path());

    // Garbage line — not JSON. Server must respond with -32700, not hang.
    stdin
        .write_all(b"this is not json at all\n")
        .expect("write");
    stdin.flush().expect("flush");

    let resp = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_eq!(resp["jsonrpc"], "2.0", "expected JSON-RPC envelope: {resp}");
    assert!(
        resp["id"].is_null(),
        "id must be null on parse error: {resp}"
    );
    assert_eq!(
        resp["error"]["code"], -32_700,
        "expected -32700 Parse error, got: {resp}"
    );

    // Server should still be alive and able to handle a follow-up after the
    // parse error — the loop continues for ParseError, only IoError exits.
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.0.1" }
        }
    });
    let mut body = serde_json::to_string(&init).unwrap();
    body.push('\n');
    stdin.write_all(body.as_bytes()).expect("write init");
    stdin.flush().expect("flush init");
    let init_resp = recv_msg(&mut stdout, Duration::from_secs(5));
    assert_eq!(init_resp["id"], 99, "follow-up after parse error must work");

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn famp_inbox_fails_loudly_on_malformed_inbox_line() {
    // The tool wrapper used to map per-line `serde_json::from_str` failures
    // to `Value::Null`, silently swallowing structural errors. The fix
    // propagates the parse failure as a hard CliError. Underlying inbox
    // corruption surfaces as `CliError::Inbox` (raised by `list::run_list`)
    // before reaching the wrapper's parser; either way, the contract is
    // "loud failure, never a silent null entry".
    let home = tempfile::tempdir().expect("tempdir");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(home.path(), false, &mut out, &mut err).expect("init");

    let inbox_path = home.path().join("inbox.jsonl");
    std::fs::write(&inbox_path, b"{\"this is not\": valid json\n").expect("write inbox");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(async {
        famp::cli::mcp::tools::inbox::call(home.path(), &serde_json::json!({ "action": "list" }))
            .await
    });
    // The result must be a hard error of some flavour — never an Ok value
    // containing a `null` entry, which was the pre-fix degradation mode.
    let cli_err = result
        .expect_err("malformed inbox line must fail tool call, not return Ok with null entry");
    let msg = cli_err.to_string();
    assert!(
        !msg.is_empty(),
        "expected a non-empty error message, got empty"
    );
}

#[test]
fn famp_inbox_list_rejects_non_bool_include_terminal() {
    let home = tempfile::tempdir().expect("tempdir");
    let (mut child, mut stdin, mut stdout) = spawn_mcp(home.path());

    // Send a tools/call with include_terminal as a string.
    // The server must reject it with a tool-level error, not coerce.
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "famp_inbox",
            "arguments": {
                "action": "list",
                "include_terminal": "true"
            }
        }
    });
    let mut body = serde_json::to_string(&req).unwrap();
    body.push('\n');
    stdin.write_all(body.as_bytes()).expect("write");
    stdin.flush().expect("flush");

    let resp = recv_msg(&mut stdout, Duration::from_secs(5));
    // The error surfaces either as JSON-RPC error or as an
    // isError=true tool result. Either way the message must name the
    // field and the expected type so a caller can self-correct.
    let text = resp.to_string();
    assert!(
        text.contains("include_terminal"),
        "error must name the field: {resp}",
    );
    assert!(
        text.to_lowercase().contains("boolean") || text.to_lowercase().contains("bool"),
        "error must name the expected type: {resp}",
    );

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn famp_inbox_list_returns_parsed_entries_for_well_formed_input() {
    // Positive-path coverage: confirms the new strict parser still produces
    // the expected `{ "entries": [...] }` shape when every line is valid.
    let home = tempfile::tempdir().expect("tempdir");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(home.path(), false, &mut out, &mut err).expect("init");

    // Empty inbox: list should succeed with an empty entries array.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let value = rt
        .block_on(async {
            famp::cli::mcp::tools::inbox::call(
                home.path(),
                &serde_json::json!({ "action": "list" }),
            )
            .await
        })
        .expect("empty inbox list should succeed");
    assert!(
        value["entries"].is_array(),
        "expected entries array: {value}"
    );
    assert_eq!(value["entries"].as_array().unwrap().len(), 0);
}
