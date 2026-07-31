# FAMP — Claude Code plugin

Packages FAMP's Claude Code integration as an installable plugin, so it arrives
through the host's own install path instead of `famp install-claude-code`
mutating files under `~/.claude/`.

```shell
/plugin marketplace add thebenlamm/FAMP
/plugin install famp@famp
/famp:setup
```

This is the first of three host packagings — see [`../README.md`](../README.md)
for the Codex and Grok Build picture and what blocks them.

## What it replaces

`famp install-claude-code` writes five things into the user's home directory.
All five are native plugin components:

| `install-claude-code` writes | Plugin equivalent |
| :--- | :--- |
| `~/.claude.json :: mcpServers.famp` | `.mcp.json` |
| `~/.claude/commands/famp-*.md` | `commands/` |
| `~/.claude/settings.json :: hooks.Stop` | `hooks/hooks.json` |
| `~/.famp/hook-runner.sh` | `hooks/hook-runner.sh` |
| `~/.claude/hooks/famp-await.sh` | `hooks/famp-await.sh` |

Measured on a machine with the plugin installed and `install-claude-code`
removed: zero `Stop` hooks in `~/.claude/settings.json`, zero entries in
`mcpServers`, zero files in `~/.claude/commands/`. Everything lives inside the
plugin.

The `Stop` hooks are the main motivation. Today they merge into the user's
**global** settings, so they arm in every project — including ones with no
connection to FAMP. Uninstall becomes `/plugin uninstall` instead of a bespoke
command performing surgery on a shared JSON file, and updates go through
`/plugin update`.

To be fair to the existing installer: its merge is careful. On a machine with
10 pre-existing hooks from an unrelated toolchain, all 10 survived install and
uninstall untouched, and the config came back byte-identical to a pre-install
backup. This is not a bug report about the merge — it is about the merge not
needing to happen.

## Layout

```
claude-code/
├── .claude-plugin/plugin.json   manifest
├── .mcp.json                    registers `famp mcp` as the MCP server
├── bin/
│   ├── famp                     resolver shim — puts `famp` on the Bash PATH
│   └── famp-listen-monitor      background listen-mode watcher
├── commands/                    7 slash commands  (generated)
├── hooks/
│   ├── hooks.json               Stop-hook registration
│   ├── hook-runner.sh           (generated — copied from assets)
│   └── famp-await.sh            (generated — copied from assets)
├── monitors/monitors.json       listen-mode monitor declaration
└── skills/
    ├── setup/SKILL.md           /famp:setup — binary + daemon bootstrap
    └── listen/SKILL.md          /famp:listen — starts the monitor
```

## Two things the plugin system cannot do

**No install-time lifecycle script.** Nothing in a plugin runs shell at install
time, so the compiled binary and the launchd/systemd broker service cannot be
bootstrapped automatically. That is what `/famp:setup` is for — one idempotent
command, run once per machine.

**No vendored binary.** `bin/` would work — it is prepended to the Bash tool's
PATH — but it would mean committing a ~7.6 MB artifact per platform triple into
the marketplace repo, and FAMP currently publishes neither to crates.io nor to
GitHub releases. `bin/famp` is therefore a resolver shim that execs the first
binary it finds at `$FAMP_BIN`, `~/.famp/bin/famp`, `~/.cargo/bin/famp`,
`/usr/local/bin/famp`, or `/opt/homebrew/bin/famp`, and exits 127 with a
pointer to `/famp:setup` otherwise. Once release artifacts exist, `/famp:setup`
can download the right one and the shim keeps working unchanged.

## Generated files

`commands/*.md` and `hooks/*.sh` come from `crates/famp/assets/` via
`just plugin-gen`; `just plugin-check` fails on drift. They are committed
because an installed plugin is a git clone and must contain real files.

The commands cannot be copied verbatim: a plugin-provided MCP server is exposed
under a scoped tool name. Verified in a live session —

```console
$ claude --plugin-dir ./plugins/claude-code -p "list tools matching famp"
mcp__plugin_famp_famp__famp_register    # plugin
mcp__famp__famp_register                # install-claude-code, user scope
```

— so `allowed-tools` frontmatter and in-body tool references are rewritten. The
generator fails loudly if any bare `mcp__famp__` reference survives, because a
missed rewrite yields a command whose `allowed-tools` matches no real tool,
which fails silently at runtime.

Files are also renamed (`famp-send.md` → `send.md`) since plugin skills are
already namespaced by the plugin — the original names would give
`/famp:famp-send`.

The two Stop-hook shims are copied unchanged: they resolve the binary through
PATH and match tool names with `.endswith()`, so they are namespace-agnostic
already.

## Do not run both

If `famp install-claude-code` has already run on the same machine, both MCP
servers load and the model sees 24 tools — every FAMP tool twice, under both
namespaces — plus four `Stop` hooks instead of two. Run
`famp uninstall-claude-code` first.
