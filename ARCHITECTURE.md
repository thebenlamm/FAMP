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

The MCP server (`famp mcp`, stdio JSON-RPC) exposes six tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_inbox`, `famp_await`,
`famp_peers`. The server starts **unbound** — identity is not read from
`FAMP_HOME` at startup. Each window calls `famp_register` once at session
start to bind an identity by name; the name resolves to
`$FAMP_LOCAL_ROOT/agents/<name>/` (default `~/.famp-local/agents/<name>/`).
Pre-registration calls to `famp_send`, `famp_inbox`, `famp_await`, and
`famp_peers` return a typed `not_registered` error. `famp_whoami` reports
the current binding and never errors. `famp_inbox` action=list hides
entries for tasks that reached a terminal FSM state (opt back in with
`include_terminal: true`); `famp_await` stays unfiltered and is the
canonical real-time signal for task completion.

Note: the federation transport side (`famp listen`, `famp setup`,
`famp send`, `famp peer import`) still reads `FAMP_HOME` per identity —
each identity's keypair, TLS cert, and durable inbox live under that
directory. The bifurcation (MCP session-bound; federation `FAMP_HOME`-based)
is intentional and collapses when v0.9's local bus replaces the transport.

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
- Stable MCP tool surface — v0.9 inherits `famp_register`, `famp_whoami`,
  `famp_send`, `famp_inbox`, `famp_await`, `famp_peers` unchanged from
  v0.8.x (where `famp_register` and `famp_whoami` first shipped as part
  of the session-bound identity bridge phase). v0.9 adds `famp_join` and
  `famp_leave` for IRC-style channel support; the register/whoami contract
  is not altered. The full v0.9 surface:
  `famp_register`, `famp_whoami`, `famp_send`, `famp_inbox`, `famp_await`,
  `famp_peers`, `famp_join`, `famp_leave` — the same contract that will
  gain transparent remote routing in v1.0.

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
(`famp-local wire <dir>`) for same-host MCP clients. It auto-pins TLS
fingerprints from disk (bypassing TOFU), manages daemon lifecycles with
PID files, drops project-scoped `.mcp.json` files for Claude Code, and
can register user-scope MCP entries for Codex. That Codex path is global
per user (`~/.codex/config.toml`), not repo-scoped like Claude Code's
`.mcp.json`. It exists to validate the local-first UX before the v0.9
broker ships; when the broker lands, the script becomes redundant.

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
