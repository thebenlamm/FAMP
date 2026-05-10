#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

struct McpProc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpProc {
    fn spawn(sock: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .arg("mcp")
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
        let mut proc = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        let _ = proc.rpc("initialize", &json!({}));
        proc
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
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }

    fn tool_call(&mut self, name: &str, args: &Value) -> Value {
        self.rpc("tools/call", &json!({ "name": name, "arguments": args }))
    }

    fn ok_text_json(reply: &Value) -> Value {
        assert!(reply.get("error").is_none(), "tool failed: {reply}");
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }
}

impl Drop for McpProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn wait_reply_finds_existing_terminal_reply_that_await_misses() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let mut alice = McpProc::spawn(&sock);
    let mut bob = McpProc::spawn(&sock);

    let reg = alice.tool_call("famp_register", &json!({ "name": "alice" }));
    assert_eq!(McpProc::ok_text_json(&reg)["active"], "alice");
    let reg = bob.tool_call("famp_register", &json!({ "name": "bob" }));
    assert_eq!(McpProc::ok_text_json(&reg)["active"], "bob");

    let send = alice.tool_call(
        "famp_send",
        &json!({
            "peer": "bob",
            "mode": "new_task",
            "title": "please answer",
        }),
    );
    let send_body = McpProc::ok_text_json(&send);
    let task_id = send_body["task_id"].as_str().unwrap();

    let reply = bob.tool_call(
        "famp_send",
        &json!({
            "peer": "alice",
            "mode": "terminal",
            "task_id": task_id,
            "body": "done",
        }),
    );
    McpProc::ok_text_json(&reply);

    let await_out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args([
            "await",
            "--as",
            "alice",
            "--task",
            task_id,
            "--timeout",
            "1s",
        ])
        .env("FAMP_BUS_SOCKET", &sock)
        .env_remove("FAMP_HOME")
        .env_remove("FAMP_LOCAL_ROOT")
        .env_remove("FAMP_LOCAL_IDENTITY")
        .output()
        .unwrap();
    assert!(
        await_out.status.success(),
        "await failed: stderr={}",
        String::from_utf8_lossy(&await_out.stderr)
    );
    let await_json: Value = serde_json::from_slice(&await_out.stdout).unwrap();
    assert_eq!(await_json["timeout"], true);
    assert!(
        await_json["diagnostic"]
            .as_str()
            .unwrap_or_default()
            .contains("already present in the inbox"),
        "diagnostic should explain missed existing reply: {await_json}"
    );

    let wait_reply_out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args([
            "wait-reply",
            "--as",
            "alice",
            "--task",
            task_id,
            "--timeout",
            "1s",
        ])
        .env("FAMP_BUS_SOCKET", &sock)
        .env_remove("FAMP_HOME")
        .env_remove("FAMP_LOCAL_ROOT")
        .env_remove("FAMP_LOCAL_IDENTITY")
        .output()
        .unwrap();
    assert!(
        wait_reply_out.status.success(),
        "wait-reply failed: stderr={}",
        String::from_utf8_lossy(&wait_reply_out.stderr)
    );
    let envelope: Value = serde_json::from_slice(&wait_reply_out.stdout).unwrap();
    assert_eq!(envelope["causality"]["ref"], task_id);
    assert!(
        envelope["body"].to_string().contains("done"),
        "wait-reply returned wrong envelope: {envelope}"
    );
}
