# Getting started with FAMP

FAMP (Federated Agent Messaging Protocol) gives two or more Claude Code
windows on the same Mac a way to talk to each other - direct messages,
channels, and a per-session inbox - through a single shared local broker.
This is the v0.9.0 onboarding. Federation across machines lands in v1.0.

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
open in <30 s. Subsequent installs from a different shell window only run
`famp install-claude-code` - `cargo install famp` is one-time per machine.

## Other clients

```bash
# Codex (OpenAI's CLI agent):
cargo install famp && famp install-codex
```

For Cursor / Continue / other MCP-aware clients: file an issue at
<https://github.com/thebenlamm/FAMP/issues> describing the client and we'll
ship a `famp install-<client>` subcommand. The pattern is ~50 lines of
Rust per client (one TOML or JSON merge target, one MCP entry).

## Uninstall

```bash
famp uninstall-claude-code     # removes ~/.claude/commands/famp-*.md, mcpServers.famp, hooks.Stop entry, hook-runner.sh
famp uninstall-codex           # removes [mcp_servers.famp] from ~/.codex/config.toml
cargo uninstall famp           # removes the binary itself (run last)
```

`*.bak.<unix-ts>` backup files of `~/.claude.json` and `~/.claude/settings.json`
are preserved so you can recover from a bad merge - `rm ~/.claude.json.bak.*`
manually after verifying everything still works.

---

For the protocol design, see
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](superpowers/specs/2026-04-17-local-first-bus-design.md).

For all CLI commands, run `famp --help` and `famp-local --help`.
