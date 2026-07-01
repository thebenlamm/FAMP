#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Debug 999.1 reproducer — `famp await` task-filter vs broker `await_offset`
//! ordering.
//!
//! ## Falsify-first background
//!
//! The original 999.1 SPEC (`.planning/phases/999.1-.../SPEC.md`) described
//! three mechanisms (M1/M2/M3) all keyed on the v0.8 client-side
//! `await_cmd/poll.rs` file-cursor + `cli/send/fsm_glue.rs` on-disk
//! `TaskRecord` FSM. Falsify-first investigation (2026-07-01, see debug file
//! `.planning/debug/999-1-await-fsm-ordering.md`) confirmed all three are
//! DEAD in v0.11: `poll.rs` is `#[allow(dead_code)]`, `fsm_glue.rs` no longer
//! exists, and no production code path writes a mutable on-disk task-FSM
//! record. The SPEC's preferred fix (broker-owned FSM derivation) also did
//! NOT land — `famp-bus` has zero FSM vocabulary; the `include_terminal`
//! filter is a wire-accepted no-op (v1 scope, per code comment in
//! `crates/famp-bus/src/broker/handle.rs`).
//!
//! BUT the same architectural flaw — a cursor/offset that leaps past
//! non-matching entries with no accounting for what was skipped — was
//! reincarnated at the broker layer: `drain_await_batch`
//! (`crates/famp-bus/src/broker/awaiting.rs`) advances `next_offset`
//! unconditionally for every drained line, regardless of whether
//! `filter_matches` accepted it. That offset is persisted per
//! `(owner, mailbox)` — shared across ALL future `Await` calls from that
//! client regardless of filter.
//!
//! This test reproduces the SPEC's exact recipe (interleaved replies across
//! two fanned-out tasks; originator awaits task A, then task B) against the
//! CURRENT `famp` binary. Expected (buggy) result: the task-B await times
//! out despite task B's commit + terminal envelopes already sitting,
//! unconsumed, in alice's mailbox — because task A's filtered await already
//! walked (and cursor-advanced past) them.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

/// Per-test isolation: a unique tempdir holding the bus socket. Mirrors the
/// `Bus` helper in `cli_dm_roundtrip.rs`.
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

/// Parses a `famp send` JSONL stdout line and returns the `task_id` field.
fn extract_task_id(send_stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(send_stdout);
    let line = text.lines().next().expect("send stdout should have a line");
    let value: serde_json::Value = serde_json::from_str(line).expect("send stdout is valid JSON");
    value["task_id"]
        .as_str()
        .expect("task_id present")
        .to_string()
}

/// Alice fans out two new tasks to bob. Returns `(task_a, task_b)`.
fn fan_out_two_tasks(bus: &Bus) -> (String, String) {
    let send_a = bus.famp_cmd(&[
        "send",
        "--as",
        "alice",
        "--to",
        "bob",
        "--new-task",
        "task A",
    ]);
    assert!(
        send_a.status.success(),
        "new-task A failed: {}",
        String::from_utf8_lossy(&send_a.stderr)
    );
    let task_a = extract_task_id(&send_a.stdout);

    let send_b = bus.famp_cmd(&[
        "send",
        "--as",
        "alice",
        "--to",
        "bob",
        "--new-task",
        "task B",
    ]);
    assert!(
        send_b.status.success(),
        "new-task B failed: {}",
        String::from_utf8_lossy(&send_b.stderr)
    );
    let task_b = extract_task_id(&send_b.stdout);

    (task_a, task_b)
}

/// Bob replies, interleaved: commit A, commit B, terminal A, terminal B.
fn send_interleaved_replies(bus: &Bus, task_a: &str, task_b: &str) {
    let commit_a = bus.famp_cmd(&[
        "send", "--as", "bob", "--to", "alice", "--task", task_a, "--body", "commit A",
    ]);
    assert!(
        commit_a.status.success(),
        "commit A failed: {}",
        String::from_utf8_lossy(&commit_a.stderr)
    );

    let commit_b = bus.famp_cmd(&[
        "send", "--as", "bob", "--to", "alice", "--task", task_b, "--body", "commit B",
    ]);
    assert!(
        commit_b.status.success(),
        "commit B failed: {}",
        String::from_utf8_lossy(&commit_b.stderr)
    );

    let terminal_a = bus.famp_cmd(&[
        "send",
        "--as",
        "bob",
        "--to",
        "alice",
        "--task",
        task_a,
        "--terminal",
        "--body",
        "done A",
    ]);
    assert!(
        terminal_a.status.success(),
        "terminal A failed: {}",
        String::from_utf8_lossy(&terminal_a.stderr)
    );

    let terminal_b = bus.famp_cmd(&[
        "send",
        "--as",
        "bob",
        "--to",
        "alice",
        "--task",
        task_b,
        "--terminal",
        "--body",
        "done B",
    ]);
    assert!(
        terminal_b.status.success(),
        "terminal B failed: {}",
        String::from_utf8_lossy(&terminal_b.stderr)
    );
}

/// Alice round-robins filtered awaits between task A and task B.
///
///    Fix semantics (post-999.1): a filtered await now STOPS draining the
///    instant it walks past a real, filter-mismatched envelope, rather
///    than silently cursor-skipping over it (which is the bug this test
///    guards against — see module docs). This means a single filtered
///    `await --task A` call is no longer guaranteed to return BOTH of
///    task A's envelopes in one shot when task B's traffic is
///    interleaved in between them: it returns only up to the first
///    task-B entry it encounters, and requires the caller (or another
///    consumer) to drain past that blocking entry via a matching or
///    unfiltered await before task A's later envelope becomes visible.
///    This is the accepted, documented tradeoff (see debug file
///    Resolution / reasoning_checkpoint): batch-completeness-per-call is
///    sacrificed so that NO envelope ever becomes permanently
///    unreachable and NO envelope is ever delivered twice.
///
///    Round-robin polling both filters converges on delivering all four
///    envelopes within a bounded number of round trips. This is exactly
///    what the SPEC's "Assert both task FSMs reach COMPLETED on the
///    originator side" recipe requires in v0.11 terms: eventually, via
///    `Await`, the originator observes every envelope for every task —
///    nothing is silently and permanently lost.
fn round_robin_collect(bus: &Bus, task_a: &str, task_b: &str) -> String {
    let mut collected = String::new();
    for round in 0..3 {
        let out_a = bus.famp_cmd(&[
            "await",
            "--as",
            "alice",
            "--task",
            task_a,
            "--timeout",
            "300ms",
        ]);
        assert!(
            out_a.status.success(),
            "await A round {round} failed: {}",
            String::from_utf8_lossy(&out_a.stderr)
        );
        collected.push_str(&String::from_utf8_lossy(&out_a.stdout));

        let out_b = bus.famp_cmd(&[
            "await",
            "--as",
            "alice",
            "--task",
            task_b,
            "--timeout",
            "300ms",
        ]);
        assert!(
            out_b.status.success(),
            "await B round {round} failed: {}",
            String::from_utf8_lossy(&out_b.stderr)
        );
        collected.push_str(&String::from_utf8_lossy(&out_b.stdout));
    }
    collected
}

/// Debug 999.1 reproducer / regression guard.
///
/// Recipe (per SPEC): fan out two open tasks from one originator (alice).
/// Interleave replies from the peer (bob): taskA-commit, taskB-commit,
/// taskA-deliver-terminal, taskB-deliver-terminal. Originator round-robins
/// filtered `famp await --task A` / `famp await --task B` calls. Every
/// envelope from both tasks must eventually be observed exactly once — the
/// FSM-equivalent assertion for v0.11 (there is no more persisted on-disk
/// FSM; `Await` delivery IS the originator's only signal that a task
/// progressed).
///
/// Pre-fix this failed differently depending on call shape tried: a
/// single-shot `await --task B` after `await --task A` timed out forever
/// (task B's envelopes were silently stranded behind the shared
/// per-mailbox `await_offset` that task A's call advanced past them).
/// Post-fix, a single `await --task A` call may only return task A's
/// FIRST envelope when task B's traffic is interleaved before task A's
/// second envelope (batch-completeness-per-call is intentionally
/// sacrificed — see module docs) — but round-robin polling converges on
/// zero data loss and zero duplicate delivery within a bounded number of
/// calls, which is what this test asserts.
#[test]
fn filtered_await_round_robin_delivers_all_interleaved_task_envelopes_without_loss() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    let mut bob = bus.famp_spawn(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    // 1. Alice fans out two new tasks to bob.
    let (task_a, task_b) = fan_out_two_tasks(&bus);

    // 2. Bob replies, interleaved: commit A, commit B, terminal A, terminal B.
    send_interleaved_replies(&bus, &task_a, &task_b);

    // 3/4. Alice round-robins filtered awaits between task A and task B.
    let collected = round_robin_collect(&bus, &task_a, &task_b);

    // No data loss: every envelope from both tasks is eventually observed.
    for marker in ["commit A", "done A", "commit B", "done B"] {
        assert!(
            collected.contains(marker),
            "BUG 999.1 (broker await_offset skip): '{marker}' was never delivered via Await \
             across 3 round-robin rounds — an interleaved, filter-mismatched envelope \
             permanently stranded it behind the shared per-mailbox await_offset. \
             Collected output: {collected}"
        );
    }
    // No duplicate delivery: each envelope surfaces exactly once.
    for marker in ["commit A", "done A", "commit B", "done B"] {
        let count = collected.matches(marker).count();
        assert_eq!(
            count, 1,
            "'{marker}' should be delivered via Await exactly once, got {count}. \
             Collected output: {collected}"
        );
    }

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}

/// Debug 999.1 — parked-wake path regression guard.
///
/// `drain_await_batch` stopping early on a filter mismatch (the fix above)
/// introduces a second edge case that only surfaces via the broker's
/// PARKED-await wake path (`await_reply_for_mailbox`, invoked when a new
/// envelope arrives while a client is blocked in `Await`): if an earlier,
/// already-on-disk, filter-mismatched envelope for a DIFFERENT task sits
/// between this client's offset and a brand-new envelope that DOES match
/// its filter (and therefore triggers the wake), the drain correctly
/// refuses to skip over the earlier mismatch — which means the newly
/// woken client's batch comes back empty even though something matching
/// its filter genuinely just arrived.
///
/// A naive fix would surface this as `BusReply::Err { Internal, "await
/// wake produced no matching envelopes" }`, turning silent data loss into
/// a spurious hard error. The actual fix reports it the same way a normal
/// expiry would (`AwaitTimeout`) so the client just retries — the blocking
/// entry gets drained once some call (this client's own differently
/// filtered await, or another consumer) walks past it.
///
/// This test asserts: alice's parked `await --task A` call, woken by a
/// genuine task-A reply while task B's earlier commit still blocks her
/// offset, returns quickly (well under her requested timeout) with a
/// timeout-shaped reply — NOT a broker Internal error.
#[test]
fn parked_await_woken_behind_earlier_mismatch_reports_timeout_not_internal_error() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    let mut bob = bus.famp_spawn(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    let send_a = bus.famp_cmd(&[
        "send",
        "--as",
        "alice",
        "--to",
        "bob",
        "--new-task",
        "task A",
    ]);
    assert!(
        send_a.status.success(),
        "new-task A failed: {}",
        String::from_utf8_lossy(&send_a.stderr)
    );
    let task_a = extract_task_id(&send_a.stdout);

    let send_b = bus.famp_cmd(&[
        "send",
        "--as",
        "alice",
        "--to",
        "bob",
        "--new-task",
        "task B",
    ]);
    assert!(
        send_b.status.success(),
        "new-task B failed: {}",
        String::from_utf8_lossy(&send_b.stderr)
    );
    let task_b = extract_task_id(&send_b.stdout);

    // Task B's commit lands FIRST and is left un-drained (nobody has
    // called `await --task B` yet). This is the earlier, real,
    // filter-A-mismatched envelope that will block alice's offset.
    let commit_b = bus.famp_cmd(&[
        "send", "--as", "bob", "--to", "alice", "--task", &task_b, "--body", "commit B",
    ]);
    assert!(
        commit_b.status.success(),
        "commit B failed: {}",
        String::from_utf8_lossy(&commit_b.stderr)
    );

    // Alice starts a filtered await for task A with a generous timeout.
    // Nothing matching task A exists yet, so this call parks on the
    // broker (per `await_envelope`'s ParkAwait path).
    let alice_await = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", bus.sock())
        .env("HOME", bus.tmp.path())
        .args([
            "await",
            "--as",
            "alice",
            "--task",
            &task_a,
            "--timeout",
            "10s",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Give alice's await time to register as parked before bob replies.
    std::thread::sleep(Duration::from_millis(500));

    let start = std::time::Instant::now();

    // Bob's commit for task A matches alice's filter and triggers the
    // broker's wake path (`await_reply_for_mailbox`) — but task B's
    // earlier commit is still sitting un-drained ahead of it.
    let commit_a = bus.famp_cmd(&[
        "send", "--as", "bob", "--to", "alice", "--task", &task_a, "--body", "commit A",
    ]);
    assert!(
        commit_a.status.success(),
        "commit A failed: {}",
        String::from_utf8_lossy(&commit_a.stderr)
    );

    let out = alice_await.wait_with_output().unwrap();
    let elapsed = start.elapsed();

    assert!(
        out.status.success(),
        "BUG 999.1 (spurious wake-path Internal error): alice's parked await should not \
         fail even when woken behind an earlier filter-mismatched envelope. stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("Internal"),
        "alice's await should not surface a broker Internal error: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The wake fired almost immediately (well before the 10s timeout) —
    // proves the broker actually replied on the wake, rather than alice's
    // process just sitting there until the full deadline elapsed.
    assert!(
        elapsed < Duration::from_secs(5),
        "alice's await should resolve promptly on wake, not wait out its full timeout: {elapsed:?}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}
