# Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-28
**Phase:** 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
**Areas discussed:** Identity binding model, MCP rewire architecture, `famp register` foreground UX
**Areas presented but not selected:** Hook subcommand placement (deferred to Claude's discretion — see CONTEXT.md)

---

## Identity binding model

How non-register CLI commands (`send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`) acquire the sending identity. Critical because `famp register <name>` blocks (process = identity), so other terminals running CLI commands need a way to claim "I am alice".

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid precedence | `--as <name>` flag → `$FAMP_LOCAL_IDENTITY` env → cwd lookup in `~/.famp-local/wires.tsv` → hard error. Reuses Sofer-validated cwd→identity mechanism; matches `feedback_identity_naming` rule (default + override). | ✓ |
| cwd-only + `--as` override | `wires.tsv` cwd lookup is the only auto-source; `--as` overrides; no env var. Simpler mental model. Risk: shells outside any wired dir hit a hard error. | |
| `--as` required everywhere | No auto-detection; every non-register command requires explicit `--as <name>`. Zero magic. Risk: ergonomic tax. | |
| env-only + `--as` override | `$FAMP_LOCAL_IDENTITY` only; no cwd magic. Risk: forgotten exports leak identity across repos. | |

**User's choice:** Hybrid precedence (Recommended)
**Notes:** Matches `feedback_identity_naming` saved rule. The cwd→identity tier reuses `scripts/famp-local cmd_identity_of` semantics already validated in the prep sprint. Researcher will mirror it byte-for-byte in the Rust resolver so bash and native paths agree.

---

### Identity binding follow-up — unregistered-send behavior

Window 2 runs `famp send --as alice --to bob "hi"`. If `alice` is NOT currently registered by any long-running process, what should happen?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard error | Broker rejects with `BusErrorKind::NotRegistered`. CLI prints `alice is not registered — start \`famp register alice\` in another terminal first`. Strictest, matches "process = identity" mental model. | ✓ |
| Auto-register ephemerally | Send opens connection, sends ephemeral `Register{name:alice, ephemeral:true}`, sends, disconnects. Most permissive UX. Risk: race with future real register; brief slot ownership. | |
| Block until alice registers | CLI waits up to N seconds for alice to appear in sessions table, then sends or times out. Awkward when alice was never going to register. | |

**User's choice:** Hard error (Recommended)
**Notes:** No ephemeral auto-registration; no block-until-registered. The long-running `famp register <name>` IS the identity; one-shots ride on it but never claim the slot themselves.

---

## MCP rewire architecture

How to restructure `crates/famp/src/cli/mcp/` to drop `reqwest`/`rustls`/`FAMP_HOME` and connect to the UDS bus instead. The existing module already has 6/8 of the v0.9 tool surface (`register`, `send`, `inbox`, `await_`, `peers`, `whoami`); need to add `join` + `leave`.

| Option | Description | Selected |
|--------|-------------|----------|
| Incremental swap | Keep all existing module structure; per-tool, replace `HttpTransport::send(envelope)` with `BusClient::send(BusMessage::Send{...})`. Lowest risk, smallest diff. | |
| Clean-room rewrite | New `crates/famp/src/cli/mcp_v09/` module structured around bus semantics from day one. Cleanest long-term boundaries, more code churn. | |
| Hybrid: keep loop, refactor session | Keep `server.rs` JSON-RPC loop + `error_kind.rs` mapping unchanged. Reshape `session.rs` significantly: drop `home_path`, add long-lived `BusClient`, keep `active_identity: Option<String>`. Rewrite `tools/*.rs` against the new session. Middle ground. | ✓ |

**User's choice:** Hybrid: keep loop, refactor session
**Notes:** Preserves the proven JSON-RPC dispatch loop and the v0.8 bridge's `not_registered` gating. Isolates the actual transport swap to the session struct + tool bodies. `tools/join.rs` + `tools/leave.rs` added as the 7th and 8th tools.

---

### MCP rewire follow-up — broker socket path & test isolation

How do MCP and CLI locate the broker socket? Matters for TEST-05's two-stdio-MCP E2E harness, which needs to isolate from the user's real broker.

| Option | Description | Selected |
|--------|-------------|----------|
| Env override | Default `~/.famp/bus.sock`; `$FAMP_BUS_SOCKET` env var overrides. Single resolution point in `BusClient::connect()`. Test harness sets the env var to a tempdir path; production users never touch it. Mirrors v0.8's `FAMP_LOCAL_ROOT` pattern. | ✓ |
| Hardcoded path | Strict `~/.famp/bus.sock` always. Tests share the default — risk of cross-test contamination unless serialized. | |
| Env + explicit flag | Env var override AND `--bus-socket <path>` flag on every CLI subcommand. Maximum flexibility, more surface to document. | |

**User's choice:** Env override (Recommended)
**Notes:** Single resolution point. Whether broker's other state (mailboxes dir, sessions.jsonl, broker.log) follows the socket's parent directory or needs its own `$FAMP_BUS_DIR` env var is left to researcher; lean toward deriving from `socket.parent()` for v0.9 simplicity.

---

## `famp register` foreground UX

CLI-01 says `famp register <name>` blocks (process = identity). What does the developer see in that terminal all day?

| Option | Description | Selected |
|--------|-------------|----------|
| Status line + opt-in `--tail` | Default: print one startup line (`registered as alice (pid 12345, joined: []) — Ctrl-C to release`), then block silently. `--tail` opt-in adds live one-line-per-event tail. Default + override pattern. | ✓ |
| Silent block | After handshake, nothing prints. Risk: user can't tell if register actually succeeded. | |
| Live event tail (always on) | Every received envelope prints a one-line summary. Risk: terminal becomes a busy log; bad for backgrounding. | |
| Auto-tail when stdout is a TTY | TTY: live tail; non-TTY (pipe / `&` / CI): silent block. Risk: surprising behavior shift. | |

**User's choice:** Status line + opt-in `--tail` (Recommended)
**Notes:** Matches "process holds the identity slot, real interaction happens via MCP" mental model. Quiet-by-default keeps the terminal usable as a backgrounded slot-holder. Opt-in tail satisfies developers who want visibility.

---

### `famp register` follow-up — broker disconnect behavior

If the broker dies (`pkill famp-broker`, `kill -9`, idle-exit miscalibrated) while `famp register alice` is running, what should the register process do?

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-reconnect with backoff | On disconnect, retry every 1s/2s/4s/8s/16s/30s/60s; one-line warning per retry on stderr; on reconnect, redo Hello+Register. Re-drains mailbox on new register. `--no-reconnect` opts out for tests/CI. | ✓ |
| Exit on disconnect | Print error, exit non-zero. User restarts manually. Brittle for long-running developer sessions. | |
| Hold + retry forever | Retry indefinitely with backoff capped at e.g. 30s. Most resilient, can mask permanently-broken state. | |

**User's choice:** Auto-reconnect with backoff (Recommended)
**Notes:** Matches Unix-y daemon ergonomics; satisfies TEST-03's reconnect-after-kill-9 invariant. Backoff curve: `1s → 2s → 4s → 8s → 16s → 30s → 60s` (locked). `--no-reconnect` flag exits non-zero on first disconnect for deterministic tests/CI.

---

## Claude's Discretion

The user explicitly opted out of discussing the **Hook subcommand placement** gray area. Resolution recorded in CONTEXT.md `<decisions>`:

- Implement `cmd_hook_*` family on `scripts/famp-local` per literal HOOK-01 spec wording (`famp-local hook add --on Edit:<glob> --to <peer-or-#channel>`). Phase 4 deprecates `scripts/famp-local` wholesale; migration to a native `famp hook` happens then. Researcher should validate the bash-script complexity ceiling — `scripts/famp-local` is currently 1230 LoC; if hook parsing pushes past ~1500 LoC, reconsider implementing in the Rust binary directly.

Additional researcher-decidable items captured in CONTEXT.md `<decisions>` "Claude's Discretion" subsection:

- Exact wire mechanism for `--as <name>` (lean: additive `BusMessage::Send { as: Option<String>, .. }` field, validated by same-UID trust)
- NFS detection mechanism (BROKER-05): `statfs` magic-number on Linux, `getmntinfo()` on macOS
- Idle-exit timer state machine (tokio interval vs sleep-future + cancellation token vs broker `Tick` events)
- `--tail` output line format
- JSON-RPC error code allocation per `BusErrorKind` variant
- Reconnect backoff ceiling fine-tuning vs idle-exit timing
- `Sessions` CLI output format (lean: JSON-Line per row, consistent with v0.8 `inbox list`)
- CARRY-02 TD-3 INBOX-01 wording rewrite vs structured wrapper (lean: rewrite wording)
- Channel-name auto-prefix UX (lean: accept both `--channel planning` and `--channel #planning`, normalize internally)
- `famp register` startup-line target stream (stdout vs stderr; lean stderr)
- `BusClient` first-connect spawn helper (shared module reusable from CLI and MCP)

## Deferred Ideas

Locked-out alternatives (do not revisit without new evidence):
- Ephemeral auto-registration for non-register sends — rejected per D-02.
- `block-until-registered` mode for `famp send` — rejected per D-02.
- Auto-tail when `isatty(stdout)` — rejected in favor of explicit `--tail` opt-in.
- `--bus-socket <path>` explicit CLI flag — rejected in favor of env-only override.

Deferred to later phases (already in ROADMAP backlog):
- Hook subcommand migration to native `famp hook` Rust binary — Phase 4 cleanup.
- `$FAMP_BUS_DIR` env var for full broker-state directory override — revisit if needed.
- Strict-mode reconnect (refuse if broker `bus_proto` differs from initial) — defer; only one bus_proto in v0.9.
- `ChannelEvent` join/leave broadcast — already deferred to v0.9.1 per ROADMAP.
- `famp mailbox rotate` / `famp mailbox compact` — already deferred to v0.9.1.
- Crash-safety cursor advance flush (Phase 999.1) — backlog.
- Multi-listener lock semantics on concurrent consumers (Phase 999.2) — backlog.
- Heartbeat envelope class (Phase 999.3) — backlog.
- `user_attention` envelope class (Phase 999.4) — backlog.
