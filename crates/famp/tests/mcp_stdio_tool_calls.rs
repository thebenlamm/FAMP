//! Integration tests for the `famp mcp` stdio JSON-RPC server.
//!
//! Each test spawns `famp mcp` as a subprocess, sends newline-delimited
//! JSON-RPC requests over stdin, and reads NDJSON responses from stdout.
//! Tests are time-bounded to 10 s per read.

// Integration test binaries inherit all of famp's transitive deps.
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use base64::Engine as _;
use famp::FampSigningKey;

// ── frame helpers ─────────────────────────────────────────────────────────────

/// Write one newline-delimited JSON-RPC message to `stdin`.
fn send_msg(stdin: &mut ChildStdin, msg: &serde_json::Value) {
    let mut body = serde_json::to_string(msg).expect("serialize");
    body.push('\n');
    stdin.write_all(body.as_bytes()).expect("write to stdin");
    stdin.flush().expect("flush stdin");
}

/// Read one newline-delimited JSON-RPC message from `stdout`.
/// Panics if no complete message arrives within `timeout`.
fn recv_msg(stdout: &mut ChildStdout, timeout: Duration) -> serde_json::Value {
    use std::io::BufRead;
    let deadline = std::time::Instant::now() + timeout;
    let mut reader = std::io::BufReader::new(stdout);

    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for MCP response"
        );
        let mut line = String::new();
        reader.read_line(&mut line).expect("read line");
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return serde_json::from_str(trimmed).expect("parse JSON line");
        }
    }
}

// ── harness ───────────────────────────────────────────────────────────────────

struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    home: tempfile::TempDir,
}

impl McpHarness {
    /// Spawn `famp mcp` with a fresh initialized FAMP home.
    fn new() -> Self {
        let home = tempfile::tempdir().expect("tempdir");
        let status = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["init"])
            .env("FAMP_HOME", home.path())
            .status()
            .expect("famp init");
        assert!(status.success(), "famp init failed: {status}");

        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["mcp"])
            .env("FAMP_HOME", home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn famp mcp");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        Self {
            child,
            stdin,
            stdout,
            home,
        }
    }

    /// Spawn `famp mcp` reusing an already-initialized `TempDir` home.
    ///
    /// The caller retains ownership of the `TempDir` to keep it alive for the
    /// duration of the test; the harness holds only the path string.
    fn with_home(home: &tempfile::TempDir) -> (Child, ChildStdin, ChildStdout) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["mcp"])
            .env("FAMP_HOME", home.path())
            // The MCP test exercises the real `famp send` code path, which
            // hits an unknown TLS leaf on first contact. Production now
            // refuses that without an explicit opt-in; tests opt in.
            .env("FAMP_TOFU_BOOTSTRAP", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn famp mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        (child, stdin, stdout)
    }

    fn send(&mut self, msg: &serde_json::Value) {
        send_msg(&mut self.stdin, msg);
    }

    fn recv(&mut self) -> serde_json::Value {
        recv_msg(&mut self.stdout, Duration::from_secs(10))
    }

    fn home(&self) -> &Path {
        self.home.path()
    }

    /// Perform the MCP initialize handshake.
    fn initialize(&mut self) {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "0.0.1" }
            }
        }));
        let resp = self.recv();
        assert_eq!(resp["jsonrpc"], "2.0", "initialize response: {resp}");
        assert!(
            resp["result"].is_object(),
            "initialize must return result: {resp}"
        );

        // Send initialized notification (no response expected).
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    /// Read the self pubkey (base64url) from `FAMP_HOME/key.ed25519`.
    fn self_pubkey_b64(&self) -> String {
        let key_bytes = std::fs::read(self.home().join("key.ed25519")).expect("key file");
        let key: [u8; 32] = key_bytes.try_into().expect("32 bytes");
        let sk = FampSigningKey::from_bytes(key);
        let vk = sk.verifying_key();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(vk.as_bytes())
    }

    /// Add a peer to `peers.toml` via the CLI entry point.
    fn add_peer(&self, alias: &str, endpoint: &str, pubkey_b64: &str, principal: Option<&str>) {
        famp::cli::peer::add::run_add_at(
            self.home(),
            alias.to_string(),
            endpoint.to_string(),
            pubkey_b64.to_string(),
            principal.map(str::to_string),
        )
        .expect("peer add");
    }
}

impl Drop for McpHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

/// `famp mcp` responds to initialize and advertises exactly four tools.
#[test]
fn mcp_initialize_lists_four_tools() {
    let mut h = McpHarness::new();
    h.initialize();

    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let resp = h.recv();
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();

    assert_eq!(names.len(), 4, "expected exactly 4 tools, got: {names:?}");
    for expected in &["famp_send", "famp_await", "famp_inbox", "famp_peers"] {
        assert!(
            names.contains(expected),
            "missing tool {expected}, got: {names:?}"
        );
    }
}

/// `famp_send` with mode `new_task` returns a 36-char `task_id` UUID and a `state`.
///
/// Starts an in-process `famp listen` daemon on an ephemeral port so that
/// `famp_send` can actually POST the envelope and receive an HTTP 200.
#[test]
fn mcp_famp_send_new_task_returns_structured() {
    // Build a multi-thread tokio runtime to drive the async listener while the
    // blocking MCP subprocess I/O runs on the main test thread.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    // Create + initialize a FAMP home via conversation_harness.
    let home_dir = common::conversation_harness::setup_home();

    // Spawn the in-process listener on an ephemeral port.
    let (addr, listener_handle, shutdown_tx) = rt.block_on(
        common::conversation_harness::spawn_listener(home_dir.path()),
    );

    // Register the peer pointing at the real bound address.
    common::conversation_harness::add_self_peer(home_dir.path(), "self", addr);

    // Spawn the `famp mcp` subprocess sharing the same FAMP_HOME.
    let (mut child, mut stdin, mut stdout) = McpHarness::with_home(&home_dir);

    // MCP handshake.
    let init_msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "0.0.1" }
        }
    });
    send_msg(&mut stdin, &init_msg);
    let init_resp = recv_msg(&mut stdout, Duration::from_secs(10));
    assert_eq!(
        init_resp["jsonrpc"], "2.0",
        "initialize response: {init_resp}"
    );
    assert!(
        init_resp["result"].is_object(),
        "initialize must return result: {init_resp}"
    );
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    );

    // Call famp_send new_task.
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "famp_send",
                "arguments": {
                    "peer": "self",
                    "mode": "new_task",
                    "title": "hello from mcp test"
                }
            }
        }),
    );
    let resp = recv_msg(&mut stdout, Duration::from_secs(15));

    // Clean up subprocess and listener before asserting.
    let _ = child.kill();
    let _ = child.wait();
    rt.block_on(common::conversation_harness::stop_listener(
        listener_handle,
        shutdown_tx,
    ));

    assert!(resp["error"].is_null(), "unexpected error: {resp}");
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let result: serde_json::Value = serde_json::from_str(text).expect("parse result text");
    let task_id = result["task_id"].as_str().expect("task_id field");
    assert_eq!(
        task_id.len(),
        36,
        "task_id should be 36-char UUID, got: {task_id}"
    );
    assert!(result["state"].is_string(), "state field missing: {result}");
}

/// `famp_peers` list returns the peers that were added via the CLI.
#[test]
fn mcp_famp_peers_list_returns_entries() {
    let mut h = McpHarness::new();
    let pubkey = h.self_pubkey_b64();
    h.add_peer("alice", "https://127.0.0.1:9443", &pubkey, None);
    h.initialize();

    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "famp_peers",
            "arguments": { "action": "list" }
        }
    }));
    let resp = h.recv();
    assert!(resp["error"].is_null(), "unexpected error: {resp}");
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let result: serde_json::Value = serde_json::from_str(text).expect("parse result text");
    let peers = result["peers"].as_array().expect("peers array");
    assert_eq!(peers.len(), 1, "expected 1 peer, got: {peers:?}");
    assert_eq!(peers[0]["alias"], "alice");
}

// ── famp_inbox action=list: include_terminal round-trip ──────────────────────
//
// Spec 2026-04-20: `famp_inbox` action=list filters terminal tasks
// unless include_terminal=true. These two tests assert the MCP
// surface, driving the binary through its real stdio JSON-RPC loop.

const TID_ACTIVE_MCP: &str = "01913000-0000-7000-8000-0000000000f1";
const TID_DONE_MCP: &str = "01913000-0000-7000-8000-0000000000f2";

/// Write a four-entry inbox fixture (two per task) + matching taskdir
/// records (one active, one terminal) into `home`.
fn seed_filter_fixture(home: &Path) {
    use famp_taskdir::{TaskDir, TaskRecord};
    let entries = [
        serde_json::json!({
            "id": TID_ACTIVE_MCP, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE_MCP },
            "body": { "text": "active-request" },
        }),
        serde_json::json!({
            "id": "01913000-0000-7000-8000-0000000000e1", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE_MCP },
            "body": { "text": "active-deliver" },
        }),
        serde_json::json!({
            "id": TID_DONE_MCP, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE_MCP },
            "body": { "text": "done-request" },
        }),
        serde_json::json!({
            "id": "01913000-0000-7000-8000-0000000000e2", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE_MCP },
            "body": { "text": "done-deliver" },
        }),
    ];
    let mut body = Vec::<u8>::new();
    for e in &entries {
        body.extend_from_slice(serde_json::to_string(e).unwrap().as_bytes());
        body.push(b'\n');
    }
    std::fs::write(home.join("inbox.jsonl"), body).unwrap();

    let dir = TaskDir::open(home.join("tasks")).unwrap();
    dir.create(&TaskRecord::new_requested(
        TID_ACTIVE_MCP.to_string(),
        "a".to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    ))
    .unwrap();
    let mut done = TaskRecord::new_requested(
        TID_DONE_MCP.to_string(),
        "a".to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    );
    done.state = "COMPLETED".to_string();
    done.terminal = true;
    dir.create(&done).unwrap();
}

fn call_inbox_list(h: &mut McpHarness, include_terminal: Option<bool>) -> serde_json::Value {
    let mut args = serde_json::json!({ "action": "list" });
    if let Some(b) = include_terminal {
        args["include_terminal"] = serde_json::Value::Bool(b);
    }
    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": "famp_inbox", "arguments": args }
    }));
    h.recv()
}

/// Extract the `entries` array from a tools/call result. The MCP
/// wrapper returns tool output in result.content[0].text as a JSON
/// string; parse it back.
fn entries_from_response(resp: &serde_json::Value) -> Vec<serde_json::Value> {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text in response: {resp}"));
    let parsed: serde_json::Value = serde_json::from_str(text)
        .unwrap_or_else(|_| panic!("tool output not JSON: {text}"));
    parsed["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("no entries array: {parsed}"))
        .clone()
}

#[test]
fn famp_inbox_list_filters_terminal_by_default() {
    let mut h = McpHarness::new();
    seed_filter_fixture(h.home());
    h.initialize();

    let resp = call_inbox_list(&mut h, None);
    let entries = entries_from_response(&resp);
    assert_eq!(entries.len(), 2, "default filter: {resp}");
    for e in &entries {
        assert_eq!(e["task_id"].as_str().unwrap(), TID_ACTIVE_MCP);
    }

    drop(h);
}

#[test]
fn famp_inbox_list_include_terminal_true_returns_all() {
    let mut h = McpHarness::new();
    seed_filter_fixture(h.home());
    h.initialize();

    let resp = call_inbox_list(&mut h, Some(true));
    let entries = entries_from_response(&resp);
    assert_eq!(entries.len(), 4, "include_terminal=true: {resp}");

    drop(h);
}

/// Calling `famp_send` with an unknown peer alias returns `famp_error_kind == "peer_not_found"`.
#[test]
fn mcp_error_has_famp_error_kind() {
    let mut h = McpHarness::new();
    h.initialize();

    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "famp_send",
            "arguments": {
                "peer": "nobody",
                "mode": "new_task",
                "title": "hello"
            }
        }
    }));
    let resp = h.recv();
    // Tool errors are returned as JSON-RPC error responses.
    let error = &resp["error"];
    assert!(error.is_object(), "expected error object: {resp}");
    let kind = error["data"]["famp_error_kind"]
        .as_str()
        .expect("famp_error_kind in error.data");
    assert_eq!(kind, "peer_not_found", "wrong error kind: {resp}");
}
