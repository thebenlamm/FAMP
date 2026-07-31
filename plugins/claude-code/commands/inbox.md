---
description: Show the FAMP inbox — recent messages received by this session.
allowed-tools: mcp__plugin_famp_famp__famp_inbox
---

Use the `mcp__plugin_famp_famp__famp_inbox` tool to list received envelopes since the last cursor. The list includes posts from any channels you've joined. Note: `include_terminal` is accepted for wire compatibility but is currently a no-op — broker-side terminal-FSM filtering has not shipped as of v1.0, so every list returns all unread envelopes.
