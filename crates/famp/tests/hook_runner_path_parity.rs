#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

//! HOOK-04b path-parity tests.
//!
//! These tests assert that `crates/famp/assets/hook-runner.sh` reads its
//! `hooks.tsv` from the same path that `scripts/famp-local hook add` writes to:
//! `${FAMP_LOCAL_ROOT:-$HOME/.famp-local}/hooks.tsv`.
//!
//! Phase 05-02, requirement HOOK-04b. The runner originally hardcoded
//! `${HOME}/.famp-local/hooks.tsv` at line 9; if a user set `FAMP_LOCAL_ROOT`
//! to a non-default location the writer would honor it but the runner would
//! silently miss every match. These two tests lock the parity in place.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn shim_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("hook-runner.sh")
}

fn stage_fake_famp(bin_dir: &Path) -> PathBuf {
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

fn write_transcript(path: &Path, files: &[&str]) {
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

fn run_shim(
    home: &Path,
    famp_local_root: Option<&Path>,
    bin_dir: &Path,
    log: &Path,
    transcript: &Path,
) -> std::process::Output {
    let stop_json = format!(
        r#"{{"transcript_path":"{}","session_id":"s1","cwd":"/tmp","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut cmd = Command::new("bash");
    cmd.arg(shim_path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", log);
    if let Some(root) = famp_local_root {
        cmd.env("FAMP_LOCAL_ROOT", root);
    }
    let mut child = cmd
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
    child.wait_with_output().unwrap()
}

/// HOOK-04b path parity: when `FAMP_LOCAL_ROOT` is set to a non-default path,
/// the runner MUST read `hooks.tsv` from that path. With the writer
/// (`scripts/famp-local hook add`) honoring the same env var, this test
/// proves end-to-end parity.
///
/// Setup: hooks.tsv lives ONLY at `$FAMP_LOCAL_ROOT/hooks.tsv`. There is no
/// `$HOME/.famp-local/hooks.tsv`. If the runner reads the hardcoded
/// `$HOME/.famp-local/hooks.tsv` path, no match fires; if it honors
/// `FAMP_LOCAL_ROOT`, the stub `famp` is invoked once with `--to alice`.
#[test]
fn test_hook_runner_honors_famp_local_root() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let root = dir.path().join("custom-root");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&root).unwrap();

    let bin_dir = dir.path().join("bin");
    let _famp = stage_fake_famp(&bin_dir);
    let log = dir.path().join("famp.log");

    // Hook row in NON-default path:
    std::fs::write(
        root.join("hooks.tsv"),
        "h1\tEdit:**/*.md\talice\t2026-05-03T00:00:00Z\n",
    )
    .unwrap();

    // Make sure $HOME/.famp-local does NOT have a hooks.tsv — defensive.
    let default_dir = home.join(".famp-local");
    std::fs::create_dir_all(&default_dir).unwrap();
    // Intentionally no hooks.tsv at default location.

    let edited = home.join("README.md");
    std::fs::write(&edited, "x").unwrap();
    let transcript = home.join("transcript.jsonl");
    write_transcript(&transcript, &[edited.to_str().unwrap()]);

    let out = run_shim(&home, Some(&root), &bin_dir, &log, &transcript);
    assert!(
        out.status.success(),
        "runner must exit 0 (Stop hook contract). stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.contains("send") && calls.contains("--to") && calls.contains("alice"),
        "expected `famp send --to alice` fired from FAMP_LOCAL_ROOT path; got: {calls:?}"
    );
}

/// HOOK-04b default-path fallback: when `FAMP_LOCAL_ROOT` is unset, the
/// runner MUST fall back to `$HOME/.famp-local/hooks.tsv`, matching the
/// writer's default.
#[test]
fn test_hook_runner_default_path_when_root_unset() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let bin_dir = dir.path().join("bin");
    let _famp = stage_fake_famp(&bin_dir);
    let log = dir.path().join("famp.log");

    // Hook row in DEFAULT path under the test's HOME:
    let default_dir = home.join(".famp-local");
    std::fs::create_dir_all(&default_dir).unwrap();
    std::fs::write(
        default_dir.join("hooks.tsv"),
        "h1\tEdit:**/*.md\tbob\t2026-05-03T00:00:00Z\n",
    )
    .unwrap();

    let edited = home.join("X.md");
    std::fs::write(&edited, "x").unwrap();
    let transcript = home.join("transcript.jsonl");
    write_transcript(&transcript, &[edited.to_str().unwrap()]);

    // FAMP_LOCAL_ROOT INTENTIONALLY unset:
    let out = run_shim(&home, None, &bin_dir, &log, &transcript);
    assert!(
        out.status.success(),
        "runner must exit 0 (Stop hook contract). stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.contains("send") && calls.contains("--to") && calls.contains("bob"),
        "expected default-path hook to fire (--to bob); got: {calls:?}"
    );
}
