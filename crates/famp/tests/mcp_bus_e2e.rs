#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! TEST-05 — full bus-side E2E across two `famp mcp` stdio processes.
//!
//! v0.9 equivalent of the v0.8 `e2e_two_daemons` HTTPS round-trip. Spawns
//! TWO `famp mcp` subprocesses (`alice_mcp`, `bob_mcp`) sharing one
//! `FAMP_BUS_SOCKET` and proves that:
//!
//! 1. each MCP process can `famp_register` itself as a distinct identity
//!    on the local broker (canonical-holder semantics from D-04 + D-10);
//! 2. alice's `famp_send` to bob delivers a typed envelope through the
//!    UDS broker (with no `FAMP_HOME` and no `FAMP_LOCAL_ROOT` set, per
//!    MCP-01 startup-path isolation);
//! 3. bob's `famp_await` unblocks with that envelope, with the broker
//!    stamping `from = agent:local.bus/alice` and the body carrying the
//!    sent text under `body.details.summary` (Phase-2 `audit_log` shape).
//!
//! ## Wire framing
//!
//! `famp mcp` speaks newline-delimited JSON-RPC (NDJSON) on stdin/stdout
//! — NOT LSP-style `Content-Length` framing. See `crates/famp/src/cli/
//! mcp/server.rs::read_msg` and the existing `mcp_stdio_tool_calls.rs`
//! harness which this file shares conventions with.
//!
//! ## Tool input shapes (post-02-09)
//!
//! - `famp_register`: accepts `name` OR `identity` (both wire-equivalent;
//!   we use `name` to mirror the broker's `BusMessage::Register.name`).
//! - `famp_send`: `{ peer, mode: "new_task", title }` — flat surface, NOT
//!   nested `to: {kind, name}`. The "hello" text rides on `title`, which
//!   the bus path projects into `body.details.summary` of the envelope.
//! - `famp_await`: `{ timeout_seconds: u64 }` (default 30) — NOT
//!   `timeout_ms`. We use 10s to absorb broker-spawn-on-demand latency
//!   without making the test wait long on the happy path.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{json, Value};

// ── harness ───────────────────────────────────────────────────────────────────

/// One scripted `famp mcp` subprocess. Each instance owns its own stdin
/// writer and a buffered stdout reader; both processes in a single test
/// share the same `FAMP_BUS_SOCKET` so they meet on one broker.
struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    label: &'static str,
}

impl McpHarness {
    /// Spawn `famp mcp` with the requested bus socket env override and
    /// `FAMP_HOME` / `FAMP_LOCAL_ROOT` REMOVED from the environment (the
    /// MCP-01 startup-path isolation requirement). Performs the MCP
    /// `initialize` handshake before returning so the server is ready
    /// for `tools/call` requests.
    fn spawn(sock: &Path, label: &'static str) -> Self {
        let mut child = Command::cargo_bin("famp")
            .unwrap()
            .args(["mcp"])
            .env("FAMP_BUS_SOCKET", sock)
            .env_remove("FAMP_HOME")
            .env_remove("FAMP_LOCAL_ROOT")
            // Tier-2 / tier-3 identity-resolver paths must NOT leak into
            // this test. We rely entirely on `tools::*` plumbing the MCP
            // session's `active_identity` into `act_as`.
            .env_remove("FAMP_LOCAL_IDENTITY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("{label}: famp mcp spawn failed: {e}"));
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut h = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
            label,
        };
        // MCP initialize handshake. The server replies once before it
        // accepts any `tools/call`.
        let _ = h.call_raw("initialize", &json!({}));
        h
    }

    #[allow(clippy::missing_const_for_fn)]
    fn next_id(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }

    /// Send one JSON-RPC frame over stdin (NDJSON: one JSON object + `\n`).
    fn send_msg(&mut self, msg: &Value) {
        let mut body = serde_json::to_string(msg).expect("serialize");
        body.push('\n');
        self.stdin
            .write_all(body.as_bytes())
            .unwrap_or_else(|e| panic!("{}: stdin write failed: {e}", self.label));
        self.stdin
            .flush()
            .unwrap_or_else(|e| panic!("{}: stdin flush failed: {e}", self.label));
    }

    /// Read one JSON-RPC reply (NDJSON), with a generous timeout to
    /// absorb broker-spawn-on-demand and CI scheduling jitter.
    fn recv_msg(&mut self, timeout: Duration) -> Value {
        let deadline = Instant::now() + timeout;
        loop {
            assert!(
                Instant::now() < deadline,
                "{}: timed out waiting for MCP response",
                self.label
            );
            let mut line = String::new();
            let n = self
                .stdout
                .read_line(&mut line)
                .unwrap_or_else(|e| panic!("{}: stdout read failed: {e}", self.label));
            assert!(n > 0, "{}: stdout closed unexpectedly", self.label);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            return serde_json::from_str(trimmed).unwrap_or_else(|e| {
                panic!("{}: parse JSON failed ({e}); line: {trimmed}", self.label)
            });
        }
    }

    /// Generic JSON-RPC call: pick a fresh id, send, await one reply.
    fn call_raw(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id();
        self.send_msg(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        self.recv_msg(Duration::from_secs(15))
    }

    /// Convenience: invoke a `tools/call` with `name` + `arguments`.
    fn tool_call(&mut self, tool: &str, args: &Value) -> Value {
        self.call_raw(
            "tools/call",
            &json!({
                "name": tool,
                "arguments": args,
            }),
        )
    }

    /// Pull the inner JSON document a tool wrote into `result.content[0].text`
    /// and parse it. Panics with the full reply on any unexpected shape.
    fn ok_result(reply: &Value, what: &str) -> Value {
        assert!(
            reply.get("error").is_none(),
            "{what}: JSON-RPC error: {reply}"
        );
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{what}: missing result.content[0].text: {reply}"));
        serde_json::from_str(text)
            .unwrap_or_else(|e| panic!("{what}: tool result not JSON ({e}): {text}"))
    }

    fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── the test ──────────────────────────────────────────────────────────────────

/// TEST-05 — full bus-side E2E. See module-level docs for the contract.
#[test]
fn test_mcp_bus_e2e() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sock = tmp.path().join("test-bus.sock");

    // Spawn alice's MCP process first. The first `famp_register` call
    // through it will lazily spawn the broker on this socket if no
    // broker is listening yet (BusClient::connect drives
    // spawn_broker_if_absent).
    let mut alice = McpHarness::spawn(&sock, "alice");
    let mut bob = McpHarness::spawn(&sock, "bob");

    // ── 1. alice registers ────────────────────────────────────────────────────
    let reg_alice = alice.tool_call("famp_register", &json!({ "name": "alice" }));
    let body = McpHarness::ok_result(&reg_alice, "alice register");
    assert_eq!(
        body["active"], "alice",
        "alice register active mismatch: {body}"
    );
    assert!(
        body["drained"].is_number(),
        "alice register drained must be a count: {body}"
    );
    assert!(
        body["peers"].is_array(),
        "alice register peers must be array: {body}"
    );

    // ── 2. bob registers ──────────────────────────────────────────────────────
    let reg_bob = bob.tool_call("famp_register", &json!({ "name": "bob" }));
    let body = McpHarness::ok_result(&reg_bob, "bob register");
    assert_eq!(
        body["active"], "bob",
        "bob register active mismatch: {body}"
    );

    // ── 3. bob parks an await BEFORE alice sends ─────────────────────────────
    //
    // The v0.9 broker's `Await { timeout_ms, task }` handler simply parks
    // the calling client and unparks it when a NEW envelope arrives —
    // it does NOT scan the mailbox for already-queued lines. So the
    // proper pattern for an "await unblocks on send" test is to park
    // bob's await BEFORE alice sends. To do that on a single test
    // thread, we send the JSON-RPC request frame from bob, do NOT read
    // the reply yet, drive alice's send to completion, then read bob's
    // reply (which should now carry the unparked envelope).
    let bob_await_id = bob.next_id();
    bob.send_msg(&json!({
        "jsonrpc": "2.0",
        "id": bob_await_id,
        "method": "tools/call",
        "params": {
            "name": "famp_await",
            "arguments": { "timeout_seconds": 10 },
        },
    }));

    // Give the broker a moment to receive bob's Await frame and insert
    // the parked entry. Without this, alice's Send can race ahead and
    // deliver into bob's mailbox before bob's parked-await is recorded
    // — the same race the v0.8 `cli_dm_roundtrip::test_await_unblocks`
    // mitigates with a 500 ms sleep.
    std::thread::sleep(Duration::from_millis(500));

    // ── 4. alice sends to bob ─────────────────────────────────────────────────
    //
    // NB: `famp_send`'s actual input shape is the flat v0.8 surface
    // (peer/mode/title), NOT the nested `to: {kind, name}` shape the
    // plan must-haves sketched. The plan body explicitly authorizes
    // adapting the shape to whatever the harness/server uses.
    let send = alice.tool_call(
        "famp_send",
        &json!({
            "peer": "bob",
            "mode": "new_task",
            "title": "hello from alice",
        }),
    );
    let send_body = McpHarness::ok_result(&send, "alice send");
    assert!(
        send_body.get("task_id").is_some(),
        "alice send missing task_id field: {send_body}"
    );
    let task_id = send_body["task_id"]
        .as_str()
        .unwrap_or_else(|| panic!("alice send task_id not a string: {send_body}"));
    assert!(
        !task_id.is_empty(),
        "alice send task_id is empty: {send_body}"
    );

    // ── 5. bob's parked await unblocks with alice's envelope ──────────────────
    let awaited = bob.recv_msg(Duration::from_secs(15));
    let await_body = McpHarness::ok_result(&awaited, "bob await");
    assert!(
        await_body.get("timeout").is_none(),
        "bob await unexpectedly timed out: {await_body}"
    );
    let envelope = &await_body["envelope"];
    assert!(
        !envelope.is_null(),
        "bob await missing envelope: {await_body}"
    );

    // The broker stamps `from` via D-10's `effective_identity(state)`,
    // so for a proxy connection (`bind_as = Some("alice")`) the
    // resulting envelope's `from` is the canonical Principal-shaped
    // string `"agent:local.bus/alice"` (cli::send::build_envelope_value).
    let from = envelope["from"]
        .as_str()
        .unwrap_or_else(|| panic!("envelope.from not a string: {envelope}"));
    assert!(
        from.contains("alice"),
        "envelope.from should contain 'alice', got: {from}"
    );

    // The body field is the typed envelope's `body` object. For the
    // Phase-2 wrapped `audit_log` shape, the user's `title` ("hello
    // from alice") is projected into `body.details.summary` (see
    // cli::send::build_inner_payload's `new_task` branch). Use a
    // serialized-substring match instead of a structural lookup so
    // this test stays robust to envelope-shape evolution within Phase 2.
    let body_json = envelope["body"].to_string();
    assert!(
        body_json.contains("hello from alice"),
        "envelope.body should contain the sent text 'hello from alice', got: {body_json}"
    );

    // Sanity: the MCP-tool envelope projection at `task_id` matches the
    // task_id alice's send returned (via causality.ref).
    let envelope_task_id = envelope
        .get("body")
        .and_then(|b| b.get("details"))
        .and_then(|d| d.get("summary"))
        .and_then(Value::as_str);
    assert_eq!(
        envelope_task_id,
        Some("hello from alice"),
        "envelope.body.details.summary mismatch: {envelope}"
    );

    // ── 6. clean shutdown ─────────────────────────────────────────────────────
    alice.shutdown();
    bob.shutdown();
}

/// TEST-07 — famp_inbox task_id fallback for new_task messages.
///
/// Regression test for the bug where famp_inbox returned `task_id: null`
/// for new_task messages because it only checked `causality.ref` (which
/// only exists on reply envelopes). For new_task, the canonical task_id
/// lives in `envelope.id` — the broker uses this in SendOk. The inbox
/// tool must fall back to `envelope.id` so agents can always reply.
#[test]
fn test_inbox_task_id_populated_for_new_task() {
    use std::time::Duration;

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sock = tmp.path().join("task-id-test-bus.sock");

    let mut alice = McpHarness::spawn(&sock, "alice-tid");
    let mut bob = McpHarness::spawn(&sock, "bob-tid");

    let reg_a = alice.tool_call("famp_register", &json!({ "name": "alice-tid" }));
    McpHarness::ok_result(&reg_a, "alice register");
    let reg_b = bob.tool_call("famp_register", &json!({ "name": "bob-tid" }));
    McpHarness::ok_result(&reg_b, "bob register");

    // alice sends a new_task to bob
    let send = alice.tool_call(
        "famp_send",
        &json!({
            "peer": "bob-tid",
            "mode": "new_task",
            "title": "ping from alice",
        }),
    );
    let send_body = McpHarness::ok_result(&send, "alice send");
    let send_task_id = send_body["task_id"]
        .as_str()
        .unwrap_or_else(|| panic!("alice send missing task_id: {send_body}"))
        .to_string();
    assert!(!send_task_id.is_empty(), "send task_id should be non-empty");

    // Give broker time to deliver
    std::thread::sleep(Duration::from_millis(200));

    // bob reads inbox — task_id must be populated from envelope.id (not null)
    let inbox = bob.tool_call("famp_inbox", &json!({ "action": "list" }));
    let inbox_body = McpHarness::ok_result(&inbox, "bob inbox");
    let entries = inbox_body["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("inbox entries not an array: {inbox_body}"));
    assert!(!entries.is_empty(), "bob inbox should have at least one entry");

    let entry = &entries[0];
    let inbox_task_id = entry["task_id"]
        .as_str()
        .unwrap_or_else(|| panic!("inbox entry task_id is null — new_task fallback broken: {entry}"));
    assert_eq!(
        inbox_task_id, send_task_id,
        "inbox task_id must match the send task_id"
    );

    alice.shutdown();
    bob.shutdown();
}

/// TEST-06 — listen mode notification path.
///
/// Verifies that when alice sends to bob and bob is parked on
/// `famp_await --as bob`, the await unblocks and returns an envelope
/// with a non-empty `from` field. The hook then emits a notification;
/// the agent would call `famp_inbox` to retrieve content. Here we just
/// assert the envelope arrived with correct routing metadata.
#[test]
fn test_listen_mode_await_unblocks_on_send() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sock = tmp.path().join("listen-test-bus.sock");

    let mut alice = McpHarness::spawn(&sock, "alice-listen");
    let mut bob = McpHarness::spawn(&sock, "bob-listen");

    // Register both (listen:true is a hook hint, ignored by the MCP tool itself)
    let reg_a = alice.tool_call("famp_register", &json!({ "name": "alice-listen" }));
    McpHarness::ok_result(&reg_a, "alice-listen register");

    let reg_b = bob.tool_call(
        "famp_register",
        &json!({ "name": "bob-listen", "listen": true }),
    );
    let bob_body = McpHarness::ok_result(&reg_b, "bob-listen register");
    assert_eq!(bob_body["active"], "bob-listen");

    // Bob parks await before alice sends
    let bob_await_id = bob.next_id();
    bob.send_msg(&json!({
        "jsonrpc": "2.0",
        "id": bob_await_id,
        "method": "tools/call",
        "params": {
            "name": "famp_await",
            "arguments": { "timeout_seconds": 10 },
        },
    }));
    std::thread::sleep(Duration::from_millis(500));

    // Alice sends
    let send = alice.tool_call(
        "famp_send",
        &json!({
            "peer": "bob-listen",
            "mode": "new_task",
            "title": "Listen mode test message",
        }),
    );
    McpHarness::ok_result(&send, "alice send");

    // Read bob's await response (parked frame unblocks once alice's message arrives)
    let bob_await_resp = bob.recv_msg(Duration::from_secs(15));
    let await_body = McpHarness::ok_result(&bob_await_resp, "bob-listen await");

    // The envelope must have arrived (not timeout)
    assert!(
        await_body.get("timeout").is_none() || await_body["timeout"] != true,
        "expected envelope, got timeout: {await_body}"
    );

    // Must have an `envelope` field with a `from` field (hook uses this for the notification string)
    let envelope = &await_body["envelope"];
    assert!(
        !envelope.is_null(),
        "bob-listen await missing envelope: {await_body}"
    );
    let from = envelope["from"]
        .as_str()
        .unwrap_or_else(|| panic!("envelope.from not a string: {envelope}"));
    assert!(
        from.contains("alice-listen"),
        "envelope.from should contain 'alice-listen', got: {from}"
    );

    alice.shutdown();
    bob.shutdown();
}
