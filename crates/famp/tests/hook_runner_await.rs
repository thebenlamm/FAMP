#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

//! Tests for the transcript-detection path of `famp-await.sh`.
//!
//! Each test spawns the hook with a crafted transcript and a mock `famp`
//! binary that records its argv. Tests assert whether `famp await --as
//! <name>` was invoked (listen mode entered) or not (no-op).

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn hook_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir")
        .join(".claude/hooks/famp-await.sh")
}

/// The repo asset — the SOURCE OF TRUTH for the hook (installed copies are
/// `include_str!`-embedded from here). Issue #21 tests MUST exercise this,
/// not the installed `~/.claude/hooks/famp-await.sh`, so they test the code
/// under version control and do not depend on `famp install` having run.
fn asset_hook_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/famp-await.sh")
}

/// Skip the test if the hook is not installed (e.g., fresh CI checkout that
/// hasn't run `famp install-claude-code`). Use at the top of every test that
/// calls `run_hook()` or passes `hook_path()` to bash.
macro_rules! require_hook {
    () => {
        if !hook_path().exists() {
            eprintln!(
                "SKIP: {} not installed; run `famp install-claude-code` first",
                hook_path().display()
            );
            return;
        }
    };
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
/// and a matching `tool_result`. `listen` controls the input flag; `ok`
/// controls whether the result is a success.
fn make_transcript(path: &Path, identity: &str, listen: bool, ok: bool, with_leave_after: bool) {
    use std::fmt::Write as _;
    let tool_use_id = "toolu_test1";
    let result_content = if ok {
        // Use a simple text payload — the extractor only checks is_error, not content.
        // (The original nested-JSON format produced invalid JSONL via \\\" escaping.)
        format!(r#"[{{"type":"text","text":"registered as {identity}"}}]"#)
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
        writeln!(
            body,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{leave_id}","name":"mcp__famp__famp_leave","input":{{}}}}]}}}}"#
        )
        .unwrap();
    }

    std::fs::write(path, body).unwrap();
}

fn run_hook(
    hook: &Path,
    transcript: &Path,
    bin_dir: &Path,
    _log: &Path,
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
        .env("XDG_STATE_HOME", xdg_state)
        // Hermetic: a live `famp mcp` under the same IDE parent chain must not
        // turn deliberate no-op transcripts into listen-mode awaits.
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The hook reads stdin via `cat` first (to extract transcript_path), then calls
    // `exec 0</dev/null`. EPIPE is possible if cat finishes before write_all completes.
    // Swallow the error — the hook has already consumed what it needs.
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

fn run_hook_with_stdin(
    hook: &Path,
    stop_json: &str,
    bin_dir: &Path,
    xdg_state: &Path,
    codex_home: Option<&Path>,
) -> std::process::Output {
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut command = Command::new("bash");
    command
        .arg(hook)
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", xdg_state)
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(codex_home) = codex_home {
        command.env("CODEX_HOME", codex_home);
    }

    let mut child = command.spawn().unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

fn write_codex_state_db(codex_home: &Path, session_id: &str, rollout_path: &Path) {
    std::fs::create_dir_all(codex_home).unwrap();
    let status = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import sqlite3, sys
con = sqlite3.connect(sys.argv[1])
con.execute("create table threads (id text primary key, rollout_path text)")
con.execute("insert into threads (id, rollout_path) values (?, ?)", (sys.argv[2], sys.argv[3]))
con.commit()
"#,
        )
        .arg(codex_home.join("state_5.sqlite"))
        .arg(session_id)
        .arg(rollout_path)
        .status()
        .unwrap();
    assert!(status.success(), "failed to create Codex state_5.sqlite");
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn listen_true_and_successful_register_enters_listen_mode() {
    require_hook!();
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
    assert!(
        out.status.success(),
        "hook failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as dk"),
        "expected famp await --as dk invocation, got: {argv:?}"
    );
}

#[test]
fn listen_absent_enters_listen_mode() {
    // Fix 1 (2026-05-12): the MCP tool defaults listen=true when the
    // field is absent (register.rs:80, unwrap_or(true)). The hook MUST
    // match this default — treat absent listen as listen-active so the
    // Stop hook actually blocks on inbound messages for the agent
    // window. Before the fix, the hook's `if inp.get("listen"):`
    // treated absent as falsy and exited no-op, silently disabling
    // auto-wake whenever the MCP caller omitted the listen field.
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    // Transcript with NO `listen` field in the famp_register input — the
    // input shape is `{"identity":"dk"}` (no listen key at all).
    let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t_absent","name":"mcp__famp__famp_register","input":{"identity":"dk"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t_absent","is_error":false,"content":[{"type":"text","text":"registered as dk"}]}]}}
"#;
    std::fs::write(&transcript, body).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(
        out.status.success(),
        "hook failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as dk"),
        "expected famp await --as dk invocation (listen defaults ON when absent), got: {argv:?}"
    );
}

#[test]
fn listen_null_enters_listen_mode() {
    // Companion to `listen_absent_enters_listen_mode`: a JSON `null` for
    // the listen field is treated identically to absent (both arrive in
    // Python as `None`, which is `not False`).
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t_null","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":null}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t_null","is_error":false,"content":[{"type":"text","text":"registered as dk"}]}]}}
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
        argv.contains("await --as dk"),
        "expected famp await --as dk invocation (listen:null treated as ON), got: {argv:?}"
    );
}

#[test]
fn listen_false_is_noop() {
    // Asset hook (not the installed ~/.claude copy): hermetic under
    // FAMP_DISABLE_PID_FALLBACK so a live host `famp mcp` cannot false-fire.
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", false, true, false);

    let out = run_hook(
        &asset_hook_path(),
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
fn codex_mcp_register_rollout_enters_listen_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_mock_famp(&bin_dir, &log);
    let transcript = dir.path().join("rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"timestamp":"2026-06-30T16:55:10.000Z","type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
"#,
    )
    .unwrap();

    let out = run_hook(&asset_hook_path(), &transcript, &bin_dir, &log, &xdg);
    assert!(
        out.status.success(),
        "hook failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as codex"),
        "expected Codex rollout registration to arm listen mode, got: {argv:?}"
    );
}

#[test]
fn codex_mcp_register_with_string_arguments_enters_listen_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_mock_famp(&bin_dir, &log);
    let transcript = dir.path().join("rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":"{\"identity\":\"codex\",\"listen\":true}"},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
"#,
    )
    .unwrap();

    let out = run_hook(&asset_hook_path(), &transcript, &bin_dir, &log, &xdg);
    assert!(out.status.success());
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as codex"),
        "expected string-arguments Codex rollout to arm listen mode, got: {argv:?}"
    );
}

#[test]
fn codex_function_call_with_string_arguments_enters_listen_mode_without_mcp_end() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_mock_famp(&bin_dir, &log);
    let transcript = dir.path().join("rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"response_item","payload":{"type":"function_call","name":"famp_register","namespace":"mcp__famp","arguments":"{\"identity\":\"codex\",\"listen\":true}","call_id":"call_register"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"call_register","output":"Wall time: 0.1 seconds\nOutput:\n[{\"type\":\"text\",\"text\":\"{\\\"active\\\":\\\"codex\\\"}\"}]"}}
"#,
    )
    .unwrap();

    let out = run_hook(&asset_hook_path(), &transcript, &bin_dir, &log, &xdg);
    assert!(out.status.success());
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as codex"),
        "expected function_call fallback to arm listen mode, got: {argv:?}"
    );
}

#[test]
fn codex_mcp_end_failure_overrides_function_call_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_mock_famp(&bin_dir, &log);
    let transcript = dir.path().join("rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"response_item","payload":{"type":"function_call","name":"famp_register","namespace":"mcp__famp","arguments":"{\"identity\":\"codex\",\"listen\":true}","call_id":"call_register"}}
{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"name taken"}],"isError":true}}}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"call_register","output":"Wall time: 0.1 seconds\nOutput:\n[{\"type\":\"text\",\"text\":\"name taken\"}]"}}
"#,
    )
    .unwrap();

    let out = run_hook(&asset_hook_path(), &transcript, &bin_dir, &log, &xdg);
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected failed mcp_tool_call_end to keep listen mode inactive"
    );
}

#[test]
fn codex_set_listen_false_cancels_listen_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_mock_famp(&bin_dir, &log);
    let transcript = dir.path().join("rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_set_listen","invocation":{"server":"famp","tool":"famp_set_listen","arguments":{"listen":false}},"result":{"Ok":{"content":[{"type":"text","text":"{\"listen_mode\":false}"}],"isError":false}}}}
"#,
    )
    .unwrap();

    let out = run_hook(&asset_hook_path(), &transcript, &bin_dir, &log, &xdg);
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation after successful famp_set_listen(false)"
    );
}

#[test]
fn codex_session_id_fallback_resolves_rollout_without_transcript_path() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    let codex_home = dir.path().join("codex-home");
    stage_mock_famp(&bin_dir, &log);

    let session_id = "019f824d-971f-7ec1-8c9b-8929d3f97c7a";
    let transcript = codex_home
        .join("sessions/2026/07/20")
        .join(format!("rollout-2026-07-20T21-32-30-{session_id}.jsonl"));
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
"#,
    )
    .unwrap();

    let stop_json = format!(r#"{{"session_id":"{session_id}","hook_event_name":"Stop"}}"#);
    let out = run_hook_with_stdin(
        &asset_hook_path(),
        &stop_json,
        &bin_dir,
        &xdg,
        Some(&codex_home),
    );
    assert!(
        out.status.success(),
        "hook failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as codex"),
        "expected session_id fallback to arm listen mode, got: {argv:?}"
    );
}

#[test]
fn codex_session_id_fallback_resolves_rollout_from_sqlite_state_db() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    let codex_home = dir.path().join("codex-home");
    stage_mock_famp(&bin_dir, &log);

    let session_id = "019f824d-dbdb-7ec1-8c9b-8929d3f97c7a";
    let transcript = codex_home.join("sessions/db-only-rollout.jsonl");
    std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    std::fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
"#,
    )
    .unwrap();
    write_codex_state_db(&codex_home, session_id, &transcript);

    let stop_json = format!(r#"{{"session_id":"{session_id}","hook_event_name":"Stop"}}"#);
    let out = run_hook_with_stdin(
        &asset_hook_path(),
        &stop_json,
        &bin_dir,
        &xdg,
        Some(&codex_home),
    );
    assert!(out.status.success());
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as codex"),
        "expected sqlite rollout_path fallback to arm listen mode, got: {argv:?}"
    );
}

#[test]
fn codex_sqlite_fallback_rejects_rollout_path_outside_codex_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    let codex_home = dir.path().join("codex-home");
    stage_mock_famp(&bin_dir, &log);

    let session_id = "019f824d-bad0-7ec1-8c9b-8929d3f97c7a";
    let transcript = dir.path().join("outside-rollout.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"event_msg","payload":{"type":"mcp_tool_call_end","call_id":"call_register","invocation":{"server":"famp","tool":"famp_register","arguments":{"identity":"codex","listen":true}},"result":{"Ok":{"content":[{"type":"text","text":"{\"active\":\"codex\"}"}],"isError":false}}}}
"#,
    )
    .unwrap();
    write_codex_state_db(&codex_home, session_id, &transcript);

    let stop_json = format!(r#"{{"session_id":"{session_id}","hook_event_name":"Stop"}}"#);
    let out = run_hook_with_stdin(
        &asset_hook_path(),
        &stop_json,
        &bin_dir,
        &xdg,
        Some(&codex_home),
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected out-of-tree sqlite rollout_path to be ignored"
    );
}

#[test]
fn failed_register_result_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, false, false); // ok=false

    let out = run_hook(
        &asset_hook_path(),
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
fn register_then_channel_leave_still_listens() {
    // famp_leave is a channel operation (requires a `channel` param), NOT an
    // unregister. Leaving a channel must NOT cancel listen mode.
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, true); // with_leave_after=true

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    let log_contents = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !log_contents.is_empty(),
        "expected famp await to be invoked even after a channel famp_leave"
    );
    assert!(
        log_contents.contains("await") && log_contents.contains("dk"),
        "expected 'await --as dk' in mock famp log, got: {log_contents}"
    );
}

#[test]
fn no_register_in_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    std::fs::write(
        &transcript,
        r#"{"type":"user","message":{"role":"user","content":"hello"}}"#,
    )
    .unwrap();

    let out = run_hook(
        &asset_hook_path(),
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
        &asset_hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(
        out.status.success(),
        "hook must exit 0 on missing transcript"
    );
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
        &asset_hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(
        out.status.success(),
        "hook must exit 0 on malformed transcript"
    );
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for malformed transcript"
    );
}

#[test]
fn last_registration_wins_when_multiple_in_transcript() {
    require_hook!();
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

#[test]
fn block_decision_is_notification_only_no_envelope_bytes() {
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that prints a fake envelope when called with `await`
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        r#"#!/usr/bin/env bash
if [ "$1" = "await" ]; then
    printf '{"from":"alice","body":{"details":{"summary":"SECRET_PAYLOAD"}}}\n'
fi
exit 0
"#,
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Stage a jq shim so the test works on CI hosts where jq may not be installed.
    // The hook requires jq to emit the block-decision JSON; without it, it exits 0 silently.
    // The shim delegates to real jq if found, else falls back to python3 for the specific
    // `jq -n --arg KEY VALUE '{decision:...,reason:...}'` invocation the hook uses.
    let jq = bin_dir.join("jq");
    std::fs::write(
        &jq,
        r#"#!/usr/bin/env bash
for candidate in /opt/homebrew/bin/jq /usr/local/bin/jq /usr/bin/jq; do
    [ -x "$candidate" ] && exec "$candidate" "$@"
done
# Minimal python3 fallback: handles `jq -n --arg KEY VALUE FILTER`
python3 - "$@" << 'PY'
import json, sys
args = sys.argv[1:]
obj = {}
i = 0
while i < len(args):
    if args[i] == '--arg':
        obj[args[i+1]] = args[i+2]; i += 3
    else:
        i += 1
print(json.dumps({'decision': 'block', 'reason': obj.get('r', '')}))
PY
"#,
    )
    .unwrap();
    std::fs::set_permissions(&jq, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "bob", true, true, false);

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Must be valid JSON with decision=block
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}\nstdout={stdout:?}"));
    assert_eq!(v["decision"], "block", "stdout: {stdout}");

    // Peer-controlled content must NOT appear in reason
    let reason = v["reason"].as_str().unwrap_or("");
    assert!(
        !reason.contains("SECRET_PAYLOAD"),
        "peer bytes leaked into reason field: {reason:?}"
    );

    // Reason must mention famp_inbox
    assert!(
        reason.contains("famp_inbox"),
        "reason must direct agent to call famp_inbox: {reason:?}"
    );
}

#[test]
fn timeout_exits_zero_with_no_stdout() {
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that exits 0 with no output (simulates timeout)
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(&famp, "#!/usr/bin/env bash\nexit 0\n").unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);
    let _log = dir.path().join("famp.log");

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(out.status.success(), "must exit 0 on timeout");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "no stdout expected on timeout: {stdout:?}"
    );
}

#[test]
fn broker_error_fails_open_exit_zero() {
    require_hook!();
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that exits non-zero with no stdout (broker unreachable)
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        "#!/usr/bin/env bash\nprintf 'broker unreachable' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(
        out.status.success(),
        "must fail-open (exit 0) on broker error"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "no stdout expected on broker error"
    );
}

#[test]
fn identity_with_shell_metacharacters_is_noop() {
    require_hook!();
    // A crafted transcript with an identity containing shell metacharacters must
    // be rejected before any subprocess is invoked. The hook's identity validation
    // guard (`case $'\n'` + grep) must catch this; if it doesn't, the mock famp
    // would be called and leave an argv log entry.
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");

    // Identity with a shell-injection attempt and a space (both invalid per [A-Za-z0-9._-]+)
    let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"$(evil cmd)","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":[{"type":"text","text":"registered as evil"}]}]}}
"#;
    std::fs::write(&transcript, body).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(
        out.status.success(),
        "hook must exit 0 on metacharacter identity"
    );
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "hook must not invoke famp for invalid identity"
    );
}

// ── issue #21: cancellation-seam watcher tests ─────────────────────────────
//
// These exercise the ASSET hook (`asset_hook_path()`), driving its transcript
// queue-watcher via a mock `famp` whose `await --abort-on-fd <fd>` reads one
// byte from <fd>: a byte (written by the hook's watcher when the predicate
// fires) => abort (exit 3); a timeout => no message (exit 0). Whether the hook
// aborted is observed through its own log line, since both abort and
// timeout end the hook with exit 0 and no stdout.

/// Mock `famp` for the #21 tests: reads one byte from the `--abort-on-fd`
/// fd with a short timeout, exiting 3 (abort) on a byte or 0 (no message)
/// on timeout. Lets a test assert the HOOK's watcher/predicate behaviour
/// without a real bus.
fn stage_abort_mock_famp(bin_dir: &Path) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        r#"#!/usr/bin/env bash
fd=""
prev=""
for a in "$@"; do
    if [ "$prev" = "--abort-on-fd" ]; then fd="$a"; fi
    prev="$a"
done
if [ "$1" = "await" ] && [ -n "$fd" ]; then
    if read -t 3 -r -n 1 _ <&"$fd" 2>/dev/null; then
        printf '{"aborted":true}\n'
        exit 3
    fi
fi
exit 0
"#,
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Write a listen-mode transcript: a successful `famp_register(listen:true)`
/// followed by `extra` raw JSONL lines (queue-operation records, etc.).
fn write_listen_transcript(path: &Path, identity: &str, extra: &str) {
    let body = format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"toolu_reg","name":"mcp__famp__famp_register","input":{{"identity":"{identity}","listen":true}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_reg","is_error":false,"content":[{{"type":"text","text":"registered as {identity}"}}]}}]}}}}
{extra}
"#
    );
    std::fs::write(path, body).unwrap();
}

fn await_hook_log(xdg: &Path) -> String {
    std::fs::read_to_string(xdg.join("famp/await-hook.log")).unwrap_or_default()
}

/// Spawn the ASSET hook with a fast (1s) watcher interval so tests don't
/// wait the production 2s cadence.
fn spawn_asset_hook(transcript: &Path, bin_dir: &Path, xdg: &Path) -> std::process::Child {
    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(asset_hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", xdg)
        .env("FAMP_QWATCH_INTERVAL", "1")
        // Same hermetic default as `run_hook` — #26 / no-op tests must not
        // adopt a live sibling `famp mcp` from the host process tree.
        .env("FAMP_DISABLE_PID_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes());
    drop(child.stdin.take());
    child
}

const ABORT_LOG_LINE: &str = "aborted: host queue has pending input";

/// TEST 5 (C2 — pre-existing enqueue): a background agent that finished
/// mid-turn lands its enqueue BEFORE the Stop hook runs, so no *new* enqueue
/// ever arrives. The predicate is "outstanding right now" (enqueues >
/// dequeues), so the hook still aborts.
#[test]
fn hook_aborts_when_transcript_has_outstanding_enqueue() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(
        &transcript,
        "dk",
        r#"{"type":"queue-operation","operation":"enqueue","content":"pending notif"}"#,
    );

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        log.contains(ABORT_LOG_LINE),
        "hook must abort on an outstanding enqueue (C2); log:\n{log}"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "abort must emit NO block decision"
    );
}

/// TEST 6 (the observed bug): the transcript starts drained, then a
/// background agent finishes WHILE the hook blocks and appends a fresh
/// enqueue. The polling watcher catches it and aborts.
#[test]
fn hook_aborts_when_enqueue_appears_while_blocked() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    // Start balanced (one enqueue already drained) => no abort initially.
    write_listen_transcript(
        &transcript,
        "dk",
        "{\"type\":\"queue-operation\",\"operation\":\"enqueue\",\"content\":\"old\"}\n{\"type\":\"queue-operation\",\"operation\":\"dequeue\"}",
    );

    let child = spawn_asset_hook(&transcript, &bin_dir, &xdg);
    // While blocked, a background agent completes: append a NEW enqueue.
    std::thread::sleep(Duration::from_millis(600));
    {
        let mut f = OpenOptions::new().append(true).open(&transcript).unwrap();
        writeln!(
            f,
            r#"{{"type":"queue-operation","operation":"enqueue","content":"new notif"}}"#
        )
        .unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        log.contains(ABORT_LOG_LINE),
        "hook must abort when an enqueue appears mid-block; log:\n{log}"
    );
}

/// TEST 7 (drained): an enqueue matched by a dequeue is NOT outstanding —
/// keep blocking, do not abort.
#[test]
fn hook_does_not_abort_when_enqueue_is_matched_by_dequeue() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(
        &transcript,
        "dk",
        "{\"type\":\"queue-operation\",\"operation\":\"enqueue\",\"content\":\"x\"}\n{\"type\":\"queue-operation\",\"operation\":\"dequeue\"}",
    );

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        !log.contains(ABORT_LOG_LINE),
        "hook must NOT abort on a drained queue; log:\n{log}"
    );
}

/// TEST 8 (C1 regression): a normal assistant message whose `content`
/// embeds the literal string `"operation":"enqueue"`. A grep implementation
/// aborts here (false positive that silently kills listen mode); a JSON
/// parse checks `type == "queue-operation"` and does not.
#[test]
fn hook_does_not_abort_on_a_nested_non_toplevel_queue_operation() {
    // A record whose TOP-LEVEL type is not `queue-operation`, but which
    // embeds an unescaped nested object carrying those keys — the shape a
    // structured tool result takes. The literal bytes
    // `"operation":"enqueue"` are present and a substring grep matches
    // them, but this is not a host queue operation and must not abort.
    //
    // (Text inside a JSON *string* would be escaped as `\"operation\":...`
    // and could not match a grep, so a nested object — not a content
    // string — is the shape that actually distinguishes the two
    // implementations.)
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(
        &transcript,
        "dk",
        r#"{"type":"user","toolUseResult":{"type":"queue-operation","operation":"enqueue"}}"#,
    );

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        !log.contains(ABORT_LOG_LINE),
        "a nested, non-top-level queue-operation must NOT abort; log:\n{log}"
    );
}

#[test]
fn hook_does_not_abort_when_the_enqueued_item_was_removed() {
    // `remove` is a queued message the user deleted before it ran. It is a
    // DRAIN, and it is the op that breaks a naive enqueue/dequeue counter:
    // the counter never decrements, so it latches positive and aborts every
    // subsequent Stop hook, silently disabling listen mode for the rest of
    // the session. Measured across 96 real transcripts: {enqueue: 710,
    // dequeue: 434, remove: 269, popAll: 6}.
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(
        &transcript,
        "dk",
        concat!(
            r#"{"type":"queue-operation","operation":"enqueue","content":"queued then deleted"}"#,
            "\n",
            r#"{"type":"queue-operation","operation":"remove"}"#
        ),
    );

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        !log.contains(ABORT_LOG_LINE),
        "`remove` drains the queue — hook must NOT abort; log:\n{log}"
    );
}

#[test]
fn hook_does_not_abort_when_the_queue_was_pop_all_ed() {
    // `popAll` drains every queued item at once. Same latch hazard as
    // `remove`: a counter that only knows enqueue/dequeue never clears.
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_abort_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(
        &transcript,
        "dk",
        concat!(
            r#"{"type":"queue-operation","operation":"enqueue","content":"a"}"#,
            "\n",
            r#"{"type":"queue-operation","operation":"enqueue","content":"b"}"#,
            "\n",
            r#"{"type":"queue-operation","operation":"popAll"}"#
        ),
    );

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let log = await_hook_log(&xdg);
    assert!(
        !log.contains(ABORT_LOG_LINE),
        "`popAll` drains the queue — hook must NOT abort; log:\n{log}"
    );
}

/// Mock `famp` for #26: `await` returns a large agent-mailbox batch
/// (historical replay), while `inspect identities --json` reports the
/// disk-ack unread count (and last_sender) that `famp_inbox` would see.
fn stage_overcount_mock_famp(bin_dir: &Path, unread: u64, last_sender: &str) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    // Sixteen historical envelopes from worker-2, matching the beta report.
    let mut envelopes = String::from("[");
    for i in 0..16 {
        if i > 0 {
            envelopes.push(',');
        }
        envelopes.push_str(r#"{"from":"worker-2","body":"old"}"#);
    }
    envelopes.push(']');
    let script = format!(
        r#"#!/usr/bin/env bash
if [ "$1" = "await" ]; then
  printf '%s\n' '{{"mailbox":{{"kind":"agent","name":"dk"}},"envelopes":{envelopes},"next_offset":999}}'
  exit 0
fi
if [ "$1" = "inspect" ] && [ "$2" = "identities" ]; then
  cat <<'EOF'
{{
  "rows": [
    {{
      "name": "dk",
      "listen_mode": true,
      "cwd": null,
      "registered_at_unix_seconds": 0,
      "last_activity_unix_seconds": 0,
      "mailbox_unread": {unread},
      "mailbox_total": 16,
      "last_sender": "{last_sender}",
      "last_received_at_unix_seconds": null
    }}
  ]
}}
EOF
  exit 0
fi
exit 0
"#
    );
    std::fs::write(&famp, script).unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// #26: when await replays already-acked envelopes (batch=16) but disk-ack
/// unread is 0, the hook must NOT claim "16 new" — exit 0 with no block.
#[test]
fn hook_suppresses_wake_when_disk_ack_unread_is_zero() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_overcount_mock_famp(&bin_dir, 0, "(none)");
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(&transcript, "dk", "");

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "unread=0 must emit NO block decision; stdout:\n{stdout}"
    );
    let log = await_hook_log(&xdg);
    assert!(
        log.contains("disk-ack unread=0"),
        "log must record the #26 no-actionable-wake path; log:\n{log}"
    );
    assert!(
        !stdout.contains("16 new"),
        "must not announce the await-batch size as new"
    );
}

/// #26: when await batch is oversized relative to disk-ack unread, the
/// notification must report the actionable unread count (and its sender).
#[test]
fn hook_reports_disk_ack_unread_not_await_batch_size() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_overcount_mock_famp(&bin_dir, 1, "worker-2");
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(&transcript, "dk", "");

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("New FAMP message from worker-2"),
        "must report singular actionable message from disk-ack sender; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("16 new"),
        "must not over-count from the await batch; stdout:\n{stdout}"
    );
    let log = await_hook_log(&xdg);
    assert!(
        log.contains("reduced to disk-ack unread=1") || log.contains("count=1"),
        "log should show the reduced count; log:\n{log}"
    );
}

/// Mock `famp` for the delimiter-injection regression: `await` returns one
/// agent-mailbox envelope whose `from` carries a literal `|` (`evil|channel`),
/// and `inspect identities` reports a clean actionable unread from a normal
/// sender. Models a non-conformant/malicious peer on the shared bus.
fn stage_pipe_injection_mock_famp(bin_dir: &Path) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    let script = r#"#!/usr/bin/env bash
if [ "$1" = "await" ]; then
  printf '%s\n' '{"mailbox":{"kind":"agent","name":"dk"},"envelopes":[{"from":"evil|channel","body":"x"}],"next_offset":5}'
  exit 0
fi
if [ "$1" = "inspect" ] && [ "$2" = "identities" ]; then
  printf '%s\n' '{"rows":[{"name":"dk","listen_mode":true,"mailbox_unread":1,"mailbox_total":1,"last_sender":"worker-2"}]}'
  exit 0
fi
exit 0
"#;
    std::fs::write(&famp, script).unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// SECURITY (#2 review): a peer-controlled `from` containing a literal `|`
/// must not shift the shim's positional `count|sender|kind|name` META parse
/// and flip MAILBOX_KIND to "channel". Flipping it would bypass the #26
/// disk-ack gate and misdirect the wake to `famp_channel_log`. Falsification
/// control: WITHOUT the `_clean()` fix this test routes to channel_log and
/// fails; WITH it the agent mailbox stays an agent mailbox → famp_inbox.
#[test]
fn hook_rejects_pipe_injection_in_sender() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    stage_pipe_injection_mock_famp(&bin_dir);
    let transcript = dir.path().join("t.jsonl");
    write_listen_transcript(&transcript, "dk", "");

    let out = spawn_asset_hook(&transcript, &bin_dir, &xdg)
        .wait_with_output()
        .unwrap();
    assert!(out.status.success(), "hook must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Call famp_inbox"),
        "agent mailbox must route to famp_inbox; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("famp_channel_log"),
        "a `|` in `from` must not flip the mailbox kind to channel; stdout:\n{stdout}"
    );
}

/// Missing `transcript_path` → PID-correlated fallback resolves identity
/// from a sibling `famp mcp` in the process tree (asset hook; PID fallback
/// ENABLED — do NOT set FAMP_DISABLE_PID_FALLBACK).
///
/// Process tree:
/// ```text
/// supervisor (S)
///   ├── fake "famp mcp" (M)   — argv contains "famp mcp", ppid=S
///   └── bash famp-await.sh (H) — ancestors include S
/// ```
#[test]
fn missing_transcript_path_uses_pid_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();

    // Mock famp: sessions / inspect identities / await — log argv always.
    let famp = bin_dir.join("famp");
    let mock = format!(
        r#"#!/usr/bin/env bash
printf '%s\n' "$*" >> "{log}"
case "$1" in
  sessions)
    # JSONL row for the sibling mcp pid (injected via FAMP_TEST_MCP_PID).
    printf '%s\n' "{{\"pid\":${{FAMP_TEST_MCP_PID:-0}},\"name\":\"pid-fb-test\"}}"
    exit 0
    ;;
  inspect)
    if [ "$2" = "identities" ]; then
      printf '%s\n' '{{"rows":[{{"name":"pid-fb-test","listen_mode":true}}]}}'
      exit 0
    fi
    exit 0
    ;;
  await)
    # Immediate timeout so the hook does not park for 23h.
    printf '%s\n' '{{"timeout":true}}'
    exit 0
    ;;
esac
exit 0
"#,
        log = log.display()
    );
    std::fs::write(&famp, mock).unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let supervisor = dir.path().join("supervisor.sh");
    let hook = asset_hook_path();
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    // Supervisor parents both the fake mcp and the hook.
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
# Child whose argv contains the literal substring "famp mcp".
bash -c 'exec -a "famp mcp" sleep 120' &
MCP_PID=$!
export FAMP_TEST_MCP_PID="$MCP_PID"
# Give the exec a moment to rewrite argv.
sleep 0.15
# Run the asset hook with NO transcript_path; PID fallback enabled.
printf '%s' '{{"hook_event_name":"Stop"}}' | \
  env -u FAMP_DISABLE_PID_FALLBACK \
      PATH="{path}" \
      XDG_STATE_HOME="{xdg}" \
      FAMP_TEST_MCP_PID="$MCP_PID" \
      bash "{hook}"
STATUS=$?
kill "$MCP_PID" 2>/dev/null || true
wait "$MCP_PID" 2>/dev/null || true
exit "$STATUS"
"#,
        path = new_path,
        xdg = xdg.display(),
        hook = hook.display(),
    );
    std::fs::write(&supervisor, script).unwrap();
    std::fs::set_permissions(&supervisor, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = Command::new("bash")
        .arg(&supervisor)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "supervisor/hook failed: status={:?}\nstderr={}\nstdout={}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await --as pid-fb-test"),
        "expected famp await --as pid-fb-test via PID fallback; mock log:\n{argv}\nhook log:\n{}",
        await_hook_log(&xdg)
    );
}
