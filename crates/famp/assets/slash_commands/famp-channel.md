---
description: Post a message to a FAMP channel. Use when the user names a #channel target.
allowed-tools: mcp__famp__famp_send
argument-hint: <#channel> <body...>
---

Use the `mcp__famp__famp_send` tool. The first argument `$1` is the channel (must start with `#`); the rest of `$ARGUMENTS` is the body.

Construct the call as:
- `to`: `{"kind": "channel", "name": "$1"}`
- `new_task`: the body text.

If `$1` does not start with `#`, ask the user to confirm the channel name.
