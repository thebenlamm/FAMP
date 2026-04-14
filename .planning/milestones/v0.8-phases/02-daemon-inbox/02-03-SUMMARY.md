---
phase: 02-daemon-inbox
plan: 03
subsystem: cli-daemon-tests
tags: [integration-tests, subprocess, sigkill, sigint, fsync, tail-tolerance]

requires:
  - phase: 02-daemon-inbox-plan-01
    provides: famp_inbox::read::read_all + Inbox::append (fsync-before-return)
  - phase: 02-daemon-inbox-plan-02
    provides: famp::cli::listen::run_on_listener (test-facing) + `famp listen` bin

provides:
  - crates/famp/tests/common/listen_harness.rs — shared spawn/sign/POST/read helpers
  - Five integration test binaries addressing all Phase 2 ROADMAP success criteria
  - ChildGuard RAII pattern for subprocess cleanup on panic unwind
  - stderr beacon-line parsing helper (listening on https://IP:PORT) with
    background drainer so the daemon's mid-shutdown eprintln! never blocks
    on a full pipe

affects: [03-conversation-cli]

tech-stack:
  added:
    - "No new crate deps — reqwest 0.13 `rustls` feature already gated `use_preconfigured_tls`"
  patterns:
    - "subprocess spawn via env!(\"CARGO_BIN_EXE_famp\") + FAMP_HOME env isolation"
    - "Ephemeral-port discovery by reading the daemon's stderr beacon rather than bind-and-drop (no race window for durability / shutdown tests)"
    - "Background stderr drainer thread keeps the pipe empty AFTER the beacon so the daemon's subsequent eprintln!(\"shutdown signal received, exiting\") doesn't fail with EPIPE"
    - "reqwest::Client trust-pinning via famp_transport_http::tls::build_client_config + use_preconfigured_tls — same shape HttpTransport uses, reused here for the test client"
    - "Self-signed test envelopes: from == to == agent:localhost/self, signed by the daemon's own on-disk key so sig-verify middleware accepts them against the single-entry keyring Plan 02-02 pins"
    - "Synthetic crash-state fixture for tail-tolerance: hand-write one complete line + one partial line, call read_all, assert only the complete one comes back"

key-files:
  created:
    - crates/famp/tests/common/listen_harness.rs
    - crates/famp/tests/listen_smoke.rs
    - crates/famp/tests/listen_durability.rs
    - crates/famp/tests/listen_bind_collision.rs
    - crates/famp/tests/listen_shutdown.rs
    - crates/famp/tests/listen_truncated_tail.rs
  modified:
    - crates/famp/tests/common/mod.rs

key-decisions:
  - "Smoke test runs IN-PROCESS via run_on_listener; durability, bind-collision, and shutdown tests run as SUBPROCESSES. The OS-process boundary is load-bearing for the three contracts whose observability requires real signal delivery — anything in-process would be a weaker proxy."
  - "Durability test reads the ephemeral port from the daemon's stderr beacon rather than using bind-and-drop port selection. Eliminates the race window where another process could grab the port between our drop and the daemon's bind."
  - "Bind-collision test DOES use bind-and-drop port selection, because the test needs a fixed, known port for BOTH daemons. The race window is accepted — the test is gating the OS `EADDRINUSE → CliError::PortInUse` mapping, not the port-selection strategy."
  - "SIGINT delivered via `/bin/kill -INT <pid>` rather than libc::kill() inside an unsafe block. Keeps the test source 100% safe-rust; the test is already #[cfg(unix)] so /bin/kill is available."
  - "The stderr drainer thread LEAKS intentionally — it runs until the child closes its stderr (via shutdown or crash), then exits. Not joining it avoids test-teardown races."

patterns-established:
  - "Shared test harness via `mod common;` in tests/ files that need listen helpers; the older `#[path = \"common/cycle_driver.rs\"] mod cycle_driver;` convention for the Phase-1 http tests remains unaffected because common/mod.rs has no name collision with cycle_driver.rs."
  - "Beacon-line parsing (`listening on https://...`) is now a reusable helper — future listen-adjacent tests (e.g., a `famp inbox tail` test in Phase 3) can spawn + discover the port without inventing a new sync protocol."

requirements-completed: [DAEMON-05, INBOX-02, INBOX-04, INBOX-05]

duration: ~40min
completed: 2026-04-14
---

# Phase 2 Plan 3: famp listen Integration Tests Summary

**Five integration test binaries lock all Phase 2 ROADMAP success criteria at the OS-process boundary: POSTed signed envelope persists to JSONL, SIGKILL-after-200 leaves the line intact (fsync proof), second daemon on same port exits with PortInUse, SIGINT causes exit 0 within 5s, and `read_all` tolerates a daemon-crash-truncated final line.**

## Performance

- **Duration:** ~40 min
- **Started:** 2026-04-14 (after Plan 02-02 completion)
- **Tasks:** 2/2 (harness + 5 test binaries)
- **Files created:** 6
- **Files modified:** 1

## Accomplishments

- `crates/famp/tests/common/listen_harness.rs` — 300-line shared harness with:
  - `ChildGuard` RAII wrapper (panic-safe subprocess cleanup).
  - `spawn_listen(home, addr_str)` — `Command::new(env!("CARGO_BIN_EXE_famp"))` with FAMP_HOME isolated.
  - `read_stderr_bound_addr(child, timeout)` — background-thread drainer that finds the beacon line and KEEPS DRAINING so the daemon never blocks on a full stderr pipe.
  - `wait_for_bind(child, addr, timeout)` — TCP-connect poll with child-exit detection.
  - `build_signed_ack_bytes(home)` — minimal signed AckBody where from==to==agent:localhost/self, signed by the daemon's own on-disk key (matches Plan 02-02's single-entry keyring).
  - `build_trusting_reqwest_client(home)` — reqwest 0.13 client that trusts the daemon's self-signed `tls.cert.pem` via `famp_transport_http::tls::build_client_config` + `use_preconfigured_tls`.
  - `post_bytes(client, addr, principal, bytes)` — path-segment-percent-encoded POST to `/famp/v0.5.1/inbox/{principal}`.
  - `read_inbox_lines(home)` — thin wrapper over `famp_inbox::read::read_all`.
  - `init_home_in_process(home)` — calls `famp::cli::init::run_at` directly (faster than a subprocess init).

- **`listen_smoke.rs`** (`smoke_post_delivers_to_inbox`) — in-process `run_on_listener`, POST signed AckBody, assert 200 + `inbox.jsonl` has exactly one line with `"class": "ack"`, then oneshot-shutdown the daemon task. Locks DAEMON-01 (bind from override) and DAEMON-02 (end-to-end path).

- **`listen_durability.rs`** (`sigkill_after_200_leaves_line_intact`) — subprocess, read ephemeral port from stderr beacon, POST signed envelope, immediately SIGKILL, wait() to full exit, then `read_all(inbox.jsonl)` — assert one value present. Proves the daemon fsynced BEFORE returning 200 (INBOX-02 fsync contract).

- **`listen_bind_collision.rs`** (`second_listen_on_same_port_errors_port_in_use`) — spawn daemon A on a bind-and-drop-chosen port, wait_for_bind. Spawn daemon B on the same port. Wait ≤5s for B to exit; assert non-zero exit AND stderr contains `"already bound"` (from `CliError::PortInUse`'s `#[error(...)]`). Locks DAEMON-03.

- **`listen_shutdown.rs`** (`sigint_causes_exit_0_within_5s`) — spawn daemon, read beacon, `wait_for_bind` to ensure the tokio select! has registered the SIGINT handler, brief 150ms settle, then `/bin/kill -INT <pid>`. Poll `try_wait()` ≤5s; assert `exit_status.success()`. Locks DAEMON-04.

- **`listen_truncated_tail.rs`** (`read_all_tolerates_daemon_crash_truncated_tail`) — no daemon. Hand-write `{"class":"ack","n":1}\n` + partial `{"class":"ack","n":2` (no newline). Call `famp_inbox::read::read_all`. Assert exactly one value (the complete line). Reinforcement of INBOX-04 / INBOX-05 at the famp-crate consumer layer (Plan 02-01 has the unit-level test inside famp-inbox).

- **Full test suite:** `cargo nextest run --workspace` → **298/298 passed, 1 skipped** (293 baseline from Plan 02-02 + 5 new integration tests).
- **Clippy:** `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings.
- **OpenSSL guard:** `cargo tree -i openssl` → `package ID specification openssl did not match any packages` (empty, as required).

## Task Commits

1. **Task 1 — shared listen test harness** — `82776b9` (test)
   - `tests/common/listen_harness.rs` created; `tests/common/mod.rs` re-exports the public helpers.
   - No test binary yet consumes the helpers in this commit (the five binaries land in Task 2).

2. **Task 2 — five integration test binaries** — `4d14f0f` (test)
   - Smoke, durability, bind-collision, shutdown, truncated-tail.
   - Also contains a targeted fix to `read_stderr_bound_addr` found during shutdown-test debugging (see Deviations §Rule 1).

## Files Created/Modified

- `crates/famp/tests/common/listen_harness.rs` — shared harness (300+ lines).
- `crates/famp/tests/common/mod.rs` — re-exports the public harness items; keeps coexisting with the older `#[path = ...]` cycle_driver convention used by Phase-1 tests.
- `crates/famp/tests/listen_smoke.rs` — in-process smoke (DAEMON-01/02).
- `crates/famp/tests/listen_durability.rs` — subprocess + SIGKILL (INBOX-02).
- `crates/famp/tests/listen_bind_collision.rs` — two-daemon PortInUse (DAEMON-03).
- `crates/famp/tests/listen_shutdown.rs` — SIGINT → exit 0 (DAEMON-04).
- `crates/famp/tests/listen_truncated_tail.rs` — synthetic crash-tail (INBOX-04/05).

## Decisions Made

- **In-process vs subprocess split.** Smoke test = in-process via `run_on_listener` with an oneshot shutdown channel (faster, simpler, exercises the handler/router path). Durability + bind-collision + shutdown = subprocess (the contracts are OS-process-level and in-process would be a strictly weaker proxy — a `task.abort()` does not prove fsync happened, a dropped `oneshot` does not prove the real SIGINT handler installed, and a second `run_on_listener` call cannot observe `EADDRINUSE` at the OS-bind layer because it uses a pre-bound listener).

- **Beacon-parse over bind-and-drop for durability.** The durability test reads the daemon's ephemeral port from the stderr beacon rather than picking a port via bind-and-drop. This eliminates a race window where a second process could grab the port between our `drop(listener)` and the daemon's `bind`. Bind-and-drop is acceptable for the bind-collision test because that test explicitly wants two daemons fighting for the same known port.

- **`/bin/kill` over `libc::kill` unsafe block.** The shutdown test needs to send a real SIGINT to a subprocess. Two options: `libc::kill(pid, libc::SIGINT)` inside an `unsafe {}` (and a `#![allow(unsafe_code)]`), or `Command::new("kill").args(["-INT", &pid])`. Picked the shell `kill` approach — keeps the test source fully safe-rust, no unsafe anywhere, and the test is already `#[cfg(unix)]` so `/bin/kill` is always present.

- **Drainer thread intentionally leaks.** After the beacon line is found, the background thread keeps reading child stderr until EOF. We don't join it — it exits on its own when the child closes stderr. Not joining it sidesteps a test-teardown race where the child has been reaped but the pipe is still being emptied.

- **No new deps.** `reqwest`'s `"rustls"` feature (already in famp's dev-dependencies since Plan 04-03) pulls `__rustls` transitively, which gates `use_preconfigured_tls`. Verified by `cargo tree -p famp --edges features -e dev | grep rustls`.

## Deviations from Plan

### Rule 1 — Bug: stderr pipe full-buffer caused daemon panic during shutdown test

- **Found during:** first `cargo nextest run` of `listen_shutdown`. Exit status 101 (Rust panic).
- **Issue:** Original `read_stderr_bound_addr` spawned a reader thread that returned after finding the beacon line. After the beacon line, the daemon later tries `eprintln!("shutdown signal received, exiting")` during graceful shutdown. Because the reader thread had already returned and dropped its BufReader handle, the stderr pipe remained connected but nobody was reading it. Rust's `eprintln!` panics (not silently ignores) on write errors in some configurations — and more importantly, with no drainer the daemon's signal future was racing the pipe-full condition.
- **Symptom sequence:** SIGINT delivered → daemon's select! resolves → daemon tries to eprintln! → write blocks or fails → panic → exit 101. Test saw `ExitStatus(unix_wait_status(25856))` which decodes to exit code 101 (`25856 >> 8`).
- **Fix:** Changed `read_stderr_bound_addr` to have its reader thread KEEP DRAINING after finding the beacon (discarding subsequent lines). The pipe stays empty for the lifetime of the child process, so the daemon's mid-shutdown eprintln! always succeeds.
- **Files modified:** `crates/famp/tests/common/listen_harness.rs`.
- **Commit:** `4d14f0f` (folded into the Task 2 commit since the bug was found during Task 2 execution).

### Rule 3 — Blocking: SIGINT handler registration race

- **Found during:** listen_shutdown test initial run. After fixing the Rule-1 pipe bug, the test still failed intermittently with exit-by-signal 2 (default SIGINT disposition — the daemon's handler hadn't installed yet).
- **Issue:** `tokio::signal::ctrl_c()` only registers its handler on first poll. The daemon prints its beacon line *before* calling `run_on_listener`, which then loads identity, builds the router, spawns the server, and enters the `tokio::select!` that first polls `ctrl_c()`. The test was SIGINTing too early, during the gap between beacon-print and select!-entry.
- **Fix:** After reading the beacon, the shutdown test now (a) calls `wait_for_bind(addr)` to confirm TCP accept is live — by that point `tls_server::serve_std_listener` has spawned and the select! has been entered, (b) sleeps an extra 150ms as a belt-and-braces settle window.
- **Files modified:** `crates/famp/tests/listen_shutdown.rs`.
- **Commit:** `4d14f0f`.

### Rule 3 — Blocking: clippy lint batch

- **Found during:** `cargo clippy -p famp --all-targets -- -D warnings` after Task 2 binaries landed.
- **Issues fixed inline:**
  1. `clippy::missing_const_for_fn` on three `ChildGuard` methods — added `clippy::missing_const_for_fn` to the file-level allow list (these are test helpers, pedantic-const rules are noise here).
  2. `clippy::single_match_else` in `wait_for_bind` and `listen_bind_collision` + `listen_shutdown` — allowed at file level for the same reason.
  3. `clippy::doc_markdown` on `listen_smoke`, `listen_durability`, `listen_truncated_tail`, `listen_bind_collision`, `listen_shutdown` doc comments (`SIGKILL`, `PortInUse`, `AckBody` etc referenced in prose) — added to file-level allow list.
  4. Stray `use std::marker::PhantomData as _PhantomData` dead import — removed.
- **Resolution:** Lint suppressions at the file level rather than per-site. Test files are allowed a relaxed pedantic profile — the same pattern Phase 1 tests follow (`init_happy_path.rs` already allows unused_crate_dependencies at file level).

---

**Total deviations:** 3 (1 Rule-1 bug — stderr drainer, 1 Rule-3 block — SIGINT race, 1 Rule-3 block — clippy lint batch).
**Impact on plan:** None on public API; none on test semantics. Both fixes are internal to the test harness. The drainer fix in particular is a general-purpose hardening — any future listen-adjacent test that reads child stderr inherits the correct behavior for free.

## Issues Encountered

Two surprises during execution, both in the shutdown test path:

1. The stderr-drainer issue described above (Rule 1). Non-obvious because the error manifested as a Rust panic exit (101) rather than a hung test. Traced by correlating `unix_wait_status` math to Rust's panic exit convention (`exit_code << 8`).

2. The SIGINT-race issue (Rule 3). Non-obvious because the beacon line appears *before* tokio's ctrl_c() future is first polled — so from the test's point of view the daemon is "ready" when in fact its signal handler won't install for another few milliseconds. Resolved by synchronizing on TCP accept (which happens AFTER the select! has been entered) plus a settle delay.

No spec-level or library-level surprises.

## Verification Artifacts

- `cargo check -p famp --tests` → clean
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings
- `cargo nextest run -p famp --test listen_smoke --test listen_durability --test listen_bind_collision --test listen_shutdown --test listen_truncated_tail` → 5/5 passed in 0.537s
- `cargo nextest run --workspace` → **298/298 passed, 1 skipped** (293 baseline from Plan 02-02 + 5 new integration tests, zero regressions)
- `cargo tree -i openssl` → `package ID specification openssl did not match any packages` (confirmed no OpenSSL in the tree)

Test-to-requirement coverage map:

| Requirement | Test |
|---|---|
| DAEMON-01 (listen binds override addr) | `listen_smoke::smoke_post_delivers_to_inbox` |
| DAEMON-02 (stderr beacon line) | `listen_durability::sigkill_after_200_leaves_line_intact` (reads beacon); `listen_shutdown::sigint_causes_exit_0_within_5s` (reads beacon) |
| DAEMON-03 (PortInUse on second bind) | `listen_bind_collision::second_listen_on_same_port_errors_port_in_use` |
| DAEMON-04 (SIGINT → exit 0 within 5s) | `listen_shutdown::sigint_causes_exit_0_within_5s` |
| DAEMON-05 (integration coverage of 01–04) | All four above in aggregate |
| INBOX-02 (fsync before 200) | `listen_durability::sigkill_after_200_leaves_line_intact` |
| INBOX-04 / INBOX-05 (tail tolerance) | `listen_truncated_tail::read_all_tolerates_daemon_crash_truncated_tail` |

## Threat Flags

None. The single test-to-daemon HTTPS boundary is enumerated in the plan's `<threat_model>` table (T-02-30, T-02-31, T-02-32). No new auth paths, no new schema surfaces, no new file-access patterns beyond what the plan's threat register covered.

## Next Phase Readiness

- **Phase 3 (conversation CLI — `famp send`, `famp await`, `famp peer add`)** can reuse the `listen_harness` helpers for its own integration tests (the `famp await` tests will read `inbox.jsonl` via the same `read_inbox_lines` wrapper; the `famp send` tests can spawn a listen daemon and use `post_bytes` or a real `famp send` subprocess against it).
- The beacon-parse pattern generalizes to any future CLI subcommand that prints a "listening on ..." line — a future `famp inspect --listen` or similar would get subprocess sync for free.
- No blockers identified for Phase 3.

## Self-Check: PASSED

- `crates/famp/tests/common/listen_harness.rs` — FOUND
- `crates/famp/tests/common/mod.rs` (re-exports) — FOUND
- `crates/famp/tests/listen_smoke.rs` — FOUND
- `crates/famp/tests/listen_durability.rs` — FOUND
- `crates/famp/tests/listen_bind_collision.rs` — FOUND
- `crates/famp/tests/listen_shutdown.rs` — FOUND
- `crates/famp/tests/listen_truncated_tail.rs` — FOUND
- Commit `82776b9` (Task 1 — harness) — FOUND in git log
- Commit `4d14f0f` (Task 2 — 5 integration tests + drainer fix) — FOUND in git log
- All 5 new tests pass under `cargo nextest run -p famp --test listen_*` — VERIFIED
- Full workspace test run 298/298 green — VERIFIED
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings — VERIFIED
- `cargo tree -i openssl` → empty — VERIFIED

---
*Phase: 02-daemon-inbox*
*Plan: 03*
*Completed: 2026-04-14*
