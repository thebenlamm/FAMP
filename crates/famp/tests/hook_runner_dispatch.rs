#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn shim_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("hook-runner.sh")
}

fn stage_fake_famp(bin_dir: &std::path::Path) -> PathBuf {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> \"$FAKE_FAMP_LOG\"\nexit 0\n",
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
    famp
}

fn write_transcript(path: &std::path::Path, files: &[&str]) {
    let mut blocks = Vec::new();
    for f in files {
        blocks.push(format!(
            r#"{{"type":"tool_use","name":"Edit","input":{{"file_path":"{f}"}}}}"#
        ));
    }
    let body = format!(
        "{{\"role\":\"user\",\"content\":[{{\"type\":\"text\",\"text\":\"hi\"}}]}}\n{{\"role\":\"assistant\",\"content\":[{}]}}\n",
        blocks.join(",")
    );
    std::fs::write(path, body).unwrap();
}

fn write_claude_code_transcript(path: &std::path::Path, file: &str) {
    let body = format!(
        r#"{{"type":"user","message":{{"role":"user","content":"edit file"}}}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Write","input":{{"file_path":"{file}","content":"uat\n"}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"tool_use_id":"toolu_1","type":"tool_result","content":"ok"}}]}}}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"done"}}]}}}}
"#
    );
    std::fs::write(path, body).unwrap();
}

#[test]
fn shim_dispatches_one_send_per_matching_glob() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let bin_dir = home.join("bin");
    let _famp = stage_fake_famp(&bin_dir);
    let log = home.join("famp.log");
    let famp_local = home.join(".famp-local");
    std::fs::create_dir_all(&famp_local).unwrap();
    std::fs::write(
        famp_local.join("hooks.tsv"),
        "h1\tEdit:**/*.rs\tbob\t2026-05-02T00:00:00Z\nh2\tEdit:**/*.py\talice\t2026-05-02T00:00:00Z\n",
    )
    .unwrap();
    let transcript = home.join("transcript.jsonl");
    write_transcript(
        &transcript,
        &["/tmp/foo.rs", "/tmp/src/lib.rs", "/tmp/bar.md"],
    );
    let stop_json = format!(
        r#"{{"transcript_path":"{}","session_id":"s1","cwd":"/tmp","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut child = Command::new("bash")
        .arg(shim_path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "shim exit = {:?}, stderr = {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let log_body = std::fs::read_to_string(&log).unwrap_or_default();
    let lines: Vec<&str> = log_body.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "expected 1 famp invocation, got {}: {log_body:?}",
        lines.len()
    );
    assert!(lines[0].contains("--to bob"), "args: {}", lines[0]);
    assert!(lines[0].contains("send"));
}

#[test]
fn shim_dispatches_from_real_claude_code_nested_message_shape() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let bin_dir = home.join("bin");
    let _famp = stage_fake_famp(&bin_dir);
    let log = home.join("famp.log");
    let famp_local = home.join(".famp-local");
    std::fs::create_dir_all(&famp_local).unwrap();
    std::fs::write(
        famp_local.join("hooks.tsv"),
        "h1\tEdit:*STOP_HOOK_UAT.md\tbob\t2026-05-03T00:00:00Z\n",
    )
    .unwrap();
    let transcript = home.join("transcript.jsonl");
    let body = r#"{"type":"user","message":{"role":"user","content":"/famp-register alice"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"mcp__famp__famp_register","input":{"identity":"alice"}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":[{"type":"text","text":"{\"active\":\"alice\"}"}]}]}}
"#;
    std::fs::write(&transcript, body).unwrap();
    let mut existing = std::fs::read_to_string(&transcript).unwrap();
    existing.push_str(
        &std::fs::read_to_string({
            let nested = home.join("nested-transcript.jsonl");
            write_claude_code_transcript(&nested, "/Users/benlamm/Workspace/FAMP/STOP_HOOK_UAT.md");
            nested
        })
        .unwrap(),
    );
    std::fs::write(&transcript, existing).unwrap();
    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut child = Command::new("bash")
        .arg(shim_path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "shim exit = {:?}, stderr = {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let log_body = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(log_body.contains("--as alice"), "args: {log_body}");
    assert!(log_body.contains("--to bob"), "args: {log_body}");
    assert!(log_body.contains("send"), "args: {log_body}");
}

#[test]
fn shim_dispatches_zero_when_no_globs_match() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let bin_dir = home.join("bin");
    let _ = stage_fake_famp(&bin_dir);
    let log = home.join("famp.log");
    let famp_local = home.join(".famp-local");
    std::fs::create_dir_all(&famp_local).unwrap();
    std::fs::write(
        famp_local.join("hooks.tsv"),
        "h1\tEdit:**/*.rs\tbob\t2026-05-02\n",
    )
    .unwrap();
    let transcript = home.join("transcript.jsonl");
    write_transcript(&transcript, &["/tmp/only.md"]);
    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(shim_path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(!log.exists() || std::fs::read_to_string(&log).unwrap().is_empty());
}
