#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn shim_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("hook-runner.sh")
}

fn run_shim(home: &std::path::Path, path: &str, stdin: Option<&str>) -> std::process::Output {
    let mut child = Command::new("bash")
        .arg(shim_path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(s) = stdin {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

#[test]
fn shim_exits_zero_on_no_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_shim(dir.path(), "/usr/bin:/bin", None);
    assert!(out.status.success(), "exit = {:?}", out.status.code());
}

#[test]
fn shim_exits_zero_on_malformed_json() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_shim(dir.path(), "/usr/bin:/bin", Some("not json {{{"));
    assert!(out.status.success());
}

#[test]
fn shim_exits_zero_when_transcript_path_missing_from_json() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_shim(
        dir.path(),
        "/usr/bin:/bin",
        Some(r#"{"hook_event_name":"Stop"}"#),
    );
    assert!(out.status.success());
}

#[test]
fn shim_exits_zero_when_transcript_file_does_not_exist() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_shim(
        dir.path(),
        "/usr/bin:/bin",
        Some(r#"{"transcript_path":"/nonexistent/path.jsonl","hook_event_name":"Stop"}"#),
    );
    assert!(out.status.success());
}

#[test]
fn shim_exits_zero_when_hooks_tsv_absent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let transcript = home.join("transcript.jsonl");
    std::fs::write(
        &transcript,
        r#"{"role":"assistant","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/tmp/x.rs"}}]}"#,
    )
    .unwrap();
    let out = run_shim(
        home,
        "/usr/bin:/bin",
        Some(&format!(
            r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
            transcript.display()
        )),
    );
    assert!(out.status.success());
}

#[test]
fn shim_exits_zero_when_famp_binary_not_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let famp_local = home.join(".famp-local");
    std::fs::create_dir_all(&famp_local).unwrap();
    std::fs::write(
        famp_local.join("hooks.tsv"),
        "h1\tEdit:**/*.rs\tbob\t2026-05-02\n",
    )
    .unwrap();
    let transcript = home.join("transcript.jsonl");
    std::fs::write(
        &transcript,
        r#"{"role":"assistant","content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/tmp/x.rs"}}]}"#,
    )
    .unwrap();
    let out = run_shim(
        home,
        "/usr/bin:/bin",
        Some(&format!(
            r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
            transcript.display()
        )),
    );
    assert!(out.status.success(), "exit = {:?}", out.status.code());
}
