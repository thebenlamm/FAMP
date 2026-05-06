#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

//! Tests for the transcript-detection path of `famp-await.sh`.
//!
//! Each test spawns the hook with a crafted transcript and a mock `famp`
//! binary that records its argv. Tests assert whether `famp await --as
//! <name>` was invoked (listen mode entered) or not (no-op).

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn hook_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir")
        .join(".claude/hooks/famp-await.sh")
}

/// Write a mock `famp` binary into `bin_dir` that records its full argv
/// to `log_file` and then exits 0.
fn stage_mock_famp(bin_dir: &Path, log_file: &Path) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> \"{}\"\nexit 0\n",
            log_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Build a Claude Code transcript JSONL with a `famp_register` tool call
/// and a matching tool_result. `listen` controls the input flag; `ok`
/// controls whether the result is a success.
fn make_transcript(
    path: &Path,
    identity: &str,
    listen: bool,
    ok: bool,
    with_leave_after: bool,
) {
    let tool_use_id = "toolu_test1";
    let result_content = if ok {
        format!(r#"[{{"type":"text","text":"{{\\"active\\":\\"{identity}\\",\\"drained\\":0,\\"peers\\":[]}}"}}"#)
    } else {
        r#"[{"type":"text","text":"name already taken"}]"#.to_string()
    };
    let is_error = if ok { "false" } else { "true" };
    let listen_str = if listen { "true" } else { "false" };

    let mut body = format!(
        r#"{{"type":"user","message":{{"role":"user","content":"register"}}}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{tool_use_id}","name":"mcp__famp__famp_register","input":{{"identity":"{identity}","listen":{listen_str}}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_use_id}","is_error":{is_error},"content":{result_content}}}]}}}}
"#
    );

    if with_leave_after {
        let leave_id = "toolu_leave1";
        body.push_str(&format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{leave_id}","name":"mcp__famp__famp_leave","input":{{}}}}]}}}}
"#
        ));
    }

    std::fs::write(path, body).unwrap();
}

fn run_hook(
    hook: &Path,
    transcript: &Path,
    bin_dir: &Path,
    log: &Path,
    xdg_state: &Path,
) -> std::process::Output {
    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut child = Command::new("bash")
        .arg(hook)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", log)
        .env("XDG_STATE_HOME", xdg_state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The hook does `exec 0</dev/null` at the top (it doesn't parse stdin in the
    // current sentinel-file implementation), so write_all may get EPIPE.
    // Swallow the broken-pipe error — the hook already has what it needs via
    // the transcript_path env / PATH; stdin content is irrelevant here.
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn listen_true_and_successful_register_enters_listen_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook failed: {:?}", String::from_utf8_lossy(&out.stderr));

    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await") && argv.contains("--as") && argv.contains("dk"),
        "expected famp await --as dk invocation, got: {argv:?}"
    );
}

#[test]
fn listen_false_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", false, true, false);

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for listen:false"
    );
}

#[test]
fn failed_register_result_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, false, false);  // ok=false

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for failed register"
    );
}

#[test]
fn register_then_leave_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, true);  // with_leave_after=true

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation after famp_leave"
    );
}

#[test]
fn no_register_in_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    std::fs::write(&transcript, r#"{"type":"user","message":{"role":"user","content":"hello"}}"#).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation with no register"
    );
}

#[test]
fn missing_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("does_not_exist.jsonl");

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook must exit 0 on missing transcript");
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for missing transcript"
    );
}

#[test]
fn malformed_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    std::fs::write(&transcript, "not json at all\n{broken\n").unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook must exit 0 on malformed transcript");
}

#[test]
fn last_registration_wins_when_multiple_in_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");

    // First register as "alice" (listen:true, ok), then re-register as "dk" (listen:true, ok)
    let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"alice","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":[{"type":"text","text":"{\"active\":\"alice\"}"}]}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","is_error":false,"content":[{"type":"text","text":"{\"active\":\"dk\"}"}]}]}}
"#;
    std::fs::write(&transcript, body).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("--as dk"),
        "expected last identity 'dk', got: {argv:?}"
    );
    assert!(
        !argv.contains("--as alice"),
        "must not use first identity 'alice': {argv:?}"
    );
}
