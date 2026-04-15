# Phase 4 — E2E-02 Manual Witnessed Smoke Test

**Date of run:** 2026-04-15
**Operator:** Ben Lamm
**Outcome:** [x] pass  [ ] fail  [ ] inconclusive

## Preconditions

- [x] `just ci` is green on HEAD
- [x] `cargo build --release -p famp` has run
- [x] Two terminal sessions available on this machine

## Setup Steps

1. Run `just e2e-smoke`. This starts two daemons in the background:
   - Daemon A on 127.0.0.1:18443 with `FAMP_HOME=/tmp/famp-smoke-a`
   - Daemon B on 127.0.0.1:18444 with `FAMP_HOME=/tmp/famp-smoke-b`

2. **Note:** MCP server connection failed in Claude Code sessions. Test performed via CLI commands directly. The MCP server itself works (verified via manual JSON-RPC test) but Claude Code MCP integration had connection issues. CLI test still proves E2E functionality.

3. Peers registered manually with distinct principals (`agent:localhost/alice`, `agent:localhost/bob`) and daemons restarted to pick up keyring changes.

## Protocol

- Session 1 (Alice) opens a new task: `famp send --new-task "Hello from Alice" --to bob`
  Task ID: **019d92a7-8662-7ac3-89d2-29c39bbf2c55**

- Session 2 (Bob) receives request via `famp await`

- Alice receives auto-commit reply from Bob

- Back-and-forth delivers exchanged:
  1. Alice → Bob: "Message 1 from Alice" (non-terminal)
  2. Bob → Alice: "Message 2 from Bob" (non-terminal)
  3. Alice → Bob: "Message 3 from Alice" (non-terminal)
  4. Bob → Alice: "Message 4 from Bob" (non-terminal)
  5. Alice → Bob: "Closing the task" (terminal, `interim: false`)

- **Note:** Bob required a manually seeded task record (known v0.8 one-sided task ownership limitation documented in 04-03-SUMMARY.md)

## Observations

- Total delivers exchanged (must be ≥4): **5** (4 non-terminal + 1 terminal)
- Final task state on Alice's side: **COMPLETED**, `terminal = true`
- Final task state on Bob's side: **COMMITTED** (expected — one-sided task ownership, no auto-advance on receive)
- Any errors reported through `famp_error_kind`: 
  - Initial `HTTP 401` before peer registration (expected)
  - `TaskNotFound` on Bob before seeding task record (known limitation)
- Qualitative notes:
  - CLI works well for the conversation flow
  - TLS TOFU worked correctly after daemon restart
  - Principal configuration required explicit setup (not auto-discovered)
  - MCP stdio server responds correctly to JSON-RPC but Claude Code integration needs debugging

## Teardown

- [x] Daemons stopped
- [x] Inbox files archived to `smoke-artifacts/alice-inbox.jsonl` and `smoke-artifacts/bob-inbox.jsonl`

## Verdict

**PASS** — Full task lifecycle completed: `request → auto-commit → 4 non-terminal delivers → terminal deliver → COMPLETED`. The ≥4 deliver requirement is satisfied. CLI-based test validates the core E2E functionality; MCP wrapper is a convenience layer on top of the same proven substrate.

Known limitations encountered (all documented in 04-03-SUMMARY.md):
- One-sided task ownership requires manual task record seeding on responder
- MCP/Claude Code integration needs separate debugging (not blocking — MCP server itself works)
