#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

//! CLI-05 (v0.9): `famp await --as <name>` returns {"timeout":true} when
//! no message arrives before the deadline. Uses a real broker subprocess
//! spawned via BusClient's spawn-on-demand path.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{json, Value};

struct McpProc {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

impl McpProc {
    fn spawn(sock: &std::path::Path) -> Self {
        let mut child = Command::cargo_bin("famp")
            .unwrap()
            .args(["mcp"])
            .env("FAMP_BUS_SOCKET", sock)
            .env_remove("FAMP_HOME")
            .env_remove("FAMP_LOCAL_ROOT")
            .env_remove("FAMP_LOCAL_IDENTITY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut p = Self { child, stdin, stdout, next_id: 0 };
        let _ = p.rpc("initialize", &json!({}));
        p
    }

    fn rpc(&mut self, method: &str, params: &Value) -> Value {
        self.next_id += 1;
        let req = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": method,
            "params": params,
        }))
        .unwrap();
        writeln!(self.stdin, "{req}").unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or(Value::Null)
    }

    fn tool_call(&mut self, name: &str, args: &Value) -> Value {
        self.rpc("tools/call", &json!({ "name": name, "arguments": args }))
    }
}

impl Drop for McpProc {
    fn drop(&mut self) { let _ = self.child.kill(); }
}

#[test]
fn await_returns_timeout_when_no_message_arrives() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let mut proc = McpProc::spawn(&sock);

    // Register with listen:false so we can call famp_await directly
    // without the Stop hook interfering.
    let reg = proc.tool_call("famp_register", &json!({ "name": "waiter" }));
    let reg_body = &reg["result"]["content"][0]["text"];
    let reg_val: Value = serde_json::from_str(reg_body.as_str().unwrap_or("{}")).unwrap();
    assert_eq!(reg_val["active"], "waiter", "register failed: {reg}");

    // famp_await with 2s timeout — no sender, so it must time out.
    let start = Instant::now();
    let await_resp = proc.tool_call("famp_await", &json!({ "timeout_seconds": 2 }));
    let elapsed = start.elapsed();

    let body_str = await_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("{}");
    let body: Value = serde_json::from_str(body_str).unwrap_or(Value::Null);

    assert_eq!(
        body["timeout"], true,
        "expected timeout:true, got: {body}"
    );
    assert!(
        elapsed >= Duration::from_secs(1),
        "await returned too quickly ({elapsed:?}); should have blocked"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "await took too long ({elapsed:?})"
    );
}
