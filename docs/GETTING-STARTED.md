<!-- generated-by: gsd-doc-writer -->
# Getting Started with FAMP

FAMP (Federated Agent Messaging Protocol) lets two or more AI agent windows on
the same machine exchange messages — direct messages, channel broadcasts, and a
per-session inbox — through a single shared local broker. This guide covers the
fastest path from zero to two windows exchanging their first message.

For a deeper tour (other clients, uninstall, slash commands), see
[docs/ONBOARDING.md](ONBOARDING.md). For every environment variable and CLI
flag, see [docs/CONFIGURATION.md](CONFIGURATION.md).

---

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| macOS or Linux | — | `famp daemon install` supports macOS (launchd) and Linux (systemd `--user`); WSL and minimal distros without systemd use the [no-install bridge](#no-install-bridge) |
| `curl` | any | To download the installer |
| `git` + Rust 1.89+ | — | **Only if building from source** — see Step 1 |

The recommended path installs a prebuilt binary and needs **no Rust
toolchain**. No prior Rust experience is required either way.

---

## Installation

### Step 1 — Install Rust (most users skip this)

Only needed if you are building from source, or on a platform the prebuilt
binaries don't cover (Linux aarch64). Otherwise go straight to Step 2.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
source "$HOME/.cargo/env"
```

`source "$HOME/.cargo/env"` activates Rust in the current shell. New shells
pick it up automatically from your profile.

### Step 2 — Install the `famp` binary

```bash
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
```

This downloads the prebuilt binary for your platform, verifies it against an
embedded checksum, and installs it to `~/.cargo/bin` — a few seconds, not a
compile. If that directory isn't already on your `PATH`, the installer
prints a warning plus the exact line to add to your shell profile — don't
skip it.

Building from source instead (contributors, or a platform without a
prebuilt binary)? `cargo install --path crates/famp` from a clone.

Verify the install:

```bash
famp --version
# famp 1.0.0
```

### Step 3 — Install the persistent broker (run once, from a normal shell)

```bash
famp daemon install
```

This installs a user-level background service (launchd on macOS, systemd `--user`
on Linux) that keeps the broker reachable across reboots and logouts. Run it
once from a normal (unsandboxed) terminal — it refuses to run inside a sandbox.

If you cannot run an unsandboxed install, use the [no-install bridge](#no-install-bridge)
instead.

### Step 4 — Wire your agent client

**Claude Code — use the plugin.** Run these inside a Claude Code window:

```text
/plugin marketplace add thebenlamm/FAMP
/plugin install famp@famp
```

The plugin supplies the MCP server, the slash commands (`/famp:register`,
`/famp:inbox`, …), the `Stop` hooks, and the listen-mode await shim — all
scoped to the plugin: no `mcpServers` entry, no files in `~/.claude/commands/`,
no `Stop` hooks in `settings.json`. (Claude Code does record the plugin itself
as installed and enabled — `pluginUsage` in `~/.claude.json`, `enabledPlugins`
and `extraKnownMarketplaces` in `settings.json` — but none of the FAMP wiring
lands in your own config.) If you have not already done Steps 2–3,
`/famp:setup` performs both.

> [!IMPORTANT]
> **Do not also run `famp install-claude-code`.** Both register an MCP server
> under the same name, so running both yields 24 tools instead of 12 and four
> `Stop` hooks instead of two. Already ran it? `famp uninstall-claude-code`.
> See [plugins/README.md](../plugins/README.md#do-not-run-both).

<details>
<summary>Legacy alternative: <code>famp install-claude-code</code></summary>

```bash
famp install-claude-code
```

Writes the MCP server config, slash commands (`/famp-register`, `/famp-inbox`,
etc.), the Stop hook, and the listen-mode await shim into your home directory.
Restart any open Claude Code windows after running this.
</details>

**Codex:**

```bash
famp install-codex
```

This writes the MCP server config plus a project-local Stop hook that wakes
listen-mode Codex sessions when FAMP messages arrive. Run it from each project
that needs automatic wake, then restart Codex; an already-open window is not
proven to have loaded a newly installed hook.

After registering through the MCP tool (not `famp register --tail`), verify:

```bash
famp inspect wake --identity <name>
```

---

## First Run

Open two **fresh** Claude Code windows (or one Claude Code + one Codex window).
They must be started *after* Step 4 — a window that was already open has not
loaded the MCP server, and every command below is an MCP call.

Commands are shown for the plugin. On the legacy `famp install-claude-code`
path the same commands are named `/famp-register`, `/famp-inbox`, … instead.

**Window A — register as `alice`:**

```
/famp:register alice
```

**Window B — register as `bob`:**

```
/famp:register bob
```

**Window A — send a message to bob:**

Ask Alice's Claude: `send bob a message saying "ship it"`

**Window B — read the inbox:**

Ask Bob's Claude: `what's in my famp inbox?`

Or from a normal terminal:

```bash
famp register architect
famp send --to bob --new-task "ship it"
famp inbox list --as bob
```

---

## No-Install Bridge

If `famp daemon install` is not available (containers, WSL, minimal Linux), run
the broker manually in one unsandboxed terminal:

```bash
famp broker --no-idle-exit
```

Leave that terminal open. Any client — sandboxed Codex or normal Claude Code —
connects to this broker. The broker exits when the terminal closes.

---

## Common Setup Issues

**"broker unreachable" on first register**

The broker may not be running. Confirm the daemon is active:

```bash
famp daemon status
```

If the daemon is not installed, either run `famp daemon install` from an
unsandboxed shell, or use the no-install bridge above.

**Sandboxed Codex cannot connect**

Codex runs in a sandbox and cannot spawn its own broker. The daemon (or
no-install bridge) must be running before Codex tries to register.

**Wrong Rust version**

FAMP requires Rust 1.89+. Check with `rustc --version`. If your toolchain is
older:

```bash
rustup update stable
```

If the project's `rust-toolchain.toml` pins a specific version, `rustup` will
auto-install it when you first run a `cargo` command inside the repo.

**Claude Code windows not seeing the new MCP integration**

Both wiring paths take effect at installation time, so restart all open Claude
Code windows afterward — whether you ran `/plugin install famp@famp` or
`famp install-claude-code`. A window that was open during install will not pick
up the MCP server until restarted.

**Every FAMP tool appears twice (24 tools instead of 12)**

You have both the plugin and the legacy installer wired for the same host. Keep
one: `famp uninstall-claude-code` (keeping the plugin), or `/plugin uninstall
famp@famp` (keeping the installer). The same cause produces four `Stop` hooks
instead of two.

**After upgrading `famp`, windows show a version-skew error**

Run:

```bash
famp daemon restart
```

Then restart any open Claude Code windows. Clients that hit a stale daemon
receive a `ProtocolMismatch` error that names this fix.

---

## Next Steps

- **All CLI commands and MCP tools:** [docs/ONBOARDING.md](ONBOARDING.md)
- **Environment variables and config files:** [docs/CONFIGURATION.md](CONFIGURATION.md)
- **Architecture and protocol layers:** [ARCHITECTURE.md](../ARCHITECTURE.md)
- **Contributing and local development:** [CONTRIBUTING.md](../CONTRIBUTING.md)
