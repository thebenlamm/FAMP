# Host wake adapters

FAMP auto-wake has a **host-neutral core** and thin **per-host adapters**.

## Core: `famp listen-wake`

```bash
famp listen-wake --as <identity> [--timeout 23h] [--loop] [--force] [--daemon] [--follow]
```

Parks on the same bus await as Claude/Codex/Grok Stop hooks, off the agent
turn. On each inbound batch it prints **one scrubbed stdout line** and
appends the same line to `~/.famp/listen-wake-<identity>.wake`:

```
FAMP_WAKE identity=<id> sender=<sender|unknown> count=<n>
```

- Never prints peer message body (there is none on this line).
- Exit codes: `0` message (once), `2` timeout (once; stderr `TIMEOUT`), `3`
  aborted, `1` error / already running.
- `--loop`: re-await after each wake; timeouts re-await silently; bus errors
  retry with exponential backoff (1s → 60s, cap 100 consecutive failures).
- **Pidfile singleton** (`~/.famp/listen-wake-<id>.pid`): refuse start if a
  live listen-wake holds the lock (`ALREADY_RUNNING pid=<n>`). Dead pid →
  take over. `--force` kills the old waiter then takes the lock.
- `--daemon`: spawn a detached `--loop` waiter (log + wake file + pidfile)
  and exit.
- `--follow`: tail the `.wake` file only — **no second bus await**. Use when
  something else already armed the singleton.

Any host that can inject a turn from an external process can implement
auto-wake by reacting to `FAMP_WAKE` lines (stdout or the `.wake` file).

## Adapters

| Host | Install | Wake mechanism | Blocking UI? |
|---|---|---|---|
| **Claude Code** | `famp install-claude-code` | Stop hook → `famp-await.sh` → `decision: block` | Yes (by design) |
| **Codex** | `famp install-codex` | Project Stop hook → same `famp-await.sh` | Yes (by design) |
| **Grok** | `famp install-grok` | Stop hook → `famp-await.sh` → `decision: block` (same as Claude) | Yes (by design) |

### Claude / Codex / Grok (blocking Stop)

1. Agent registers with `listen: true` (MCP default). User says "register
   with famp" → `famp_register` only (no monitor memory required).
2. Host Stop fires `famp-await.sh` (timeout 86400).
3. Hook parks on `famp await --as <id> --timeout 23h`.
4. On message, emits `{"decision":"block","reason":"..."}` so the agent
   calls `famp_inbox` (or channel tools). Peer bytes never enter `reason`.

When the host omits `transcript_path` / `transcriptPath`, the hook still
tries the PID-correlated fallback before no-op'ing (fail-open exit 0).

**Grok specifics:**

- `famp install-grok` writes **only** under `~/.grok/`:
  `[mcp_servers.famp]` (absolute `famp` path), `hooks/famp-await.sh`,
  `hooks/famp-listen-stop.json` (timeout 86400), and the `famp-listen`
  skill. It does **not** touch `~/.claude/` (single Stop arming path).
- Grok stdin is camelCase (`sessionId`, `transcriptPath`); the await shim
  accepts both snake_case and camelCase.
- Grok also fires Stop at session end (`reason: channel_closed` /
  `shutdown`); the shim exits 0 without parking on those observe fires.
- **Host limit:** Grok caps Stop continuations at **8 per turn**. After
  that the turn ends; the next user prompt re-arms the Stop hook. Not
  “infinite foolproof” across a long agent↔agent loop without a human
  re-prompt.
- **Dual-host:** if you also ran `install-claude-code`, Grok’s Claude-compat
  hook scan may load Claude’s Stop entry too. The await shim
  **singleton-locks** per identity so only one await parks. To load only
  native Grok hooks: `[compat.claude] hooks = false` in `~/.grok/config.toml`.
- Optional fallback: `famp listen-wake --as <id> --follow` if Stop is
  unavailable. Prefer Stop.
- **Verification status:** Stop `decision:block` re-entry is the documented
  Grok contract (host docs). Live capture of a full Grok session re-prompt
  should be re-run after each major Grok Build upgrade.

### Residual / optional

`famp listen-wake` remains available as a host-neutral inject primitive for
future hosts (or as a Grok fallback). MCP register never arms a `listen-wake`
supervisor — Stop is the foolproof path and a second bus waiter would double
with it.

## Future hosts

Implement inject-only: run `famp listen-wake` (or `--follow` on the wake
file) and inject a turn on `FAMP_WAKE`. Prefer Stop `decision:block` when
the host supports long Stop parks (Claude/Codex/Grok). Reuse the scrubbed
line format; never forward peer body from the wake path.
