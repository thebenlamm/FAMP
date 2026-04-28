# Skill: famp-setup

Set up FAMP (Federated Agent Messaging Protocol) on a Mac so that two or more
Claude Code or Codex windows can exchange signed messages as distinct FAMP
identities.

---

## Key model: one MCP server config per client, identity chosen per window

As of v0.8.x, the `famp mcp` server starts **unbound**. The `.mcp.json` (or
Codex equivalent) does **not** contain `FAMP_HOME` or any identity marker.
Each Claude Code or Codex window calls `famp_register` once at session start
to pick an identity.

Identity backing store: `$FAMP_LOCAL_ROOT/agents/<name>/` (default
`~/.famp-local/agents/<name>/`). Each identity directory is created by
`famp-local wire` or `famp-local init`; it must contain a readable
`config.toml`.

---

## Recommended path: famp-local wrapper

`scripts/famp-local` compresses the 8-step federation flow into one command.

### Prerequisite — `famp` binary on PATH (or `FAMP_BIN` set)

`famp-local` resolves the binary in one of two ways:

1. `$FAMP_BIN` if exported (absolute path)
2. `famp` on `$PATH`

Otherwise it dies with `famp binary not found on PATH (set FAMP_BIN=... to override)`.

Three options to satisfy this from outside the FAMP repo:

- **Per-invocation override** — no install needed:
  `FAMP_BIN=$HOME/Workspace/FAMP/target/release/famp ~/Workspace/FAMP/scripts/famp-local wire <dir>`
- **Shell export** — set `FAMP_BIN` in `~/.zshrc` pointing at
  `target/release/famp` so `cargo build --release` is the only refresh step.
  Recommended for FAMP contributors: no global stale binary.
- **Global install** — `cargo install --path crates/famp` once. Frozen
  copy in `~/.cargo/bin/famp`; you must re-run install after protocol
  changes. Best for FAMP *consumers* who don't iterate on the protocol.

Version skew warning: if a globally-installed `famp` lags the repo, signature
or canonicalization (INV-10) bugs can mask. The export-`FAMP_BIN` path avoids this.

### Step 1 — Wire each repo directory

From any directory:

```bash
~/Workspace/FAMP/scripts/famp-local wire ~/Workspace/RepoA
~/Workspace/FAMP/scripts/famp-local wire ~/Workspace/RepoB
# Override identity name if the repo basename doesn't fit:
~/Workspace/FAMP/scripts/famp-local wire ~/Workspace/God --as architect
```

Or, when CWD is the FAMP repo, `scripts/famp-local wire …`.

`wire` creates the identity (if new), starts a daemon, exchanges peer cards
with existing agents, and drops a project-scoped `.mcp.json` in the target
directory. The `.mcp.json` contains only `command` + `args` — no `FAMP_HOME`.

### Step 2 — Restart the client window

Claude Code: restart the window for that repo. The new `.mcp.json` is picked
up on restart.

Codex (user-scope registration): run once, then restart:

```bash
scripts/famp-local mcp-add --client codex RepoA RepoB
```

### Step 3 — Register in every new window (required before messaging)

In each Claude Code or Codex window after it opens, call `famp_register`:

```text
register as RepoA
```

Or directly via the MCP tool:

```json
{ "tool": "famp_register", "arguments": { "identity": "RepoA" } }
```

Until `famp_register` succeeds, `famp_send`, `famp_inbox`, `famp_await`, and
`famp_peers` all return a typed `not_registered` error.

Use `famp_whoami` to confirm the binding:

```json
{ "identity": "RepoA", "source": "explicit" }
```

### Step 4 — Send messages

```text
send a message to RepoB saying hello
```

The window's MCP client invokes `famp_send`; the message arrives at `RepoB`'s
inbox. The receiving window polls with `famp_await` or `famp_inbox`.

---

## Minimal manual .mcp.json (no wrapper)

If you are not using `scripts/famp-local`:

```json
{
  "mcpServers": {
    "famp": {
      "command": "/absolute/path/to/famp",
      "args": ["mcp"]
    }
  }
}
```

Do **not** add `FAMP_HOME` to the `env` block — that pattern was removed in
v0.8.x. Identity is chosen at runtime via `famp_register`.

Optional: set `FAMP_LOCAL_ROOT` to point at a non-default backing store:

```json
{
  "mcpServers": {
    "famp": {
      "command": "/absolute/path/to/famp",
      "args": ["mcp"],
      "env": { "FAMP_LOCAL_ROOT": "/path/to/custom-root" }
    }
  }
}
```

---

## Federation CLI (manual, cross-machine)

These commands still use `FAMP_HOME` per identity — that has not changed. Use
this path for cross-machine setups or when you need explicit control over
ports, keypairs, and TOFU pinning.

```bash
# Create two identities with separate HOME dirs
famp setup --name alice --home /tmp/famp-alice --port 8443
famp setup --name bob   --home /tmp/famp-bob   --port 8444

# Exchange peer cards
FAMP_HOME=/tmp/famp-alice famp info | FAMP_HOME=/tmp/famp-bob famp peer import
FAMP_HOME=/tmp/famp-bob   famp info | FAMP_HOME=/tmp/famp-alice famp peer import

# Start daemons
FAMP_HOME=/tmp/famp-alice famp listen &
FAMP_HOME=/tmp/famp-bob   famp listen &

# Send (TOFU bootstrap required on first contact)
FAMP_TOFU_BOOTSTRAP=1 FAMP_HOME=/tmp/famp-alice famp send \
  --to bob --new-task "hello from alice"

# Read Bob's inbox
FAMP_HOME=/tmp/famp-bob famp inbox list
```

After initial TOFU pinning, subsequent sends drop the `FAMP_TOFU_BOOTSTRAP=1`
flag. The fingerprint in `peers.toml` is the trust anchor from that point on.

---

## MCP six-tool surface

| Tool | When to call | Notes |
|---|---|---|
| `famp_register` | **First, every new window** | Binds session to identity; idempotent |
| `famp_whoami` | Debug / confirm binding | Never errors; returns `null` if unregistered |
| `famp_send` | Send a message | Requires prior `famp_register` |
| `famp_inbox` | List active inbox entries | Filters terminal tasks by default |
| `famp_await` | Wait for next message (real-time) | Use this to detect task completion |
| `famp_peers` | List or add peers | Requires prior `famp_register` |

---

## Troubleshooting

**`not_registered` on first tool call** — run `famp_register` with the
identity name, then retry. Re-running `scripts/famp-local wire <dir>` migrates
old `.mcp.json` files that still had `FAMP_HOME` (pre-v0.8.x wired repos).

**`unknown_identity` from `famp_register`** — the `~/.famp-local/agents/<name>/`
directory doesn't exist or is missing `config.toml`. Wire the repo first:
`scripts/famp-local wire <dir> --as <name>`.

**Two windows, same identity** — both windows register as the same name and
share the same inbox. Concurrent `famp_await` calls: the second returns
`LockHeld { pid }` — this is expected v0.8 behavior.

**Daemon not running** — `scripts/famp-local status` shows daemon state.
Restart with `scripts/famp-local stop && scripts/famp-local init <names>`.
