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
directly — `~/.cargo/bin` after either the installer or a source build.

- **Prints a version** → go to Step 3.
- **`command not found`** → Step 2.

## Step 2 — install the binary

FAMP ships prebuilt binaries. Prefer the installer — no Rust toolchain, seconds
rather than minutes:

```bash
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
```

It writes to `$CARGO_HOME/bin`, defaulting to `~/.cargo/bin`; make sure that
directory is on your `PATH` and relay any `PATH` warning it prints verbatim.
Re-run `famp --version`, then go to Step 3.

Prebuilt binaries cover macOS (arm64, x86_64) and Linux x86_64 (glibc 2.35+).
**Only if the installer reports no archive for this platform** — Linux aarch64
is the known gap — fall back to a source build, which requires Rust 1.89+:

```bash
cargo --version
```

If there is no toolchain, tell the user to install one and stop — do not install
it for them. Recommend rustup; specifically do not suggest `brew install rust`
on macOS, which pulls a `python@3.14` dependency whose link step can collide
with an existing python.org framework install.

Use a throwaway directory so a failed or partial run does not leave a sticky
`/tmp/famp-src` that breaks the next attempt. The trap removes the tree on
success and on failure:

```bash
SRC="$(mktemp -d "${TMPDIR:-/tmp}/famp-src.XXXXXX")"
cleanup() { rm -rf "$SRC"; }
trap cleanup EXIT
git clone --depth 1 https://github.com/thebenlamm/FAMP.git "$SRC"
cargo install --path "$SRC/crates/famp" --locked
```

Roughly 90 seconds. Ensure `~/.cargo/bin` is on `PATH`, then re-run
`famp --version`. Note that this builds whatever the default branch is at clone
time, which may be ahead of the latest release.

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
