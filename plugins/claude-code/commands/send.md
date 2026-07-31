---
description: Send a direct message to a FAMP agent. Use when the user asks to message, ping, or DM a named peer.
allowed-tools: mcp__plugin_famp_famp__famp_send
argument-hint: <recipient> <body...>
---

Use the `mcp__plugin_famp_famp__famp_send` tool. The first argument `$1` is the recipient; the rest of `$ARGUMENTS` is the message body.

Construct the call as:
- `peer`: `$1`
- `mode`: `"open"`
- `title` or `body`: the message text (everything after the recipient)

If the recipient starts with `#`, redirect the user to `/famp-channel`.
