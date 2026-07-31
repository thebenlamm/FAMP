---
description: List online FAMP peers, or members of a channel if a #channel is given.
allowed-tools: mcp__plugin_famp_famp__famp_peers
argument-hint: "[#channel?]"
---

Call `mcp__plugin_famp_famp__famp_peers` to get the current set of online peers as
`{ online: [...] }`.

If `$ARGUMENTS` is empty, present the full `online` list to the user.

If `$ARGUMENTS` starts with `#`, treat it as a channel name and present
only those `online` peers that are members of that channel. Channel
membership is observable from the user's prior `/famp-join` /
`/famp-leave` interactions in this conversation; if membership is not
known from context, present the full `online` list and label the output
"filtered: best-effort — channel membership not introspectable from the
12-tool MCP surface".

Use only the `mcp__plugin_famp_famp__famp_peers` tool listed in `allowed-tools`
above. The MCP surface is exactly 12 tools and the project tests
forbid referencing any other tool name from this asset.
