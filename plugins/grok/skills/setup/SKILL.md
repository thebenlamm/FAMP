---
description: One-time FAMP setup — install the broker binary and the persistent broker service. Run once per machine, before the first /famp:register.
---

# FAMP setup

Plugins have no install-time lifecycle script, so the parts of FAMP that are not
files — the compiled binary and the launchd/systemd broker service — are
bootstrapped here. Idempotent: re-running on a configured machine reports state
and changes nothing.

Work through the steps in order, stop at the first failure, and report the
actual command output rather than a summary.

## Step 1 — is the binary present?

```bash
famp --version
```

This plugin does not ship a PATH shim, so `famp` must be on your `PATH`
directly — `~/.cargo/bin` after a source build.

- **Prints a version** → go to Step 3.
- **`command not found`** → Step 2.

## Step 2 — install the binary

FAMP is not published to crates.io and ships no release binaries, so this builds
from source. Requires Rust 1.89+.

```bash
cargo --version
```

If there is no toolchain, tell the user to install one and stop — do not install
it for them. Recommend rustup; specifically do not suggest `brew install rust`
on macOS, which pulls a `python@3.14` dependency whose link step can collide
with an existing python.org framework install.

```bash
git clone https://github.com/thebenlamm/FAMP.git /tmp/famp-src \
  && cargo install --path /tmp/famp-src/crates/famp --locked
```

Roughly 90 seconds. Ensure `~/.cargo/bin` is on `PATH`, then re-run
`famp --version`.

## Step 3 — install the broker service

```bash
famp daemon install
```

Writes a launchd agent (macOS) or systemd `--user` unit (Linux). It refuses to
run inside a sandbox — if it reports a sandbox error, tell the user to run this
one command in a normal terminal themselves rather than working around it.

```bash
famp daemon status
```

Expect `state: RUNNING` with a pid and socket path.

## Step 4 — report

Tell the user the resolved binary version, the daemon state, and that they
should now run `/famp:register <name>` in each session they want to message
from — `register` holds the identity for the lifetime of that session, so the
session must stay open.

## Notes

- This plugin's hooks and MCP server stay inactive until the plugin is trusted.
  Install with `grok plugin install <source> --trust`, or trust it from the
  Plugins tab, or the broker will never be reachable from this session.
- Do not also run `famp install-grok`. It registers a second, unscoped MCP
  server under the same name and duplicates the hooks.
- After upgrading the binary, run `famp daemon restart` so the running broker
  picks it up. A stale daemon surfaces as `ProtocolMismatch`.
