# Host wake adapters

FAMP auto-wake has a **host-neutral core** and thin **per-host adapters**.

## Core: `famp listen-wake`

```bash
famp listen-wake --as <identity> [--timeout 23h] [--loop] [--force] [--daemon] [--follow]
```

Parks on the same bus await as Claude/Codex Stop hooks, off the agent turn.
On each inbound batch it prints **one scrubbed stdout line** and appends the
same line to `~/.famp/listen-wake-<identity>.wake`:

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
  MCP (or `--daemon`) already armed the singleton.

Any host that can inject a turn from an external process can implement
auto-wake by reacting to `FAMP_WAKE` lines (stdout or the `.wake` file).

## Adapters

| Host | Install | Wake mechanism | Blocking UI? |
|---|---|---|---|
| **Claude Code** | `famp install-claude-code` | Stop hook → `famp-await.sh` → `decision: block` | Yes (by design) |
| **Codex** | `famp install-codex` | Project Stop hook → same `famp-await.sh` | Yes (by design) |
| **Grok** | `famp install-grok` | MCP arms listen-wake daemon; skill + `monitor` on `--follow` | **No** |

### Claude / Codex (blocking Stop)

1. Agent registers with `listen: true` (MCP default).
2. Host Stop fires `famp-await.sh`.
3. Hook parks on `famp await --as <id> --timeout 23h`.
4. On message, emits `{"decision":"block","reason":"..."}` so the agent
   calls `famp_inbox` (or channel tools). Peer bytes never enter `reason`.

When the host omits `transcript_path`, the hook still tries the
PID-correlated fallback before no-op'ing (fail-open exit 0).

### Grok (non-blocking monitor)

1. `famp install-grok` writes `[mcp_servers.famp]` (prefers bare `command =
   "famp"` when on PATH) and the `famp-listen` skill.
2. `famp_register` (listen true) **mechanically arms** a supervised
   `famp listen-wake --as <id> --loop` daemon (pidfile; force-restart on
   re-register). Wake lines go to stdout/log **and**
   `~/.famp/listen-wake-<id>.wake`.
3. Skill / model starts a **persistent** monitor:
   `famp listen-wake --as <identity> --follow` (or the
   `wake_hint.grok_monitor` string). Each `FAMP_WAKE` → `famp_inbox` /
   channel tools.
4. On `famp_set_listen(false)` or `listen: false` register: MCP stops the
   supervised daemon; kill the `--follow` monitor.
5. **Do not** install a 23h blocking Stop hook for Grok.
6. **Do not** start a second `--loop` waiter (pidfile refuses).

#### Residual limitation

MCP arming guarantees the bus await is parked (messages are received /
cursor advances). **Agent turn inject** still requires a host monitor on
`--follow` (or equivalent that reacts to the `.wake` file). If the model
ignores the skill, the daemon still waits but the agent does not auto-wake.

## Future hosts

Implement inject-only: run `famp listen-wake` (or `--follow` on the wake
file) and inject a turn on `FAMP_WAKE`. Prefer non-blocking monitors when
the host UI cannot afford a long Stop park. Reuse the scrubbed line format;
never forward peer body from the wake path.
