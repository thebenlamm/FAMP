# Getting started with FAMP

FAMP (Federated Agent Messaging Protocol) gives two or more agent windows on
the same machine a way to talk — DMs, channels, per-session inbox — through
a shared local broker. Federation across machines shipped in v1.0 via
`famp-gateway`; see [GATEWAY-SETUP.md](GATEWAY-SETUP.md).

## Install

```bash
# Install once (prebuilt binary, a few seconds)
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
famp install-claude-code

# In one Claude Code window:
/famp-register alice

# In another Claude Code window:
/famp-register bob

# Then ask alice's Claude: "send bob a message saying ship it"
# Then ask bob's Claude:   "what's in my inbox?"
```

The installer downloads and checksum-verifies a prebuilt binary — a few
seconds, not a compile; subsequent windows open in <30 s. It is one-time
per machine. Building from source instead? `cargo install --path
crates/famp` from a clone.

## Other clients

```bash
# Codex (OpenAI's CLI agent) — MCP + blocking Stop hook:
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh && famp install-codex
# Restart Codex, register through the famp_register MCP tool, then verify:
famp inspect wake --identity <name>

# Grok — MCP + blocking Stop hook (same wake model as Claude):
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh && famp install-grok
# Then: "register with famp" → famp_register only; Stop auto-wakes.
```

See [`HOST-WAKE-ADAPTERS.md`](HOST-WAKE-ADAPTERS.md) for Claude/Codex/Grok
wake models. For other MCP clients: file an issue at
<https://github.com/thebenlamm/FAMP/issues>.

Codex installation has two scopes: the MCP entry is global, but the Stop hook
is project-local. MCP tools working in a project therefore does not by itself
mean automatic wake is configured. `famp register --tail` is a terminal event
stream, not a substitute for binding the Codex MCP session.

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
