---
name: famp-listen
description: >
  Non-blocking FAMP listen-mode for Grok. After famp_register with listen
  true (the default), MCP arms a supervised listen-wake daemon; start a
  persistent monitor on `famp listen-wake --follow` so inbound messages
  inject an agent turn without a long blocking Stop hook.
metadata:
  short-description: "FAMP non-blocking auto-wake via listen-wake --follow"
---

# FAMP listen mode (Grok)

Grok must **not** use Claude/Codex-style long blocking Stop-hook awaits.
Auto-wake is split into two roles:

1. **Arm (mechanical):** `famp_register` with listen true spawns a
   supervised `famp listen-wake --as <identity> --loop` daemon (pidfile
   singleton under `~/.famp/`). It appends each wake to
   `~/.famp/listen-wake-<identity>.wake`.
2. **Inject (monitor):** start a **persistent** Grok `monitor` that
   follows the wake file — no second bus awaiter:

```
famp listen-wake --as <identity> --follow
```

## After successful `famp_register` with listen true (default)

Immediately start a **persistent** Grok `monitor` tool:

- `command`: `famp listen-wake --as <identity> --follow`
  (use the full path to `famp` if it is not on PATH)
- `description`: `FAMP listen`
- `persistent`: `true`

`<identity>` is the name just registered (the `active` field from
`famp_register`).

`famp_register` with listen true also returns a `wake_hint.grok_monitor`
string you can pass through as the monitor command.

Do **not** start a second `famp listen-wake --loop` — the pidfile
singleton will refuse (`ALREADY_RUNNING`). Use `--follow` only. Use
`--force` only when intentionally replacing a wedged waiter.

## On each monitor notification

When monitor output starts with `FAMP_WAKE`:

1. Call `famp_inbox` for DMs (or channel tools for channel wakes).
2. Do **not** dump peer message body from the wake line — there is none.
   The line is only `FAMP_WAKE identity=… sender=… count=…`.

## Turning listen off

On `famp_set_listen(false)` or a register with `listen: false`:

- MCP stops the supervised listen-wake daemon.
- Kill the FAMP listen `--follow` monitor.
- Do not re-arm until listen is true again.

## Re-arm

- MCP re-arms the daemon on register(listen=true) / set_listen(true).
- If the `--follow` monitor dies and listen is still true, restart it
  with the same `--follow` command (not a second `--loop`).

## Never do this on Grok

- Do not install or rely on a 23h blocking Stop-hook await.
- Do not block the UI turn on `famp await` / `famp_await` for listen mode.
- Do not run a second `listen-wake --loop` alongside the MCP-armed daemon.
