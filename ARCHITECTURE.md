# Architecture

## Current state (v0.8)

**Federation-first.** Every agent is an independent identity with:
- A persistent `FAMP_HOME` directory containing its Ed25519 keypair, self-signed
  TLS cert, config, peers.toml, and durable inbox.
- A `famp listen` HTTPS daemon on a dedicated TCP port.
- A TOFU-pinned peer registry — first-contact pins the leaf-cert SHA-256;
  subsequent connections reject on mismatch.

All inter-agent traffic on the wire is signed over canonical JSON
(RFC 8785) with Ed25519 under the domain prefix `FAMP-sig-v1\0`, every
envelope, no exceptions (INV-10). Five message classes: `request`,
`commit`, `deliver`, `ack`, `control/cancel`. Five-state task FSM
(`famp-fsm`): REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED},
all terminal absorbing.

The Claude Code MCP server (`famp mcp`, stdio JSON-RPC) exposes
`famp_send`, `famp_inbox`, `famp_await`, `famp_peers` as tools, each
operating against the `FAMP_HOME` the MCP process was spawned with.

## v0.9 direction — local-first bus (in design)

Observed during dogfooding: forcing same-host, same-user agents to pay
federation-grade costs (cert generation, TOFU pinning, per-identity HOME
dirs, peer-card exchange) made basic onboarding require 8+ manual steps.
The filesystem is already the trust boundary between two processes owned
by one UID; running Ed25519 signatures and TLS handshakes between them
is theatre.

The v0.9 re-scope introduces a **local bus**:
- Unix domain socket broker (`~/.famp/bus.sock`), single process, all
  same-host agents share it.
- Zero crypto on the bus — no signing, no TLS, no TOFU.
- IRC-style channels (`#planning`) as a first-class primitive for 3+ agent
  broadcast.
- Durable per-name mailboxes (reuses `famp-inbox` format) so offline
  recipients queue rather than fail.
- Stable MCP tool surface (`famp_register`, `famp_send`, `famp_inbox`,
  `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) —
  the same surface that will gain transparent remote routing in v1.0.

**Layer split:**

| Layer | Scope | Crates | Wire | Crypto |
|---|---|---|---|---|
| 0 — Protocol primitives | Transport-neutral | `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` | N/A | N/A |
| 1 — Local bus (v0.9) | Same-host, same-UID | `famp-bus` (new), broker subcommand | UDS + canonical JSON framing | None |
| 2 — Federation gateway (v1.0) | Cross-host | `famp-gateway` (new), reuses `famp-transport-http`, `famp-keyring` | HTTPS + canonical JSON + Ed25519 | Full |

Layer 0 is untouched by v0.9. Layer 1 is net-new. Layer 2 is designed in
v0.9 but not built — its internals (`famp-transport-http`,
`famp-keyring`) stay compiling and tested in CI so they don't rot before
being wrapped.

Full v0.9 design:
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).

## Pre-v0.9 scaffolding

[`scripts/famp-local`](scripts/famp-local) is a bash wrapper over the v0.8
CLI that compresses the 8-step federation flow into one command
(`famp-local wire <dir>`) for same-host Claude Code agents. It auto-pins
TLS fingerprints from disk (bypassing TOFU), manages daemon lifecycles
with PID files, and drops project-scoped `.mcp.json` files. It exists to
validate the local-first UX before the v0.9 broker ships; when the
broker lands, the script becomes redundant.

## When working in the codebase

- **Protocol-primitive crates are transport-neutral.** `famp-canonical`,
  `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` — used by both
  v0.9 bus and v1.0 gateway. Changes here ripple everywhere.
- **Transport crates are federation-specific.** `famp-transport-http`,
  `famp-keyring` — will be wrapped by `famp-gateway` in v1.0, not by
  `famp-bus` in v0.9. Treat them as v1.0 internals.
- **The MCP tool surface is the stable contract** across v0.8, v0.9, and
  v1.0. If you find yourself changing tool signatures, stop — that's a
  cross-version UX decision.
