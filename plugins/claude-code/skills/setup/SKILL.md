---
description: One-time FAMP setup — install the broker binary and the persistent broker service. Run once per machine, before the first /famp:register.
disable-model-invocation: true
allowed-tools: Bash
---

# FAMP setup

Claude Code plugins have no install-time lifecycle script, so the parts of FAMP
that are not files — the compiled binary and the launchd/systemd broker service
— are bootstrapped here instead. This is idempotent; running it again on a
configured machine reports state and changes nothing.

Work through the steps in order and stop at the first one that fails, reporting
the actual command output rather than a summary.

## Step 1 — is the binary already there?

```bash
famp --version
```

The plugin puts a resolver shim on PATH that finds a binary at `$FAMP_BIN`,
`~/.famp/bin/famp`, `~/.cargo/bin/famp`, `/usr/local/bin/famp`, or
`/opt/homebrew/bin/famp`.

- **Prints a version** → skip to Step 3.
- **Exits 127 with "no broker binary found"** → continue to Step 2.

## Step 2 — install the binary

FAMP is not published to crates.io and ships no release binaries, so this
builds from source. Requires Rust 1.89+.

Check for a toolchain first:

```bash
cargo --version
```

If there is no toolchain, tell the user to install one and stop — do not
install it for them, and specifically do not suggest `brew install rust` on
macOS, which pulls a `python@3.14` dependency whose link step can collide with
an existing python.org framework install. rustup is also the only installer
that honors this repo's `rust-toolchain.toml` pin:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

With a toolchain present, build and install:

```bash
git clone https://github.com/thebenlamm/FAMP.git /tmp/famp-src \
  && cargo install --path /tmp/famp-src/crates/famp --locked
```

This takes roughly 90 seconds. Then re-run `famp --version` to confirm the
shim now resolves it.

## Step 3 — install the broker service

```bash
famp daemon install
```

This writes a launchd agent (macOS) or a systemd `--user` unit (Linux) so the
broker survives reboots. It refuses to run inside a sandbox — if it reports a
sandbox error, tell the user to run this one command in a normal terminal
themselves. Do not attempt to work around it.

Verify:

```bash
famp daemon status
```

Expect `state: RUNNING` with a pid and socket path. `NOT_INSTALLED` or a
missing socket means Step 3 did not take effect.

## Step 4 — report

Tell the user:

- the resolved binary path and version,
- the daemon state,
- that they should now run `/famp:register <name>` in each window they want to
  message from, keeping those windows open — `register` holds the identity for
  the lifetime of the session,
- and that `/famp:listen` additionally starts a background watcher that
  surfaces inbound messages without them having to check the inbox.

## Notes

- No files in `~/.claude/` are modified. The plugin supplies its skills, Stop
  hooks, and MCP server registration directly, so the global-config mutations
  performed by `famp install-claude-code` are not needed. Do not run that
  command alongside this plugin — it would register a second, unscoped MCP
  server and duplicate both Stop hooks.
- If the user upgrades the binary later, run `famp daemon restart` so the
  running broker picks it up. A stale daemon surfaces as `ProtocolMismatch`.
