#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 02 plan 02-12 — `famp sessions [--me]` integration test
//! (CLI-07).
//!
//! Two `famp register` holders (alice + bob), `famp sessions` lists
//! both as JSONL rows; `famp sessions --me` (with
//! `FAMP_LOCAL_IDENTITY=alice`) filters to a single row.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

struct Bus {
    tmp: tempfile::TempDir,
    sock: std::path::PathBuf,
}

impl Bus {
    fn new() -> Self {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        Self { tmp, sock }
    }

    fn sock(&self) -> &Path {
        self.sock.as_path()
    }

    fn famp_cmd(&self, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .output()
            .unwrap()
    }

    fn famp_spawn(&self, args: &[&str]) -> Child {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn wait_for_register(&self, name: &str) {
        for _ in 0..50 {
            let out = self.famp_cmd(&["whoami", "--as", name]);
            if out.status.success() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("{name} did not register within 5s");
    }
}

fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// CLI-07: `famp sessions` (no flag) lists every live registered
/// holder; `famp sessions --me` filters to the caller's resolved
/// identity. Two holders → 2 rows; `--me` with
/// `FAMP_LOCAL_IDENTITY=alice` → 1 row.
#[test]
fn test_sessions_list() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    let mut bob = bus.famp_spawn(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    // Unfiltered: 2 rows, one per holder.
    let out = bus.famp_cmd(&["sessions"]);
    assert!(
        out.status.success(),
        "sessions failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        2,
        "expected 2 sessions, got {} in {stdout}",
        lines.len()
    );
    assert!(
        stdout.contains("\"alice\""),
        "sessions output should include alice: {stdout}"
    );
    assert!(
        stdout.contains("\"bob\""),
        "sessions output should include bob: {stdout}"
    );

    // `--me` with FAMP_LOCAL_IDENTITY=alice should resolve identity to
    // alice and filter to a single row.
    let me_out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", bus.sock())
        .env("HOME", bus.tmp.path())
        .env("FAMP_LOCAL_IDENTITY", "alice")
        .args(["sessions", "--me"])
        .output()
        .unwrap();
    assert!(
        me_out.status.success(),
        "sessions --me failed: stderr={}",
        String::from_utf8_lossy(&me_out.stderr)
    );
    let me_stdout = String::from_utf8_lossy(&me_out.stdout);
    let me_lines: Vec<&str> = me_stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        me_lines.len(),
        1,
        "expected 1 session with --me, got {} in {me_stdout}",
        me_lines.len()
    );
    assert!(
        me_stdout.contains("\"alice\""),
        "--me output should include alice: {me_stdout}"
    );
    assert!(
        !me_stdout.contains("\"bob\""),
        "--me output should NOT include bob: {me_stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);

    // Anti-flake guard: `--me` matched on `--me=alice` only because
    // `alice` was still alive. Document that so future readers don't
    // accidentally remove the `wait_for_register` calls above.
    let _ = Duration::from_millis(0);
}
