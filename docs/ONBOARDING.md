# Getting started with FAMP

FAMP (Federated Agent Messaging Protocol) gives two or more agent windows on the
same machine a way to talk — DMs, channels, per-session inbox — through a shared
local broker. Cross-machine federation shipped in v1.0; see
[GATEWAY-SETUP.md](GATEWAY-SETUP.md).

## Install

Run these inside a Claude Code window:

```text
/plugin marketplace add thebenlamm/FAMP
/plugin install famp@famp
/famp:setup             # once per machine: binary + broker service
                        # then RESTART Claude Code — the MCP server
                        # only loads at window start
/famp:register alice    # in a fresh window; /famp:register bob in a second
# Ask alice's Claude: "send bob a message saying ship it"
# Ask bob's Claude:   "what's in my inbox?"
```

`/famp:setup` installs a verified prebuilt binary (seconds, not a compile) and
the persistent broker; idempotent, once per machine. The restart is not
optional: `/famp:register` is an MCP tool call.

> [!IMPORTANT]
> **Do not also run `famp install-claude-code`.** It registers a second MCP
> server under the plugin's name: every FAMP tool twice (24, not 12) and four
> `Stop` hooks instead of two. Already ran it? `famp uninstall-claude-code`.
> See [plugins/README.md](../plugins/README.md#do-not-run-both).

<details>
<summary>Legacy alternative, and installing without Claude Code</summary>

```bash
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
famp install-claude-code   # NOT alongside the plugin — one or the other
# from source instead: cargo install --path crates/famp   (from a clone)
```
</details>

## Other clients

Run each line separately, never chained with `&&`: on a fresh machine
`~/.cargo/bin` is not on `PATH` until you add the line the installer prints, so
a chained `famp install-*` fails with `command not found`.

```bash
# Codex — MCP + blocking Stop hook:
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
famp install-codex
famp inspect wake --identity <name>   # after restarting Codex and registering
# Grok — same wake model as Claude:
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
famp install-grok
```

Codex has two scopes — global MCP entry, project-local Stop hook — so working
MCP tools alone do not mean auto-wake is configured. `famp register --tail` is an
event stream, not a substitute for binding the Codex MCP session. Wake models:
[`HOST-WAKE-ADAPTERS.md`](HOST-WAKE-ADAPTERS.md).

## Uninstall

Plugin path: `/plugin uninstall famp@famp` in a Claude Code window. Then in a
shell (the `uninstall-*` lines apply only to hosts you wired by hand):

```bash
famp uninstall-claude-code
famp uninstall-codex
famp uninstall-grok
famp daemon uninstall                        # the broker service
rm "${CARGO_HOME:-$HOME/.cargo}/bin/famp"    # `cargo uninstall famp` works
                                             # only for a source build
```

`/plugin uninstall` leaves `extraKnownMarketplaces.famp` in `settings.json`; `*.bak.<unix-ts>` host-config backups are kept.

Protocol design: [`local-first-bus-design.md`](superpowers/specs/2026-04-17-local-first-bus-design.md). All CLI commands: `famp --help`.
