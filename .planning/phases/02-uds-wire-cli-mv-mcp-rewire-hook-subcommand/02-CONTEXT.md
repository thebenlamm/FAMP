# Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand — Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Wrap the Phase 1 `famp-bus` library in a real wire (tokio UDS broker with `posix_spawn` + `setsid` + `bind()`-exclusion + 5-min idle exit), a real top-level CLI surface (`register`, `send`, `inbox list/ack`, `await`, `join`, `leave`, `sessions`, `whoami`), rewire `famp mcp` to talk to the bus instead of HTTPS (drops `reqwest`/`rustls`/`FAMP_HOME` from the MCP startup path), and ship Sofer's biggest leverage gap as a declarative `famp-local hook add/list/remove` subcommand.

**Scope-locked from ROADMAP.md:** `BROKER-01..05` (5), `CLI-01..11` (11), `MCP-01..10` (10), `HOOK-01..04` (4), `TEST-01..05` (5), `CARRY-02` — **36 requirements total**.

**Not in this phase:**
- `famp install-claude-code`, slash commands, the 12-line/30s README acceptance gate — Phase 3.
- Federation CLI removals (`famp setup`/`listen`/`init`/`peer add`/`peer import`/old TLS-form `send`), `e2e_two_daemons` library-API refactor, `v0.8.1-federation-preserved` tag, migration doc — Phase 4.
- Any cross-host transport, any crypto on the local path, any Agent Cards — out of scope (v1.0 Federation Profile).

**Carrying forward from Phase 1 (already locked, do NOT re-decide):**
- Pure broker actor: `Broker::handle(BrokerInput, Instant) -> Vec<Out>` is `total / infallible / time-as-input` (Phase 1 D-01..D-05). Phase 2 wire layer = thin `Out`-intent executor.
- `Out` ordering carries crash-safety semantics: `AppendMailbox` before `Reply(SendOk)`; `AdvanceCursor` after `Reply(RegisterOk)` (Phase 1 D-04). Wire layer MUST execute the vec in order.
- `MailboxName::Agent(String)` / `Channel(String)`; `#`-prefixed display form for channels (Phase 1 D-10).
- `BUS-11`: bus envelopes carry `sig: None`; `BusEnvelope<B>` and `AnyBusEnvelope` enforce this at the type level. No crypto on the bus.
- `BusErrorKind` is exhaustive (no wildcard); MCP-side error mapping is compile-checked match — adding a `BusErrorKind` variant fails compile until MCP handles it (`MCP-10` carry-forward).
- `MailboxRead` trait is read-only; all writes are `Out::AppendMailbox` intents. The wire layer owns cursor-file IO via `Out::AdvanceCursor` (Phase 1 D-07, D-11).
- `audit_log` MessageClass already shipped in v0.5.2; non-FSM-firing per Δ31. Phase 2 just propagates it through the wire (no new FSM transitions).

</domain>

<decisions>
## Implementation Decisions

### Identity Binding Model (CLI-01..08)

- **D-01: Hybrid precedence for non-register CLI commands.** When a command needs to know "who is sending", resolve in this order:
  1. `--as <name>` flag (highest priority; explicit override).
  2. `$FAMP_LOCAL_IDENTITY` env var.
  3. Canonical `$PWD` lookup in `~/.famp-local/wires.tsv` (existing prep-sprint mechanism, already implemented in `scripts/famp-local cmd_identity_of`).
  4. Hard error: `no identity bound — pass --as, set $FAMP_LOCAL_IDENTITY, or run \`famp-local wire <dir>\` first`.
  > **Rationale:** matches `feedback_identity_naming` rule (default + override path is non-optional); reuses Sofer-validated cwd→identity mechanism; preserves "explicit beats magic" for power users while keeping the cwd-mapped mesh ergonomic.

- **D-02: Hard error if `--as <name>` (or any resolved identity) is not currently registered.** Broker rejects with `BusErrorKind::NotRegistered`. CLI prints `<name> is not registered — start \`famp register <name>\` in another terminal first` and exits non-zero. No ephemeral auto-registration; no block-until-registered. Strictest possible model: the long-running `famp register <name>` process IS the identity; one-shots ride on it but never claim the slot themselves.

- **D-03: `~/.famp-local/wires.tsv` stays under `~/.famp-local/` (NOT moved to `~/.famp/`).** Keep the user-config dir (`wires.tsv`, `hooks.tsv`) separate from broker state (`bus.sock`, `mailboxes/`, `sessions.jsonl`, `broker.log`). Phase 4 will deprecate `~/.famp-local/` together with the bash script. Phase 2 keeps the existing `wires.tsv` semantics from prep-sprint T3 unchanged.

### MCP Server Rewire Architecture (MCP-01..10)

- **D-04: Hybrid rewire — keep the outer scaffolding, redesign the inner state.** Preserve `crates/famp/src/cli/mcp/server.rs` JSON-RPC dispatch loop and `crates/famp/src/cli/mcp/error_kind.rs` mapping unchanged. Reshape `crates/famp/src/cli/mcp/session.rs` significantly:
  - Drop `home_path: PathBuf` (and the `FAMP_LOCAL_ROOT` env-var read on startup).
  - Add a long-lived `bus: BusClient` field (single connection per MCP-server stdio process; opens on first tool call, kept alive for session lifetime).
  - Keep `active_identity: Option<String>` (the v0.8 bridge's session-bound model) — set by `famp_register`, read by every other tool.
  - Rewrite each `tools/*.rs` against the new session shape: `HttpTransport::send(envelope)` becomes `bus.send(BusMessage::Send{...})` etc.
  - Add `tools/join.rs` + `tools/leave.rs` (the 7th and 8th tools).
  > **Rationale:** lowest-risk diff that hits all 10 MCP requirements; keeps the proven JSON-RPC loop and the v0.8 bridge's `not_registered` gating intact; isolates the actual transport swap to the session struct + tool bodies.

- **D-05: `famp_register` is gating-required as the first tool call (preserved from v0.8 bridge).** All other tools (`famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) return `BusErrorKind::NotRegistered` (mapped to a structured JSON-RPC error) until `famp_register` succeeds. On `famp_register`, the MCP process IS the registered identity for the broker — its PID claims the slot per BROKER-03.

- **D-06: MCP `BusErrorKind` mapping is compile-checked exhaustive match (MCP-10).** Reuse the v0.8 pattern in `error_kind.rs`. Adding a `BusErrorKind` variant must fail compile in MCP error mapping until handled. Each variant maps to a stable JSON-RPC error code + message; Claude's discretion on the exact code numbers.

### Broker Socket Path & Test Isolation (BROKER-01..05, TEST-05)

- **D-07: `~/.famp/bus.sock` is the default; `$FAMP_BUS_SOCKET` env var overrides.** Single resolution point in `BusClient::connect()` (and the broker's `bind()` site). Production users never touch the env var; the TEST-05 two-stdio-MCP harness sets `$FAMP_BUS_SOCKET=$TMPDIR/test-bus.sock` to isolate from the user's real broker. Mirrors v0.8's `FAMP_LOCAL_ROOT` pattern.
  - **Open detail (researcher):** whether the broker's other state (mailboxes dir, sessions.jsonl, broker.log) follows the socket path's parent directory (`socket.parent()`) or needs its own `$FAMP_BUS_DIR` env var. Recommend deriving from socket parent for v0.9 simplicity; document.

### `famp register` Foreground UX (CLI-01)

- **D-08: Default = single startup status line, then silent block. `--tail` opt-in for live event stream.** On successful Hello+Register, print exactly one line to stderr (or stdout — researcher pick) like:
  ```
  registered as alice (pid 12345, joined: []) — Ctrl-C to release
  ```
  Then block silently. The `--tail` flag adds a live one-line-per-event tail (e.g. `< from bob: task=abc \"hi\"`). `--no-reconnect` (see D-09) opts out of auto-reconnect for tests/CI.
  > **Rationale:** matches the "process holds the identity slot, real interaction happens via MCP" mental model. Quiet-by-default keeps the terminal usable as a backgrounded slot-holder. Opt-in tail satisfies developers who want visibility. Default + override pattern.

- **D-09: Auto-reconnect with bounded exponential backoff on broker disconnect.** Backoff schedule: `1s → 2s → 4s → 8s → 16s → 30s → 60s` (cap at 60s). One-line stderr warning per retry: `broker disconnected — reconnecting in Ns`. On reconnect, redo Hello+Register; broker auto-spawns from any client per design spec §"Spawn"; mailbox re-drains atomically per Phase 1 D-04. `--no-reconnect` flag exits non-zero on first disconnect (for tests/CI).
  > **Rationale:** matches Unix-y daemon ergonomics; satisfies TEST-03's reconnect-after-kill-9 invariant; backoff curve prevents tight-loop on permanently-broken brokers; opt-out flag keeps tests deterministic.

### Identity-Binding Wire Protocol (CLI-01..08, MCP-01..10) — supersedes original Claude's-discretion item

- **D-10: Identity binding is a connection property, not a per-message field. Promote it to `Hello.bind_as: Option<String>`.** The previously-flagged Claude's-discretion item ("`BusMessage::Send { as: Option<String> }` additive field" / SendAs variant / ephemeral-register dance) is **REJECTED**. Per-message `as` fields scale poorly to 7 one-shot commands (inbox/await/join/leave/sessions/whoami/send) and conflate identity with payload. Locked shape:
  ```rust
  // crates/famp-bus/src/proto.rs
  BusMessage::Hello {
      bus_proto: u32,
      client: String,
      #[serde(skip_serializing_if = "Option::is_none")]
      bind_as: Option<String>,
  }
  ```
  Semantics — STRICT:
  - `bind_as: None` → normal unbound connection. Must `Register` before any identity-required op.
  - `bind_as: Some("alice")` → broker validates `alice` is currently held by a live long-running `famp register alice` process (same-UID trust). If not live, return `BusErrorKind::NotRegistered`. If live, the connection becomes a **proxy** to alice's identity for its lifetime.
  - **Proxy semantics** (load-bearing): a `bind_as` connection does NOT create a session row, does NOT own the name, does NOT register, does NOT auto-leave channels on disconnect. It is read/write-through to the canonical registered holder.
  - Operations resolve via `effective_identity(connection)`:
    - Registered holder → `ClientState.name` (its own).
    - `bind_as` proxy → the bound name (validated live at Hello-time; broker re-verifies on each op).
    - Otherwise → `BusErrorKind::NotRegistered`.
  - For `Join`/`Leave`: mutate the canonical live registered identity's `joined` state, NOT the short-lived proxy connection's. Otherwise `famp join --as alice #x` would auto-leave when the one-shot process exits — wrong.
  - Existing per-message variants (`Inbox`, `Await`, `Join`, `Leave`, `Sessions`, `Whoami`, `Send`) stay shape-stable. **No `as` / `send_as` field on any message.** Plan 02-04's `BusMessage::Send { send_as: Option<String> }` is dropped.
  > **Rationale:** identity is a connection property; this matches MCP's long-lived session model, scales uniformly to all one-shots, keeps the wire surface minimal, and preserves the strict "register process owns the slot" property from D-02.

- **D-11: MCP-01 audit is a source-import grep, not a `cargo tree` reachability proof.** `crates/famp/Cargo.toml` lists `reqwest`/`rustls` as plain `[dependencies]` and a feature split into a `federation` flag is deferred to Phase 4 (where `cli/listen`, `cli/init`, `cli/peer`, `cli/setup` are deleted wholesale, making the deps trivially unreachable). For Phase 2 the success criterion is satisfied by:
  ```bash
  # scripts/check-mcp-deps.sh
  ! grep -rE 'use (reqwest|rustls)' \
      crates/famp/src/cli/mcp/ \
      crates/famp/src/bus_client/ \
      crates/famp/src/broker/
  ```
  This proves no MCP/bus/broker source file imports the federation transports. The original ROADMAP §"Phase 2 success criterion 3" wording ("`cargo tree -p famp` shows reqwest/rustls are no longer reached from the MCP startup path") is reinterpreted accordingly — "reached from the MCP startup path" = reachable from MCP source via `use`, not via Cargo dependency resolution. Phase 4 will close the cargo-tree-strict reading.
  > **Rationale:** honest scoping. Forcing a feature split here couples Phase 2 to a Phase 4 deletion that's already planned. Source-import grep is testable today and fully captures the architectural property the milestone cares about.

- **D-12: HOOK-04 splits into HOOK-04a (registration, Phase 2) and HOOK-04b (execution runner, Phase 3).** REQUIREMENTS.md edits HOOK-04 → HOOK-04a (registration round-trip via add/list/remove with TSV row format) and adds HOOK-04b (a registered hook fires `famp send` to the configured peer/channel on a matching file-system event — implementation likely a Claude-Code Stop/Edit hook shim, not native Rust). ROADMAP §"Phase 2" requirements line `HOOK-01..04 (4)` becomes `HOOK-01..04a (4)`; ROADMAP §"Phase 3" adds `HOOK-04b`. Phase 2 still claims 36 reqs — the HOOK count is unchanged.
  > **Rationale:** HOOK-04's original wording said "fires `famp send`" — that's the runner, not the registration surface. Counting registration as HOOK-04 was honest-but-imprecise; splitting makes Phase 2 closure exact.

### Claude's Discretion (remaining)
- **Hook subcommand placement** — implement on `scripts/famp-local hook add/list/remove` per literal HOOK-01 spec wording. Phase 4 deprecates `scripts/famp-local` wholesale; migration of `hooks.tsv` reading to a native `famp hook` command happens then. Researcher should validate the bash-script complexity ceiling — if `hooks.tsv` parsing + Edit-event glob matching pushes `scripts/famp-local` past ~1500 LoC, reconsider implementing in the Rust binary directly. (`scripts/famp-local` is currently 1230 LoC.)
- **NFS detection mechanism (BROKER-05)** — `statfs` magic-number on Linux; `getmntinfo()` on macOS; cross-platform via `sys-info` / `fsinfo` crate. Researcher picks; warning fires once at startup, not blocking.
- **Idle-exit timer state machine specifics** — per design spec §"Idle exit", count 1→0 starts 5-min timer; new connection cancels. Concrete tokio implementation (interval vs sleep-future + cancellation token vs broker-internal `Tick` events) — researcher decides; must round-trip cleanly through the BROKER-04 integration test.
- **`--tail` output format** — Claude's discretion; lean toward one-line-per-event with `<` prefix for received and `>` prefix for sent (if any), `[YYYY-MM-DDTHH:MM:SSZ from=alice to=bob task=<uuid> body="..."]` shape.
- **JSON-RPC error code numbers for `BusErrorKind` variants** — researcher picks consistent block (e.g. -32100 to -32109).
- **`BusClient` connection retry on first connect** — when MCP starts and `bus.sock` doesn't exist, MCP-spawned client should `posix_spawn` the broker (per design spec §"Spawn" — "First CLI invocation or first MCP connection that finds no broker spawns one"). Keep the spawn logic in a shared module reusable from CLI and MCP entry points.
- **Reconnect backoff ceiling adjustment** — 60s upper bound is a guess; researcher may tune based on idle-exit timing (5min) and integration-test expected runtimes.
- **`Sessions` CLI output format** — JSON-Line per row (consistent with v0.8 `inbox list` precedent). `--me` filter to caller's resolved identity only.
- **`CARRY-02` (TD-3, INBOX-01 wording vs structured wrapper)** — recommend rewriting REQUIREMENTS.md INBOX-01 wording to match the raw-bytes-per-line implementation rather than introducing a structured `InboxLine` wrapper. Cheaper, accurate to behavior; matches Phase 1 D-09 wire-layer raw-bytes contract. Researcher can override with technical justification.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design authority
- `docs/superpowers/specs/2026-04-17-local-first-bus-design.md` — the v0.9 local-first bus design spec (506 lines). **§"Broker lifecycle / Spawn / Exclusion / Idle exit / Upgrade path"** (lines 202–225) is the authoritative source for `posix_spawn` + `setsid`, `bind()`-exclusion algorithm, 5-min idle timer, NFS warning. **§"Concurrency model"** (227–238) locks the single-threaded actor model. **§"Data flow"** (240–299) per-message Register/Send/Inbox/Await/Join/Leave/Sessions/Whoami flows. **§"Mailbox format"** (301–307) and **§"Sessions file"** (309–311) are mandatory file-format references. **§"CLI surface"** (313–329) and **§"MCP surface"** (331–354) are the v0.9 user-facing contract. **§"Phasing / Phase 2"** (401–411) is the exit criteria for this phase.
- `.planning/V0-9-PREP-SPRINT.md` — establishes T9 (`famp-local hook add` Sofer-driven scope addition).

### Spec & invariants
- `FAMP-v0.5.1-spec.md` (project root, amended in-place to v0.5.2 by commit `f44f3ee`) — wire format authority. Bus envelopes carry `sig: None` per BUS-11; the underlying envelope schema and canonical-JSON encoding are unchanged. Phase 2 does NOT amend the spec.
- `crates/famp-envelope/src/version.rs` — `FAMP_SPEC_VERSION = "0.5.2"`. URL path `/famp/v0.5.1/inbox/{principal}` intentionally NOT bumped (transport URL versioning is out of v0.9 scope; locked in Phase 1 STATE notes).

### Requirements
- `.planning/REQUIREMENTS.md` §BROKER, §CLI, §MCP, §HOOK, §TEST, §CARRY-02 — the 36 active requirements for this phase.
- `.planning/ROADMAP.md` §"v0.9 Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand" — five-bullet success-criteria block (shell-level usability end-to-end; single-broker exclusion provable at OS level; `famp mcp` connects to UDS not TLS, exhaustive `BusErrorKind` match; `famp-local hook add/list/remove` round-trip; INBOX-01 wording rewrite + full CI green).

### Phase 1 substrate (the artifact this phase wraps)
- `crates/famp-bus/src/` — Phase 1 library: `BusMessage`, `BusReply`, `Target`, `BusErrorKind`, `Delivered`, `SessionRow`, `Broker::handle(BrokerInput, Instant) -> Vec<Out>`, `MailboxRead` + `LivenessProbe` traits, length-prefixed canonical-JSON codec, `InMemoryMailbox`. Phase 2 MUST NOT modify the pure broker; only add the wire layer + disk-backed `MailboxRead` impl + `LivenessProbe` impl.
- `.planning/phases/01-famp-bus-library-and-audit-log/01-CONTEXT.md` — Phase 1 decisions D-01..D-19. **D-04 (Out ordering carries durability semantics), D-07 (writes-are-intents), D-11 (cursor file is wire-layer responsibility), D-09 (raw `Vec<Vec<u8>>` lines on the wire), D-10 (`MailboxName` agent/channel namespacing) are all hard constraints on Phase 2.**
- `.planning/phases/01-famp-bus-library-and-audit-log/01-VERIFICATION.md` — Phase 1 PASS evidence; carry-forward TD list.

### Reused crates (unchanged in Phase 2)
- `crates/famp-canonical/` — RFC 8785 byte-exact encode/decode; bus framing payload encoder.
- `crates/famp-core/` — `MessageClass` (incl. `AuditLog`), `TerminalStatus`, `ProtocolErrorKind`, identity types.
- `crates/famp-envelope/` — body-schema dispatch, decode pipeline, `BusEnvelope<B>` + `AnyBusEnvelope` (BUS-11 enforcement).
- `crates/famp-fsm/` — task FSM (5-state); receiver-side broker-auto-`REQUESTED` insertion happens here at `Send`-receive time.
- `crates/famp-inbox/` — raw-`&[u8]`-per-line append-only JSONL with fsync; tail-tolerant reader. **Disk-backed `MailboxRead` impl wraps this in Phase 2.**

### Existing v0.8 surface to evolve
- `crates/famp/src/bin/famp.rs` — top-level `famp` binary entry point.
- `crates/famp/src/cli/{send,inbox,await_cmd}/` — v0.8 CLI surfaces that get rewired from HTTPS-via-`famp listen` to UDS-via-`famp broker`. Subcommand argument shapes evolve to match design spec §"CLI surface".
- `crates/famp/src/cli/{init,listen,peer,setup,info,home.rs,paths.rs,perms.rs}` — federation surfaces. **Stay compiling and routable in Phase 2.** Removed in Phase 4 (FED-01).
- `crates/famp/src/cli/mcp/{server.rs, session.rs, error_kind.rs, tools/{await_,inbox,peers,register,send,whoami}.rs}` — existing 6/8 of v0.9 tool surface; rewire per D-04. Add `tools/join.rs`, `tools/leave.rs`. Drop the `home_path` / `FAMP_LOCAL_ROOT` field on session.
- `crates/famp/src/runtime/{adapter,error,loop_fn,peek}.rs` — runtime glue (envelope→FSM adapter, `peek_sender`, runtime error type). Reusable by the new bus path.
- `scripts/famp-local` (1230 LoC bash) — `wires.tsv` registry already in place (`cmd_wire`, `cmd_unwire`, `wires_lookup`). HOOK-01..04 add a new `cmd_hook_*` family of subcommands here per Claude's-discretion D in MCP rewire decisions section. Deprecated wholesale in Phase 4.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`scripts/famp-local cmd_identity_of` + `wires_lookup`** — D-01's cwd→identity tier is already implemented in bash. The Rust CLI's identity-resolver should mirror this exactly (read `~/.famp-local/wires.tsv`, exact-match canonical `$PWD`) so the bash and native paths agree. Researcher considers extracting the resolver into a small shared module (`crates/famp/src/cli/identity.rs`).
- **`crates/famp/src/cli/send/{client,fsm_glue,mod}.rs`** — v0.8 send already has the `--new-task`/`--task`/`--terminal` flag matrix wired (`conflicts_with`, `requires`); reuse the flag definitions and FSM-glue helpers, swap the transport from `HttpTransport` to a new `BusClient`.
- **`crates/famp/src/runtime/peek.rs`** — `peek_sender` already exists; bus path needs an analogous helper for stamping `from` on outbound `Send` messages.
- **`crates/famp/src/cli/mcp/error_kind.rs`** — existing error-mapping module; reuse the exhaustive-match enforcement pattern, just retarget at `BusErrorKind` instead of `RuntimeError`.
- **v0.8 bridge's `not_registered` gating in MCP** — preserved verbatim per D-05; no design change needed.

### Established Patterns
- **Exhaustive enums + `#![deny(unreachable_patterns)]` consumer stub** — `BusErrorKind` (Phase 1), `MessageClass`, `TerminalStatus`. MCP-10 and any other downstream `BusErrorKind` consumer (e.g. CLI error printer) must follow.
- **`#[serde(deny_unknown_fields)]` on every body struct** — applies to any new wire types Phase 2 introduces (e.g. potentially the optional `as` field on `BusMessage::Send`).
- **`#[arg(conflicts_with = ...)]` / `#[arg(requires = ...)]` for clap flag matrices** — v0.8 send precedent; reuse for `--as`, `--no-reconnect`, `--tail` flag interactions.
- **Atomic temp-file-rename for cursor writes** — Phase 1 D-11 + design spec §"Mailbox format". Reuse `famp-inbox`'s atomic-write helper (or equivalent) for `~/.famp/mailboxes/.<name>.cursor` writes.
- **JSONL-per-line CLI output** — v0.8 `inbox list` precedent. Apply to `sessions list`, `inbox list`, any other multi-record command.
- **`assert_cmd` for shelled CLI integration tests** — TEST-01..04 follow this pattern; v0.8 has substantial precedent in `crates/famp/tests/`.

### Integration Points
- `crates/famp/Cargo.toml` — add dependencies: `famp-bus` (Phase 1 crate), `tokio` (already present for v0.8), keep `reqwest`/`rustls` (Phase 4 removes them).
- `crates/famp/src/bin/famp.rs` — add `Broker` and (likely) `Register`, `Join`, `Leave`, `Sessions`, `Whoami` subcommand variants to the top-level `Command` enum. Existing `Send`, `Inbox`, `Await` keep their identifiers, evolve their internals.
- `crates/famp/src/cli/mcp/session.rs` — D-04 reshape target.
- `crates/famp/src/cli/mcp/tools/mod.rs` — register `join` and `leave` modules.
- `~/.famp-local/wires.tsv` — read-only consumer for the Rust identity resolver. `~/.famp-local/hooks.tsv` — new file written by the bash hook subcommand per HOOK-02.
- `~/.famp/bus.sock` — broker UDS path. `~/.famp/mailboxes/<name>.jsonl` + `~/.famp/mailboxes/.<name>.cursor` — disk-backed mailbox state. `~/.famp/sessions.jsonl` — diagnostic-only append-only sessions log. `~/.famp/broker.log` — broker stdout/stderr capture.

</code_context>

<specifics>
## Specific Ideas

- **Test-isolation env var named `$FAMP_BUS_SOCKET`** (D-07) — researcher should expose this name explicitly in `BusClient::connect()` documentation; mirrors `FAMP_LOCAL_ROOT` test-pattern from v0.8 bridge.
- **Reconnect backoff schedule `1s → 2s → 4s → 8s → 16s → 30s → 60s`** (D-09) — locked sequence; researcher may tune the cap upward (not down) if it conflicts with idle-exit (5min) timing in integration tests.
- **`--no-reconnect` flag exists on `famp register`** (D-09) — tests/CI use it for deterministic exit on disconnect.
- **`--tail` flag exists on `famp register`** (D-08) — opt-in live event stream; researcher picks line format (suggested: `<` for received, `[<timestamp> from=<name> to=<name> task=<uuid> body="..."]`).
- **`--as <name>` flag exists on every non-register CLI command** — `send`, `inbox list`, `inbox ack`, `await`, `join`, `leave`, `sessions` (when filtering by identity), `whoami`. Resolution order is D-01.
- **The cwd→identity reader reads `~/.famp-local/wires.tsv` directly** (D-01) — exact-match canonical `$PWD` (via `realpath`), tab-separated rows of `<canonical_dir>\t<identity>`. Mirror `scripts/famp-local cmd_identity_of` semantics byte-for-byte.
- **Hook subcommand lives on `scripts/famp-local hook add|list|remove`** (Claude's-discretion item) — bash implementation per literal HOOK-01 spec wording; deprecate alongside `scripts/famp-local` in Phase 4.

</specifics>

<deferred>
## Deferred Ideas

- **Hook subcommand placement on the native Rust binary** — not selected for this phase's discussion. Phase 4 cleanup decision: when `scripts/famp-local` is deprecated, the hook surface migrates to a native `famp hook` subcommand (or possibly a separate `famp-hook-runner` daemon). Defer until Phase 4 planning.
- **`--bus-socket <path>` explicit CLI flag** — rejected in favor of env-only override (D-07). Reconsider if a v0.9.x user reports needing per-invocation socket switching (extremely unlikely).
- **Auto-tail when `isatty(stdout)`** — rejected in favor of explicit `--tail` opt-in (D-08). Reconsider if developers complain about needing to remember the flag.
- **Ephemeral auto-registration for non-register sends** — rejected in favor of hard-error model (D-02). Reconsider only if a real workflow surfaces that requires send-without-register (none anticipated).
- **`block-until-registered` mode for `famp send`** — rejected in favor of hard-error (D-02).
- **`$FAMP_BUS_DIR` env var for full broker-state directory override** — researcher decides; lean toward deriving from socket parent. If the test harness needs separate dirs, revisit.
- **Per-tool JSON-RPC error code allocation table** — Claude's discretion; researcher pins down in PLAN.md. Not user-decidable.
- **Strict-mode reconnect** (refuse reconnect if broker version differs from initial connect's `bus_proto`) — defer; v0.9 has only one bus_proto version. Revisit when v2 broker exists.
- **CARRY-02 TD-3 INBOX-01 wording rewrite vs structured wrapper** — recommendation locked to "rewrite wording" (Claude's discretion); revisit in Phase 2 implementation if a structured wrapper falls out naturally.
- **Channel-name auto-prefix UX** (`--channel planning` vs `--channel #planning`) — not raised; researcher can pick. Lean: accept both, normalize to leading-`#` internally; reject `--channel planning planning` redundancy.
- **`famp register` startup-line target stream (stdout vs stderr)** — researcher picks; lean stderr (so `famp register alice 2>/dev/null` cleanly suppresses).
- **Channel-event broadcast on join/leave** (`ChannelEvent`) — already deferred to v0.9.1 per ROADMAP.
- **`famp mailbox rotate` / `famp mailbox compact`** — already deferred to v0.9.1.

</deferred>

---

*Phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand*
*Context gathered: 2026-04-28*
