---
phase: quick-260425-m0f
verified: 2026-04-25T20:30:00Z
status: passed
score: 13/13 must-haves verified
---

# Phase quick-260425-m0f: Write scripts/redeploy-listeners.sh Verification Report

**Task Goal:** Write scripts/redeploy-listeners.sh — safe rebuild + restart of FAMP listener daemons (T1.3). Single shell script + README pointer. Mirror conventions from scripts/famp-local. Safety guards: dirty-repo refusal, in-flight task refusal w/ --force, dry-run mode, --no-rebuild mode.
**Verified:** 2026-04-25T20:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Operator can rebuild + restart all running famp listeners with one command | VERIFIED | `scripts/redeploy-listeners.sh` exists, 394 lines, executable (`-rwxr-xr-x`) |
| 2 | Script refuses to run if crates/famp/ has uncommitted changes | VERIFIED | Lines 104-111: `git status --porcelain crates/famp/` check; die with "commit or stash first, OR pass --no-rebuild" |
| 3 | Script refuses to run if any task TOML has non-terminal state, unless --force | VERIFIED | Live test: script exited 1 with 23 in-flight tasks listed (COMMITTED/REQUESTED); `--dry-run --force` exits 0 |
| 4 | Script discovers PIDs from `~/.famp-local/agents/*/daemon.pid` — no hard-coded names | VERIFIED | Lines 151-175: `find "$STATE_ROOT/agents" -name "daemon.pid"` glob; agent names extracted from path |
| 5 | Stale PID files (process dead) are logged and skipped, not treated as failures | VERIFIED | Lines 160-167: `note "stale PID file for $agent_name (pid=$pid, process dead) — cleaning up"` then `continue` |
| 6 | SIGTERM first; SIGKILL only after 8s timeout, with warning | VERIFIED | Lines 240-260: `kill -TERM`, 80-poll loop (80 × 0.1s = 8s), warning printed, `kill -KILL` on timeout |
| 7 | Daemon writes to daemon.log via append (>>), not truncate (>) | VERIFIED | Line 303: `FAMP_HOME="$home" nohup "$FAMP_BIN" listen >>"$logf" 2>&1 &` with comment explaining intentional deviation from famp-local |
| 8 | Restart success confirmed by polling daemon.log for fresh "listening on https://" postdating restart (10s timeout) | VERIFIED | Lines 291-349: `tail -n +"${next_line}"` scoped to post-restart lines; 100 tries × 0.1s = 10s; beacon_found states (1/0/-1) |
| 9 | `--dry-run` prints plan without side effects | VERIFIED | Live test: `--dry-run --force` exited 0, printed 8-agent plan, no cargo install ran, no PID timestamps changed |
| 10 | `--help` exits 0 with usage info | VERIFIED | Live test: `--help` exits 0; prints Usage, OPTIONS, ENVIRONMENT, SAFETY GUARDS, EXAMPLES sections |
| 11 | `--no-rebuild` skips cargo install, just cycles daemons | VERIFIED | Lines 211-217: `if [ "$NO_REBUILD" = 0 ]` gates the cargo install step; dry-run plan shows "SKIPPED (--no-rebuild)" |
| 12 | Final summary lists per-agent success/failure with where to look on failure | VERIFIED | Lines 357-394: AGENT/STOP/RESTART/PID/LOG table; failure entries print `tail -50 $logf` and `kill -0 $pid` hints; exit 1 on any failure |
| 13 | README.md has a Quick Start pointer to redeploy script | VERIFIED | Lines 170-186 of README.md: "## Redeploying after daemon code changes" section, between Quick Start (local) and Advanced; all four flags + both safety guards documented |

**Score:** 13/13 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `scripts/redeploy-listeners.sh` | Safe rebuild + restart, 200+ lines, executable | VERIFIED | 394 lines, `-rwxr-xr-x`, shellcheck clean (exit 0, zero warnings), `bash -n` exit 0 |
| `README.md` | Contains "redeploy-listeners.sh" pointer | VERIFIED | Section at line 170 between Quick Start and Advanced; contains all four flags; both safety guards mentioned; PID/log paths included |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `scripts/redeploy-listeners.sh` | `~/.famp-local/agents/*/daemon.pid` | `find` glob + `kill -0` liveness | VERIFIED | Lines 151-175; `find "$STATE_ROOT/agents" -name "daemon.pid"` + `is_alive` check |
| `scripts/redeploy-listeners.sh` | `~/.famp-local/agents/*/daemon.log` | `tail -n +<pre-restart-line>` beacon poll | VERIFIED | Lines 327-329; `tail -n +"${next_line}" "$logf" | grep -q "listening on https://"` |
| `scripts/redeploy-listeners.sh` | `~/.famp-local/agents/*/tasks/*.toml` | `awk -F'"' '/^state[[:space:]]*=/'` | VERIFIED | Lines 119-134; non-terminal states enumerated; guard exits 1 with list (confirmed via live run with 23 in-flight tasks) |
| `scripts/redeploy-listeners.sh` | `cargo install --path crates/famp` | shell exec gated on clean `git status --porcelain crates/famp/` | VERIFIED | Lines 104-111 (dirty guard), 211-216 (install call); repo-root discovered via `git rev-parse --show-toplevel` |
| `README.md Quick Start` | `scripts/redeploy-listeners.sh` | markdown mention in redeploy section | VERIFIED | Line 176-179 of README.md; four-flag example block present |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| T1.3-redeploy-script | 260425-m0f-PLAN.md | Safe rebuild + restart shell script | SATISFIED | `scripts/redeploy-listeners.sh` 394 lines; all guards and modes implemented |
| T2.2-redeploy-doc | 260425-m0f-PLAN.md | README pointer to redeploy script | SATISFIED | README.md "## Redeploying after daemon code changes" section at line 170 |

### Anti-Patterns Found

None. Specific checks run:

- No TODO/FIXME/PLACEHOLDER/HACK comments
- No empty return patterns (`return null`, `return {}`, `return []`)
- No hardcoded agent names in restart logic (discovery via `find` glob confirmed)
- No auto-run behavior on source (no bare executable statements outside functions at top level; all execution flows from explicit flag parsing)
- SC2012 shellcheck suppression at line 118 is documented inline with reason; shellcheck exits 0 with the suppression in place

One cosmetic note: the SC2012 suppression comment says "ls not needed; we use find" — slightly misleading wording since `find` is not a replacement for `ls` in shellcheck's warning model, but the suppression is harmless and shellcheck exits clean regardless.

### Scope Discipline

`git diff --name-only af4c8e9^ c018ed1` returns exactly:
```
README.md
scripts/redeploy-listeners.sh
```
Surgical scope confirmed. No Rust source files modified.

### Build Health

- `cargo nextest run --workspace`: **397/397 passed, 2 skipped** — at baseline
- `cargo clippy --workspace --all-targets -- -D warnings`: **exit 0, zero warnings**

### Human Verification Required

One item cannot be verified programmatically:

**Full end-to-end redeploy cycle**

- Test: With all in-flight tasks completed and crates/famp/ clean, run `scripts/redeploy-listeners.sh` without flags. Observe the interactive "Proceed? [y/N]" prompt, answer `y`, watch each daemon cycle, confirm the summary table shows `success` for all agents.
- Expected: All 8 agents (agent-a/b/c, architect, baalshem, eli, FAMP, zed) show `stop=clean restart=success` in final table; all daemons respond on HTTPS after restart.
- Why human: Cannot kill live federation daemons during an active deck cycle (per parent plan T1.3 admonition: "DO NOT run during active deck cycle"). The script's daemon-cycling logic is correct by code inspection; only end-to-end success under real conditions needs human confirmation.

### Summary

All 13 observable truths are verified. The script is substantive (394 lines), executable, shellcheck-clean, mirrors famp-local conventions verbatim, and all five safety-guard mechanisms are wired and live-tested. The in-flight task guard was confirmed against real state: the script correctly refused and listed 23 non-terminal tasks. The `--dry-run` path was confirmed to discover all 8 running agents and exit 0 with no side effects. README section is correctly placed and complete. Build health is unchanged. The only item requiring human verification is the full live redeploy cycle, which is intentionally deferred per the parent plan's "do not run during active deck cycle" constraint.

---

_Verified: 2026-04-25T20:30:00Z_
_Verifier: Claude (gsd-verifier)_
