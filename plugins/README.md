# Host plugin packagings

FAMP targets three agent hosts, and all three now ship a plugin marketplace.
This directory holds one packaging per host.

| Host | Install | Packaging | Status |
| :--- | :--- | :--- | :--- |
| Claude Code | `/plugin marketplace add thebenlamm/FAMP` | [`claude-code/`](claude-code/) | **verified end to end** |
| Codex | `codex plugin marketplace add thebenlamm/FAMP` | [`codex/`](codex/) | installs; tool naming unverified |
| Grok Build | `grok plugin marketplace add thebenlamm/FAMP` | [`grok/`](grok/) | installs + validates; no live session |

> **Only the Claude Code packaging has been exercised end to end** — installed
> from a clean checkout, a real message sent through its MCP server, delivery
> confirmed. The Codex packaging installs and loads (`installed, enabled
> 1.0.0`), but no live session has confirmed its tool names. The Grok packaging
> passes `grok plugin validate` and installs from a local path with `--trust`
> (`components: 1 skill dir(s), 1 command dir(s), 0 agent dir(s), hooks, MCP
> servers`), but no live session has run it either; its namespace comes from
> reading `xai-org/grok-build`.
>
> This matters because of *how* it would fail. The commands hard-code tool names
> in `allowed-tools`. If a host's real namespacing differs from the table below,
> the commands load cleanly and then match no tool — nothing errors. Before
> trusting the Codex or Grok packaging, run `<host> plugin validate` against it
> and confirm one real tool call in a live session.
>
> **The same caveat applies to Codex hook execution.** The Codex manifest now
> declares `"hooks": "./hooks/hooks.json"` — accepted by `codex plugin add`, and
> the file is copied into the plugin cache alongside `hooks/famp-await.sh`. What
> is *not* yet confirmed is that Codex runs a plugin-provided Stop hook in a live
> session. The check takes one turn: start a Codex session with the plugin
> installed and grep `~/.codex/config.toml` for a `[hooks.state]` key ending in
> `hooks/hooks.json:stop:0:0`. Codex prompts to trust a newly seen hook before it
> will run, so expect a trust prompt on that first turn.

Then install and run the one-time setup:

```shell
# Claude Code
/plugin install famp@famp   →  /famp:setup

# Codex
codex plugin add famp@famp  →  /famp:setup

# Grok Build
grok plugin install famp --trust   →  /famp:setup
```

## Why packagings and not three installers

FAMP currently reaches each host through a bespoke `install-<host>` command that
writes into the user's home directory: MCP entries, slash-command files, and
hooks merged into shared config. Three hosts, three installers, three sets of
global mutation to apply and unpick.

Marketplace packaging replaces all of that with the host's own install path and
scopes every component to the plugin. Measured on Claude Code with the plugin
installed and `install-claude-code` removed: **0** `Stop` hooks in
`~/.claude/settings.json`, **0** entries in `mcpServers`, **0** files in
`~/.claude/commands/`.

## The one thing that genuinely differs: MCP tool naming

Manifests turned out to be largely cross-compatible — each host falls back to
the others' manifest paths (see [Manifest discovery](#manifest-discovery)). The
real divergence is how a plugin-provided MCP server's tools are named when
exposed to the model. FAMP's slash commands name their tools explicitly in
`allowed-tools` frontmatter, so this cannot be papered over.

| Host | Tool name for FAMP's `famp_register` |
| :--- | :--- |
| Claude Code | `mcp__plugin_famp_famp__famp_register` |
| Codex | `mcp__famp__famp_register` |
| Grok Build | `famp__famp_register` |

Three hosts, three rules — including Grok, which carries no `mcp__` prefix at
all. `scripts/gen-plugin.sh <host>` holds the table and rewrites the commands
accordingly; `just plugin-check <host>` fails on drift.

**Provenance.** Claude Code's rule was verified at runtime by loading the plugin
and enumerating tools. Codex's and Grok's are derived from their (open) source,
because driving either CLI to enumerate tools needs provider credentials:

- **Codex** — `codex-mcp/src/mcp/mod.rs` composes
  `"mcp" + "__" + server_name + "__"`; `core-plugins/src/loader.rs` does *not*
  prefix plugin MCP server names, keeping the declared name and deduping
  globally (`"skipping duplicate plugin MCP server name"`); and
  `plugin_namespace` is consumed only by `core-skills`, never by MCP naming. Net
  effect: identical to the bare namespace, so the rewrite is a deliberate no-op.
- **Grok Build** — `xai-grok-mcp/src/servers.rs` keys tool metadata by
  `"{server}__"`, and `docs/user-guide/22-permissions-and-safety.md` states it
  outright: *"Grok tool names carry no `mcp__` prefix, so a rule written as
  `mcp__server__tool` never matches an MCP call."*

## Manifest discovery

Each host looks for its own manifest first and then falls back to the others'.
Codex, from `exec-server-protocol/src/protocol.rs`:

```rust
pub const DISCOVERABLE_PLUGIN_MANIFEST_PATHS: &[&str] = &[
    ".codex-plugin/plugin.json",
    ".claude-plugin/plugin.json",
    ".cursor-plugin/plugin.json",
];
```

Grok does the same, accepting `.grok-plugin/` and "the `.claude-plugin/`
equivalents". Both also expand `${CLAUDE_PLUGIN_ROOT}` alongside their native
token.

**That fallback is a trap for a repo like this one.** With only
`.claude-plugin/marketplace.json` present, `codex plugin marketplace add .`
resolved the marketplace successfully — and pointed at `plugins/claude-code`,
whose commands carry Claude's namespace and would silently match no tool under
Codex. Observed directly:

```console
$ codex plugin list
Marketplace `famp`
/Users/…/FAMP/.claude-plugin/marketplace.json
famp@famp  not installed   /Users/…/FAMP/plugins/claude-code    # ← wrong packaging
```

So each host needs its own marketplace index, ordered ahead of the fallback:

| File | Read by | Points at |
| :--- | :--- | :--- |
| `.claude-plugin/marketplace.json` | Claude Code | `./plugins/claude-code` |
| `.agents/plugins/marketplace.json` | Codex (preferred over `.claude-plugin/`) | `./plugins/codex` |
| `.grok-plugin/marketplace.json` | Grok Build (preferred over `.claude-plugin/`) | `./plugins/grok` |

`.agents/plugins/` is Codex's vendor-neutral path — its own bundled marketplaces
use it, and it sits above `.claude-plugin/marketplace.json` in
`MARKETPLACE_MANIFEST_RELATIVE_PATHS`. With it in place, Codex resolves
correctly:

```console
$ codex plugin add famp@famp
Added plugin `famp` from marketplace `famp`.
$ codex plugin list
famp@famp  installed, enabled  1.0.0   /Users/…/FAMP/plugins/codex
```

## Differences between the packagings

Beyond namespacing:

- **`bin/` PATH injection is Claude-Code-only.** The Claude packaging ships a
  `bin/famp` resolver shim, so its `.mcp.json` can point at
  `${CLAUDE_PLUGIN_ROOT}/bin/famp`. The Codex and Grok packagings invoke a bare
  `famp` and therefore require it on `PATH`; their `/famp:setup` skills say so.
- **Listen mode is Stop-hook only in v1.** All three packagings park via
  `famp-await.sh` on turn end. A Claude background monitor / `/famp:listen` skill
  is not shipped — a second bus waiter would race the Stop hook. Grok's
  host-native non-blocking wake adapter (`famp listen-wake`) remains available as
  a CLI for non-plugin workflows.
- **Each host declares hooks its own way.** Claude and Grok auto-discover
  `hooks/hooks.json` inside the plugin; Codex needs the manifest to name it
  (`"hooks": "./hooks/hooks.json"` in `.codex-plugin/plugin.json`). Note that
  Codex's *bundled* `plugin-creator` skill contradicts itself here — its
  `references/plugin-json-spec.md` sample lists `hooks` as a manifest field while
  its "Required behavior" section says to omit `hooks` as unsupported. The sample
  is the one that matches observed behavior: `codex plugin add` accepts the field
  and copies the file. The root-token also differs: Grok injects
  `GROK_PLUGIN_ROOT`, while **Codex has no `CODEX_PLUGIN_ROOT`** — its hook
  command runner injects `CLAUDE_PLUGIN_ROOT`/`CLAUDE_PLUGIN_DATA`, so the Codex
  `hooks.json` uses the Claude token deliberately.
- **`hook-runner.sh` is Claude-Code-only** — and is no longer shipped to the
  other two. HOOK-04b edit-glob dispatch parses Claude's `transcript_path` JSONL
  looking for `Edit`/`Write`/`MultiEdit` `tool_use` blocks. Codex's Stop payload
  carries no Claude-format transcript, and Grok's documented Stop input carries
  no transcript field at all, so on either host the shim exits 0 at step 2 on
  every turn. It shipped inert in both packagings until 2026-08-04. The legacy
  `install-codex` / `install-grok` paths never installed it either, so dropping
  it is parity, not a feature removal. `famp-await.sh`, by contrast, *is*
  host-aware: it accepts snake_case (Claude/Codex) and camelCase (Grok) input
  keys and resolves Codex's rollout file from its state DB when the Stop payload
  omits a transcript path.
- **Hook binary pin is rendered at gen time.** Canonical assets use
  `FAMP_BIN=@FAMP_BIN@` for `install-*`; `scripts/gen-plugin.sh` renders Claude
  hooks to `${CLAUDE_PLUGIN_ROOT}/bin/famp` and Codex/Grok hooks to bare `famp`.
  `just plugin-check-all` fails if the token leaks into `plugins/*/hooks`.
- **Grok requires explicit trust.** A Grok plugin's hooks and MCP servers stay
  inactive until the plugin is trusted — `grok plugin install <source> --trust`.

## Do not run both

Installing a packaging *and* running the matching `install-<host>` command on the
same machine loads two MCP servers under the same name. On Claude Code that
surfaces as 24 tools instead of 12 — every FAMP tool twice, under both
namespaces — plus four `Stop` hooks instead of two. Run
`famp uninstall-<host>` first.

The duplicate-hook half of that applies to all three hosts now that each
packaging wires its own `Stop` await: two awaiters would park on the same
identity. `famp-await.sh` carries a per-identity mkdir-lock singleton, so the
second one exits as a no-op rather than double-parking — but that is a backstop,
not a reason to run both.

## If you added this repo as a `directory` marketplace

Adding a marketplace from a local path (rather than `thebenlamm/FAMP`) records a
**`directory` source pointing at your working tree** — the plugin is not a
pinned, copied artifact. Editing `plugins/<host>/hooks/*` changes what executes
at the next session `Stop` immediately, with no install or update step, and a
`git pull` or branch switch re-arms your hooks the same way.

That is convenient when you are developing the packaging and surprising
otherwise. If you want a stable plugin, install from the GitHub source
(`/plugin marketplace add thebenlamm/FAMP`) instead; check which you have with
`extraKnownMarketplaces` in `~/.claude/settings.json`.
