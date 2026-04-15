# Phase 4 — E2E-02 Manual Witnessed Smoke Test

**Date of run:** _____
**Operator:** _____
**Outcome:** [ ] pass  [ ] fail  [ ] inconclusive

## Preconditions

- [ ] `just ci` is green on HEAD
- [ ] `cargo build --release -p famp` has run
- [ ] Two Claude Code sessions are available on this machine

## Setup Steps

1. Run `just e2e-smoke`. This starts two daemons in the background:
   - Daemon A on 127.0.0.1:18443 with `FAMP_HOME=/tmp/famp-smoke-a`
   - Daemon B on 127.0.0.1:18444 with `FAMP_HOME=/tmp/famp-smoke-b`
   and prints the `.mcp.json` snippet each Claude Code session needs.
2. Paste snippet A into Claude Code session 1 (the "Alice" session).
   Paste snippet B into Claude Code session 2 (the "Bob" session).
3. Confirm each session can list the four tools (`famp_send`,
   `famp_await`, `famp_inbox`, `famp_peers`).

## Protocol

- Session 1 (Alice) opens a new task: asks `famp_send` with
  `mode=new_task, title="hello from alice", peer=bob`.
  Record the task_id: _____
- Session 2 (Bob) waits: asks `famp_await` with `timeout_seconds=60`.
  Observes the commit reply + original request.
- Back-and-forth: each session alternates `famp_await` (to receive)
  and `famp_send` (to reply) until AT LEAST FOUR non-terminal
  `deliver` messages have been exchanged — i.e., at least 2 from each
  side — driven by actual conversational LLM output, not pasted text.
- Session 1 (Alice) closes the task: `famp_send` with `mode=terminal,
  task_id=<above>`.

## Observations (fill in live)

- Total delivers exchanged (must be ≥4): _____
- Final task state on Alice's side (COMPLETED?): _____
- Final task state on Bob's side (if a record exists): _____
- Any errors reported through `famp_error_kind`: _____
- Qualitative notes on Claude Code's tool-call experience: _____

## Teardown

- Stop both daemons (the `just e2e-smoke` runner prints a stop command).
- Archive `/tmp/famp-smoke-a/inbox.jsonl` and `/tmp/famp-smoke-b/inbox.jsonl`
  into `.planning/milestones/v0.8-phases/04-mcp-server-e2e/smoke-artifacts/`
  as evidence.

## Verdict

Fill in Outcome above. If pass, the gsd-verifier marks E2E-02 satisfied.
If fail or inconclusive, record blockers here and cycle back to
`/gsd:plan-phase --gaps`.
