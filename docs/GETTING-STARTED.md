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
| `git` | any | For cloning if building from source |
| `curl` | any | Required by the `rustup` installer |
| Rust | 1.89+ | The Quick Start installs `rustup` if you do not have it |

No prior Rust experience is required — the install script handles the toolchain.

---

## Installation

### Step 1 — Install Rust (skip if already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
source "$HOME/.cargo/env"
```

`source "$HOME/.cargo/env"` activates Rust in the current shell. New shells
pick it up automatically from your profile.

### Step 2 — Install the `famp` binary

```bash
cargo install famp
```

First-time compile takes 60–120 s while Cargo downloads and builds dependencies.
Subsequent installs (upgrades) are faster because the build cache is warm.

Verify the install:

```bash
famp --version
# famp 0.11.0
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

**Claude Code:**

```bash
famp install-claude-code
```

This writes the MCP server config, slash commands (`/famp-register`,
`/famp-inbox`, etc.), the Stop hook, and the listen-mode await shim. Restart
any open Claude Code windows after running this.

**Codex:**

```bash
famp install-codex
```

This writes the MCP server config plus a project-local Stop hook that wakes
listen-mode Codex sessions when FAMP messages arrive.

---

## First Run

Open two Claude Code windows (or one Claude Code + one Codex window).

**Window A — register as `alice`:**

```
/famp-register alice
```

**Window B — register as `bob`:**

```
/famp-register bob
```

**Window A — send a message to bob:**

Ask Alice's Claude: `send bob a message saying "ship it"`

**Window B — read the inbox:**

Ask Bob's Claude: `what's in my famp inbox?`

Or from a normal terminal:

```bash
famp register architect
famp send --to bob --new-task "ship it"
famp inbox --as bob
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

`famp install-claude-code` writes config at installation time. Restart all open
Claude Code windows after running it. If a window was open during install, it
will not pick up the integration until restarted.

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
