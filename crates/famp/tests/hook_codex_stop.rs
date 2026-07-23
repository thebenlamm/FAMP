//! Integration tests for native `famp hook codex-stop`.
//!
//! Proves the critical acceptance criteria:
//! - wake path works with jq/python3 absent from PATH
//! - block decision is native JSON
//! - fail-open exit 0 on uncertainty / timeout
//! - listen:true / listen:false / set_listen(false) parity
//! - install/uninstall trust the native command string

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::cli::hook::transcript;

fn famp_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("famp")
}

fn write_codex_rollout(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// PATH with no jq/python3 — only absolute essentials for the dynamic linker.
fn minimal_path() -> String {
    "/usr/bin:/bin".to_string()
}

struct Bus {
    tmp: tempfile::TempDir,
    sock: PathBuf,
    holders: Vec<Child>,
}

impl Bus {
    fn new() -> Self {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        Self {
            tmp,
            sock,
            holders: Vec::new(),
        }
    }

    fn famp(&self, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", &self.sock)
            .env("HOME", self.tmp.path())
            .args(args)
            .output()
            .unwrap()
    }

    fn register(&mut self, name: &str) {
        let child = Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", &self.sock)
            .env("HOME", self.tmp.path())
            .args(["register", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.holders.push(child);
        for _ in 0..50 {
            let out = self.famp(&["whoami", "--as", name]);
            if out.status.success() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("register {name} did not become live");
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        for h in &mut self.holders {
            let _ = h.kill();
            let _ = h.wait();
        }
    }
}

#[test]
fn unit_listen_true_resolves_identity() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path().join("rollout.jsonl");
    write_codex_rollout(
        &t,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );
    assert_eq!(
        transcript::extract_listen_identity(&t).as_deref(),
        Some("codex")
    );
}

#[test]
fn unit_listen_false_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path().join("rollout.jsonl");
    write_codex_rollout(
        &t,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":false}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );
    assert_eq!(transcript::extract_listen_identity(&t), None);
}

#[test]
fn unit_set_listen_false_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path().join("rollout.jsonl");
    write_codex_rollout(
        &t,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c2","invocation":{"server":"famp","tool":"famp_set_listen","arguments":{"listen":false}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );
    assert_eq!(transcript::extract_listen_identity(&t), None);
}

#[test]
fn unit_malformed_transcript_fail_open() {
    let dir = tempfile::tempdir().unwrap();
    let t = dir.path().join("bad.jsonl");
    std::fs::write(&t, "not json\n{{{{\n").unwrap();
    assert_eq!(transcript::extract_listen_identity(&t), None);
}

#[test]
fn unit_session_id_glob_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let codex_home = dir.path().join("codex");
    let sid = "019f824d-971f-7ec1-8c9b-8929d3f97c7a";
    let rollout = codex_home
        .join("sessions/2026/07/20")
        .join(format!("rollout-2026-07-20T21-32-30-{sid}.jsonl"));
    write_codex_rollout(
        &rollout,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );
    let path = famp::cli::hook::codex_rollout::resolve_rollout_path_with_home(
        sid,
        &codex_home,
        &codex_home,
    );
    assert_eq!(path.as_deref(), Some(rollout.as_path()));
}

#[test]
fn cli_hook_codex_stop_exits_zero_on_missing_transcript() {
    let bin = famp_bin();
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    std::fs::create_dir_all(&xdg).unwrap();

    let mut child = Command::new(&bin)
        .args(["hook", "codex-stop", "--timeout", "1s"])
        .env("PATH", minimal_path())
        .env("XDG_STATE_HOME", &xdg)
        .env("HOME", dir.path())
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .env_remove("CODEX_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(br#"{"session_id":"no-such","hook_event_name":"Stop"}"#)
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "must fail-open exit 0: status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("\"decision\""),
        "no block decision on missing transcript: {stdout}"
    );
}

#[test]
fn cli_hook_emits_block_without_jq_or_python_on_path() {
    let bin = famp_bin();
    let mut bus = Bus::new();
    bus.register("alice");
    bus.register("bob");

    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    std::fs::create_dir_all(&xdg).unwrap();

    let transcript = dir.path().join("rollout.jsonl");
    write_codex_rollout(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"bob","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );

    let mut hook = Command::new(&bin)
        .args(["hook", "codex-stop", "--timeout", "8s"])
        .env("PATH", minimal_path())
        .env("XDG_STATE_HOME", &xdg)
        .env("HOME", bus.tmp.path())
        .env("FAMP_BUS_SOCKET", &bus.sock)
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = hook.stdin.as_mut().unwrap();
        let payload = format!(
            r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
            transcript.display()
        );
        stdin.write_all(payload.as_bytes()).unwrap();
    }

    // Wait for bob's await to park, then deliver from alice.
    std::thread::sleep(Duration::from_millis(500));
    let send = bus.famp(&[
        "send",
        "--as",
        "alice",
        "--to",
        "bob",
        "--new-task",
        "hello from peer",
    ]);
    assert!(
        send.status.success(),
        "send failed: {}",
        String::from_utf8_lossy(&send.stderr)
    );

    let out = hook.wait_with_output().expect("hook finish");
    assert!(
        out.status.success(),
        "hook must exit 0: status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"decision\""),
        "expected block decision without jq/python; stdout={stdout} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = stdout.lines().find(|l| l.contains("decision")).unwrap();
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["decision"], "block");
    let reason = v["reason"].as_str().unwrap();
    assert!(reason.contains("FAMP listen mode"));
    assert!(reason.contains("famp_inbox") || reason.contains("famp_channel_log"));
    assert!(
        !reason.contains("hello from peer"),
        "peer body must never enter reason"
    );
}

#[test]
fn cli_hook_timeout_exits_zero_no_decision() {
    let bin = famp_bin();
    let mut bus = Bus::new();
    bus.register("solo");

    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    std::fs::create_dir_all(&xdg).unwrap();
    let transcript = dir.path().join("rollout.jsonl");
    write_codex_rollout(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"c1","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"solo","listen":true}},"result":{"Ok":{"content":[],"isError":false}}}}
"#,
    );

    let mut hook = Command::new(&bin)
        .args(["hook", "codex-stop", "--timeout", "1s"])
        .env("PATH", minimal_path())
        .env("XDG_STATE_HOME", &xdg)
        .env("HOME", bus.tmp.path())
        .env("FAMP_BUS_SOCKET", &bus.sock)
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = hook.stdin.as_mut().unwrap();
        let payload = format!(
            r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
            transcript.display()
        );
        stdin.write_all(payload.as_bytes()).unwrap();
    }
    let out = hook.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("\"decision\""));
}

#[test]
fn install_codex_trusts_native_command_string() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

    let hooks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
            .unwrap();
    let command = hooks["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(command.contains("hook codex-stop"));

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out2, &mut err2).unwrap();
    let hooks2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert!(hooks2["hooks"]
        .as_object()
        .is_none_or(|h| !h.contains_key("Stop") || h["Stop"].as_array().is_none_or(Vec::is_empty)));
}

#[test]
fn uninstall_removes_legacy_shim_and_native_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".codex/hooks")).unwrap();
    let shim = home.join(".codex/hooks/famp-await.sh");
    std::fs::write(&shim, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    let abs_shim = std::fs::canonicalize(&shim).unwrap_or(shim.clone());
    std::fs::write(
        home.join(".codex/hooks.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "hooks": {
                "Stop": [
                    {"hooks": [{"type":"command","command": abs_shim.display().to_string(), "timeout": 86400}]},
                    {"hooks": [{"type":"command","command": "/opt/famp hook codex-stop", "timeout": 86400}]}
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out, &mut err).unwrap();

    assert!(!shim.exists());
    let hooks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
            .unwrap();
    let stop = hooks["hooks"]
        .get("Stop")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        stop.is_empty(),
        "both native and legacy FAMP Stop entries must be removed: {stop:?}"
    );
}
