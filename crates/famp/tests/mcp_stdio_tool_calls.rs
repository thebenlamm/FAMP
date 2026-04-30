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
    /// After 01-03 this is the `FAMP_LOCAL_ROOT` (backing-store root), not the
    /// per-identity agent home. Use `agent_home()` when you need the path that
    /// was passed to `famp init` (i.e. `local_root/agents/alice/`).
    home: tempfile::TempDir,
}

impl McpHarness {
    /// Spawn `famp mcp` with a fresh `local_root` containing one pre-initialized
    /// identity 'alice', then perform the initialize handshake and register as
    /// alice so that the existing test bodies (which use the four messaging
    /// tools) keep working without modification.
    fn new() -> Self {
        let local_root = tempfile::tempdir().expect("tempdir");
        let agent_home = local_root.path().join("agents").join("alice");
        std::fs::create_dir_all(&agent_home).expect("create agent dir");
        let status = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["init"])
            .env("FAMP_HOME", &agent_home)
            .status()
            .expect("famp init");
        assert!(status.success(), "famp init failed: {status}");

        // v0.9: isolate the broker per harness instance.
        let sock = local_root.path().join("bus.sock");
        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["mcp"])
            .env("FAMP_LOCAL_ROOT", local_root.path())
            .env("FAMP_BUS_SOCKET", &sock)
            .env_remove("FAMP_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn famp mcp");

        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let mut h = Self {
            child,
            stdin,
            stdout,
            home: local_root,
        };

        // Perform the MCP initialize handshake and register as alice so that
        // callers who invoke h.initialize() again will just get a second
        // (idempotent) initialize response, and the session is already bound.
        send_msg(
            &mut h.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
            }),
        );
        let _ = recv_msg(&mut h.stdout, Duration::from_secs(5));

        send_msg(
            &mut h.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "famp_register", "arguments": { "identity": "alice" } }
            }),
        );
        let reg = recv_msg(&mut h.stdout, Duration::from_secs(5));
        assert!(
            reg.get("result").is_some(),
            "register as alice failed: {reg}"
        );

        h
    }

    /// Spawn `famp mcp` reusing an already-initialized `TempDir` as the
    /// backing store for identity 'alice'. The provided `home` dir is
    /// placed (via symlink) at `local_root/agents/alice/` so the MCP
    /// server can resolve the identity without copying files.
    /// The caller retains ownership of `home` to keep it alive.
    ///
    /// Returns `(child, stdin, stdout)` without performing the initialize
    /// handshake — the caller is responsible for initialize + `famp_register`.
    fn with_home(home: &tempfile::TempDir) -> (Child, ChildStdin, ChildStdout) {
        // Build a local_root with alice's home symlinked at agents/alice/.
        let local_root = tempfile::tempdir().expect("tempdir for local_root");
        let agents_dir = local_root.path().join("agents");
        std::fs::create_dir_all(&agents_dir).expect("create agents dir");
        // Symlink local_root/agents/alice -> home so the listener and MCP
        // server share the same inode tree without file copying.
        #[cfg(unix)]
        std::os::unix::fs::symlink(home.path(), agents_dir.join("alice"))
            .expect("symlink alice -> home");

        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["mcp"])
            .env("FAMP_LOCAL_ROOT", local_root.path())
            .env_remove("FAMP_HOME")
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
        // Keep local_root alive by leaking it — it will be cleaned up when the
        // test process exits.
        std::mem::forget(local_root);
        (child, stdin, stdout)
    }

    fn send(&mut self, msg: &serde_json::Value) {
        send_msg(&mut self.stdin, msg);
    }

    fn recv(&mut self) -> serde_json::Value {
        recv_msg(&mut self.stdout, Duration::from_secs(10))
    }

    /// Returns the agent home (`local_root/agents/alice/`) — the directory
    /// where inbox, tasks, peers.toml and key.ed25519 live.
    fn home(&self) -> std::path::PathBuf {
        self.home.path().join("agents").join("alice")
    }

    /// Perform the MCP initialize handshake.
    ///
    /// Note: `McpHarness::new()` already performs the initialize + register
    /// sequence during construction. Calling this again is idempotent (the
    /// MCP server accepts re-initialize) and is kept here so existing test
    /// bodies that call `h.initialize()` continue to compile.
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

    /// Read the self pubkey (base64url) from the agent's `key.ed25519`.
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
            &self.home(),
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

/// `famp mcp` responds to initialize and advertises exactly eight tools
/// after Phase 02 plan 02-09 (the v0.9 surface adds `famp_join` and
/// `famp_leave` to the v0.8 six).
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

    assert_eq!(names.len(), 8, "expected exactly 8 tools, got: {names:?}");
    for expected in &[
        "famp_send",
        "famp_await",
        "famp_inbox",
        "famp_peers",
        "famp_register",
        "famp_whoami",
        "famp_join",
        "famp_leave",
    ] {
        assert!(
            names.contains(expected),
            "missing tool {expected}, got: {names:?}"
        );
    }
}

/// The `famp_send` tool's `body` parameter description must explicitly call
/// out that it is REQUIRED for `new_task` mode (so MCP clients understand
/// that the title field alone won't carry content). Regression guard for
/// the body-loss class of bugs (see quick task 260424-7z5).
#[test]
fn mcp_famp_send_body_description_flags_required_for_new_task() {
    let mut h = McpHarness::new();
    h.initialize();

    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let resp = h.recv();
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let send_tool = tools
        .iter()
        .find(|t| t["name"] == "famp_send")
        .expect("famp_send tool present");
    let body_desc = send_tool["inputSchema"]["properties"]["body"]["description"]
        .as_str()
        .expect("body.description string");

    assert!(
        body_desc.contains("REQUIRED for new_task"),
        "body description must flag REQUIRED for new_task, got: {body_desc:?}"
    );
    assert!(
        !body_desc.contains("Message content"),
        "body description must not regress to generic 'Message content', got: {body_desc:?}"
    );
}

/// `famp_send` with mode `new_task` returns a 36-char `task_id` UUID and a `state`.
///
/// Starts an in-process `famp listen` daemon on an ephemeral port so that
/// `famp_send` can actually POST the envelope and receive an HTTP 200.
#[test]
#[ignore = "Phase 04 (v0.9 federation deletion): tests v0.8 HTTPS-via-listen \
            from MCP. v0.9 broker-backed coverage already lives in \
            crates/famp/tests/mcp_bus_e2e.rs (test_mcp_bus_e2e). Phase 04 will \
            delete or migrate this file with the v0.8 CLI surface."]
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

    // Register as alice before using any messaging tool (required after 01-03).
    send_msg(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "famp_register", "arguments": { "identity": "alice" } }
        }),
    );
    let reg_resp = recv_msg(&mut stdout, Duration::from_secs(5));
    assert!(
        reg_resp.get("result").is_some(),
        "famp_register failed: {reg_resp}"
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
///
/// SUPERSEDED IN V0.9: plan 02-09 reshapes `famp_peers` to return
/// `{ online: [name, ...] }` from broker memory, NOT
/// `{ peers: [{ alias, ... }] }` from peers.toml. The peer-add CLI path
/// is also removed (v0.9 has no `add_peer` since federation isn't local).
/// A v0.9-shaped equivalent test belongs in the broker integration suite.
#[test]
#[ignore = "Phase 04 (v0.9 federation deletion): v0.8 peers.toml fixture shape; \
            v0.9 famp_peers returns the broker-memory `online` list (covered by \
            mcp_bus_e2e.rs). Phase 04 will delete this with the v0.8 peer-file \
            surface."]
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
    let parsed: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|_| panic!("tool output not JSON: {text}"));
    parsed["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("no entries array: {parsed}"))
        .clone()
}

#[test]
#[ignore = "Phase 04 (v0.9 federation deletion): v0.8 file-fixture test (writes \
            inbox.jsonl directly). v0.9 broker reads from in-memory mailbox state; \
            broker-driven E2E coverage already lives in mcp_bus_e2e.rs. Phase 04 \
            will delete this with the v0.8 file-fixture surface."]
fn famp_inbox_list_filters_terminal_by_default() {
    let mut h = McpHarness::new();
    seed_filter_fixture(&h.home());
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
#[ignore = "Phase 04 (v0.9 federation deletion): v0.8 file-fixture test (writes \
            inbox.jsonl directly). v0.9 broker reads from in-memory mailbox state; \
            broker-driven E2E coverage already lives in mcp_bus_e2e.rs. Phase 04 \
            will delete this with the v0.8 file-fixture surface."]
fn famp_inbox_list_include_terminal_true_returns_all() {
    let mut h = McpHarness::new();
    seed_filter_fixture(&h.home());
    h.initialize();

    let resp = call_inbox_list(&mut h, Some(true));
    let entries = entries_from_response(&resp);
    assert_eq!(entries.len(), 4, "include_terminal=true: {resp}");

    drop(h);
}

/// Calling `famp_send` with an unknown peer alias returns `famp_error_kind == "peer_not_found"`.
#[test]
#[ignore = "Phase 04 (v0.9 federation deletion): v0.8 `peer_not_found` failure mode \
            is replaced by D-10 `not_registered_hint` on the v0.9 bus path. The \
            v0.9 negative path is covered via mcp_error_kind_exhaustive.rs and the \
            broker proxy semantics tests. Phase 04 will delete this with the v0.8 \
            CLI surface."]
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
