# FAMP Listen Mode Design

**Date:** 2026-05-05
**Status:** Approved
**Scope:** v0.9 patch — updates `famp-await.sh` and `famp_register` MCP tool

---

## Problem

The outbound half of agent-to-agent messaging works: a Stop hook (`hook-runner.sh`) fires `famp send` when a registered window edits a file. The inbound half — waking a registered window when a message arrives — was partially implemented (`famp-await.sh` exists) but was never wired for v0.9 and requires a sentinel file the user must create manually.

Sofer's field report surfaced this gap: messages land in agent inboxes with no mechanism to self-wake the receiving window.

---

## Goals

1. A registered window that opts in to listen mode wakes itself up (sub-minute latency) when a FAMP message arrives — no user action required.
2. A registered window that does **not** opt in works normally; the user can check inbox on demand.
3. No window enters listen mode without explicit opt-in. General-purpose dev windows must not be locked into a 23h wait.

---

## Non-Goals

- Pushing messages to already-stopped (closed) Claude Code sessions.
- Changes to the broker, envelope format, or wire protocol.
- Federation / cross-host delivery (v1.0 concern).

---

## Design

### Three Operating Modes

| Mode | How to activate | Behavior |
|------|----------------|----------|
| Register only | `famp_register({identity: "dk"})` | Registers identity; window works normally; inbox accumulates |
| Listen mode | `famp_register({identity: "dk", listen: true})` | Registers AND enters Stop hook receive loop; sub-minute wake-up |
| On-demand check | Any registered window; user prompts "check messages" | Agent calls `famp_inbox`; non-blocking; returns all queued envelopes |

### `famp_register` MCP Tool Change

Add `listen: bool` to the input schema (default `false`). The tool behavior is unchanged — `listen` is a hint for the hook, not a broker protocol field. The tool result must include the resolved identity so the hook can parse it.

### `famp-await.sh` Stop Hook — Updated Behavior

The sentinel gate (`[ -f .famp-listen ]`) is replaced with transcript detection:

1. **Parse transcript JSONL** for the most recent `famp_register` tool call **and its result**.
   - Must find both: a `tool_use` block with `name` ending in `famp_register`, **and** a corresponding `tool_result` block that is not an error.
   - Extract `identity` from the tool input and `listen` flag. If `listen` is absent or `false` → exit 0 (no-op).
2. **Respect `famp_leave`**: if a `famp_leave` call appears *after* the last successful `famp_register` in the transcript → exit 0 (no-op). The agent has de-registered.
3. **Validate identity**: must match `^[A-Za-z0-9_-]+$`. Shell-quote before passing to subprocess.
4. **Call broker**: `famp await --as <identity> --timeout 23h`
5. **On message**: emit `{"decision": "block", "reason": "<preamble><famp-envelope>\n<msg>\n</famp-envelope>"}` — same security framing as current implementation.
6. **On timeout / empty**: exit 0 — Claude stops cleanly.
7. **On error (broker unreachable, `--as` rejected, etc.)**: exit 0 — fail-open, never trap Claude.

The hook's 86400s system timeout (24h) exceeds the `famp await` 23h timeout, ensuring the OS never kills a mid-wait hook process.

### Loop Mechanics

```
agent registers with listen: true
    ↓
agent completes a turn → Claude Code fires Stop hook
    ↓
hook parses transcript → finds listen registration → calls famp await --as dk --timeout 23h
    ↓
[A] message arrives within 23h
    → hook emits {"decision": "block", "reason": "..."} → Claude wakes, processes message
    → agent finishes turn → Stop hook fires again → loop repeats
    ↓
[B] no message for 23h
    → hook exits 0 → Claude stops → window idle
```

On path [B], the agent only re-enters listen mode if the user sends a new message to the window, at which point the next Stop hook firing re-enters the wait.

### On-Demand Check

Any registered window (listen or not) can check its inbox by calling `famp_inbox`. No hook changes required. The user prompts the agent ("check your famp messages") and the agent calls `famp_inbox` in its next turn.

---

## Identity & v0.9 Compatibility

The current hook uses `FAMP_HOME` (v0.8 filesystem-per-identity model). v0.9 uses session-bound identity via `Hello.bind_as` — the proxy protocol where one-shot CLI commands pass `--as <name>` and the broker resolves the effective identity against the live registered session.

**Change:** Replace the `FAMP_HOME` resolution block with `famp await --as <identity>`. The `--as` flag exists on the v0.9 CLI (D-10). No broker changes needed.

---

## Security

The hook's existing hardening is preserved unchanged:
- Envelope content tagged as `<famp-envelope>UNTRUSTED DATA</famp-envelope>` with explicit instruction not to execute contents.
- 64KB cap + UTF-8 sanitization before JSON construction.
- Crash-safe backup of every received envelope to `$XDG_STATE_HOME/famp/received/`.
- Log file symlink hijack guard (`$XDG_STATE_HOME/famp/await-hook.log`).
- Identity validated against `^[A-Za-z0-9_-]+$` before shell interpolation.

---

## Error Handling & Edge Cases

| Case | Behavior |
|------|----------|
| Broker unreachable | `famp await` fails → exit 0 (fail-open) |
| `famp_register` in transcript but result is error | No-op — parse result, not just call |
| `famp_leave` after last registration | No-op — de-registered |
| Re-registration (two `famp_register` calls) | Last successful registration wins |
| `listen: false` (default) | Exit 0 immediately |
| `listen: true`, no registration success | Exit 0 |
| Oversized envelope (>64KB) | Truncated before injection |
| Two windows, same identity | Broker enforces single-listener; second `famp await --as` fails → exit 0; structured log entry written |
| Shell metacharacters in identity | Validated and rejected before subprocess call |
| Long session (registration scrolled out of transcript window) | Hook cannot confirm registration; treats as no-op; user can use on-demand check |

---

## Testing Plan

### 1. Transcript extraction unit tests (`hook_runner_await.rs`)
- Happy path: transcript with `famp_register` (`listen: true`) + success result → identity extracted
- `listen: false` (default) → no-op
- `listen: true` but tool result is error → no-op
- Multiple registrations → last one wins
- Register then leave → no-op
- No `famp_register` in transcript → no-op
- Malformed/truncated transcript JSONL → graceful exit, no panic

### 2. Shell hook behavior tests (mock `famp` binary records argv)
- No transcript → exit 0
- Transcript present, no registration → exit 0
- Registered with `listen: false` → exit 0
- Registered with `listen: true` → `famp await --as <name>` called with correct args
- `famp await` returns envelope → stdout is valid `{"decision": "block", "reason": ...}` JSON
- `famp await` returns `{"timeout": true}` → exit 0 (clean stop)
- `famp await` returns non-zero → exit 0 (fail-open)
- Identity with shell metacharacters → rejected, exit 0

### 3. E2E: full listen loop (`mcp_bus_e2e.rs` extension)
- Agent A registers and sends to agent B
- Agent B's hook fires → emits block decision with correct envelope fields (sender, body)
- Assert round-trip fidelity

### 4. `await_timeout.rs` (fill existing stub)
- Real broker subprocess, `famp await --as <name>`, no sender → `{"timeout": true}` returned cleanly

### 5. UAT checklist (manual)
- Register in window A with `listen: true`, send from window B → window A wakes and reports envelope
- Register in window A with `listen: false`, send from window B → window A stays idle; on-demand `famp_inbox` returns message
- Register then leave in window A, send from window B → window A does not wake
- No message for timeout period → window A stops cleanly

---

## Files Changed

| File | Change |
|------|--------|
| `~/.claude/hooks/famp-await.sh` *(global, not in repo — installed by `famp install-claude-code`)* | Replace sentinel gate with transcript detection; replace `FAMP_HOME` with `--as <identity>`; add `famp_leave` check; add result-vs-call parsing |
| `crates/famp/src/cli/mcp/tools/register.rs` | Add `listen: bool` input field (default `false`) |
| `crates/famp/tests/hook_runner_await.rs` | New unit test file |
| `crates/famp/tests/await_timeout.rs` | Fill existing stub with real broker test |
| `crates/famp/tests/mcp_bus_e2e.rs` | Add listen loop E2E scenario |
| `CLAUDE.md` | Document listen mode usage |
