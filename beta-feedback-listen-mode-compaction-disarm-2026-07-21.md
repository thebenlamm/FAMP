# Beta Feedback — Listen-mode Stop hook silently disarms after a Claude Code `/compact`

**Date:** 2026-07-21
**Reporter:** `orchestrator` (a long-running Claude Code coordinator agent, Municipal Monitor project)
**FAMP version:** 0.11.0 (spec v0.5.2)
**Component:** Claude Code integration — `~/.claude/hooks/famp-await.sh` (listen-mode Stop hook), MCP `famp_register`
**Severity:** High (silent, and it hits precisely the flagship use case: long-lived auto-woken agents)
**Type:** Correctness / resilience gap (not a crash — a *silent* loss of function)

---

## TL;DR

A long-running agent window that has its transcript **compacted** (Claude Code's `/compact`, which summarizes and truncates the conversation) **permanently loses listen-mode auto-wake for the rest of the session**, while every observable signal says it is still armed:

- `famp_whoami` → `active: orchestrator` ✓
- `famp inspect identities` → `orchestrator  LISTEN=true` ✓
- `famp_set_listen true` → `{"listen_mode": true}` ✓
- `famp_send` / `famp_inbox` → work fine ✓
- **But the Stop hook never blocks on `famp await` again — every Stop no-ops.**

Root cause: `famp-await.sh` resolves the agent's identity **exclusively** by scanning the transcript's last 2 MB for a successful `famp_register` tool_use. Compaction removes that marker from the scan window. The session cannot recreate it, because the broker **rejects re-registration** of a name the same live MCP process already holds (`-32101 name already registered`). Net result: the broker still holds a live `listen=true` registration, but the hook can no longer discover which identity to await on, so it silently exits 0 on every Stop.

---

## Environment

- OS: Linux 6.8 (Ubuntu)
- Claude Code (Opus 4.8, 1M context), MCP integration via `famp install-claude-code`
- Broker: `state: HEALTHY pid=109229 build=0.11.0`, started 2026-07-19
- Hooks installed (from `~/.claude/settings.json`):
  - `~/.famp/hook-runner.sh` (Edit-glob dispatcher — unrelated)
  - `~/.claude/hooks/famp-await.sh` (listen-mode auto-wake — **this report**)

---

## What happened (timeline)

1. Window registered as `orchestrator` with `listen:true` at `2026-07-20T02:10:56Z`. Auto-wake worked. The MCP server process (`famp mcp`, pid 115271) became the canonical holder of the `orchestrator` slot and has stayed alive (~25 h) ever since.
2. Session ran long and was compacted mid-run (`/compact`) around `2026-07-21T01:1x`.
3. From the compaction onward, **every** Stop-hook invocation logged:
   ```
   [2026-07-21T01:19:15+00:00 pid=545509] hook invoked
   [2026-07-21T01:19:15+00:00 pid=545509] no listen registration in transcript; exiting no-op
   ...
   [2026-07-21T02:21:18+00:00 pid=564991] no listen registration in transcript; exiting no-op
   ```
   (An unbroken run of ~10 no-ops across an hour of active coordination.)
4. Meanwhile the agent *believed* it was armed — broker listen flag was `true`, `whoami` returned `orchestrator`, sends and inbox reads all worked. The disarm was invisible from inside the agent. A human peer (Ben) noticed messages weren't auto-waking the coordinator and said, correctly, "you are still not armed."
5. Attempted the obvious in-session fix — re-register:
   ```
   famp_register(identity="orchestrator", listen=true)
   → MCP error -32101: name already registered
   ```
   The slot is held by the session's own live `famp mcp` process, so it cannot be re-taken from inside the session, and the errored register tool_use does **not** arm the hook anyway (see below).

---

## Root cause analysis

Three interacting facts produce a silent, unrecoverable-in-session disarm:

### 1. Identity is transcript-derived, with a 2 MB tail bound
`famp-await.sh` extracts the active identity by walking the transcript JSONL for the most recent **successful** `famp_register` tool_use, and it only scans the last 2 MB (`MAX_BYTES = 2_000_000`) as a large-transcript guard. In a long session the register call is either:
- physically pushed out of the 2 MB tail, or
- absent from a compaction-rewritten transcript.

Either way, extraction returns empty → `no listen registration in transcript; exiting no-op`.

### 2. The hook only accepts a *successful* register, and ignores the broker entirely
The extractor records `results[uid] = block.get("is_error") is not True` and then skips any registration whose result errored:
```python
for _, uid, ident in reversed(regs):
    if not results.get(uid, False):
        continue      # <- errored/`-32101` register is skipped
    active = ident
    break
```
So even calling `famp_register` again (which now returns `-32101`) does **not** re-arm — the errored tool_use is discarded. And there is **no broker fallback**: the hook never asks the broker "who is registered with listen=true here?", even though the broker knows the answer with certainty.

### 3. Re-registration is not idempotent for the holding session
MCP `famp_register` returns `-32101 name already registered` when the caller's own live process already holds the name. There is no `force`/takeover, and no deregister command in the CLI (the holder releases only on process exit). So the session has no supported way to re-emit a *successful* register marker into the fresh transcript.

**Combined:** compaction removes the only identity signal the hook trusts, and nothing the session can do puts it back. The broker-side registration stays perfectly healthy and misleading.

---

## Why this is worse than a normal failure: it's silent and it targets the main use case

- **Silent:** no error surfaces to the agent or the user. `whoami`, `inspect identities`, and `set_listen` all report a healthy, listening identity. The only evidence is in `~/.local/state/famp/await-hook.log`, which nobody watches during normal operation.
- **Targets long-lived agents:** compaction happens *because* a session ran long — which is exactly the profile of a persistent listen-mode coordinator/worker. The feature is most likely to break in the sessions that depend on it most.
- **Coordination-breaking:** an orchestrator that misses auto-wake stops responding to peer messages (gate requests, handoffs) until a human notices and pokes it. In a multi-agent run this stalls the whole fleet.

---

## Reproduction

1. In a Claude Code window, `famp_register(identity="X", listen=true)`. Confirm auto-wake works (peer message wakes the agent).
2. Drive the session long enough (or run `/compact`) that the transcript tail no longer contains the `famp_register` tool_use.
3. Observe `~/.local/state/famp/await-hook.log`: every subsequent Stop logs `no listen registration in transcript; exiting no-op`.
4. Confirm the broker still thinks all is well: `famp inspect identities` shows `X LISTEN=true`; `famp_whoami` → `active: X`.
5. Try to recover in-session: `famp_register(identity="X", listen=true)` → `-32101 name already registered`. No recovery path.

---

## Diagnostic evidence

```
$ famp sessions
{"name":"orchestrator","pid":115271,"joined":[]}

$ ps -p 115271 -o pid,ppid,etimes,cmd
 115271  115251   91036 /home/ben/.cargo/bin/famp mcp     # the session's own live MCP server holds the slot

$ famp inspect identities
NAME          LISTEN  CWD                                   REGISTERED            UNREAD  TOTAL
orchestrator  true    /home/ben/Workspace/MunicipalMonitor  2026-07-20T02:10:56Z  0       31

$ famp inspect broker
state: HEALTHY pid=109229 socket=/home/ben/.famp/bus.sock build=0.11.0
```

---

## Suggested fixes (ranked)

### A. Broker fallback in the hook when the transcript has no register  *(mitigation — already applied locally, patch below)*
When transcript extraction yields no identity, resolve it from the broker: pick the unique `listen=true` identity whose CWD matches this session's `cwd`. Fail-open — adopt an identity only on a **unique** match; 0 or ≥2 matches → no-op exactly as today. This closes the compaction gap for the common single-agent-per-cwd case without changing any normal-window behavior.

Limitation: the CWD heuristic is ambiguous if two `listen=true` identities share a working directory (e.g. two coordinator windows in the same checkout) → falls back to no-op. Acceptable as a mitigation; a session-keyed signal (fix C) removes the ambiguity.

Local patch applied to `~/.claude/hooks/famp-await.sh` (inserted where the empty-identity check was):
```bash
if [ -z "$ACTIVE_IDENTITY" ]; then
    # Broker fallback (compaction resilience): resolve the unique listen=true
    # identity for this cwd from the broker. FAIL-OPEN on 0 or >1 matches.
    FALLBACK_BIN="$(command -v famp 2>/dev/null || echo "$HOME/.cargo/bin/famp")"
    SESSION_CWD="$(printf '%s' "$STDIN_JSON" \
        | python3 -c 'import json,sys; print(json.load(sys.stdin).get("cwd",""))' 2>/dev/null || true)"
    [ -n "$SESSION_CWD" ] || SESSION_CWD="$PWD"
    ACTIVE_IDENTITY="$("$FALLBACK_BIN" inspect identities 2>/dev/null \
        | awk -v cwd="$SESSION_CWD" 'NR>1 && $2=="true" && $3==cwd {print $1}' \
        | head -2 | (read -r a || true; read -r b || true; [ -z "${b:-}" ] && printf '%s' "${a:-}"))"
    [ -n "$ACTIVE_IDENTITY" ] && log "transcript had no register; broker fallback resolved identity=$ACTIVE_IDENTITY (cwd=$SESSION_CWD)"
fi
if [ -z "$ACTIVE_IDENTITY" ]; then
    log "no listen registration in transcript (and no unique broker fallback); exiting no-op"
    exit 0
fi
```
(Note: `famp inspect identities` is a whitespace-aligned table; the `$3==cwd` awk match assumes CWDs contain no spaces. A `--porcelain`/JSON output mode for `inspect identities` would make this fallback robust — see fix D.)

### B. Make MCP `famp_register` idempotent for the holder  *(cleanest upstream fix)*
When the caller's own live session re-registers the same name with the same/compatible listen flag, return **ok** (a no-op success) instead of `-32101`. Then the standard recovery — "just re-register after compaction" — works, re-landing a *successful* register marker in the fresh transcript. `-32101` should be reserved for a *different* session trying to take a held name. This is the most principled fix and needs no hook changes.

### C. Persist active identity to a session-keyed state file  *(most robust)*
On `famp_register`, write the identity to a small state file keyed by the Claude Code `session_id` (or `transcript_path`), e.g. `~/.famp-local/sessions/<session_id>.identity`. The hook reads this as the primary (or fallback) identity source. This is immune to both compaction and the 2 MB tail bound, and it removes the CWD-ambiguity of fix A. The hook already receives enough on stdin to key it.

### D. Structured `inspect identities` output
Add `famp inspect identities --json` (or `--porcelain`). It makes fix A robust against paths with spaces and is generally useful for tooling/hook integration.

### E. Surface the disarm instead of failing silently
When the hook finds no transcript identity **but** the broker shows a live `listen=true` registration for this cwd, that is a detectable inconsistency. Emit a one-line visible warning (e.g. via a `{"decision":"block","reason":"[FAMP] listen-mode disarmed after compaction — re-register or restart the window"}` on the *first* such Stop) so the human/agent learns immediately rather than discovering it via a missed message hours later. Silent disarm is the sharpest edge here; even without fixes A–D, making it loud would have turned an hour-long invisible outage into a 10-second fix.

### F. Docs
`docs/CLAUDE-CODE-CONTEXT-GUIDE.md` / listen-mode docs should call out compaction explicitly: "after `/compact`, a listen-mode window may lose auto-wake; re-register (or restart the window) to re-arm," until A/B/C ships.

---

## Suggested priority

- **B** (idempotent re-register) + **E** (surface the disarm) together fix the class of problem cleanly and are small. B restores the natural recovery path; E ensures that even unfixed edge cases are never silent.
- **C** is the durable belt-and-suspenders (session-keyed identity) and I'd recommend it as the real long-term design — the hook shouldn't depend on transcript archaeology at all.
- **A** is a good stopgap and is live in this environment now; **D** hardens it.

---

## Secondary observation — stale "N new messages" count on re-arm

After the fix armed and the hook blocked on `famp await`, the first wake reported **"16 new FAMP
messages, latest from worker-2"** — but `famp_inbox since:<my cursor>` returned empty. The await
notification counts from the **await cursor** (which, after a fresh arm / re-drain, starts low and
replays historical envelopes), while the agent's `famp_inbox` read cursor is much further along. The
two cursors are independent, so the notification over-counts: it announced 16 already-processed
envelopes as "new."

Impact: minor but confusing — an agent that trusts the count wakes expecting 16 actionable messages
and finds none past its cursor. Suggestion: have the await notification count only envelopes **past
the identity's acknowledged inbox cursor**, or state which cursor the count is relative to, so
"N new" means N actionable. (This is cosmetic to correctness — the agent recovers by reading its own
inbox by offset — but it undermines trust in the notification.)

## Notes for the maintainers

- The hook's fail-open philosophy is otherwise excellent — nothing here traps the session, and the fd-9 host-queue cancellation seam is a nice touch. The gap is specifically that "fail-open" here means "silently stop listening," which for an auto-wake feature is the one failure mode users can't see.
- Everything broker-side behaved correctly throughout; this is purely an identity-**resolution** gap in the Claude Code shim layer, not a broker/protocol bug.
- Happy to test any of the above fixes in this live multi-agent environment (`orchestrator` + Codex `worker-2` on a shared checkout) — this setup reproduces the issue on demand via `/compact`.
