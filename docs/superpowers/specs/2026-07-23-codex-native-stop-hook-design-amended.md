# Codex Native Stop Hook Design (Amended)

**Date:** 2026-07-23  
**Status:** Proposed (amended after adversarial review)  
**Supersedes for implementation planning:** this document  
**Original (unchanged):** [`2026-07-23-codex-native-stop-hook-design.md`](./2026-07-23-codex-native-stop-hook-design.md)  
**Scope:** Codex listen-mode Stop-hook path, with a shared Rust hook engine that Claude can adopt later without a second rewrite.

---

## Relationship to the original

This amended design keeps the original’s core direction:

- own the Codex Stop-hook lifecycle in the `famp` binary
- remove `jq` / Python / shell JSON templating from the **critical emission path**
- keep fail-open semantics and notification-only reasons

It rewrites the plan where the adversarial review found load-bearing gaps:

- invented Codex `hooks.json` schema (`args` is not used today)
- “reuse transcript logic” when no Rust module exists (full port required)
- silent deletion of battle-tested shell behaviors
- solving a secondary failure mode while the primary Codex report remains residual
- incomplete install/uninstall/trust migration
- under-specified “high-confidence” and testing

---

## Problem

### Primary failure class this design eliminates

The current Codex wake path parks correctly on the bus, then can still fail to wake the host because the final block decision is assembled outside the binary:

- shell + `python3` for stdin / transcript / rollout / meta extraction
- `jq` for the final `{"decision":"block","reason":"..."}`
- host `PATH` assumptions for those utilities
- exit `0` on emission failure after a successful await

That is the wrong failure mode for an onboarding path: a missing utility or shell quirk must not turn a successful wake into a no-op.

Evidence in tree: `crates/famp/assets/famp-await.sh` logs `jq not found; cannot emit block decision` and exits `0` after await has already returned envelopes.

### Failure class this design does **not** claim to fix

Host never fires the Stop hook at all (no waiter parked). That is the main symptom in `BETA-FEEDBACK-CODEX-AUTO-WAKE.md` (`famp_inspect_waiters` empty). Residual risk remains; this design adds diagnostics so that case is distinguishable, not silent.

### Pre-implementation falsifier (cheap)

For any failed Codex auto-wake report, check `await-hook.log` (or the native equivalent):

| Observation | Interpretation | Right layer |
|---|---|---|
| No “hook invoked” line | Host never ran Stop / trust / project hooks | Host wiring / diagnostics (out of emission scope) |
| Hook invoked, no listen identity | Resolution / compaction / PID fallback | Identity resolution parity |
| Await ran, wake observed, no block JSON | Emission / post-await path | **This design** |
| `jq not found` / Python missing | Emission / tool PATH | **This design** |

Do not ship the full port without at least one real failed-wake log classified this way if a live failure is available. If only the emission class is observed in the field, prefer the phased approach in § Migration (emission-first still allowed as Phase 0).

---

## Goals

1. Codex wake emission must work with a minimal host `PATH` (no `jq`, no `python3`).
2. The final block decision must be serialized from native code in the same binary that owns FAMP.
3. Fail-open remains: on malformed input or uncertainty, exit `0` and do not trap the host.
4. Wake signal stays notification-only: peer body bytes never enter `reason`.
5. **Behavioral parity** with the current shell adapter for all keep-listed behaviors (see § Parity matrix). Dropping a keep-listed behavior is a design change, not an implementation shortcut.
6. Install remains simple: `famp install-codex` (reinstall after binary upgrade when the hook argv or trust hash changes) is a valid deployment step.
7. Shared Rust engine is structured so Claude’s Stop path can switch later without a second architecture.
8. Post-wake failures must be **loud in logs** (and optionally one-shot user-visible) even when exit is fail-open.

---

## Non-Goals

- Changing the broker protocol or envelope format.
- Changing the meaning of `famp await` / bus await semantics.
- Adding a second wake channel for Codex.
- Reintroducing a long blocking shell pipeline as the primary mechanism.
- Claiming to fix hosts that never invoke Stop hooks.
- Replacing Grok’s non-blocking `listen-wake` path with a blocking Stop hook.
- Full Claude cutover in the same milestone (Claude may keep the shell asset until a follow-on).

---

## Proposed End State

### Installed Codex Stop entry (actual schema)

Codex project hooks today use a **single `command` string**. There is no proven `args` array on the command handler. Install must match the live shape used by `install-codex` and `codex_command_hook_hash`:

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/absolute/path/to/famp hook codex-stop",
            "timeout": 86400
          }
        ]
      }
    ]
  }
}
```

Notes:

- `command` is one shell-executable string. Paths with special characters are shell-quoted the same way `install_codex::shell_quote_path` does today; argv after the binary are literal tokens in that string (`hook codex-stop`).
- Absolute path to `famp` is resolved at install time (`which::which("famp")` with `~/.cargo/bin/famp` fallback), same as MCP install.
- Trust hash is seeded over the **exact** `command` string + `timeout` + event metadata via existing `codex_command_hook_hash`. Any change to that string requires reinstall to re-seed trust.
- Do **not** invent a separate `args` field unless Codex documents and we verify support in a real Codex version.

### Runtime entry

```text
famp hook codex-stop
```

Reads Stop-hook JSON from stdin, runs the shared engine, writes at most one block-decision JSON line to stdout, exits `0` on all fail-open paths.

Optional later alias for multi-host:

```text
famp hook stop --host codex
```

v1 may implement only `codex-stop` as a thin wrapper over the shared engine.

### What the helper owns

1. Read and parse Stop-hook stdin JSON.
2. Resolve transcript/rollout path (`transcript_path`, then session-id fallback).
3. Resolve active listen identity (transcript replay, then PID-correlated fallback when enabled).
4. Await on the bus via **in-process** structured await (not `PATH`-dependent `famp await` subprocess).
5. Apply post-await notification shaping (#26 unread, channel vs agent copy, validation).
6. Emit `{"decision":"block","reason":"..."}` with `serde_json` only.
7. Log every branch to the hook log file.

No shell wrapper is required for correctness. A shell shim may remain only as a temporary dual-path during migration (see § Migration).

---

## Architecture

### 1. Shared hook engine (Rust)

New module(s) under `crates/famp/src/cli/hook/` (names illustrative):

| Piece | Responsibility |
|---|---|
| `input` | Parse stdin Stop payload; disconnect/ignore further stdin |
| `transcript` | Port of current Python identity extraction (Claude + Codex JSONL) |
| `rollout` | Codex `session_id` → sqlite / glob rollout path (path allowlisted under sessions root) |
| `pid_fallback` | Compaction / missing-transcript identity recovery (anti-hijack) |
| `listen_state` | Decision table: successful register / set_listen → active identity |
| `await_run` | Call existing structured await (`run_at_structured` or equivalent) |
| `notify` | Count/sender/mailbox kind → reason strings; never include body |
| `emit` | Serialize block decision; sole place that prints host-facing JSON |
| `log` | Append-only hook log (same state dir family as today) |

Codex adapter (`hook codex-stop`) wires host-specific bits only:

- stdin field names Codex actually sends
- whether Claude queue-watch abort applies (default **off** for Codex unless we prove Codex emits `queue-operation` records)
- block JSON shape Codex accepts

Claude can later call the same engine with `host=claude` and enable queue-watch.

### 2. Install-time wiring

`famp install-codex` continues to write:

- `~/.codex/config.toml` MCP entry (absolute `famp` + `args = ["mcp"]`)
- project `.codex/hooks.json` Stop entry
- Codex hook-trust state

**Change:** Stop `command` becomes the native invocation string, not the path to `famp-await.sh`.

Install must:

1. Resolve absolute `famp` path.
2. Build command string: `<quoted-famp-path> hook codex-stop`.
3. Remove prior FAMP Stop handlers matching **both**:
   - old shim path patterns (`.../famp-await.sh`)
   - new native command patterns (`.../famp hook codex-stop`, quoted variants)
4. Seed trust for the new command hash; prune stale FAMP trust entries for that hooks path (existing prune helpers, extended pattern set).
5. Optionally stop writing the project `.codex/hooks/famp-await.sh` once Phase 2 is default (Phase 1 may still install it for dual-path).

### 3. Uninstall-time wiring

`famp uninstall-codex` must remove:

- MCP entry (unchanged)
- native Stop command entries (new patterns)
- legacy shim Stop entries (old patterns)
- shim file if present
- matching trust keys / stale FAMP hashes for both pattern sets

Leaving a native hook after uninstall is a ship blocker.

### 4. Await: in-process only

Call the existing structured await path in-process (`AwaitArgs` / `run_at_structured`), not `Command::new("famp")`.

Reasons:

- no recursive PATH discovery
- same process can log outcomes without parsing CLI stdout
- abort-on-fd remains available if a host adapter arms it

Default timeout remains **23h** (aligned with today’s shell). Codex Stop entry timeout remains **86400** seconds.

### 5. Fail-open semantics (refined)

| Condition | Exit | Stdout block decision | Log |
|---|---|---|---|
| Malformed stdin | 0 | none | yes |
| No transcript and unresolved fallback | 0 | none | yes |
| Transcript parse uncertainty / no confident listen identity | 0 | none | yes |
| Await timeout / empty | 0 | none | yes |
| Await abort (exit 3 semantics) | 0 | none | yes (aborted) |
| #26 agent unread = 0 | 0 | none | yes (suppressed) |
| Wake observed, then notify/emit error | 0 | none **or** one-shot diagnostic block (see below) | **error-level, required** |
| High-confidence listen + wake + valid notify | 0 | block decision | yes |

**High-confidence** is defined only by the decision table in § Listen-state decision table — not by implementer gut feel.

**Post-wake loudness:** if envelopes were returned (or unread > 0 was confirmed) and the helper still cannot emit a valid block decision, it must:

1. log a distinct error line including identity, mailbox kind, and failure reason
2. preferably emit a **notification-only diagnostic** block once, e.g. reason  
   `[FAMP listen mode] wake received but notification could not be built; call famp_inbox`  
   (still no peer body). If the host schema forbids unknown reasons, log-only is acceptable but must be tested and documented.

Never exit non-zero to “force” attention if that traps Codex in a broken Stop state.

---

## Runtime Flow

1. Codex ends a turn and fires the Stop hook (host responsibility).
2. Codex runs the installed command string (`/abs/famp hook codex-stop`).
3. Helper reads stdin JSON; closes/ignores further stdin (equivalent of shell `exec 0</dev/null`).
4. Resolve transcript: `transcript_path` if file exists; else session-id rollout resolution.
5. Resolve identity: transcript listen-state replay; else PID-correlated fallback unless `FAMP_DISABLE_PID_FALLBACK=1`.
6. If no confident listen identity → log, exit 0.
7. Validate identity (`^[A-Za-z0-9._-]{1,64}$`, no newlines).
8. In-process await `--as <identity>` for 23h (optional abort-fd only if host adapter armed it).
9. On abort / timeout / empty → log, exit 0.
10. Shape notification (#26 for agent mailboxes; channel keeps batch count).
11. Validate sender; build reason string (channel vs agent templates).
12. Emit block JSON via `serde_json`; log byte length; exit 0.

---

## Listen-state decision table

Port of current shell/Python semantics. Successful tool results only.

Replay successful control actions in transcript order:

| Event | Success? | Effect on `active` / `last_identity` |
|---|---|---|
| `famp_register` with identity I, `listen` absent or not JSON false | yes | `last_identity = I`, `active = I` |
| `famp_register` with identity I, `listen: false` | yes | `last_identity = I`, `active = ""` |
| `famp_set_listen` with `listen: false` | yes | `active = ""` |
| `famp_set_listen` with `listen: true` (or absent-as-true only if current code treats it that way — **match shell exactly**) | yes | `active = last_identity` if `last_identity` non-empty |
| Any of the above with failed tool result | — | ignore |

Transcript scan: last **2 MB**, discard partial first line after seek (parity with shell).

Formats to accept (parity):

- Claude: `message.content[]` `tool_use` / `tool_result`
- Codex: `payload.type` `function_call` / `function_call_output` (namespace `mcp__famp` when present)
- Codex: `payload.type` `mcp_tool_call_end` with `invocation` / `result.Ok`

Tool name match: `name.endswith("famp_register")` / `famp_set_listen` (suffix match, as today).

### PID-correlated fallback (keep)

When transcript yields no active identity:

1. Walk ancestor PIDs of the hook process (depth ≤ 6; skip 0/1).
2. Find `famp mcp` children whose parent is an ancestor.
3. Map unique name via `famp sessions` (or in-process equivalent).
4. Adopt only if `inspect identities` shows `listen_mode == true` for that name.
5. Never adopt by cwd alone (anti-hijack).
6. Honor `FAMP_DISABLE_PID_FALLBACK=1` for hermetic tests.

### Codex rollout fallback (keep)

When `transcript_path` missing or not a file, and `session_id` present:

1. Read-only open of `state_5.sqlite` under `CODEX_SQLITE_HOME` / `CODEX_HOME`.
2. `select rollout_path from threads where id = ?`.
3. Allow only paths under realpath(`$CODEX_HOME/sessions`).
4. Else glob `sessions/**/rollout-*<session_id>.jsonl`, newest mtime wins.

---

## Parity matrix (shell → native)

Source of truth today: `crates/famp/assets/famp-await.sh` + install/uninstall tests.

| # | Behavior | Decision | Notes |
|---|---|---|---|
| P01 | Fail-open exit 0 always for host trap avoidance | **Keep** | |
| P02 | Stdin JSON → transcript_path / session_id | **Keep** | Native parse |
| P03 | Disconnect stdin after read | **Keep** | |
| P04 | Hook log under state dir / `FAMP_HOOK_LOG` | **Keep** | |
| P05 | Codex session_id → rollout (sqlite + glob) | **Keep** | |
| P06 | 2 MB transcript tail scan | **Keep** | |
| P07 | Claude + Codex multi-format tool parse | **Keep** | |
| P08 | Success-only action replay; listen default on | **Keep** | |
| P09 | `set_listen(false)` clears active | **Keep** | |
| P10 | PID-correlated fallback + anti-hijack | **Keep** | Compaction resilience |
| P11 | `FAMP_DISABLE_PID_FALLBACK` | **Keep** | |
| P12 | Identity regex + newline reject | **Keep** | |
| P13 | In-process / same-binary await 23h | **Keep** (form changes) | Not shell-out to PATH `famp` |
| P14 | Issue #21 queue-watch + abort-on-fd | **Defer for Codex** | Enable only if Codex emits `queue-operation`; keep in engine for Claude |
| P15 | Await exit 3 → no block, exit 0 | **Keep** when abort armed | |
| P16 | 64KB envelope cap / UTF-8 sanitize before meta | **Keep** if meta still parses await output; prefer structured fields from `AwaitOutcome` and drop string hacks when possible |
| P17 | Envelope backup under state `received/` | **Keep** | Optional but default on for parity |
| P18 | Wrapper JSON envelopes + legacy line fallback | **Keep** | Prefer structured await outcome first |
| P19 | #26 agent `mailbox_unread` / last_sender | **Keep** | Channel uses batch count |
| P20 | Unread 0 suppresses wake | **Keep** | |
| P21 | Sender validation | **Keep** | |
| P22 | Channel vs agent reason templates | **Keep** | Notification-only |
| P23 | Final block JSON via `jq` | **Replace** | `serde_json` only |
| P24 | Python for extraction / meta | **Replace** | Rust ports |
| P25 | Shell as critical path | **Remove** after Phase 2 | |

Any **Keep** row omitted from implementation is a regression.

---

## Why this is stronger (amended)

Removes:

- missing `jq` after successful await
- shell quoting / PATH-dependent emission
- Python availability for final reason construction
- partial success: waited but never woke **because of emission tooling**

Does **not** remove:

- host not configured / not trusting / not firing Stop
- project hooks not installed in the project actually running
- model ignoring inbox after a correct block reason

Those need diagnostics and install verification, not only a native helper.

---

## Alignment with host-wake architecture

Per `docs/HOST-WAKE-ADAPTERS.md`:

- **Core:** bus await + scrubbed notification (no peer body)
- **Claude / Codex:** blocking Stop → block decision
- **Grok:** non-blocking `listen-wake` + monitor

This design is a **Codex (and later Claude) adapter** over a shared engine. It must not invent a third wake stack parallel to `listen-wake`. Shared code should live as library routines usable by:

- `famp hook codex-stop` / future `hook stop --host claude`
- tests
- optionally diagnostics

Grok remains on `listen-wake`; no blocking Stop for Grok.

---

## Migration Plan

### Phase 0 (optional, fast risk reduction)

If field logs show emission-only failures and full port is delayed:

- add `famp hook emit-block --reason <text>` (or stdin reason) that only serializes block JSON
- point the last step of the shell at that binary with absolute path

This is **not** the end state; it unblocks `jq` death without claiming full native lifecycle.

### Phase 1: Native helper with full parity

- Implement shared engine + `famp hook codex-stop`.
- Port every **Keep** row in the parity matrix (P14 deferred for Codex unless proven needed).
- Unit + integration tests listed below must pass with `PATH` stripped of `jq` and `python3`.
- Do not switch install default yet; allow `FAMP_CODEX_HOOK=native` or a hidden flag for dogfood.

### Phase 2: Switch install-codex default

- Install native command string + trust hash.
- Remove dual default: Stop entry points only at native helper.
- Uninstall/install roundtrip tests cover native patterns.
- Keep shell asset in tree for Claude until Claude cutover; Codex install may stop writing project shim.

### Phase 3: Codex critical path is native-only

- Codex runtime path has no dependency on `famp-await.sh`, `jq`, or `python3`.
- Docs (`HOST-WAKE-ADAPTERS`, onboarding) describe native Codex hook.
- Claude cutover is a separate milestone reusing the engine.

### Compatibility during Phase 1–2

- Prefer a single installed Stop entry (native **or** shell), never both blocking awaits on the same identity.
- If a fallback is needed, it is **install-time** selection, not runtime “try shell then native.”

---

## Install / upgrade / uninstall rules

1. **Absolute binary path** at install; prefer stable `~/.cargo/bin/famp` when that is the real install location.
2. Warn if resolved binary is under a `target/debug` or clearly ephemeral path.
3. Reinstall after changing hook argv (required for trust hash).
4. `cargo install famp` overwriting the same path does not require reinstall **unless** hooks still point at a deleted path or still point at the shell shim after Phase 2.
5. Idempotent install: second run leaves one Stop FAMP entry and matching trust.
6. Uninstall removes native + legacy patterns and trust.

---

## Testing Plan

### Unit tests

- Decision table cases: register listen true/false/default; set_listen true/false; failed tool results ignored
- Malformed / truncated transcript → fail-open
- 2 MB boundary: register only in dropped head → no transcript identity (PID path separate)
- Codex rollout allowlist rejects paths outside sessions root
- Session-id fallback selects newest matching rollout when sqlite misses
- Identity / sender validation rejects injection shapes
- Reason templates: agent singular/plural; channel singular/plural with `#` prefix normalization
- Emit JSON is exactly `{decision: "block", reason}` with no extra body fields
- Peer body bytes never appear in reason (fixture with hostile envelope text)

### Integration tests

- Hook with `PATH=/usr/bin:/bin` (or empty of `jq`/`python3`) still emits block on synthetic wake
- Timeout / empty → exit 0, empty stdout
- #26: unread 0 suppresses; unread rewrite changes count in reason
- PID fallback: unique sibling mcp + listen true → await; cwd-only candidate → no-op
- `FAMP_DISABLE_PID_FALLBACK=1` disables fallback
- Install writes native command string; trust hash matches `codex_command_hook_hash`
- Reinstall migrates shim → native without duplicate Stop handlers
- Uninstall removes native entry + trust; second uninstall is clean

### End-to-end (manual / optional CI)

- `famp install-codex` in a real project
- register listen true in Codex → Stop parks waiter (`inspect waiters` non-empty while blocked, or log shows await)
- peer message → block reason → model can `famp_inbox`
- `famp_set_listen(false)` → subsequent Stop no-ops

### Explicit non-coverage

Tests that only strip `jq` and assert emission **do not** satisfy parity. CI must include keep-listed behaviors above.

---

## Acceptance Criteria

Design/implementation complete only when all are true:

1. Codex wake **emission** has no dependency on `jq` or `python3`.
2. Final wake JSON is emitted only from native code.
3. Install Stop entry uses a single `command` string (no unverified `args` schema).
4. Trust hash is seeded for that exact command; reinstall updates/prunes correctly.
5. Uninstall removes native and legacy FAMP Stop entries.
6. Parity matrix **Keep** rows are implemented or explicitly deferred with rationale (only P14 deferred by default).
7. Minimal `PATH` integration test proves wake emission still works.
8. Post-wake emission failure is loud in logs (and diagnostic block if enabled).
9. Docs state residual risk: host non-invocation is out of scope but classifiable via hook logs.
10. No dual concurrent awaits installed for the same Codex project Stop list.

---

## Residual Risk

| Risk | Mitigation |
|---|---|
| Host never fires Stop | Log-based falsifier; install-codex doctor messaging; out of emission scope |
| Trust hash mismatch after upgrade | Reinstall; hash tests; prune stale FAMP trust |
| Absolute path points at ephemeral binary | Install warning; prefer cargo bin |
| Compaction removes transcript markers | PID fallback kept (P10) |
| Silent post-wake failure | Required error log + optional diagnostic block |
| Dual await if both shell and native installed | Install surgically replaces FAMP handlers; forbid dual default |
| Claude still on shell / `jq` | Accepted until Claude cutover; not claimed fixed |
| Codex Stop schema changes in future Codex | Pin tested shape; integration test against observed hash algorithm |

---

## Implementation sketch (non-normative)

```text
crates/famp/src/cli/hook/
  mod.rs              // clap: hook codex-stop
  engine.rs           // orchestration
  stdin.rs
  transcript.rs       // port of PYEOF identity extract
  rollout.rs          // sqlite + glob
  pid_fallback.rs
  notify.rs           // #26 + reason templates
  emit.rs             // serde_json block decision
```

Wire into `cli/mod.rs` Commands. Reuse `await_cmd::run_at_structured`, inspect/sessions helpers already used by the shell via CLI (prefer library calls over spawning self).

---

## Summary

Ship a **native Codex Stop adapter** on a **shared Rust hook engine**, with:

- real Codex `hooks.json` + trust semantics
- full keep-list parity with `famp-await.sh` (except Codex queue-watch deferral)
- in-process await and `serde_json` emission
- honest residual risk for host non-invocation
- install/uninstall/migration that cannot leave dual waiters or orphan trust

The original “smallest reliable control surface” goal stands; this amendment makes that surface large enough to match reality without reintroducing shell as the correctness path.
