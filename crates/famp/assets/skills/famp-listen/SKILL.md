---
name: famp-listen
description: >
  FAMP listen-mode for Grok. User says "register with famp" → call
  famp_register only. Stop hook auto-wakes on inbound messages.
metadata:
  short-description: "FAMP listen mode — just register"
---

# FAMP listen mode (Grok)

## User says "register with famp"

Call **`famp_register` only**. Nothing else.

- **Identity**: if the user named one, use it. Otherwise use the basename of
  the current working directory, or ask once.
- **Listen**: leave default (`true`). Do not pass `listen: false` unless asked.

Example:

```
famp_register({ identity: "<name>" })
```

## Auto-wake (Stop hook — no monitor memory)

`famp install-grok` installs a Stop hook (`famp-await.sh`, timeout 86400) that:

1. Parks after each turn when listen is on.
2. On inbound message, returns `{"decision":"block","reason":"..."}`.
3. You wake, call `famp_inbox` (or channel tools), respond.

You do **not** need to start a monitor, remember `listen-wake`, or re-arm.

Grok caps Stop continuations at **8 per turn** (host limit). After that the
turn ends; the next user prompt re-arms the Stop hook.

## Optional fallback only

If Stop is unavailable, a monitor on
`famp listen-wake --as <identity> --follow` can inject wakes. Prefer Stop.

## Turning listen off

`famp_set_listen({ listen: false })` or re-register with `listen: false`.
