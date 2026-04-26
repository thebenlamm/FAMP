//! Integration tests for `famp_register` and `famp_whoami` MCP tools.
//! Spec: docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md
//! Phase plan: .planning/phases/01-session-bound-mcp-identity/01-03-PLAN.md

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
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
        assert!(std::time::Instant::now() < deadline, "timed out");
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap();
        assert!(n > 0, "stdout closed unexpectedly");
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return serde_json::from_str(trimmed).unwrap();
        }
    }
}

/// Initialize an agent home at `local_root/agents/<name>` by running
/// `famp init` with `FAMP_HOME` set to that subdir. Returns the home path.
fn init_agent(local_root: &Path, name: &str) -> std::path::PathBuf {
    let home = local_root.join("agents").join(name);
    std::fs::create_dir_all(&home).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["init"])
        .env("FAMP_HOME", &home)
        .status()
        .expect("famp init");
    assert!(status.success(), "famp init for '{name}' failed");
    home
}

struct Harness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    _local_root: tempfile::TempDir,
    id_counter: i64,
}

impl Harness {
    /// Build a `local_root` with the named agents pre-initialized, then
    /// spawn an unbound `famp mcp` against it.
    fn with_agents(agents: &[&str]) -> Self {
        let local_root = tempfile::tempdir().unwrap();
        for name in agents {
            init_agent(local_root.path(), name);
        }

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
        let stdout_raw = child.stdout.take().unwrap();
        let stdout = BufReader::new(stdout_raw);

        // initialize handshake
        let mut h = Self {
            child,
            stdin,
            stdout,
            _local_root: local_root,
            id_counter: 1,
        };
        h.call("initialize", &serde_json::json!({}));
        h
    }

    #[allow(clippy::missing_const_for_fn)]
    fn next_id(&mut self) -> i64 {
        self.id_counter += 1;
        self.id_counter
    }

    fn call(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        let id = self.next_id();
        send_msg(
            &mut self.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": id, "method": method, "params": params
            }),
        );
        recv_msg(&mut self.stdout, Duration::from_secs(5))
    }

    fn tool_call(&mut self, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
        self.call(
            "tools/call",
            &serde_json::json!({
                "name": name, "arguments": arguments
            }),
        )
    }

    /// Pull the parsed-out content text from a successful tool/call response.
    fn ok_content(resp: &serde_json::Value) -> serde_json::Value {
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("expected ok tool/call, got: {resp}"));
        serde_json::from_str(text).expect("inner JSON parse")
    }

    fn error_kind(resp: &serde_json::Value) -> String {
        resp["error"]["data"]["famp_error_kind"]
            .as_str()
            .unwrap_or("")
            .to_string()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn register_valid_identity_succeeds() {
    let mut h = Harness::with_agents(&["alice"]);
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let body = Harness::ok_content(&r);
    assert_eq!(body["identity"], "alice");
    assert_eq!(body["source"], "explicit");
    assert!(
        body["home"].as_str().unwrap().ends_with("/agents/alice"),
        "home path should end with /agents/alice, got: {}",
        body["home"]
    );

    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert_eq!(wb["identity"], "alice");
    assert_eq!(wb["source"], "explicit");
}

#[test]
fn register_invalid_name_returns_invalid_identity_name() {
    let mut h = Harness::with_agents(&[]);
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "foo bar" }));
    assert_eq!(Harness::error_kind(&r), "invalid_identity_name");
}

#[test]
fn register_with_empty_string_returns_invalid_identity_name() {
    let mut h = Harness::with_agents(&[]);
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "" }));
    assert_eq!(Harness::error_kind(&r), "invalid_identity_name");
}

#[test]
fn register_unknown_identity_returns_unknown_identity() {
    let mut h = Harness::with_agents(&["alice"]); // bob is NOT initialized
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));
    assert_eq!(Harness::error_kind(&r), "unknown_identity");
}

#[test]
fn register_idempotent_same_identity() {
    let mut h = Harness::with_agents(&["alice"]);
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let r = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    assert!(r.get("result").is_some(), "second register must succeed: {r}");
    let body = Harness::ok_content(&r);
    assert_eq!(body["identity"], "alice");
}

#[test]
fn register_replaces_with_different_identity() {
    let mut h = Harness::with_agents(&["alice", "bob"]);
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "alice" }));
    let _ = h.tool_call("famp_register", &serde_json::json!({ "identity": "bob" }));
    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert_eq!(wb["identity"], "bob");
}

#[test]
fn whoami_unregistered_returns_null() {
    let mut h = Harness::with_agents(&[]);
    let w = h.tool_call("famp_whoami", &serde_json::json!({}));
    let wb = Harness::ok_content(&w);
    assert!(wb["identity"].is_null(), "expected null, got {wb}");
    assert_eq!(wb["source"], "unregistered");
}

#[test]
fn tools_list_returns_six_tools() {
    let mut h = Harness::with_agents(&[]);
    let r = h.call("tools/list", &serde_json::json!({}));
    let tools = r["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names.len(), 6, "expected 6 tools, got: {names:?}");
    for expected in [
        "famp_send",
        "famp_await",
        "famp_inbox",
        "famp_peers",
        "famp_register",
        "famp_whoami",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool: {expected}; got {names:?}"
        );
    }
}
