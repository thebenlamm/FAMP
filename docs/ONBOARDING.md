# Getting started with FAMP

FAMP (Federated Agent Messaging Protocol) gives two or more agent windows on
the same machine a way to talk — DMs, channels, per-session inbox — through
a shared local broker. Federation across machines lands in v1.0.

## Install

```bash
# Install once (one-time compile, ~60-120s)
cargo install famp
famp install-claude-code

# In one Claude Code window:
/famp-register alice

# In another Claude Code window:
/famp-register bob

# Then ask alice's Claude: "send bob a message saying ship it"
# Then ask bob's Claude:   "what's in my inbox?"
```

First install includes a one-time compile (~60-120 s); subsequent windows
open in <30 s. `cargo install famp` is one-time per machine.

## Other clients

```bash
# Codex (OpenAI's CLI agent) — MCP + blocking Stop hook:
cargo install famp && famp install-codex

# Grok — MCP + non-blocking listen-wake skill (no long Stop hook):
cargo install famp && famp install-grok
```

See [`HOST-WAKE-ADAPTERS.md`](HOST-WAKE-ADAPTERS.md) for Claude/Codex vs Grok
wake models. For other MCP clients: file an issue at
<https://github.com/thebenlamm/FAMP/issues>.

## Uninstall

```bash
famp uninstall-claude-code
famp uninstall-codex
famp uninstall-grok
cargo uninstall famp           # removes the binary itself (run last)
```

`*.bak.<unix-ts>` backup files of host config are preserved so you can
recover from a bad merge — remove them manually after verifying.

---

Protocol design:
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](superpowers/specs/2026-04-17-local-first-bus-design.md).

All CLI commands: `famp --help`.
