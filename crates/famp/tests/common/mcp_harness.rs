//! Shared MCP subprocess test harness.
//!
//! Provides the [`Harness`] struct and helpers for spawning `famp mcp`
//! subprocesses in integration tests. Extracted from `mcp_register_whoami.rs`
//! so that both that file and `mcp_session_bound_e2e.rs` can share a
//! single authoritative implementation.
//!
//! The `with_agents` constructor creates a fresh `FAMP_LOCAL_ROOT` with the
//! named agents pre-initialized and spawns an unbound `famp mcp` against it.
//!
//! The `with_local_root` constructor accepts an existing `local_root` path
//! (e.g. one shared with a `TwoDaemonsLocal` harness) so two MCP servers can
//! operate side-by-side against the same backing store.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

/// Write one newline-delimited JSON-RPC message to `stdin`.
pub fn send_msg(stdin: &mut ChildStdin, msg: &serde_json::Value) {
    let mut body = serde_json::to_string(msg).unwrap();
    body.push('\n');
    stdin.write_all(body.as_bytes()).unwrap();
    stdin.flush().unwrap();
}

/// Read one newline-delimited JSON-RPC message from `stdout` within `timeout`.
pub fn recv_msg<R: std::io::Read>(reader: &mut BufReader<R>, timeout: Duration) -> serde_json::Value {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for MCP response");
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
pub fn init_agent(local_root: &Path, name: &str) -> PathBuf {
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

/// MCP subprocess harness. Spawns `famp mcp` against a `FAMP_LOCAL_ROOT`
/// and exposes helpers for JSON-RPC call/response.
pub struct Harness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    /// Optional owned `TempDir` — held alive for the duration of the test.
    _local_root: Option<tempfile::TempDir>,
    id_counter: i64,
}

impl Harness {
    /// Build a `local_root` with the named agents pre-initialized, then
    /// spawn an unbound `famp mcp` against it.
    pub fn with_agents(agents: &[&str]) -> Self {
        let local_root = tempfile::tempdir().unwrap();
        for name in agents {
            init_agent(local_root.path(), name);
        }
        let local_root_path = local_root.path().to_path_buf();
        Self::with_local_root(&local_root_path, Some(local_root))
    }

    /// Spawn an MCP server against an existing `local_root` (e.g. one shared
    /// with a `TwoDaemonsLocal` harness). The caller owns the `local_root`
    /// lifetime; pass `None` for the optional `TempDir` if the caller manages
    /// the directory's lifetime.
    pub fn with_local_root(local_root: &Path, owned: Option<tempfile::TempDir>) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_famp"))
            .args(["mcp"])
            .env("FAMP_LOCAL_ROOT", local_root)
            .env_remove("FAMP_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn famp mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout_raw = child.stdout.take().unwrap();
        let stdout = BufReader::new(stdout_raw);

        let mut h = Self {
            child,
            stdin,
            stdout,
            _local_root: owned,
            id_counter: 1,
        };
        // Perform the MCP initialize handshake so the server is ready.
        h.call("initialize", &serde_json::json!({}));
        h
    }

    #[allow(clippy::missing_const_for_fn)]
    fn next_id(&mut self) -> i64 {
        self.id_counter += 1;
        self.id_counter
    }

    /// Send a JSON-RPC method call and return the response.
    pub fn call(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        let id = self.next_id();
        send_msg(
            &mut self.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": id, "method": method, "params": params
            }),
        );
        recv_msg(&mut self.stdout, Duration::from_secs(5))
    }

    /// Send a `tools/call` request and return the response.
    pub fn tool_call(&mut self, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
        self.call(
            "tools/call",
            &serde_json::json!({
                "name": name, "arguments": arguments
            }),
        )
    }

    /// Pull the parsed content text from a successful `tools/call` response.
    ///
    /// Panics with a diagnostic if the response does not have the expected shape.
    pub fn ok_content(resp: &serde_json::Value) -> serde_json::Value {
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("expected ok tool/call, got: {resp}"));
        serde_json::from_str(text).expect("inner JSON parse")
    }

    /// Extract `error.data.famp_error_kind` from an error response.
    pub fn error_kind(resp: &serde_json::Value) -> String {
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
