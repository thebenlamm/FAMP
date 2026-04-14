---
phase: 03-conversation-cli
plan: 04
subsystem: conversation-cli-integration
tags: [inbox-lock, raii, integration-tests, restart-safety, phase-3-closure]

requires:
  - phase: 03-conversation-cli-plan-01
    provides: famp-taskdir, famp_inbox::InboxCursor
  - phase: 03-conversation-cli-plan-02
    provides: famp send, famp peer add, TaskRecord persistence
  - phase: 03-conversation-cli-plan-03
    provides: famp await, locked JSON output shape

provides:
  - famp_inbox::lock::InboxLock RAII advisory lock at <home>/inbox.lock
  - InboxError::LockHeld { path, pid } variant
  - famp await acquires the lock on entry, drops on return
  - conversation_harness shared test helpers
  - Three end-to-end integration tests locking Phase 3's full conversation surface

affects: [04-mcp-claude-integration]

tech-stack:
  added:
    - "nix 0.29 (Unix-only) — kill(pid, None) liveness check for stale-PID reaping"
  patterns:
    - "Best-effort advisory lock via create_new(0o600) PID file + drop-removes — same RAII shape as Phase 1's atomic.rs writers but for a long-lived hold rather than a single fsync"
    - "Stale-PID reaping: read existing file, parse PID, kill(pid, None) — alive=LockHeld, dead/unparseable=reap-and-reacquire. EPERM means alive (different uid)."
    - "Fail-fast on contention: famp await returns CliError::Inbox(LockHeld) immediately rather than spinning. Phase 3's CLI is single-developer; silent waiting would mask the misuse."
    - "Single-shared-FAMP_HOME tests: Phase 2's listen daemon only resolves agent:localhost/self in its keyring, so the conversation tests share one home for both sender and receiver. Documented in conversation_harness.rs and called out as Phase 4 multi-keyring work."
    - "Restart safety via kill-and-respawn: spawn_listener -> stop_listener -> spawn_listener on a fresh ephemeral port; harness exposes update_peer_endpoint to repoint peers.toml without a famp peer update subcommand"

key-files:
  created:
    - crates/famp-inbox/src/lock.rs
    - crates/famp-inbox/tests/lock_contention.rs
    - crates/famp/tests/common/conversation_harness.rs
    - crates/famp/tests/conversation_full_lifecycle.rs
    - crates/famp/tests/conversation_restart_safety.rs
    - crates/famp/tests/conversation_inbox_lock.rs
    - .planning/milestones/v0.8-phases/03-conversation-cli/03-04-SUMMARY.md
  modified:
    - crates/famp-inbox/Cargo.toml
    - crates/famp-inbox/src/lib.rs
    - crates/famp-inbox/src/error.rs
    - crates/famp-inbox/tests/cursor_roundtrip.rs
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/tests/common/mod.rs

key-decisions:
  - "Fail-fast contention semantics (NOT spin-wait). famp await returns LockHeld immediately on contention. Phase 3 is a single-developer CLI; concurrent awaits are user error and silent waiting would mask the mistake. The decision is locked by conversation_inbox_lock.rs which asserts the rejection happens in <500ms regardless of the configured --timeout."
  - "Best-effort PID-file lock (not flock/fcntl). flock would be tied to the file descriptor lifetime and require keeping the handle alive across an awaitable polling loop that crosses several .await points; a PID file with a Drop reaper is simpler, race-tolerant via create_new, and gives us cross-uid liveness detection via kill(pid, None) EPERM handling."
  - "nix 0.29 over a raw libc::kill unsafe block. The workspace forbids unsafe_code, so the liveness check goes through nix::sys::signal::kill. nix is added under [target.'cfg(unix)'.dependencies] only; the non-Unix branch is a conservative `assume alive` that is unreachable under the supported platforms."
  - "Single shared FAMP_HOME for the conversation tests (NOT two homes). The plan sketched setup_two_homes(), but the Phase 2 listen daemon's keyring only resolves agent:localhost/self. A genuine two-home flow is impossible until Phase 4 adds a multi-entry keyring. The harness uses `add_self_peer` (alias=self, principal=agent:localhost/self) and the same home for both sender and receiver. Documented inline in conversation_harness.rs."
  - "Restart-safety test repoints peers.toml in-place. Phase 3 has no `famp peer update` subcommand and the daemon's ephemeral port changes across restarts. The harness's update_peer_endpoint helper rewrites peers.toml atomically via the existing config::write_peers_atomic helper (and clears tls_fingerprint_sha256 so TOFU recaptures cleanly on the new self-signed cert)."

requirements-completed: [CONV-05]

duration: ~25min
completed: 2026-04-14
---

# Phase 3 Plan 04: `InboxLock` + Conversation Integration Tests Summary

**Locks the inbox advisory primitive (`InboxLock`) and the three Phase 3 end-to-end integration tests that prove the full conversation surface — open task, multi-deliver, terminal, restart-safety, and lock contention — composes correctly across `famp send`, `famp listen`, `famp await`, and persistent `famp-taskdir` state.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 2/2 (one commit per task)
- **Files created:** 7
- **Files modified:** 6
- **Workspace tests:** 343 passed, 1 skipped (up from 333 — +7 lock + 3 conversation = +10 new)

## Accomplishments

### Task 1 — `InboxLock` RAII + `famp await` integration

- `famp_inbox::lock::InboxLock` — RAII advisory lock at `<home>/inbox.lock`. Construct via `InboxLock::acquire(home)`; drop removes the file (best-effort).
- Stale-PID reaping: existing lock file is parsed for an ASCII-decimal PID, liveness is checked via `nix::sys::signal::kill(Pid, None)`. Alive (Ok or `EPERM`) → `LockHeld`. Dead, unparseable, or unreadable → reap and reacquire.
- 0600 mode on Unix via `OpenOptionsExt::mode`.
- New `InboxError::LockHeld { path: PathBuf, pid: u32 }` variant; `CliError::Inbox` already had `#[from]` so no CLI error wiring was needed.
- `famp await` calls `InboxLock::acquire(home)` immediately after parsing args and before the polling loop. The lock is held for the entire call; RAII drop releases it on every exit path (success, timeout, error).
- `nix 0.29` added as `[target.'cfg(unix)'.dependencies]` to `famp-inbox`. Non-Unix `is_alive` is a conservative `true` placeholder (Phase 3 does not target Windows).

### Task 2 — Three end-to-end integration tests + shared harness

- `tests/common/conversation_harness.rs`: shared helpers
  - `setup_home() -> TempDir` — calls `init_home_in_process` and returns the guard
  - `spawn_listener(home) -> (SocketAddr, JoinHandle, oneshot::Sender<()>)` — binds an ephemeral port, spawns `run_on_listener` in-process, polls until TCP accept succeeds
  - `stop_listener(handle, tx)` — sends shutdown + bounded await
  - `add_self_peer(home, alias, addr)` — calls `peer::add::run_add_at` with the daemon's own pubkey + principal `agent:localhost/self`
  - `update_peer_endpoint(home, alias, addr)` — rewrites `peers.toml` in place via `config::write_peers_atomic`, clearing `tls_fingerprint_sha256` so TOFU recaptures
  - `new_task / deliver / try_deliver / await_once / read_task / inbox_line_count` — thin wrappers around the Phase 3 CLI entry points
- `tests/conversation_full_lifecycle.rs`: opens a task, sends 3 non-terminal delivers, sends terminal, asserts `TaskTerminal` on a subsequent send, then consumes the first inbox entry via `famp await` and asserts the locked JSON shape (`offset`, `from`, `class`).
- `tests/conversation_restart_safety.rs`: opens a task, delivers once, stops the listener, respawns on a fresh ephemeral port, repoints the peer entry, asserts the on-disk task record is byte-identical to before, then sends another deliver on the SAME task id and asserts the inbox grew to 3 lines.
- `tests/conversation_inbox_lock.rs`: manually acquires `InboxLock`, runs `famp await --timeout 5s`, asserts the call returns `Inbox(LockHeld)` in under 500 ms (proving fail-fast), drops the lock, runs `famp await --timeout 200ms` against an empty inbox and asserts `AwaitTimeout` (proving release).

## Locking Semantics Decision (locked here)

`famp await` is **fail-fast** on contention:

- Acquire the lock immediately on entry.
- If a live PID already holds it → return `CliError::Inbox(InboxError::LockHeld { path, pid })` without entering the poll loop, regardless of the configured `--timeout`.
- If the lock file holds a stale PID (process gone, unparseable content, or unreadable) → reap and reacquire silently.

**Why fail-fast and not spin-wait:** Phase 3's CLI is a single-developer surface. Two simultaneous `famp await` calls in the same `FAMP_HOME` are user error (the second one almost always means the first is stuck or forgotten). Silent waiting would mask the misuse. A typed error makes the failure visible. If a real multi-consumer workflow appears in Phase 4+, the spin-wait variant is a one-line addition behind a `--wait` flag.

This decision is locked by `conversation_inbox_lock.rs`, which asserts the rejection happens in **< 500 ms** even when `--timeout` is `5s`.

## ROADMAP Phase 3 success-criteria coverage

The five Phase 3 ROADMAP success criteria are each covered by an integration test:

| # | Criterion | Test |
|---|---|---|
| 1 | `famp send --new-task` opens a task, persists `REQUESTED` record | `conversation_full_lifecycle` (steps 1-2) + `send_new_task` (Plan 03-02) |
| 2 | `famp send --task` (non-terminal × N) | `conversation_full_lifecycle` (step 3) + `send_deliver_sequence` (Plan 03-02) |
| 3 | `famp send --task --terminal` blocks subsequent sends | `conversation_full_lifecycle` (steps 4-5) + `send_terminal_blocks_resend` (Plan 03-02) |
| 4 | `famp await --timeout 30s` blocks until inbox arrives | `conversation_full_lifecycle` (step 6) + `await_blocks_until_message` (Plan 03-03) |
| 5 | Records survive daemon restart | `conversation_restart_safety` |

`INBOX-05` (advisory inbox lock) is locked by `conversation_inbox_lock.rs` + the seven `lock_contention` unit tests.

## Task Commits

1. **feat(03-04): add `InboxLock` advisory lock + wire `famp await`** — `6189c3f`
2. **test(03-04): add Phase 3 end-to-end conversation integration tests** — `21535e2`

## Test Coverage Added

### `famp-inbox` unit tests (7 new in `tests/lock_contention.rs`)

- `acquire_creates_lock_file_with_pid`
- `drop_removes_lock_file`
- `second_acquire_while_first_held_returns_lock_held`
- `second_acquire_after_drop_succeeds`
- `stale_pid_lock_is_reaped` — writes PID `2147483632` (definitely-dead sentinel) + asserts reacquire
- `unparseable_lock_is_reaped` — writes `not-a-number` + asserts reacquire
- `lock_file_is_mode_0600_on_unix`

### `famp` integration tests (3 new binaries)

- `conversation_full_lifecycle::full_long_task_conversation_completes`
- `conversation_restart_safety::task_record_survives_listener_restart`
- `conversation_inbox_lock::second_await_while_first_holds_lock_returns_lockheld`

## Decisions Made

See frontmatter `key-decisions`. Highlights:

- **Fail-fast contention** instead of spin-wait — single-developer CLI, mask-vs-surface tradeoff lands on the surface side.
- **PID file + RAII** instead of `flock` — survives across `.await` boundaries cleanly, gives cross-uid liveness via `kill(pid, None) EPERM`.
- **`nix` 0.29 on Unix only** — workspace forbids `unsafe_code`, so a raw `libc::kill` is not an option; non-Unix conservatively returns `is_alive=true`.
- **Single shared `FAMP_HOME`** for conversation tests — the Phase 2 keyring only resolves `agent:localhost/self`; multi-home flows are Phase 4 work. Documented inline.
- **`update_peer_endpoint` harness helper** instead of a `famp peer update` subcommand — out-of-scope for Phase 3, two-line atomic rewrite via existing `write_peers_atomic`.

## Deviations from Plan

### Note — Two-home setup adapted to single-home

The plan's task 2 sketched a `setup_two_homes() -> (alice, bob)` helper and full alice↔bob cross-registration. This is **impossible under Phase 2's listen daemon** because its keyring is hardcoded to `{ agent:localhost/self → own vk }`. Plan 03-02's SUMMARY already documented this constraint and used a single shared home for `send_new_task`/`send_deliver_sequence`/`send_terminal_blocks_resend`. Plan 03-04's conversation harness mirrors that established pattern. The cross-home flow is genuine Phase 4 work alongside the multi-entry keyring.

This is a Rule 3 (blocking) adaptation, not a scope change — every assertion the plan called out is still locked, just against a single shared home.

### Rule 3 — Blocking: clippy `redundant_clone` + `match_like_matches_macro` on first lint pass

Two clippy lints surfaced on the initial `cargo clippy -p famp-inbox`:

1. `clippy::redundant_clone` on `path.clone()` inside the `is_alive` early-return arm — the path is moved into the `Err(LockHeld { path, .. })` and the function returns immediately, so the clone is redundant. Removed.
2. `clippy::match_like_matches_macro` on the `kill(...)` arm enumeration — converted to `matches!(..., Ok(()) | Err(EPERM))`.

### Rule 3 — Blocking: `unused_crate_dependencies` on cursor_roundtrip test

Adding `nix` to `famp-inbox`'s Unix dependencies tripped the existing `cursor_roundtrip` test binary's `unused_crate_dependencies` lint (the test file does not directly reference `nix`). Added a `#[cfg(unix)] use nix as _;` silencer matching the existing pattern in that file.

### Rule 3 — Blocking: clippy `doc_markdown` on three new test files

Three new files used bare `UUIDv7` and `conversation_harness.rs` in doc comments and tripped `clippy::doc_markdown`. Wrapped in backticks before the second commit.

### Rule 3 — Blocking: `InboxLock` doesn't implement `Debug`

The first version of `lock_contention.rs` used `panic!("...{other:?}")` on the `Result<InboxLock, InboxError>`, which requires both arms to be `Debug`. Switched to explicit `Err(other) | Ok(_)` arms — keeps the `InboxLock` type free of a derived `Debug` impl that would expose the held file handle.

---

**Total deviations:** 1 Rule-3 plan adaptation (single-home), 4 Rule-3 lint/compile fixes. Zero architectural surprises.

## Verification Artifacts

- `cargo nextest run -p famp-inbox --test lock_contention` → **7 / 7 passed**
- `cargo nextest run -p famp --test conversation_full_lifecycle --test conversation_restart_safety --test conversation_inbox_lock` → **3 / 3 passed**
- `cargo nextest run --workspace` → **343 / 343 passed, 1 skipped** (was 333; +10 new)
- `cargo clippy --workspace --all-targets -- -D warnings` → **0 warnings**
- `cargo tree -i openssl` → `package ID specification \`openssl\` did not match any packages` (openssl gate holds)

## Threat Flags

None. The advisory lock is local-disk only — no new network surface, no auth path. The PID file is mode 0600 (Unix) so other users on a shared host cannot read or tamper with it. The liveness check via `kill(pid, None)` is information-only (no signal is delivered) and the EPERM branch correctly classifies cross-uid live processes as alive. No new crypto, no new trust boundary.

## Phase 3 Closure Note

This plan closes Phase 3. All four plans in the `03-conversation-cli` phase are now executed:

- **Plan 03-01** — storage primitives (`famp-taskdir`, `InboxCursor`, `PeerEntry` schema)
- **Plan 03-02** — outbound CLI (`famp peer add`, `famp send`, TOFU TLS pinning)
- **Plan 03-03** — inbound CLI (`famp await`, `famp inbox list/ack`, locked JSON shape)
- **Plan 03-04** — `InboxLock` + end-to-end integration tests (this plan)

Workspace tests are at **343 / 343**, clippy is clean, the openssl gate holds, and every Phase 3 ROADMAP success criterion has at least one integration test locking it. Ready for Phase 4 (MCP + Claude integration).

## Next Plan Readiness

- **Phase 4** can extend `conversation_harness.rs` with multi-keyring helpers when the multi-entry keyring lands. The `add_self_peer` helper becomes `add_peer_with_principal(home, alias, addr, pubkey, principal)`.
- The locked `InboxLock` semantics mean Phase 4's MCP server can rely on `famp await` failing fast with a typed error rather than blocking forever on a stuck holder.

## Self-Check: PASSED

- `crates/famp-inbox/src/lock.rs` — FOUND
- `crates/famp-inbox/tests/lock_contention.rs` — FOUND
- `crates/famp/tests/common/conversation_harness.rs` — FOUND
- `crates/famp/tests/conversation_full_lifecycle.rs` — FOUND
- `crates/famp/tests/conversation_restart_safety.rs` — FOUND
- `crates/famp/tests/conversation_inbox_lock.rs` — FOUND
- Commit `6189c3f` — FOUND in git log
- Commit `21535e2` — FOUND in git log

---
*Phase: 03-conversation-cli*
*Plan: 04*
*Completed: 2026-04-14*
