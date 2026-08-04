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

FAMP ships prebuilt binaries. Prefer the installer — it needs no Rust
toolchain and takes seconds rather than minutes:

```bash
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
```

It writes to `$CARGO_HOME/bin`, defaulting to `~/.cargo/bin` — which the
resolver shim already searches. If `$CARGO_HOME` is set to something else, the
shim will not find the binary: point `$FAMP_BIN` at it, or re-run with
`CARGO_HOME=$HOME/.cargo`. If the installer prints a `PATH` warning, relay it
verbatim — "installed but command not found" is the most common failure here.
Re-run `famp --version` to confirm, then go to Step 3.

Prebuilt binaries cover macOS (arm64, x86_64) and Linux x86_64 (glibc 2.35+).
**Only if the installer reports no archive for this platform** — Linux aarch64
is the known gap — fall back to building from source, which requires Rust
1.89+.

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

With a toolchain present, build and install. Use a throwaway directory so a
failed or partial run does not leave a sticky `/tmp/famp-src` that breaks the
next attempt. The trap removes the tree on success and on failure:

```bash
SRC="$(mktemp -d "${TMPDIR:-/tmp}/famp-src.XXXXXX")"
cleanup() { rm -rf "$SRC"; }
trap cleanup EXIT
git clone --depth 1 https://github.com/thebenlamm/FAMP.git "$SRC"
cargo install --path "$SRC/crates/famp" --locked
```

This takes roughly 90 seconds. Then re-run `famp --version` to confirm the
shim now resolves it. Note that this builds whatever the default branch is at
clone time, which may be ahead of the latest release.

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
- and that with listen mode on (the default for MCP `famp_register`), the
  plugin Stop hook parks until an inbound message arrives and wakes the
  window — no separate listen skill is required.

## Notes

- No files in `~/.claude/` are modified. The plugin supplies its skills, Stop
  hooks, and MCP server registration directly, so the global-config mutations
  performed by `famp install-claude-code` are not needed. Do not run that
  command alongside this plugin — it would register a second, unscoped MCP
  server and duplicate both Stop hooks.
- If the user upgrades the binary later, run `famp daemon restart` so the
  running broker picks it up. A stale daemon surfaces as `ProtocolMismatch`.
