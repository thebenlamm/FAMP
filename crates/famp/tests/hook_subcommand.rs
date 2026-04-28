#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! HOOK-04a integration tests for `scripts/famp-local hook add|list|remove`.
//!
//! Spawns the bash script with a sandboxed `FAMP_LOCAL_ROOT` so each test
//! gets its own hooks.tsv. NOTE: the script's state dir env is
//! `FAMP_LOCAL_ROOT` (not `STATE_ROOT` — that's an internal shell variable
//! computed from `FAMP_LOCAL_ROOT`). The 02-10 plan text used `STATE_ROOT`;
//! corrected here per Rule 1 (would have left the env var ignored at runtime
//! and silently exercised the user's real `~/.famp-local`).

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn run_local(state_root: &std::path::Path, args: &[&str]) -> std::process::Output {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("scripts/famp-local");
    Command::new("bash")
        .arg(&script)
        .args(args)
        .env("FAMP_LOCAL_ROOT", state_root)
        .env_remove("FAMP_HOME")
        .output()
        .expect("scripts/famp-local must run")
}

#[test]
fn test_hook_add() {
    let tmp = TempDir::new().unwrap();
    let out = run_local(
        tmp.path(),
        &["hook", "add", "--on", "Edit:*.md", "--to", "alice"],
    );
    assert!(
        out.status.success(),
        "hook add failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("hook added: id=h"),
        "unexpected stdout: {stdout}"
    );
    assert!(stdout.contains("on=Edit:*.md"));
    assert!(stdout.contains("to=alice"));
    // HOOK-02: verify TSV row format
    let tsv = std::fs::read_to_string(tmp.path().join("hooks.tsv")).unwrap();
    let line = tsv.lines().next().unwrap();
    let parts: Vec<&str> = line.split('\t').collect();
    assert_eq!(
        parts.len(),
        4,
        "TSV must have exactly 4 fields: id, event:glob, to, added_at"
    );
    assert!(parts[0].starts_with('h'), "id must start with 'h'");
    assert_eq!(parts[1], "Edit:*.md");
    assert_eq!(parts[2], "alice");
    assert!(
        parts[3].contains('T') && parts[3].ends_with('Z'),
        "added_at must be ISO-8601 UTC"
    );
}

#[test]
fn test_hook_list() {
    let tmp = TempDir::new().unwrap();
    let _ = run_local(
        tmp.path(),
        &["hook", "add", "--on", "Edit:*.md", "--to", "alice"],
    );
    let _ = run_local(
        tmp.path(),
        &["hook", "add", "--on", "Edit:src/**/*.rs", "--to", "#planning"],
    );
    let out = run_local(tmp.path(), &["hook", "list"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().count(), 2, "list must return both rows");
    assert!(stdout.contains("Edit:*.md\talice"));
    assert!(stdout.contains("Edit:src/**/*.rs\t#planning"));
}

#[test]
fn test_hook_remove() {
    let tmp = TempDir::new().unwrap();
    let add = run_local(
        tmp.path(),
        &["hook", "add", "--on", "Edit:*.md", "--to", "alice"],
    );
    let stdout = String::from_utf8_lossy(&add.stdout);
    // Parse "hook added: id=h<id> on=..." -> extract id
    let id = stdout
        .split_whitespace()
        .find_map(|w| w.strip_prefix("id="))
        .expect("id token");
    let rm = run_local(tmp.path(), &["hook", "remove", id]);
    assert!(
        rm.status.success(),
        "remove failed: {}",
        String::from_utf8_lossy(&rm.stderr)
    );
    let after = run_local(tmp.path(), &["hook", "list"]);
    let after_stdout = String::from_utf8_lossy(&after.stdout);
    assert!(
        after_stdout.is_empty() || after_stdout == "no hooks registered\n",
        "after remove, list must be empty: got {after_stdout:?}"
    );
    // Removing a non-existent id should fail
    let rm2 = run_local(tmp.path(), &["hook", "remove", "h-not-real"]);
    assert!(
        !rm2.status.success(),
        "remove of non-existent id must fail"
    );
    let stderr = String::from_utf8_lossy(&rm2.stderr);
    assert!(
        stderr.contains("hook id 'h-not-real' not found"),
        "stderr: {stderr}"
    );
}
