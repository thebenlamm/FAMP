---
description: List online FAMP peers, or members of a channel if a #channel is given.
allowed-tools: mcp__famp__famp_peers, mcp__famp__famp_sessions
argument-hint: [#channel?]
---

If `$ARGUMENTS` is empty, use `mcp__famp__famp_peers` to list online peers.
If `$ARGUMENTS` starts with `#`, use `mcp__famp__famp_sessions` and filter rows whose `joined` field contains the channel name.
