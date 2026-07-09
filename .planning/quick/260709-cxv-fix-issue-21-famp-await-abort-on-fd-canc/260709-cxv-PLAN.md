---
quick_id: 260709-cxv
date: 2026-07-09
status: planned
issue: 21
---

# Quick Task 260709-cxv — `famp await --abort-on-fd`: a cancellation seam for the listen-mode Stop hook

## Problem

A listen-mode Stop hook blocks synchronously inside `famp await`. While it blocks,
the agent turn never ends, so Claude Code never drains its session input queue.
Background-agent completion notifications sit in that queue until the user hits Esc.
Observed: `enqueue` 10:26:08.056Z → `dequeue` 11:01:02.537Z, a 34m54s stall.

## Settled design (do not re-litigate)

Three alternatives were evaluated and rejected by two independent reviewers:
raw transcript byte-growth polling; "background agent pending" no-op; short
timeout + re-arm. See `gh issue view 21`.

The fix: give `famp await` a **generic, host-neutral cancellation seam** and keep
**all** Claude-Code-specific knowledge in the bash hook.

### Why not the "durable" architecture change

The issue asks whether listen mode should block the turn at all — a watcher
process plus a fast-returning hook. **It should, and it must.** Claude Code
exposes no mechanism for an external process to inject input into an idle
session. Hooks are the only in-session control surface, and a *blocked Stop hook
is itself the wake mechanism*: it is what keeps the turn alive so an arriving
message can resume the agent. A fast-returning hook ends the turn, and a watcher
that later sees a message has no way to start a new one. The block is the
feature. The bug is that the block is uncancellable. Therefore `--abort-on-fd`
is not the smaller step — it is the correct one, and the architecture change is
not a live option. Recorded here so it is not revisited.

### Self-healing property (this is what makes abort safe)

Abort ends the turn → host drains the queue → the queued notification becomes a
user-role turn → the agent responds → that turn ends → the Stop hook fires again
→ `famp await` re-arms. Listen mode is **not** lost by aborting; it lapses only
for the drain interval. This is why aborting is strictly better than blocking.

## Two corrections to the design as written in the issue

Both are grounded in the real transcript schema, verified against
`~/.claude/projects/**/*.jsonl`:

```json
{"type":"queue-operation","operation":"enqueue","timestamp":"...","sessionId":"...","content":"<task-notification>…full agent result…</task-notification>"}
{"type":"queue-operation","operation":"dequeue","timestamp":"...","sessionId":"..."}
```

**C1 — JSON-parse, never grep.** The issue says "one transcript grep" for
`"operation":"enqueue"`. An `enqueue` record embeds the *entire* agent result in
`content`. Any transcript in which an agent discusses this string (including the
transcript of this very task) contains it as literal text inside an unrelated
record. The watcher MUST parse each line as JSON and test
`type == "queue-operation" && operation == "enqueue"`. A grep ships a hook that
aborts spuriously and silently disables listen mode — exactly the failure that
killed rejected approach #1.

**C2 — a byte baseline captured at hook start misses the pre-existing enqueue.**
If a background agent finishes *mid-turn*, its `enqueue` lands **before** the Stop
hook runs. No *new* enqueue ever arrives, so a baseline-relative watcher never
fires and the hook blocks forever — the reported bug, unfixed. The predicate must
be "is input outstanding **right now**", not "did a new enqueue appear".

**Predicate (replaces the byte baseline):** over the last 2 MB of the transcript,
count JSON-parsed `queue-operation` records; **abort iff `enqueues > dequeues`.**
- Drift-free: historical drained pairs cancel out, so it does not latch on old
  enqueues the way a naive "any enqueue seen" check would.
- Tail-truncation is safe *in the correct direction*: a `dequeue` always follows
  its `enqueue`, so a truncated window can only ever orphan a *dequeue* (yielding
  a negative count → no abort). It can never orphan an enqueue → no false abort.
- Covers C2's pre-existing-enqueue case and the observed arrives-while-blocked
  case with one predicate, evaluated on a poll loop from t=0.

## Tasks

### Task 1 — Rust: `famp await --abort-on-fd <n>`

**Files:** `crates/famp/src/cli/await_cmd/mod.rs`, `crates/famp/src/cli/error.rs`,
`crates/famp/src/bus_client/mod.rs`

- Add `#[arg(long = "abort-on-fd")] pub abort_on_fd: Option<i32>` to `AwaitArgs`.
- Validate before use: reject `n < 3` (never steal stdio) and verify the fd is
  open via `nix::fcntl::fcntl(n, F_GETFD)`. An invalid fd is a hard `CliError`,
  not UB. Set `O_NONBLOCK` on it.
- Take ownership with **one narrowly-scoped** `#[allow(unsafe_code)]`
  `unsafe { OwnedFd::from_raw_fd(n) }`. This mirrors the existing, documented
  precedent at `bus_client::spawn` (the crate is `unsafe_code = "deny"`, not
  `forbid`, expressly to permit this). **Do not touch any lint config.**
- Wrap in `tokio::io::unix::AsyncFd` and treat **readable (bytes or EOF)** as the
  abort signal. Generic over pipes and FIFOs; not Claude-specific.
- Add `BusClient::send_recv_abortable`. Structure it so the read future is
  **pinned and reused**, never dropped mid-frame:

  ```rust
  let read = codec::read_frame::<_, BusReply>(&mut reader);
  tokio::pin!(read);
  tokio::select! {
      biased;                                  // reply is polled first
      r = &mut read => /* message wins */,
      _ = abort.readable() => {
          // grace window: an AwaitOk may already be in flight on the socket
          match tokio::time::timeout(GRACE, &mut read).await {
              Ok(r)  => /* MESSAGE WINS */,
              Err(_) => /* abort */,
          }
      }
  }
  ```
  The `biased;` handles the both-ready tie. The grace re-poll of the **same
  pinned future** handles "broker already wrote the reply" without losing a
  partially-read frame. Together these are the exit-code tie-break.
- On abort: print `{"aborted":true}` to stdout and return `CliError::Exit(3)`.
  `main` already maps `CliError::Exit(code)` → `process::exit(code)` **without**
  printing an error (`crates/famp/src/bin/famp.rs:60`). Exit code **3** is the
  distinct abort code; 0 = message/timeout, 1 = real error.
- Thread the flag through the MCP wrapper's call to `run_at_structured` as `None`
  (the MCP tool surface is unchanged — **no** `just install`-visible schema change).

**Tests** (`crates/famp/tests/`, plain `cargo test -p famp`, NOT nextest):
1. `abort_fd_write_cancels_parked_await_with_exit_3` — park an await against a
   tmp broker, write one byte to the pipe, assert exit code 3.
2. `abort_fd_close_eof_also_cancels` — EOF, not just bytes, is an abort.
3. `inflight_awaitok_beats_a_simultaneous_abort` — send a message AND fire the
   abort; assert the envelope is returned and the exit code is 0. This is the
   tie-break; it is the test that matters.
4. `invalid_abort_fd_is_a_hard_error_not_ub` — `--abort-on-fd 999` errors cleanly.

Use `ChildGuard` for any spawned `famp register`/broker child (project rule —
RAII kill+wait on drop, else panics leak tmp-socket brokers).

### Task 2 — Hook: `crates/famp/assets/famp-await.sh`

`crates/famp/assets/famp-await.sh` is the **source of truth**; it is embedded via
`include_str!` at `crates/famp/src/cli/install/await_hook.rs:15`. A `~/.claude`-only
edit gets clobbered by the next `famp install`.

- bash 3.2 (macOS). Preserve: the 64 KB cap, identity validation, the
  heredoc-in-`$()` workaround, and **fail-open `exit 0` on every error path**.
- Create a FIFO in `$TMPDIR`; `exec 9<>"$FIFO"` (read-write, so it neither blocks
  on open nor EOFs spuriously). Bash does not set `CLOEXEC` on `exec`-redirected
  fds, so fd 9 is inherited by `famp await --abort-on-fd 9`.
- Background watcher subshell inherits fd 9. Every 2s it re-evaluates the C2
  predicate over the transcript via `python3` (JSON-parsed, per C1). On
  `enqueues > dequeues` it writes one byte to fd 9 and exits.
- **Fail-open direction:** any watcher/python error → do **not** abort. The hook
  then behaves exactly as it does today. Never abort on uncertainty.
- Reap the watcher after `famp await` returns (kill + wait).
- On `STATUS == 3`: log `aborted: host queue has pending input`, `exit 0` so the
  host drains. Emit **no** `{"decision":"block"}`.
- `STATUS == 0` with envelopes → the existing notification path, unchanged.
  **Peer bytes must never reach `reason`.** Preserve that.
- `just check-shellcheck` must stay clean.

**Tests** (`crates/famp/tests/hook_runner_await.rs`):
5. `hook_aborts_when_transcript_has_outstanding_enqueue` (C2 — pre-existing).
6. `hook_aborts_when_enqueue_appears_while_blocked` (the observed bug).
7. `hook_does_not_abort_when_enqueue_is_matched_by_dequeue` (drained → keep blocking).
8. `hook_does_not_abort_on_enqueue_string_inside_a_content_field` (**C1 regression** —
   this test fails against a grep implementation and passes against a JSON parse).

Existing tests read the **installed** hook at `~/.claude/hooks/famp-await.sh`
(`hook_path()`). New tests MUST instead exercise the repo asset
(`crates/famp/assets/famp-await.sh`) so they test the source of truth and do not
depend on `famp install` having run. Do not regress the existing tests.

### Task 3 — Falsification (a green test proves nothing on its own)

For **every** test 1–8: revert the fix, confirm the test **FAILS**, restore, confirm
it passes. Record the observed failure message for each in SUMMARY.md. Any test
that passes under both states is not a test — delete or rewrite it.

Then:
- `cargo test -p famp` and `cargo test -p famp-bus` (plain cargo; `cargo nextest
  -p famp` hangs in the test-binary `--list` phase in this repo).
- `cargo clippy --workspace --all-targets` — CI runs pedantic lints. **Nothing may
  be silenced, downgraded, or `#[allow]`-ed**, with the single exception of the
  one documented `unsafe_code` allow in Task 1.
- `just check-shellcheck`.

## Explicitly NOT in this task

**Deployment is deferred to the orchestrator, not the executor.** `just install` +
broker restart (bootout → bootstrap → kickstart; `launchctl kickstart` alone will
not refresh the cached code signature — issue #20). A broker restart drops every
registration and permanently deafens live listen windows until each re-registers.
`famp inspect identities` must be run **before** any restart, and never *during*
the down window (it auto-spawns an orphan broker that steals the socket).
The executor must not run `just install`, must not restart the broker, and must
not run `famp inspect`.

## Must-haves

- `famp await --abort-on-fd <n>` exits **3** on abort, **0** on message/timeout.
- An in-flight `AwaitOk` beats a simultaneous abort (test 3).
- The hook aborts on an outstanding enqueue and on a new one; it does **not**
  abort on a drained queue, nor on the literal string inside a `content` field.
- The hook is fail-open on every error path; no path can trap a session.
- `famp await` contains **zero** references to Claude Code, transcripts, or queues.
- Every test falsified against the reverted fix.
