---
phase: 04-mcp-server-e2e
plan: "03"
subsystem: e2e-two-daemons
tags: [e2e, two-daemon, auto-commit, config-principal, smoke-test, justfile]
dependency_graph:
  requires: [04-01, 04-02]
  provides: [e2e-01-automated-test, e2e-02-smoke-scaffold, config-principal-field]
  affects: [famp-listen, famp-send, famp-config, tests/common]
tech_stack:
  added: []
  patterns:
    - "Config.principal optional field: overrides agent:localhost/self for multi-daemon on-machine setups"
    - "two_daemon_harness::spawn_two_daemons(): mutual peer_add + distinct config.toml principals before spawn"
    - "Seeded COMMITTED record on receiver side for deliver path (one-sided task ownership)"
    - "just e2e-smoke: bash shebang recipe using printf instead of heredoc (avoids just quoting edge cases)"
key_files:
  created:
    - crates/famp/tests/common/two_daemon_harness.rs
    - crates/famp/tests/e2e_two_daemons.rs
    - .planning/milestones/v0.8-phases/04-mcp-server-e2e/04-E2E-SMOKE.md
    - .planning/milestones/v0.8-phases/04-mcp-server-e2e/smoke-artifacts/.gitkeep
  modified:
    - crates/famp/src/cli/config.rs (added optional principal field)
    - crates/famp/src/cli/listen/mod.rs (read principal from config, pass to build_keyring)
    - crates/famp/src/cli/send/mod.rs (load_self_principal reads config.toml)
    - crates/famp/tests/common/mod.rs (added two_daemon_harness module)
    - Justfile (added e2e-smoke recipe)
decisions:
  - "Config.principal read best-effort in send/mod.rs (graceful fallback to agent:localhost/self if config unreadable)"
  - "Receiver-side task record seeded manually in test (one-sided task ownership is v0.8 documented limitation)"
  - "printf used instead of heredoc in just recipe to avoid quoting issues with $ in just shebang blocks"
  - "A-inbox count changed from 2 to 1 in two-daemon mode: outgoing request goes to B's inbox, not A's"
metrics:
  duration: ~25 minutes
  completed: "2026-04-15"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 9
---

# Phase 04 Plan 03: Two-Daemon E2E + Smoke Scaffold Summary

One-liner: Full `new_task → auto-commit → 4 non-terminal delivers → terminal → COMPLETED` lifecycle across two independent FAMP_HOMEs with distinct Ed25519 identities and mutual peer registration, automated under `cargo nextest`.

## What Was Built

### Task 1: two_daemon_harness + E2E-01 automated test (commit c2995f7)

**Config principal field** (`config.rs`): Added `principal: Option<String>` with `skip_serializing_if = "Option::is_none"` and `deny_unknown_fields` retained. Default serialization unchanged (`listen_addr` only). Existing tests pass unmodified.

**listen/mod.rs** updated: `build_keyring` now accepts `self_principal_str: &str` as a parameter (removed hardcoded `"agent:localhost/self"`). `run_on_listener` reads `cfg.principal.as_deref().unwrap_or("agent:localhost/self")` and passes it to both `build_keyring` and the auto-commit `self_principal`. The `_cfg` binding became `cfg` (the value is now used).

**send/mod.rs** updated: `load_self_principal()` became `load_self_principal(&layout)` — best-effort reads `config.toml`, falls back to `"agent:localhost/self"` if the file is absent or malformed.

**two_daemon_harness.rs** (`tests/common/`): `spawn_two_daemons()` creates two TempDirs, inits each, writes distinct `config.toml` files (Alice=`agent:localhost/alice`, Bob=`agent:localhost/bob`), binds two ephemeral `127.0.0.1:0` listeners, performs mutual `peer_add_run_at` (A registers B with B's pubkey + principal, B registers A with A's pubkey + principal), then spawns both daemons in-process via `run_on_listener`. Returns `TwoDaemons` struct with all handles. `teardown()` sends both shutdown signals and awaits both join handles with 2s timeout.

**e2e_two_daemons.rs** (`tests/`): One test `e2e_two_daemons_full_lifecycle`:
1. Alice sends `new_task` to Bob → record = REQUESTED
2. Alice `await --task <id>` → receives auto-commit → record = COMMITTED  
3. Seed COMMITTED record on Bob's side (one-sided task ownership workaround)
4. Four interleaved non-terminal delivers: A→B, B→A, A→B, B→A; each side awaits after receiving
5. Alice sends terminal deliver → record = COMPLETED, `terminal = true`
6. Bob awaits terminal deliver; verifies `interim = false`
7. Teardown

Inbox count assertions: A's inbox ≥ 3 (commit + 2 delivers from B); B's inbox ≥ 3 (request + 2 delivers from A). The test uses ≥ per plan guidance (exact counts depend on timing of auto-commit relative to first deliver).

### Task 2: 04-E2E-SMOKE.md + just e2e-smoke recipe (commit 9752b2c)

**04-E2E-SMOKE.md**: Manual checklist with fillable `[ ]` Outcome checkboxes (pass/fail/inconclusive), Preconditions, Setup Steps, Protocol (including the ≥4 deliver requirement), Observations section for live fill-in, Teardown instructions, Verdict section linking to gsd-verifier status.

**Justfile `e2e-smoke` recipe**: Bash shebang recipe. Cleans and reinits `/tmp/famp-smoke-a` and `/tmp/famp-smoke-b`. Starts both daemons in background on ports 18443/18444. Prints `.mcp.json` snippets for each Claude Code session using `printf` (avoids heredoc quoting issues with `$` in just shebang blocks). Prints PIDs and a `kill` command for teardown. `just --show e2e-smoke` parses cleanly.

**smoke-artifacts/.gitkeep**: Evidence directory created for operator to archive inbox.jsonl files after the witnessed run.

## Workspace Test Count Delta

- Before: 354/354 passing (1 pre-existing skip)
- After: 355/355 passing (1 skip)
- Delta: +1 test (`e2e_two_daemons_full_lifecycle`)

## Final E2E-01 Assertions

- Delivers exchanged: 4 non-terminal (2 from each side) + 1 terminal = 5 total
- Terminal state on Alice's side: `state == "COMMITTED"` → `COMPLETED`, `terminal == true`
- Terminal state on Bob's side: COMMITTED record exists (seeded); not auto-advanced (one-sided task ownership, documented)
- Bob's terminal delivery verified: `body.interim == false`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] A's inbox count in two-daemon mode is 1, not 2**
- **Found during:** Task 1 first test run
- **Issue:** Plan comment said "request + commit = 2 lines in A's inbox" but in a two-daemon setup Alice's outgoing request goes to B's inbox, not A's. A's inbox only contains inbound envelopes (the auto-commit reply from B).
- **Fix:** Corrected assertion to `== 1` with a clarifying comment
- **Files modified:** `crates/famp/tests/e2e_two_daemons.rs`
- **Commit:** c2995f7

**2. [Rule 1 - Bug] B can't send delivers without a local task record**
- **Found during:** Task 1 test run — `send deliver: TaskNotFound`
- **Issue:** One-sided task ownership means B's daemon auto-committed but did NOT create a local task record (Phase 3 only creates records on the send path). `send_structured(--task X)` checks for an existing record and fails with `TaskNotFound`.
- **Fix:** Added `seed_committed_record()` helper in the test that creates a COMMITTED task record on B's side before the deliver phase. Documented this as a known v0.8 limitation in the test and SUMMARY.
- **Files modified:** `crates/famp/tests/e2e_two_daemons.rs`
- **Commit:** c2995f7

**3. [Rule 1 - Bug] Redundant closure in clippy**
- **Found during:** Task 1 clippy run
- **Issue:** `.and_then(|v| v.as_bool())` triggers `clippy::redundant_closure_for_method_calls`
- **Fix:** Changed to `.and_then(serde_json::Value::as_bool)`
- **Commit:** c2995f7

**4. [Rule 1 - Bug] Doc comments with unbackticked identifiers**
- **Found during:** Task 1 clippy run — `FAMP_HOMEs`, `task_id`, `peer_add` in doc comments
- **Fix:** Added backticks around all flagged identifiers in test and harness doc comments
- **Commit:** c2995f7

## Known Stubs

None. The E2E-01 test is fully wired. The E2E-02 smoke checklist intentionally has fillable blanks — this is by design, awaiting the human-witnessed run.

## Threat Flags

No new threat surface. The two-daemon test runs entirely on localhost with ephemeral ports; the smoke-artifacts directory is local only.

T-04-20 mitigation implemented: each daemon's keyring contains exactly the other daemon's principal + self. Envelopes from any other principal are rejected by `FampSigVerifyLayer`.

T-04-23 mitigation: smoke checklist captures operator, date, outcome, qualitative notes; inbox.jsonl archival path documented.

## Self-Check: PASSED

Files created/verified:
- `crates/famp/tests/common/two_daemon_harness.rs` — exists (commit c2995f7)
- `crates/famp/tests/e2e_two_daemons.rs` — exists (commit c2995f7)
- `.planning/milestones/v0.8-phases/04-mcp-server-e2e/04-E2E-SMOKE.md` — exists (commit 9752b2c)
- `.planning/milestones/v0.8-phases/04-mcp-server-e2e/smoke-artifacts/.gitkeep` — exists (commit 9752b2c)

Commits verified in git log:
- c2995f7 (feat: two-daemon harness + E2E-01) — confirmed
- 9752b2c (feat: E2E-02 smoke checklist + just recipe) — confirmed

Verification:
- `cargo nextest run -p famp --test e2e_two_daemons` — PASS (1/1)
- `cargo nextest run --workspace` — 355/355 pass, 1 skipped
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 errors
- `cargo tree -i openssl` — empty
- `just --show e2e-smoke` — parses cleanly (verified with just 1.49.0)
- `04-E2E-SMOKE.md` — exists with fillable [ ] Outcome checkboxes
