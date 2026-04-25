---
phase: quick-260425-m0f
plan: 01
subsystem: scripts / ops
tags: [shell, ops, daemon-management, safety-guards, idempotent]
dependency_graph:
  requires: []
  provides: [scripts/redeploy-listeners.sh, README redeploy section]
  affects: [famp daemon lifecycle, operator runbook]
tech_stack:
  added: []
  patterns: [SIGTERM-then-SIGKILL, PID-file-lifecycle, log-append-not-truncate, in-flight-task-guard]
key_files:
  created:
    - scripts/redeploy-listeners.sh
  modified:
    - README.md
decisions:
  - "--dry-run runs safety guards (in-flight task check) before printing plan; this is correct — guards are prerequisites not side-effects"
  - "APPEND (>>) to daemon.log instead of famp-local's TRUNCATE (>) — preserves restart history per task brief"
  - "tail -n +<pre-restart-line-count> scoping for beacon detection — avoids false positives from prior-boot log lines"
metrics:
  duration: "5m 29s"
  completed: "2026-04-25T20:02:03Z"
  tasks_completed: 3
  files_changed: 2
---

# Phase quick-260425-m0f Plan 01: Write scripts/redeploy-listeners.sh (Safe Redeploy) Summary

**One-liner:** Safe, idempotent daemon-redeploy script with dirty-tree + in-flight-task guards, SIGTERM/SIGKILL cycling, and per-agent restart verification via daemon.log beacon polling.

## What Was Shipped

### Task 1: scripts/redeploy-listeners.sh (394 lines, executable)

Full T1.3 implementation from parent plan `ok-now-analyze-and-toasty-waffle.md`. The script:

- Mirrors `scripts/famp-local` conventions verbatim: `STATE_ROOT`, `FAMP_BIN`, `home_for()`, `pid_for()`, `log_for()`, `is_alive()`, `die()`, `note()`.
- Discovers daemons from `~/.famp-local/agents/*/daemon.pid` — no hard-coded names.
- Guards: dirty-tree (refused `crates/famp/` uncommitted changes) + in-flight task (refused REQUESTED/COMMITTED TOMLs).
- SIGTERM → 8s poll → SIGKILL fallback; cleans PID file on clean exit.
- Appends (`>>`) to daemon.log — preserves restart history across cycles.
- Polls daemon.log for `"listening on https://"` lines that appear after restart (scoped to post-restart line count); 10s per-agent timeout.
- Final table: agent / stop-result / restart-result / pid / log-path.
- Flags: `--dry-run` / `--check`, `--force`, `--no-rebuild`, `--help`.

**Commit:** `af4c8e9`

### Task 2: README.md "Redeploying after daemon code changes" section

Inserted between "## Quick Start (local)" and "## Advanced: manual CLI (federation path)". Contains all four flag examples and both safety-guard explanations. Surgical: 18 lines added, zero other lines touched.

**Commit:** `c018ed1`

### Task 3: Regression verification (no source files modified)

- `cargo nextest run --workspace`: **397/397 passed, 2 skipped** — at or above 397 baseline.
- `cargo clippy --workspace --all-targets -- -D warnings`: **clean, exit 0**.

## Verification Output

### shellcheck
```
(clean — exit 0, zero warnings)
shellcheck 0.11.0
```

### bash -n
```
(clean — exit 0)
```

### --help output
```
Usage: scripts/redeploy-listeners.sh [OPTIONS]

Rebuild the famp binary and safely restart all running listener daemons.

OPTIONS
  --dry-run, --check   Print what would happen; take no destructive action.
  --force              Skip the in-flight task safety check.
  --no-rebuild         Skip `cargo install`; just cycle daemons.
  -h, --help           Show this message and exit 0.
...
```

### --dry-run smoke (with --force to bypass in-flight guard)
```
redeploy-listeners plan:
  rebuild:  YES (cargo install --path crates/famp)
  agents:   8
    agent-a          pid=1300     log=...daemon.log
    agent-b          pid=1311     log=...daemon.log
    agent-c          pid=1365     log=...daemon.log
    architect        pid=1320     log=...daemon.log
    baalshem         pid=1329     log=...daemon.log
    eli              pid=1338     log=...daemon.log
    FAMP             pid=1347     log=...daemon.log
    zed              pid=1356     log=...daemon.log

redeploy-listeners: --dry-run mode — no action taken.
(exit 0)
```

`cargo install` did NOT run during `--dry-run`. PID file timestamps on
`agent-a/daemon.pid` and `agent-b/daemon.pid` confirmed as `Apr 24 18:28`
(unchanged). Plan-checker concern resolved: dry-run suppresses cargo.

NOTE: Running `--dry-run` without `--force` on this machine exits 1 because
there are real in-flight tasks (COMMITTED/REQUESTED) — this is correct behavior.
The guards are prerequisites, not side effects.

### nextest
```
Summary [14.319s] 397 tests run: 397 passed, 2 skipped
```

### clippy
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.13s
(exit 0, zero warnings)
```

### Surgical scope
```
git diff --name-only HEAD~2 HEAD
README.md
scripts/redeploy-listeners.sh
```

## Shellcheck Suppressions

None. Script is clean with zero suppressions. The `SC2086` advisory noted in the
task brief (from famp-local line 587) was not triggered by this script's patterns.

## Operator Usage

```bash
# See what would happen (no action):
scripts/redeploy-listeners.sh --dry-run --force

# Full rebuild + restart (interactive prompt before destructive steps):
scripts/redeploy-listeners.sh

# Skip rebuild, just cycle daemons:
scripts/redeploy-listeners.sh --no-rebuild

# Override in-flight task check (you own the consequences):
scripts/redeploy-listeners.sh --force
```

## Parent Plan Reference

This script is T1.3 from `~/.claude/plans/ok-now-analyze-and-toasty-waffle.md`.

**IMPORTANT REMINDER from parent plan:** Do NOT run the script during the active
deck cycle — wait until agent-b ships the final PDF before cycling the listeners.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED

- `scripts/redeploy-listeners.sh` exists: FOUND
- `README.md` redeploy section: FOUND
- Commit af4c8e9: FOUND (`feat(quick-260425-m0f-01): write scripts/redeploy-listeners.sh`)
- Commit c018ed1: FOUND (`docs(quick-260425-m0f-01): add README pointer to redeploy-listeners.sh`)
- 397/397 tests green: CONFIRMED
- Clippy clean: CONFIRMED
- Only two files changed: CONFIRMED
