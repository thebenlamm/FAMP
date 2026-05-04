# Migration: FAMP v0.8 -> v0.9

## CLI mapping table

| v0.8 | v0.9 | Notes |
|------|------|-------|
| `famp init --home <dir>` | `famp register <name>` | Identity bind via `~/.famp-local/agents/<name>/`; no per-identity HOME dir |
| `famp setup --name <n> --home <d> --port <p>` | `famp register <name>` | Single command; broker handles port/socket |
| `famp listen` | (gone) | Broker auto-spawns at `~/.famp/bus.sock` on first `famp register` |
| `famp peer add --alias <a> --endpoint <u> --pubkey <pk>` | (gone) | Same-host discovery is automatic via broker; use `famp send --to <name>` directly |
| `famp peer import` | (gone) | Peer cards are v1.0 federation-internal |
| `famp send --to <a> --new-task "<x>"` (TLS) | `famp send --to <name> --new-task "<x>"` (UDS) | Same syntax; bus under the hood |
| `FAMP_HOME=/tmp/a` env var | (no longer meaningful) | `~/.famp/` is the broker root |
| `FAMP_TOFU_BOOTSTRAP=1` | (no longer meaningful) | No TLS on the local bus |
| `famp mcp` with `FAMP_HOME=...` in `.mcp.json` | `famp mcp` (no env vars) | Register identity inside the MCP session via `famp_register` tool |

**Local-first bus replaces the federation TLS listener mesh for same-host agents.**
v0.9 ships a UDS-backed broker; cross-host messaging is deferred to v1.0
(`famp-gateway`, trigger-gated on the v1.0 readiness condition documented
in [ARCHITECTURE.md](../ARCHITECTURE.md)).

## TL;DR

- Run `famp install-claude-code` -- auto-rewrites your `.mcp.json` and drops new slash commands.
- Switch `famp setup` / `famp init` -> `famp register <name>`.
- `famp listen` is gone -- the broker auto-spawns on first `famp register`.
- `famp peer add` / `famp peer import` are gone -- same-host discovery is automatic via the broker.
- `famp send` keeps the same flag surface; only the transport changed.

## `.mcp.json` cleanup

`famp install-claude-code` does this for you. Manual cleanup, if you must:

1. Open `~/.claude.json` or your project-scope `.mcp.json`.
2. Find the `mcpServers.famp` entry.
3. Delete any `env` keys for `FAMP_HOME` or `FAMP_LOCAL_ROOT`.
4. Confirm `command` points to your installed `famp` binary, with `args: ["mcp"]`.

## `~/.famp/` directory cleanup (optional)

Legacy v0.8 artifacts under `~/.famp/` are no longer read by v0.9:

- `config.toml`
- `peers.toml`
- `cert.pem`
- `key.pem`
- old per-identity directories

They do not break anything. Delete them at your leisure:

```bash
# Inspect first.
ls ~/.famp/
# Then remove old per-identity dirs. Do not remove bus.sock or broker.log.
rm -rf ~/.famp/<old-identity-dir>
```

v0.9 uses `~/.famp-local/agents/<name>/` for per-identity state: mailboxes and
sessions. It uses `~/.famp/bus.sock` and `~/.famp/broker.log` for the broker.

## If you genuinely need federation today

The `v0.8.1-federation-preserved` git tag is an escape hatch for users who
need cross-host messaging via the v0.8 TLS listener mesh:

```bash
git checkout v0.8.1-federation-preserved
cargo install --path crates/famp
```

This tag is frozen. Do not expect bug fixes there. Bug fixes ship via the v1.0
federation gateway (`famp-gateway`) when the named v1.0 readiness trigger fires.

## For federation engineering reference

Federation-tagged tests are preserved under
[`crates/famp/tests/_deferred_v1/`](../crates/famp/tests/_deferred_v1/). They
are the starting test surface for `famp-gateway`'s integration suite when the
v1.0 federation milestone fires.

See
[`crates/famp/tests/_deferred_v1/README.md`](../crates/famp/tests/_deferred_v1/README.md)
for the freeze explainer and reactivation criteria.

## Workspace internals

`famp-transport-http` remains in the workspace as a v1.0 federation internal.
`famp-keyring` remains in the workspace as a v1.0 federation internal.
They compile and stay tested in `just ci` via the refactored
[`crates/famp/tests/e2e_two_daemons.rs`](../crates/famp/tests/e2e_two_daemons.rs)
integration test. No top-level CLI subcommand reaches them in v0.9.

## Archived prep-sprint scaffolding

The `scripts/famp-local` bash wrapper used during the v0.9 prep sprint has
been archived to
[`docs/history/v0.9-prep-sprint/famp-local/`](history/v0.9-prep-sprint/famp-local/).
It is frozen and superseded by the live `famp` binary plus the `famp-local hook`
subcommand. See ROADMAP backlog 999.6 for the open issue against the archived
script.
