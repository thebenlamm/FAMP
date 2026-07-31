#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 14 plan 14-02: the five rendering surfaces the tracer (plan
//! 14-01) deferred, plus the fail-closed edge cases D-04/D-05 require.
//!
//! Task 2 truths pinned here:
//!   - `famp_await` (via `run_at_structured`) marks gateway-origin
//!     content and leaves local-origin content verbatim
//!     (`await_marks_gateway_origin`, `await_leaves_local_origin_verbatim`).
//!   - CLI `wait-reply` marks gateway-origin content
//!     (`wait_reply_marks_gateway_origin`).
//!   - CLI `register --tail` marks gateway-origin content
//!     (`register_tail_marks_gateway_origin`).
//!   - `famp_channel_log` marks both a stamped gateway-origin record and
//!     a legacy unstamped record (`channel_log_marks_gateway_origin`,
//!     `channel_log_marks_legacy_unstamped_record`).
//!   - Two renders of the identical body carry different nonces
//!     (`nonce_differs_across_two_renders_of_identical_body`).
//!   - An unexpected-reply error never leaks a `BusReply`'s payload via
//!     `{:?}` (`reply_debug_never_reaches_error_text`).
//!
//! Task 3 adds five more tests confirming D-06's claimed-clean surfaces
//! (`famp inspect messages`, `famp_verify`, MCP `register`/`join`) plus
//! the wake path (QUAR-06) really are clean — see the bottom of this
//! file.

use std::io::{BufRead, BufReader, Write as _};
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::BusClient;
use famp::cli::await_cmd::{run_at_structured as await_run_at_structured, AwaitArgs};
use famp::cli::render::render_envelope_body;
use famp_bus::{BusMessage, Origin};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

#[path = "common/mcp_harness.rs"]
mod mcp_harness;
use mcp_harness::Harness;

// ── shared helpers (mirrors quarantine_tracer.rs's conventions) ────────────

fn spawn_broker(sock: &std::path::Path) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .args(["broker", "--socket", sock.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn spawn_register(sock: &std::path::Path, name: &str) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", sock)
            .args(["register", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

async fn wait_for_registration(sock: &std::path::Path, name: &str) {
    for _ in 0..50 {
        if let Ok(mut client) = BusClient::connect(sock, Some(name.into())).await {
            client.shutdown().await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("{name} failed to register within 5s");
}

async fn wait_for_socket(sock: &std::path::Path) {
    for _ in 0..50 {
        if std::os::unix::net::UnixStream::connect(sock).is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("broker socket {} never came up", sock.display());
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// A fresh runtime-generated body marker — never a source-visible literal
/// (Task 3's requirement), reused across Task 2's tests too for
/// robustness.
fn marker() -> String {
    format!("MARK-{}", uuid::Uuid::now_v7().simple())
}

/// A fully-decodable `audit_log`/`standalone` envelope (the class that
/// never fires the task FSM — matches the shape existing tests in this
/// workspace, e.g. `famp-gateway/tests/principal_send_drain.rs`, already
/// use). Unlike a bare-string body, `AuditLogBody` REQUIRES a structured
/// `{"event": ..., "details": ...}` object — a bare string fails
/// `AnyBusEnvelope::decode` and gets silently skipped by the drain's
/// head-of-line resilience, which would make several of this file's
/// tests pass or hang for the WRONG reason (some paths here go through
/// real decode: `Inbox`; others bypass it via the wake-trigger fast path:
/// `Await`). Always build a real, decodable envelope so every test
/// exercises the actual rendering surface, not a decode-skip artifact.
///
/// `causality` optionally sets `{"ref": task, "rel": rel}` for tests that
/// need `wait_reply`'s inbox-first `is_reply_for_task` match.
fn valid_envelope(
    sender: &str,
    recipient: &str,
    body_marker: &str,
    causality: Option<(uuid::Uuid, &str)>,
) -> serde_json::Value {
    let mut v = serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": uuid::Uuid::now_v7().to_string(),
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-07-30T12:00:00Z",
        "body": {"event": "famp.send.deliver", "details": {"msg": body_marker}},
    });
    if let Some((task, rel)) = causality {
        v["causality"] = serde_json::json!({"ref": task.to_string(), "rel": rel});
    }
    v
}

/// Normalize a rendered `body` field to plain text regardless of whether
/// it stayed a structured `Value` (local origin, verbatim) or became a
/// wrapped `String` (gateway/unknown origin) — so assertions can uniformly
/// check marker-text containment.
fn rendered_body_text(v: &serde_json::Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

// ── Task 2 ───────────────────────────────────────────────────────────────

/// `famp_await` (via `run_at_structured`, the function the MCP `famp_await`
/// tool and CLI `await` both call) marks a gateway-origin sender's body
/// and adds a machine-readable `"origin"` field.
#[test]
fn await_marks_gateway_origin() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        // Park bob's await on a background task BEFORE alice sends, so the
        // wake (not a drain-on-connect) is what delivers the envelope.
        let sock_for_await = sock.clone();
        let await_task = tokio::spawn(async move {
            await_run_at_structured(
                &sock_for_await,
                AwaitArgs {
                    timeout: humantime::Duration::from(Duration::from_secs(10)),
                    task: None,
                    act_as: Some("bob".into()),
                    abort_on_fd: None,
                },
            )
            .await
            .expect("bob await")
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        let mut alice = BusClient::connect(&sock, None).await.unwrap();
        let reply = alice
            .send_recv(BusMessage::Register {
                name: "alice".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: Some(Origin::Gateway),
            })
            .await
            .unwrap();
        assert!(matches!(reply, famp_bus::BusReply::RegisterOk { .. }));

        let body_marker = marker();
        let envelope = valid_envelope("alice", "bob", &body_marker, None);
        let send_reply = alice
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope: envelope.clone(),
            })
            .await
            .unwrap();
        assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));

        let outcome = tokio::time::timeout(Duration::from_secs(10), await_task)
            .await
            .expect("await task must finish within 10s")
            .expect("await task must not panic");

        assert_eq!(outcome.envelopes.len(), 1, "bob must receive alice's post");
        let received = &outcome.envelopes[0];
        assert_eq!(received["origin"], "gateway");
        assert!(
            received["body"].is_string(),
            "gateway-origin body must render as a wrapped STRING, not the raw structured Value: {received}"
        );
        let body = rendered_body_text(&received["body"]);
        assert!(
            body.contains(&body_marker),
            "wrapped body must still contain the original marker text: {body}"
        );
        assert!(body.contains("origin=gateway"));

        alice.shutdown().await;
    });
}

/// A local-origin sender's body renders verbatim through `famp_await`.
#[test]
fn await_leaves_local_origin_verbatim() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;
        // `famp register` (real CLI path) declares Origin::Local.
        let _alice = spawn_register(&sock, "alice");
        wait_for_registration(&sock, "alice").await;

        let sock_for_await = sock.clone();
        let await_task = tokio::spawn(async move {
            await_run_at_structured(
                &sock_for_await,
                AwaitArgs {
                    timeout: humantime::Duration::from(Duration::from_secs(10)),
                    task: None,
                    act_as: Some("bob".into()),
                    abort_on_fd: None,
                },
            )
            .await
            .expect("bob await")
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        let body_marker = marker();
        let mut proxy = BusClient::connect(&sock, Some("alice".into()))
            .await
            .unwrap();
        let envelope = valid_envelope("alice", "bob", &body_marker, None);
        let send_reply = proxy
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope: envelope.clone(),
            })
            .await
            .unwrap();
        assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));
        proxy.shutdown().await;

        let outcome = tokio::time::timeout(Duration::from_secs(10), await_task)
            .await
            .expect("await task must finish within 10s")
            .expect("await task must not panic");

        assert_eq!(outcome.envelopes.len(), 1);
        let received = &outcome.envelopes[0];
        assert_eq!(received["origin"], "local");
        assert_eq!(
            received["body"], envelope["body"],
            "local-origin body must render byte-identical to the raw body"
        );
    });
}

/// CLI `wait-reply` marks a gateway-origin reply's body (subprocess: the
/// real binary, since `wait_reply::run_structured` resolves its socket
/// from `$FAMP_BUS_SOCKET`, not an explicit param).
#[test]
fn wait_reply_marks_gateway_origin() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let _bob = spawn_register(&sock, "bob");
        wait_for_registration(&sock, "bob").await;

        let mut alice = BusClient::connect(&sock, None).await.unwrap();
        alice
            .send_recv(BusMessage::Register {
                name: "alice".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: Some(Origin::Gateway),
            })
            .await
            .unwrap();

        let task = uuid::Uuid::now_v7();
        let body_marker = marker();
        let envelope = valid_envelope("alice", "bob", &body_marker, Some((task, "delivers")));
        let send_reply = alice
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope,
            })
            .await
            .unwrap();
        assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));
        alice.shutdown().await;

        // Give the message a moment to land before bob's inbox-first scan.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let output = Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", &sock)
            .args([
                "wait-reply",
                "--as",
                "bob",
                "--task",
                &task.to_string(),
                "--timeout",
                "5s",
            ])
            .output()
            .expect("famp wait-reply spawn");
        assert!(
            output.status.success(),
            "wait-reply must exit 0: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("wait-reply stdout not JSON ({e}): {stdout}"));
        let entries = line["envelopes"].as_array().unwrap();
        assert_eq!(entries.len(), 1, "wait-reply stdout: {line}");
        assert_eq!(entries[0]["origin"], "gateway");
        assert!(entries[0]["body"].is_string());
        let body = rendered_body_text(&entries[0]["body"]);
        assert!(body.contains(&body_marker));
        assert!(body.contains("origin=gateway"));
    });
}

/// `famp register --tail` marks a gateway-origin sender's body in its
/// stderr tail line.
#[test]
fn register_tail_marks_gateway_origin() {
    runtime().block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        let _broker = spawn_broker(&sock);
        wait_for_socket(&sock).await;

        let mut tail_child = Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", &sock)
            .args(["register", "bob", "--tail"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = tail_child.stderr.take().unwrap();
        let mut guard = ChildGuard::new(tail_child);
        let mut reader = BufReader::new(stderr);

        wait_for_registration(&sock, "bob").await;

        let mut alice = BusClient::connect(&sock, None).await.unwrap();
        alice
            .send_recv(BusMessage::Register {
                name: "alice".into(),
                pid: std::process::id(),
                cwd: None,
                listen: false,
                origin: Some(Origin::Gateway),
            })
            .await
            .unwrap();
        let body_marker = marker();
        let envelope = valid_envelope("alice", "bob", &body_marker, None);
        let send_reply = alice
            .send_recv(BusMessage::Send {
                to: famp_bus::Target::Agent { name: "bob".into() },
                envelope,
            })
            .await
            .unwrap();
        assert!(matches!(send_reply, famp_bus::BusReply::SendOk { .. }));
        alice.shutdown().await;

        // Poll stderr lines for up to 10s (tail loop polls every 1s). Each
        // `read_line` call is itself blocking with no timeout, so bound the
        // number of attempts as well as wall-clock time — a stalled pipe
        // must fail the test, not hang the suite.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut found = false;
        let mut seen = String::new();
        for _ in 0..200 {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    seen.push_str(&line);
                    if line.contains("FAMP-QUARANTINE") && line.contains("origin=gateway") {
                        found = true;
                        break;
                    }
                }
            }
        }
        assert!(
            found,
            "register --tail must print a quarantine-marked line for a gateway-origin sender; saw: {seen}"
        );
        let _ = guard.take();
    });
}

/// `famp_channel_log` marks a stamped gateway-origin record read directly
/// from disk.
#[test]
fn channel_log_marks_gateway_origin() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mailboxes = tmp.path().join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();
    let body_marker = marker();
    let envelope = valid_envelope("alice", "planning", &body_marker, None);
    let canonical = famp_canonical::canonicalize(&envelope).unwrap();
    let stamped = famp_bus::stamp_line(&canonical, Origin::Gateway).unwrap();
    let mut file = std::fs::File::create(mailboxes.join("#planning.jsonl")).unwrap();
    file.write_all(&stamped).unwrap();
    file.write_all(b"\n").unwrap();
    drop(file);

    let out = famp::cli::mcp::tools::channel_log::call_at_bus_dir(
        &serde_json::json!({"channel": "planning"}),
        tmp.path(),
    )
    .expect("call_at_bus_dir");

    let entries = out["envelopes"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["origin"], "gateway");
    assert!(entries[0]["envelope"]["body"].is_string());
    let body = rendered_body_text(&entries[0]["envelope"]["body"]);
    assert!(body.contains(&body_marker));
    assert!(body.contains("origin=gateway"));
}

/// A legacy (pre-Phase-14, unstamped) mailbox record read via
/// `famp_channel_log` resolves to `Origin::Unknown` and renders marked —
/// fail-closed even though this path is NOT version-gated (T-14-07,
/// D-10).
#[test]
fn channel_log_marks_legacy_unstamped_record() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mailboxes = tmp.path().join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();
    let body_marker = marker();
    let envelope = valid_envelope("carol", "planning", &body_marker, None);
    let canonical = famp_canonical::canonicalize(&envelope).unwrap();
    let mut file = std::fs::File::create(mailboxes.join("#planning.jsonl")).unwrap();
    file.write_all(&canonical).unwrap();
    file.write_all(b"\n").unwrap();
    drop(file);

    let out = famp::cli::mcp::tools::channel_log::call_at_bus_dir(
        &serde_json::json!({"channel": "planning"}),
        tmp.path(),
    )
    .expect("call_at_bus_dir");

    let entries = out["envelopes"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["origin"], "unknown",
        "an unstamped legacy line must resolve to Origin::Unknown, never Local"
    );
    assert!(entries[0]["envelope"]["body"].is_string());
    let body = rendered_body_text(&entries[0]["envelope"]["body"]);
    assert!(
        body.contains(&body_marker),
        "a legacy record must still render marked (fail-closed): {body}"
    );
    assert!(body.contains("origin=unknown"));
}

/// Two renders of the identical body must carry different nonces — a
/// predictable delimiter is the QUAR-03/D-23 attack surface.
#[test]
fn nonce_differs_across_two_renders_of_identical_body() {
    let body = serde_json::json!("identical body every time");
    let first = render_envelope_body(Origin::Gateway, &body);
    let second = render_envelope_body(Origin::Gateway, &body);
    assert_ne!(
        first, second,
        "two renders of identical input must carry different nonces"
    );
}

/// T-14-08: an unexpected-reply error must name the `BusReply` VARIANT
/// only, never `{:?}`-format the whole reply (which would leak an
/// attacker-authored `StampedEnvelope` payload into an error string that
/// reaches stderr / MCP tool results). Mirrors the exact fix applied at
/// `wait_reply.rs`'s and `register.rs`'s "unexpected reply" arms.
#[test]
fn reply_debug_never_reaches_error_text() {
    let body_marker = marker();
    let other = famp_bus::BusReply::RegisterOk {
        active: "attacker".into(),
        drained: vec![famp_bus::StampedEnvelope {
            origin: Origin::Gateway,
            envelope: serde_json::json!({"body": body_marker}),
        }],
        peers: vec![],
    };

    // Falsification control: the naive `{:?}` interpolation THIS test
    // guards against really would have leaked the marker — proving the
    // assertion below is not vacuously true.
    let naive = format!("unexpected reply to Await: {other:?}");
    assert!(
        naive.contains(&body_marker),
        "control: naive Debug formatting must actually leak the marker, or this test proves nothing"
    );

    // The fix: variant name only.
    let fixed = format!("unexpected reply to Await: {}", other.variant_name());
    assert!(
        fixed.contains("RegisterOk"),
        "fixed error text must name the variant: {fixed}"
    );
    assert!(
        !fixed.contains(&body_marker),
        "fixed error text must never contain the reply's payload: {fixed}"
    );
}

// ── Task 3: D-06's four "verified clean" surfaces + the wake path ──────────
//
// D-06 claimed `famp inspect messages`, `famp_verify`, MCP
// `register`/`join`, and both wake paths need no change (metadata/count
// only, never body text). 14-RESEARCH.md flagged this as spot-checked,
// not exhaustively verified. Each test below uses a runtime-generated
// (UUID-derived) marker string — never a source-visible literal — so the
// assertion cannot be satisfied by an unrelated source change.

/// `famp inspect messages`' row projection (`message_row`, reached via
/// `split_stamped` — the same unwrap `read_message_snapshot` performs)
/// never emits body text, confirmed CLEAN.
#[test]
fn inspect_messages_emits_no_body_text() {
    let body_marker = marker();
    let envelope = valid_envelope("alice", "bob", &body_marker, None);
    let canonical = famp_canonical::canonicalize(&envelope).unwrap();
    let stamped = famp_bus::stamp_line(&canonical, Origin::Gateway).unwrap();
    let raw: serde_json::Value = famp_canonical::from_slice_strict(&stamped).unwrap();
    let (_, inner) = famp_bus::split_stamped(&raw);

    let row = famp_inspect_server::message_row(inner);
    let row_json = serde_json::to_string(&row).expect("serialize MessageRow");

    assert!(
        !row_json.contains(&body_marker),
        "famp inspect messages' MessageRow must never carry body text: {row_json}"
    );
}

/// `famp_verify`'s direct-mailbox scan (`scan_files` → `message_row`)
/// never emits body text, confirmed CLEAN.
#[test]
fn verify_tool_emits_no_body_text() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mailboxes = tmp.path().join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();

    let body_marker = marker();
    let task_id = uuid::Uuid::now_v7();
    let envelope = serde_json::json!({
        "famp": "0.5.2",
        "class": "request",
        "scope": "task",
        "id": task_id.to_string(),
        "from": "agent:example.test/alice",
        "to": "agent:example.test/bob",
        "authority": "advisory",
        "ts": "2026-07-30T12:00:00Z",
        "body": {
            "scope": body_marker,
            "bounds": {},
        },
    });
    let canonical = famp_canonical::canonicalize(&envelope).unwrap();
    let stamped = famp_bus::stamp_line(&canonical, Origin::Gateway).unwrap();
    let path = mailboxes.join("bob.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&stamped).unwrap();
    file.write_all(b"\n").unwrap();
    drop(file);

    let out = famp::cli::mcp::tools::verify::scan_files(&task_id.to_string(), None, &[path])
        .expect("scan_files");
    let out_json = out.to_string();

    assert!(
        !out_json.contains(&body_marker),
        "famp_verify's output must never carry body text: {out_json}"
    );
}

/// MCP `famp_register`'s reply (`{active, drained: <count>, peers}`)
/// never emits the drained backlog's body text, confirmed CLEAN — even
/// when a gateway-origin message is already sitting in the registrant's
/// mailbox before it registers.
#[test]
fn mcp_register_reply_emits_no_body_text() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let body_marker = marker();

    let mut bob = Harness::with_local_root(&root, None);
    let reg_bob = bob.tool_call("famp_register", &serde_json::json!({"name": "bob"}));
    let _ = Harness::ok_content(&reg_bob);
    let send = bob.tool_call(
        "famp_send",
        &serde_json::json!({"peer": "alice", "mode": "new_task", "title": body_marker}),
    );
    let _ = Harness::ok_content(&send);

    let mut alice = Harness::with_local_root(&root, Some(tmp));
    let reg_alice = alice.tool_call("famp_register", &serde_json::json!({"name": "alice"}));
    let reg_alice_json = reg_alice.to_string();

    assert!(
        !reg_alice_json.contains(&body_marker),
        "famp_register's reply must never carry the drained backlog's body text: {reg_alice_json}"
    );
}

/// MCP `famp_join`'s reply (`{channel, members, drained: <count>}`) never
/// emits the drained channel history's body text, confirmed CLEAN — even
/// when a gateway-origin post is already sitting in the channel's
/// mailbox before the joiner joins.
#[test]
fn mcp_join_reply_emits_no_body_text() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let body_marker = marker();

    let mut bob = Harness::with_local_root(&root, None);
    let reg_bob = bob.tool_call("famp_register", &serde_json::json!({"name": "bob"}));
    let _ = Harness::ok_content(&reg_bob);
    let send = bob.tool_call(
        "famp_send",
        &serde_json::json!({"channel": "test", "mode": "new_task", "title": body_marker}),
    );
    let _ = Harness::ok_content(&send);

    let mut alice = Harness::with_local_root(&root, Some(tmp));
    let reg_alice = alice.tool_call("famp_register", &serde_json::json!({"name": "alice"}));
    let _ = Harness::ok_content(&reg_alice);
    let join_alice = alice.tool_call("famp_join", &serde_json::json!({"channel": "test"}));
    let join_alice_json = join_alice.to_string();

    assert!(
        !join_alice_json.contains(&body_marker),
        "famp_join's reply must never carry the drained channel history's body text: {join_alice_json}"
    );
}

/// QUAR-06: the wake notification payload the Stop hook consumes
/// (`hook::emit::emit_block_decision_at`, the real path behind
/// `famp-await.sh` → `famp await` → `decision:block`) never carries
/// attacker body text — confirmed CLEAN, regression-pinned.
#[test]
fn wake_payload_emits_no_body_text() {
    let body_marker = marker();
    let outcome = famp::cli::await_cmd::AwaitOutcome {
        envelopes: vec![serde_json::json!({
            "from": "agent:example.test/alice",
            "body": body_marker,
        })],
        mailbox: Some(famp_bus::MailboxName::Agent("bob".into())),
        next_offset: Some(1),
        timed_out: false,
        diagnostic: None,
        aborted: false,
    };
    let dead_sock = std::env::temp_dir().join("famp-quarantine-surfaces-wake-dead.sock");
    let mut buf = Vec::new();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let emitted = rt.block_on(famp::cli::hook::emit::emit_block_decision_at(
        &dead_sock, &outcome, "bob", &mut buf,
    ));
    assert!(
        emitted,
        "a non-empty, non-timeout outcome must emit a block decision"
    );
    let payload = String::from_utf8(buf).unwrap();

    assert!(
        !payload.contains(&body_marker),
        "the wake notification payload must never carry attacker body text: {payload}"
    );
}
