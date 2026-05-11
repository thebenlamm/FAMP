# Migration: FAMP v0.9 -> v0.10

## CLI mapping table

| v0.9 | v0.10 | Notes |
|------|-------|-------|
| (no inspect surface) | `famp inspect broker` | New; exits 0 on `HEALTHY`, exits 1 with state label + evidence row on stdout for all four down-states |
| (no inspect surface) | `famp inspect identities` | New; lists registered session identities + mailbox stats |
| (no inspect surface) | `famp inspect tasks` | New; task FSM state + envelope chain grouped by task_id (with `--orphans` bucket for task_id == 0) |
| (no inspect surface) | `famp inspect messages --to <name>` | New; mailbox envelope metadata only -- never message bodies |

**Read-only observability replaces grep-and-guess for the v0.9 broker.**
v0.10 adds a `famp.inspect.*` RPC namespace on the existing `~/.famp/bus.sock`
UDS (no new socket, no new transport) consumed by a single new CLI subcommand:
`famp inspect`. There are no breaking changes to existing v0.9 CLI surface.

## TL;DR

- `famp inspect broker / identities / tasks / messages` are the new read-only observability surface.
- All four subcommands accept `--json` for machine-readable output (stable, documented shapes -- see below).
- `famp inspect broker` is the one command that works even when the broker is dead, with a connect-handshake-based diagnosis (no PID file).
- No breaking changes to v0.9 CLI: `register`, `send`, `broker`, `mcp`, `inbox`, `await`, `peers`, `join`, `leave`, `whoami`, `install-claude-code` all unchanged.

## New surface: `famp inspect`

Four sub-subcommands, all read-only:

- `famp inspect broker` -- broker health + dead-broker diagnosis
- `famp inspect identities` -- registered session identities + mailbox stats
- `famp inspect tasks [--id <task_id>] [--full] [--orphans]` -- FSM state + envelope chain
- `famp inspect messages --to <name> [--tail N]` -- mailbox envelope metadata (default `--tail 50`)

Every subcommand accepts `--json` for piping to `jq`, CI assertions, or future
non-CLI consumers (a SPA or `famp doctor`, both deferred -- see below).

Default human-readable output is a fixed-width column-aligned table with
explicit headers. No Rust `Debug` format.

When the broker is not running, every `famp inspect` subcommand other than
`broker` exits 1 with stderr `error: broker not running at <socket-path>`
(no stack trace, no retry loop). `famp inspect broker` is the one command
that must produce a useful diagnosis against a dead broker.

## `famp inspect broker` -- down-broker states

Detection is **connect-handshake-based** (v0.9 has no PID file; `bind()` is the
single-broker lock). The state row plus its evidence is always printed to
stdout (never stderr) so a caller can capture both verdict and evidence in
one stream.

| State | Exit | Detection | Evidence row |
|-------|------|-----------|--------------|
| `HEALTHY` | 0 | `connect()` succeeds; FAMP `Hello` handshake succeeds | `pid=<u32> socket_path=<str> started_at=<ts> build_version=<str>` |
| `DOWN_CLEAN` | 1 | No socket file at the expected path | `socket_path=<str> exists=false` |
| `STALE_SOCKET` | 1 | Socket file exists but `connect()` returns `ECONNREFUSED` | `socket_path=<str> connect_errno=ECONNREFUSED` |
| `ORPHAN_HOLDER` | 1 | `connect()` succeeds but Hello rejected (non-FAMP holder) | `holder_pid=<u32 or "unknown"> pid_source=<SO_PEERCRED|LOCAL_PEERPID|lsof|none>` |
| `PERMISSION_DENIED` | 1 | `connect()` fails with `EACCES` | `socket_path=<str> connect_errno=EACCES` |

`ORPHAN_HOLDER` is the v0.9 production-incident class: a non-FAMP process
(stale `nc -lU` from a debug session, a stray test fixture, a different
program reusing the same path) holds `bus.sock`. The PID is reported via
`SO_PEERCRED` / `LOCAL_PEERPID` when the kernel provides it, falling back
to `lsof` when not. If the PID truly cannot be discovered, the row says
`holder_pid=unknown pid_source=none reason=<short>` -- the field is never
silently omitted.

## `--json` shape commitments

`famp inspect broker --json` (HEALTHY only -- the four down-states use the
text-only format above):

```
{"pid": <u32>, "socket_path": "<str>", "started_at_unix_seconds": <u64>, "build_version": "<str>", "state": "HEALTHY"}
```

`famp inspect identities --json`:

```
{"rows": [{"name": "<str>", "listen_mode": <bool>, "cwd": "<str>", "registered_at_unix_seconds": <u64>, "last_activity_unix_seconds": <u64|null>, "mailbox_unread": <u64>, "mailbox_total": <u64>, "last_sender": "<str|null>", "last_received_at_unix_seconds": <u64|null>}]}
```

`famp inspect tasks --json` is a tagged enum with `"kind"` in `"list" | "detail" | "detail_full" | "budget_exceeded"`. `--full` mode emits each envelope in canonical JCS (RFC 8785) form so that piping the output through `jq` reproduces the exact bytes that fed the signature input.

`famp inspect messages --json` is a tagged enum with `"kind"` in `"list" | "budget_exceeded"`.

The proto shapes are defined in `crates/famp-inspect-proto/src/lib.rs` and
are version-aligned with the broker via the `famp-inspect-server` crate's
workspace dependency pin (no Cargo-resolved version skew between the
inspector and the broker that wrote the envelopes being decoded).

## Read-only discipline

Every `famp.inspect.*` handler is read-only, enforced by two complementary
mechanisms:

1. **Compile-time:** Handler signatures take `&BrokerState` (shared
   borrow), never `&mut BrokerState`. The borrow checker rejects any
   mutation at compile time. This is not a property test; it is a type
   signature.
2. **Build-time:** The `just check-inspect-readonly` recipe fails CI if
   `famp-inspect-server` transitively imports any mailbox-write,
   taskdir-write, or broker `&mut self` mutation surface. Parallel to
   `just check-no-tokio-in-bus` for `famp-bus`, and
   `just check-no-io-in-inspect-proto` for `famp-inspect-proto`.

What this means for you as an operator: `famp inspect <anything>` cannot
mutate broker state, drop messages, advance the FSM, or modify mailboxes.
It is safe to run at any rate against a production broker; the inspector
explicitly refuses to be a mutation tool. (When mutation is eventually
needed, it ships under `famp doctor`, deferred -- see below.)

## No-starvation commitment

Bus message throughput under saturating `famp.inspect.*` load stays at
>= 80% of unloaded baseline (INSP-RPC-05). Verified by the
`inspect_load_does_not_starve_bus_messages` integration test
(`crates/famp/tests/inspect_load_test.rs`). The inspect dispatch path
runs under `spawn_blocking` + 500 ms timeout (INSP-RPC-03), so a
runaway inspect handler is dropped at the tokio wrapper layer with a
`BudgetExceeded` reply rather than stalling the bus event loop.

## Deferred items (not in v0.10)

The following are deliberately out of scope for v0.10. They appear in
`.planning/REQUIREMENTS.md` as v2 requirements:

- **`--body` flag for message bodies** (`INSP-MSG-BODY-01`). Body fetch
  overlaps with reading the on-disk mailbox file directly during a
  v0.10-era incident. Adds in v0.10.x only if observed CLI usage shows
  operators reach for it.
- **`famp doctor`** (`INSP-DOCTOR-01`). The read-only inspector must tell
  us *which* mutations operators reach for before we ship a mutation
  surface. Gated on ~2 weeks of CLI use.
- **Browser SPA / SSE event stream** (`INSP-SPA-01`). CLI is expected to
  cover ~70% of the observability pain. SPA reconsidered only if CLI use
  after ~2 weeks shows the gap.
- **Per-identity double-print counter** (`INSP-DBLPRINT-01`). Wrong
  instrument. The double-print failure mode (wake-up notification + inbox
  fetch each carrying the body, doubling token cost) is observable only
  at the model boundary, not the broker. A broker-side counter would
  mislead users and outlive the diagnostic that retires it. The right
  surface is token-attribution at the MCP boundary, a separate
  investigation.

## What's NOT changing in v0.10

- v0.9 broker (`famp-bus`, `~/.famp/bus.sock`, posix_spawn+setsid lifecycle, bind()-IS-the-lock single-broker exclusion) is the substrate v0.10 mounts on. No broker-side rewrites.
- 8-tool stable MCP surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) carried forward unchanged. v0.10 does **not** add MCP tools.
- `FAMP_SPEC_VERSION = "0.5.2"` unchanged. v0.10 does not require a spec amendment.
