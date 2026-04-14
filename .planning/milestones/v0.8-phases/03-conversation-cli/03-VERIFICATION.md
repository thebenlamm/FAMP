---
phase: 03-conversation-cli
verified: 2026-04-14T00:00:00Z
status: passed
score: 5/5
overrides_applied: 0
test_gates:
  nextest: "343/343 passed, 1 skipped"
  clippy: "0 warnings (--workspace --all-targets -D warnings)"
  openssl_gate: "empty (cargo tree -i openssl)"
requirements_verified:
  - CLI-03
  - CLI-04
  - CLI-05
  - CLI-06
  - CONV-01
  - CONV-02
  - CONV-03
  - CONV-04
  - CONV-05
  - INBOX-02
  - INBOX-03
  - INBOX-05
phase_narrowings:
  - area: "FSM terminal transition"
    detail: "fsm_glue::advance_terminal seeds TaskFsm at Committed before stepping to Completed; v0.7 FSM only permits Committed→Completed on terminal deliver and Phase 3 has no commit-reply round-trip. Marked TODO(phase4) inline."
    accepted_as: "intentional per-phase narrowing"
  - area: "Conversation integration tests"
    detail: "Tests share a single FAMP_HOME because Phase 2's listen daemon uses a single-entry hardcoded keyring (agent:localhost/self). Multi-home + multi-entry keyring lands in Phase 4."
    accepted_as: "intentional per-phase narrowing"
---

# Phase 3: Conversation CLI — Verification Report

**Phase Goal:** A developer can open a task, exchange multiple `deliver` messages within it across two terminal sessions, and close it with a terminal deliver — all through CLI commands — with task state persisted to disk and surviving daemon restarts.

**Verified:** 2026-04-14
**Status:** passed
**Mode:** Initial verification

## Goal Achievement

### ROADMAP Success Criteria (Observable Truths)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp send --new-task "hello" --to alice` sends signed `request`, creates `~/.famp/tasks/<uuid>.toml` in REQUESTED, prints task-id | VERIFIED | `crates/famp/src/cli/send/mod.rs` builds `RequestBody`+signs+POSTs, then `TaskDir::create` persists; locked by `crates/famp/tests/send_new_task.rs` (asserts UUIDv7, REQUESTED state, daemon inbox contains `class: request`) |
| 2 | `famp send --task <id> --to alice` sends `deliver`; record stays non-terminal; multiple sequential calls succeed (long-task shape) | VERIFIED | `send/mod.rs` DeliverBody branch with `interim = true` + `Causality::Delivers`; `crates/famp/tests/send_deliver_sequence.rs` runs 3 non-terminal delivers, asserts REQUESTED + 4 inbox lines |
| 3 | `famp send --task <id> --terminal` transitions record to COMPLETED via `famp-fsm`; subsequent sends on same task error with `task_terminal` | VERIFIED | `fsm_glue::advance_terminal` (seeds Committed per phase narrowing, then `TaskFsm::step` → Completed); `crates/famp/tests/send_terminal_blocks_resend.rs` asserts `CliError::TaskTerminal { task_id }` and byte-identical record after rejection |
| 4 | `famp await --timeout 30s` blocks up to 30s, returns structured (task-id, from, class, body), typed timeout error on expiry | VERIFIED | `crates/famp/src/cli/await_cmd/{mod,poll}.rs` uses 250 ms poll + `humantime` parse + `InboxCursor::advance`; locked JSON shape `{offset, task_id, from, class, body}`; `tests/await_blocks_until_message.rs` + `tests/await_timeout.rs` lock success + timeout paths |
| 5 | Task records survive daemon restart; after `famp listen` stop/restart, `famp send --task <id>` still finds the task record | VERIFIED | `famp-taskdir` writes plain TOML under `<home>/tasks/<uuid>.toml`; daemon never touches them; `crates/famp/tests/conversation_restart_safety.rs` stops+respawns listener, repoints peers.toml, asserts byte-identical record and successful subsequent deliver |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-taskdir/src/{lib,store,record,error,atomic}.rs` | Per-task TOML store (create/read/update/list, atomic, UUID-validated) | VERIFIED | 283 LoC total; `store::{create,read,update,list}` present; 9 roundtrip tests pass |
| `crates/famp-inbox/src/cursor.rs` | `InboxCursor::{at,read,advance}` with 0600 atomic replace | VERIFIED | 92 LoC; 5 cursor tests + atomic-concurrent-writers test pass |
| `crates/famp-inbox/src/lock.rs` | RAII `InboxLock` with PID file + stale-reap via `nix::kill(_, None)` | VERIFIED | 132 LoC; 7 lock_contention tests pass; fail-fast <500 ms locked by `conversation_inbox_lock.rs` |
| `crates/famp/src/cli/send/{mod,client,fsm_glue}.rs` | `famp send` new-task/deliver/terminal with TOFU TLS pinning + FSM glue | VERIFIED | 638 LoC total; `TofuVerifier` pins `sha256(leaf_cert)` hex into `peers.toml.tls_fingerprint_sha256`; `advance_terminal` drives FSM |
| `crates/famp/src/cli/peer/{mod,add}.rs` | `famp peer add` validated + atomic peers.toml write | VERIFIED | 119 LoC; 5 peer_add tests cover duplicate, http endpoint, short pubkey, garbage pubkey |
| `crates/famp/src/cli/await_cmd/{mod,poll}.rs` | `famp await` 250 ms poll + typed timeout + `--task` filter | VERIFIED | 150 LoC; uses `read_from` per-entry offsets; acquires `InboxLock` on entry |
| `crates/famp/src/cli/inbox/{mod,list,ack}.rs` | `famp inbox list [--since]` + `famp inbox ack <offset>` | VERIFIED | 107 LoC; non-mutating list + cursor-advance ack; `inbox_list_respects_cursor.rs` locks semantics |
| `crates/famp/tests/conversation_*.rs` | Full lifecycle + restart safety + lock contention E2E | VERIFIED | 3 integration binaries present; all pass |

### Key Links (Wiring)

| From | To | Via | Status | Evidence |
|------|----|-----|--------|----------|
| `cli::mod::run` | `cli::send`, `cli::peer`, `cli::await_cmd`, `cli::inbox` | `Commands::{Send,Peer,Await,Inbox}` variants | WIRED | `Grep` confirms all 4 variants dispatched; `bin/famp.rs` clap subcommand definitions updated |
| `famp send` | `famp-taskdir::TaskDir::create/update` | POST-first / persist-second ordering | WIRED | `send/mod.rs` writes record only after HTTP 2xx; `send_terminal_blocks_resend.rs` proves byte-identical record after rejected send |
| `famp send --terminal` | `famp-fsm::TaskFsm::step` | `fsm_glue::advance_terminal` | WIRED (narrowed) | Seeds at Committed before stepping to Completed — accepted phase narrowing; locked by `send_terminal_blocks_resend.rs` |
| `famp await` | `famp-inbox::read_from` + `InboxCursor::advance` + `InboxLock` | `await_cmd::poll` loop | WIRED | Per-entry `(Value, end_offset)` pattern; fail-fast on lock contention |
| `famp peer add` | `config::write_peers_atomic` | same-dir tempfile + fsync + rename + chmod 0600 | WIRED | `peer_add_rejects_duplicate` proves atomic persistence |
| `famp send` HTTPS client | `reqwest` + custom `rustls::ServerCertVerifier` (TOFU) | `send/client.rs::TofuVerifier` + `post_envelope` | WIRED | First-contact captures leaf SHA-256; subsequent mismatch → `CliError::TlsFingerprintMismatch` via error-string marker parse |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CLI-03 | 03-02 | `famp send` new-task / deliver / terminal modes | SATISFIED | `send/mod.rs`; 3 send_* integration tests |
| CLI-04 | 03-03 | `famp await [--timeout]` block-with-timeout | SATISFIED | `await_cmd/`; `await_blocks_until_message.rs` + `await_timeout.rs` |
| CLI-05 | 03-03 | `famp inbox [--unread]` non-blocking list | SATISFIED | `inbox/list.rs`+`ack.rs`; `inbox_list_respects_cursor.rs` |
| CLI-06 | 03-02 | `famp peer add` with validation + keyring format | SATISFIED | `peer/add.rs`; 5 `peer_add.rs` tests |
| CONV-01 | 03-02 | New-task: `request` + task record creation in REQUESTED | SATISFIED | `send_new_task.rs` |
| CONV-02 | 03-02 | Multi-deliver within one task (long-task shape) | SATISFIED | `send_deliver_sequence.rs` + `conversation_full_lifecycle.rs` step 3 |
| CONV-03 | 03-02 | Terminal deliver + FSM COMPLETED transition + fail-closed resend | SATISFIED | `send_terminal_blocks_resend.rs` + `conversation_full_lifecycle.rs` steps 4-5 |
| CONV-04 | 03-03 | Task records survive daemon restarts | SATISFIED | `conversation_restart_safety.rs` |
| CONV-05 | 03-04 | v0.7 `famp-fsm` used as-is, no new states/classes | SATISFIED | `fsm_glue.rs` imports `famp_fsm::{TaskFsm,TaskState}` only; no new variants added; compile-check protects invariant |
| INBOX-02 | 03-01 | Sidecar read cursor file | SATISFIED | `famp-inbox/src/cursor.rs`; 0600 atomic replace |
| INBOX-03 | 03-03 | `famp await` poll-with-timeout semantics | SATISFIED | `await_cmd/poll.rs` 250 ms `POLL_INTERVAL` const |
| INBOX-05 | 03-04 | `inbox.lock` advisory lock | SATISFIED | `famp-inbox/src/lock.rs`; 7 lock_contention tests + conversation_inbox_lock.rs |

No orphaned requirements — every v0.8 REQUIREMENTS.md entry mapped to Phase 3 is covered by a Phase 3 plan and verified in code.

### Test Gate Results

| Gate | Result |
|------|--------|
| `cargo nextest run --workspace` | **343 passed, 1 skipped** (exit 0) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warnings** (exit 0) |
| `cargo tree -i openssl` | **empty** — no openssl/native-tls pulled |

### Anti-Patterns Scan

| Severity | Finding |
|----------|---------|
| Info | `fsm_glue::advance_terminal` seeds TaskFsm at Committed with `TODO(phase4)` — documented phase narrowing, not a stub. Tests lock the narrowed behavior end-to-end. |
| Info | Conversation integration tests share one FAMP_HOME instead of two — documented phase narrowing awaiting Phase 4 multi-entry keyring. |
| Info | Workspace `reqwest` entry uses nonexistent `rustls-tls-native-roots` feature — latent bug inherited from v0.7 workspace manifest. Plan 03-02 worked around it by declaring `reqwest` locally in `crates/famp/Cargo.toml`. Flagged here for cleanup in a later phase, but does not gate Phase 3. |

No stub code paths, no placeholder UI, no `TODO`-blocking markers in shipped logic. The two `TODO(phase4)` comments are explicit roadmap deferrals, not gaps.

### Behavioral Spot-Checks

Performed via the full workspace test suite (343 tests run under `cargo nextest`). Notable phase-3 integration binaries that ran green:

- `send_new_task`, `send_deliver_sequence`, `send_terminal_blocks_resend`
- `peer_add` (5 rows)
- `await_blocks_until_message`, `await_timeout`
- `inbox_list_respects_cursor`
- `conversation_full_lifecycle`, `conversation_restart_safety`, `conversation_inbox_lock`
- `famp-inbox/tests/lock_contention` (7 rows), `cursor_roundtrip`, `read_from`
- `famp-taskdir/tests/roundtrip` (9 rows)

Each integration test invokes the actual CLI entry points (`run_at` / `run_add_at` / `await_cmd::run_at`) and the real `famp-transport-http` daemon on an ephemeral port — not stubs.

### Human Verification Required

None. The phase's contract is fully automated via integration tests. A live two-terminal smoke test is explicitly scoped to Phase 4 (E2E-02) and is not required to close Phase 3.

### Gaps Summary

No gaps. All 5 ROADMAP success criteria are locked by at least one integration test, all 12 requirements mapped to Phase 3 are satisfied, all gates (nextest, clippy, openssl) are green, and the two phase narrowings (FSM seeding shortcut, single-home test harness) are explicit roadmap deferrals to Phase 4 — not verification gaps.

---

*Verified: 2026-04-14*
*Verifier: Claude (gsd-verifier)*
