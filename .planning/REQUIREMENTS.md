# Requirements: FAMP v0.9 Local-First Bus

**Milestone:** v0.9 Local-First Bus
**Opened:** 2026-04-27
**Source authority:** [`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](../docs/superpowers/specs/2026-04-17-local-first-bus-design.md) (506 lines, reviewed by `zed-velocity-engineer` + `the-architect`) + [`.planning/V0-9-PREP-SPRINT.md`](V0-9-PREP-SPRINT.md) T5/T9 scope additions.

**Acceptance criterion (overall):** Two Claude Code windows exchange a message in **≤12 lines of README and ≤30 seconds**.

**REQ-ID format:** `[CATEGORY]-[NUMBER]` — milestone-local numbering (FAMP convention; consistent with v0.6/v0.7/v0.8 reset-per-milestone REQ-IDs). Categories below are alphabetically grouped, not phase-grouped — phase mapping happens in `ROADMAP.md`.

---

## v0.9 Requirements (active)

### BUS — `famp-bus` library (Layer 1 substrate)

- [x] **BUS-01**: `famp-bus` crate exists in workspace; transport-neutral, no tokio in core, no I/O in pure broker state machine
- [x] **BUS-02**: `BusMessage` enum (tagged `op`, `snake_case`) with variants `Hello`, `Register`, `Send`, `Inbox`, `Await`, `Join`, `Leave`, `Sessions`, `Whoami` round-trips byte-exact through `famp-canonical`
- [x] **BUS-03**: `BusReply` enum (tagged `op`, `snake_case`) with variants `HelloOk`, `HelloErr`, `RegisterOk`, `SendOk`, `InboxOk`, `AwaitOk`, `AwaitTimeout`, `JoinOk`, `LeaveOk`, `SessionsOk`, `WhoamiOk`, `Err` round-trips byte-exact through `famp-canonical`
- [x] **BUS-04**: `Target` enum with variants `Agent { name }` and `Channel { name }`; channel names regex-validated `^#[a-z0-9][a-z0-9_-]{0,31}$`
- [x] **BUS-05**: `BusErrorKind` enum exhaustive (no wildcard, compile-checked match downstream): `NotRegistered`, `NameTaken`, `ChannelNameInvalid`, `NotJoined`, `EnvelopeInvalid`, `EnvelopeTooLarge`, `TaskNotFound`, `BrokerProtoMismatch`, `BrokerUnreachable`, `Internal`
- [x] **BUS-06**: Length-prefixed (4-byte big-endian) canonical-JSON frame codec; max frame size 16 MiB; sync (no tokio)
- [x] **BUS-07**: Pure broker state machine `Broker::handle(from: ClientId, msg: BusMessage) -> Vec<Out>` with no I/O — testable without UDS or runtime
- [x] **BUS-08**: Hello handshake required as first frame; bus_proto version negotiation with additive-compatibility intent for v2+ brokers
- [x] **BUS-09**: Single-threaded actor model — one tokio task owns broker state, mpsc inbox, no `RwLock` / `Mutex<HashMap>` on broker state
- [x] **BUS-10**: In-memory mailbox impl for tests
- [x] **BUS-11**: `Envelope` reuse — bus uses existing `famp_envelope::Envelope` unchanged; `sig` field MUST be `None` on the bus

### TDD — Phase-1 TDD gates (RED before GREEN)

- [x] **TDD-01**: Codec fuzz proptest — truncated reads, multi-`read()` split frames, `length == MAX + 1`, `length == 0`, partial length prefixes; classic length-confusion surface
- [x] **TDD-02**: Drain cursor atomicity proptest — append N envelopes, simulate `kill -9` mid-drain, restart, resume drain; assert no envelope lost (at-least-once: duplicates OK, losses NOT OK)
- [x] **TDD-03**: PID reuse race proptest — register `alice` with PID P1, P1 dies, OS reuses PID for unrelated P2; subsequent register of `alice` by P3 must NOT be rejected on the basis of P1/P2; broker probes liveness AND cross-checks `clients` map
- [x] **TDD-04**: EOF cleanup mid-await proptest — registered client starts `Await`, disconnects before matching `Send`; `pending_awaits` cleaned; subsequent matching `Send` queued for Inbox, not silently dropped

### PROP — Property-test coverage (proptest, beyond TDD gates)

- [x] **PROP-01**: DM fan-in ordering — N senders → 1 recipient, per-sender order preserved
- [x] **PROP-02**: Channel fan-out — 1 sender → M joined subscribers each receive exactly the set sent, no dupes
- [x] **PROP-03**: Join/leave idempotency — repeated joins/leaves don't corrupt member sets or channel mailboxes
- [x] **PROP-04**: Drain completeness — offline-then-online sequences deliver all queued envelopes in order
- [x] **PROP-05**: PID-table uniqueness — arbitrary alive/dead PID mixes preserve name uniqueness invariant

### AUDIT — `audit_log` MessageClass (v0.5.2 spec amendment, lagged constant)

- [x] **AUDIT-01**: `MessageClass::AuditLog` variant added to `famp-core` (or `famp-envelope` per existing layout); fire-and-forget semantics, non-FSM-firing
- [x] **AUDIT-02**: Body schema `event` (REQUIRED string) + `subject` (OPTIONAL string) + `details` (OPTIONAL object) — matches v0.5.2 §10 amendment
- [x] **AUDIT-03**: Receiver MUST store, MUST NOT emit `ack` (Δ31 normative; future "improvements" forbidden)
- [x] **AUDIT-04**: Optional causality `rel = "audits"` distinct from `"acknowledges"` (new `rel` value, not a re-purpose)
- [x] **AUDIT-05**: **Atomic version bump** — the same commit that adds the `MessageClass::AuditLog` enum variant + dispatch + body validation MUST bump `FAMP_SPEC_VERSION` from `"0.5.1"` → `"0.5.2"` in `crates/famp-envelope/src/version.rs`. Bumping in a separate commit either lies (if before impl) or strands impl as v0.5.1-tagged (if after) — Phase 1 closes the spec-vs-constant lag T5 intentionally introduced.
- [x] **AUDIT-06**: Doc-comment updated to remove the spec-vs-constant lag note once the bump lands

### BROKER — UDS daemon + lifecycle

- [ ] **BROKER-01**: `famp broker` subcommand wraps `famp-bus::Broker` with tokio UDS listener at `~/.famp/bus.sock`
- [ ] **BROKER-02**: Spawn via `posix_spawn` + `setsid` (detaches from terminal, survives Cmd-Q on Terminal.app); broker logs to `~/.famp/broker.log`. No double-fork.
- [ ] **BROKER-03**: Single-broker exclusion via `bind()` — `EADDRINUSE` → probe via `connect()`; live → exit 0; stale (`ECONNREFUSED`) → unlink + retry once. Socket IS the lock; no `flock`, no PID file.
- [ ] **BROKER-04**: Idle exit — connected-client count `1 → 0` starts a 5-minute timer; reconnection cancels; timeout triggers clean shutdown (close mailbox handles with fsync, unlink `bus.sock`, exit)
- [ ] **BROKER-05**: NFS-mounted `~/.famp/` warning at startup — `bind()` semantics on UDS depend on kernel; document local-FS requirement, surface a startup warning otherwise

### CLI — top-level `famp` user-facing CLI surface

- [ ] **CLI-01**: `famp register <name>` — registers identity + blocks (process = identity); spawns broker if not running
- [ ] **CLI-02**: `famp send --to <name>|--channel <#name> [--new-task <text>|--task <uuid>|--terminal] [--body <text>]`
- [ ] **CLI-03**: `famp inbox list [--since <offset>] [--include-terminal]` — terminal tasks hidden by default per v0.8 design
- [ ] **CLI-04**: `famp inbox ack [--offset <offset>]`
- [ ] **CLI-05**: `famp await [--timeout <dur>] [--task <uuid>]`
- [ ] **CLI-06**: `famp join <#channel>` / `famp leave <#channel>`
- [ ] **CLI-07**: `famp sessions [--me]` — reads broker in-memory state, not file
- [ ] **CLI-08**: `famp whoami` — returns `{active, joined}`
- [ ] **CLI-09**: Mailbox impl on disk reusing `famp-inbox` JSONL format with tail-tolerant reader (existing crate); `~/.famp/mailboxes/<name>.jsonl` and `~/.famp/mailboxes/<#channel>.jsonl`
- [ ] **CLI-10**: Drain cursor — `~/.famp/mailboxes/.<name>.cursor` written atomically (temp-file + rename) after successful drain ACK; at-least-once semantics on broker side
- [ ] **CLI-11**: `Sessions` file `~/.famp/sessions.jsonl` is append-only, diagnostic only — broker in-memory state wins; dead-PID rows filtered on read

### MCP — minimum-viable `famp mcp` rewire

- [ ] **MCP-01**: `famp mcp` connects to UDS bus instead of TLS — drops `reqwest`, `rustls`, `FAMP_HOME` reads from MCP startup path
- [ ] **MCP-02**: Tool `famp_register(name)` → `{active, drained, peers}`
- [ ] **MCP-03**: Tool `famp_send(to, envelope_fields)` → `{task_id, delivered}`
- [ ] **MCP-04**: Tool `famp_inbox(since?, include_terminal?)` → `{envelopes, next_offset}`; `include_terminal` defaults to `false` per v0.8 filter-terminal-tasks design
- [ ] **MCP-05**: Tool `famp_await(timeout_ms, task?)` → `{envelope}` or `{timeout: true}`
- [ ] **MCP-06**: Tool `famp_peers()` → `{online}`
- [ ] **MCP-07**: Tool `famp_join(channel)` → `{channel, members, drained}`
- [ ] **MCP-08**: Tool `famp_leave(channel)` → `{channel}`
- [ ] **MCP-09**: Tool `famp_whoami()` → `{active, joined}`
- [ ] **MCP-10**: MCP-side error-mapping layer is exhaustive `match` over `BusErrorKind` (no wildcard) — adding a `BusErrorKind` variant fails compile until MCP error mapping handles it (v0.8 pattern repeated)

### HOOK — `famp-local hook add` declarative subcommand (Sofer-driven scope addition)

- [ ] **HOOK-01**: `famp-local hook add --on <Event>:<glob> --to <peer-or-#channel>` declarative wiring; replaces hand-written bash hook scripts
- [ ] **HOOK-02**: Hook config persisted to `~/.famp-local/hooks.tsv` (or equivalent registry, consistent with `wires.tsv` precedent from prep sprint T3)
- [ ] **HOOK-03**: `famp-local hook list` and `famp-local hook remove <id>` round-trip
- [ ] **HOOK-04a** (Phase 2): A registered hook persists round-trip via `famp-local hook add/list/remove`. TSV row format `<id>\t<event>:<glob>\t<to>\t<added_at>` in `~/.famp-local/hooks.tsv`. (D-12 split — registration surface.)
- [ ] **HOOK-04b** (Phase 3): A registered hook fires `famp send` to the configured `<peer-or-#channel>` when a matching file-system event occurs. Implementation likely a Claude-Code Stop/Edit hook shim wired by `famp install-claude-code`, NOT the Rust binary. (D-12 split — execution runner.)

### TEST — integration + property test coverage at the wire

- [ ] **TEST-01**: 2-client DM round-trip integration test via shelled CLI (`assert_cmd`)
- [ ] **TEST-02**: 3-client channel fan-out integration test via shelled CLI
- [ ] **TEST-03**: Broker-crash recovery — `kill -9` broker mid-`Send`, client reconnects, mailbox drain recovers without loss
- [ ] **TEST-04**: Broker spawn race — two near-simultaneous CLI invocations; exactly one broker survives
- [ ] **TEST-05**: MCP E2E harness — two `famp mcp` stdio processes via test harness, JSON-RPC scripted from both sides, round-trip `new_task → commit → deliver → ack` over UDS (bus-side equivalent of v0.8's `e2e_two_daemons` over HTTPS)
- [ ] **TEST-06**: Conformance gates unchanged (`famp-canonical` RFC 8785, `famp-crypto` §7.1c) continue running on every CI run; envelope-type sharing means any regression surfaces in broker tests immediately

### CC — Claude Code integration polish

- [ ] **CC-01**: `famp install-claude-code` writes user-scope MCP config to `~/.claude.json` (or invokes `claude mcp add`) and drops slash-command markdown files into `~/.claude/commands/`
- [ ] **CC-02**: Slash command `/famp-register <name>` → `famp_register(name=...)`
- [ ] **CC-03**: Slash command `/famp-join <#channel>` → `famp_join(channel=...)`
- [ ] **CC-04**: Slash command `/famp-leave <#channel>` → `famp_leave(channel=...)`
- [ ] **CC-05**: Slash command `/famp-send <to> <body>` → `famp_send(to={kind:"agent",name:...}, new_task=body)` (DM convenience). Renamed from `/famp-msg` per Phase 3 D-05 (slash command names mirror CLI verbs exactly: `famp send` ↔ `/famp-send`).
- [ ] **CC-06**: Slash command `/famp-channel <#channel> <body>` → `famp_send(to={kind:"channel",name:...}, new_task=body)`
- [ ] **CC-07**: Slash command `/famp-who [#channel?]` → `famp_sessions` filtered
- [ ] **CC-08**: Slash command `/famp-inbox` → `famp_inbox`
- [ ] **CC-09**: README Quick Start rewrite hits the **12-line / 30-second acceptance test** on a fresh macOS install (Phase 3 exit gate)
- [ ] **CC-10**: Onboarding user-journey doc (`docs/ONBOARDING.md` or equivalent) — ships with v0.9.0 tag

### FED — federation CLI unwire + federation-CI preservation (plumb-line-2)

- [x] **FED-01**: Top-level CLI removals — `famp setup`, `famp listen`, `famp init`, `famp peer add`, `famp peer import`, old `famp send` (TLS form) — moved out of user-facing CLI
- [ ] **FED-02**: `famp-transport-http` + `famp-keyring` relabeled "v1.0 federation internals" in workspace `Cargo.toml` comments; remain compiling and tested
- [x] **FED-03**: **`e2e_two_daemons` refactored to library API** — no dependency on deleted CLI subcommands; instantiates two `famp-transport-http` server instances directly, exchanges full signed `request → commit → deliver → ack` cycle over HTTPS, verifies canonical JSON + Ed25519 end-to-end
- [ ] **FED-04**: Federation e2e test green in `just ci` on every commit (plumb-line-2 commitment against mummification)
- [ ] **FED-05**: Tag `v0.8.1-federation-preserved` cut on the commit BEFORE Phase 4 deletions land — escape hatch for federation-needed users
- [ ] **FED-06**: `cargo tree` shows federation crates are consumed only by the refactored e2e test, no top-level CLI usage

### MIGRATE — v0.8 → v0.9 migration documentation

- [ ] **MIGRATE-01**: `docs/MIGRATION-v0.8-to-v0.9.md` — CLI mapping table (`famp setup` → `famp register`, `famp listen` → gone, `famp peer add` → gone, etc.)
- [ ] **MIGRATE-02**: `.mcp.json` cleanup instructions — what to delete (legacy `FAMP_HOME`/`FAMP_LOCAL_ROOT` env vars), what to add (`famp install-claude-code` auto-update path)
- [ ] **MIGRATE-03**: `README.md`, `CLAUDE.md`, `.planning/MILESTONES.md` updated — local-first is the headline; federation is the v1.0 promise
- [ ] **MIGRATE-04**: `scripts/famp-local` (prep-sprint scaffolding) marked deprecated — superseded by native broker + CLI

### CARRY — v0.8 carry-forward debt addressed during v0.9

- [ ] **CARRY-01** (TD-1): `[[profile.default.test-groups]]` pinned for listen-subprocess tests (max-threads = 4) before listen subprocess tests proliferate further. Address in Phase 4 alongside `e2e_two_daemons` refactor.
- [x] **CARRY-02** (TD-3): REQUIREMENTS.md INBOX-01 wording rewritten to match the as-shipped wire shape. **Closed in Phase 2 (plan 02-12).** The inbox-draining wire format delivers **typed envelopes** — `BusReply::InboxOk { envelopes: Vec<serde_json::Value>, next_offset: u64 }`. Phase 1 D-09 evolved past raw `Vec<Vec<u8>>` on the wire to keep BUS-02/03 byte-exact canonical-JSON round-trip; the broker decodes each on-disk line via `AnyBusEnvelope::decode` before insertion into `envelopes`. The on-disk `mailboxes/<name>.jsonl` file format is still raw application bytes per line (Phase-1 D-09 file-shape contract). Consumers parse wire envelopes via `serde_json::from_value`; the structured per-line wrapper type rejected in the original CARRY-02 evaluation never shipped.
- [x] **CARRY-03** (TD-4): Broker auto-creates `REQUESTED` task record on inbound request (eliminates receiver-side test seed). Naturally absorbed by Phase 1 broker state-machine design.
- [x] **CARRY-04** (TD-7): Backfill Nyquist `VALIDATION.md` for v0.8 phases 02-04 + bridge phase, OR formally defer per project policy. Address inside Phase 1's TDD-gates pass.

---

## Future Requirements (deferred to v0.9.1+)

- **`famp mailbox rotate` / `famp mailbox compact`** — channel mailboxes grow unbounded in v0.9; acceptable because interactive developer usage won't hit the limit for weeks. v0.9.1 follow-up before any user complains.
- **`ChannelEvent` broadcast** on join/leave — v0.9 has no notification; defer to v0.9.1.
- **Heartbeat envelope class** (Phase 999.3) — see ROADMAP backlog.
- **`user_attention` envelope class** (Phase 999.4) — see ROADMAP backlog.
- **Crash-safety cursor advance flush** (Phase 999.1) — backlog.
- **Multi-listener lock semantics on concurrent consumers** (Phase 999.2) — backlog.

---

## Out of Scope (permanent or v1.0)

- **Cross-host messaging** — v1.0 (`famp-gateway`); v0.9 is single-host, single-user only
- **Cross-user-on-same-host messaging** — v1.x if ever; out of scope for v0.9
- **Any cryptography on the local bus** — no signing, no TLS, no TOFU, no keypairs (federation primitives stay as v1.0 internals)
- **Agent Cards, federation credentials, trust registry** — v1.0+ (Federation Profile)
- **Negotiation / counter-proposal / round limits** — v1.0+
- **Three delegation forms** (`assist`, `subtask`, `transfer`) + transfer timeout + delegation ceiling — v1.0+
- **Provenance graph** + redaction + signed terminal reports — v1.0+
- **Extensions registry** + critical/non-critical classification — v1.0+
- **Removing or breaking `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope`** — they are transport-neutral and remain unchanged in v0.9
- **Deleting `famp-transport-http` / `famp-keyring`** — they stay compiling in the workspace as v1.0 internals, unwired from top-level CLI
- **Conformance vector pack** (`WRAP-V0-5-1-PLAN.md`) — deferred to v1.0 alongside federation gateway; ships when a named second implementer commits to interop. CLAUDE.md "L2+L3 in one milestone" constraint revised in T6 to allow staged conformance.
- **Vector-pack interop with named second implementer (Sofer or equivalent)** — ships at v1.0 federation milestone (trigger-gated; 4-week clock starts at v0.9.0)
- **In-place broker upgrade coordination** — v0.9 uses `pkill famp-broker` + next-invocation-spawns-new pattern; socket-activated launchd version arrives in v1.0
- **Production deployment tooling** — out of scope; library-first

---

## Traceability

Phase mapping populated by `gsd-roadmapper` 2026-04-27. v0.9 phase numbering is milestone-local (FAMP convention; v0.7 reset to Phase 1, v0.8 reset to Phase 1, v0.9 resets to Phase 1).

| REQ-ID | Phase | Status |
|--------|-------|--------|
| BUS-01 | Phase 1 | Complete |
| BUS-02 | Phase 1 | Complete |
| BUS-03 | Phase 1 | Complete |
| BUS-04 | Phase 1 | Complete |
| BUS-05 | Phase 1 | Complete |
| BUS-06 | Phase 1 | Complete |
| BUS-07 | Phase 1 | Complete |
| BUS-08 | Phase 1 | Complete |
| BUS-09 | Phase 1 | Complete |
| BUS-10 | Phase 1 | Complete |
| BUS-11 | Phase 1 | Complete |
| TDD-01 | Phase 1 | Complete |
| TDD-02 | Phase 1 | Complete |
| TDD-03 | Phase 1 | Complete |
| TDD-04 | Phase 1 | Complete |
| PROP-01 | Phase 1 | Complete |
| PROP-02 | Phase 1 | Complete |
| PROP-03 | Phase 1 | Complete |
| PROP-04 | Phase 1 | Complete |
| PROP-05 | Phase 1 | Complete |
| AUDIT-01 | Phase 1 | Complete |
| AUDIT-02 | Phase 1 | Complete |
| AUDIT-03 | Phase 1 | Complete |
| AUDIT-04 | Phase 1 | Complete |
| AUDIT-05 | Phase 1 | Complete |
| AUDIT-06 | Phase 1 | Complete |
| CARRY-03 | Phase 1 | Complete |
| CARRY-04 | Phase 1 | Complete |
| BROKER-01 | Phase 2 | Pending |
| BROKER-02 | Phase 2 | Pending |
| BROKER-03 | Phase 2 | Pending |
| BROKER-04 | Phase 2 | Pending |
| BROKER-05 | Phase 2 | Pending |
| CLI-01 | Phase 2 | Pending |
| CLI-02 | Phase 2 | Pending |
| CLI-03 | Phase 2 | Pending |
| CLI-04 | Phase 2 | Pending |
| CLI-05 | Phase 2 | Pending |
| CLI-06 | Phase 2 | Pending |
| CLI-07 | Phase 2 | Pending |
| CLI-08 | Phase 2 | Pending |
| CLI-09 | Phase 2 | Pending |
| CLI-10 | Phase 2 | Pending |
| CLI-11 | Phase 2 | Pending |
| MCP-01 | Phase 2 | Pending |
| MCP-02 | Phase 2 | Pending |
| MCP-03 | Phase 2 | Pending |
| MCP-04 | Phase 2 | Pending |
| MCP-05 | Phase 2 | Pending |
| MCP-06 | Phase 2 | Pending |
| MCP-07 | Phase 2 | Pending |
| MCP-08 | Phase 2 | Pending |
| MCP-09 | Phase 2 | Pending |
| MCP-10 | Phase 2 | Pending |
| HOOK-01 | Phase 2 | Pending |
| HOOK-02 | Phase 2 | Pending |
| HOOK-03 | Phase 2 | Pending |
| HOOK-04a | Phase 2 | Pending |
| HOOK-04b | Phase 3 | Complete |
| TEST-01 | Phase 2 | Pending |
| TEST-02 | Phase 2 | Pending |
| TEST-03 | Phase 2 | Pending |
| TEST-04 | Phase 2 | Pending |
| TEST-05 | Phase 2 | Pending |
| CARRY-02 | Phase 2 | Pending |
| CC-01 | Phase 3 | Complete |
| CC-02 | Phase 3 | Complete |
| CC-03 | Phase 3 | Complete |
| CC-04 | Phase 3 | Complete |
| CC-05 | Phase 3 | Complete |
| CC-06 | Phase 3 | Complete |
| CC-07 | Phase 3 | Complete |
| CC-08 | Phase 3 | Complete |
| CC-09 | Phase 3 | Complete |
| CC-10 | Phase 3 | Complete |
| FED-01 | Phase 4 | Complete |
| FED-02 | Phase 4 | Pending |
| FED-03 | Phase 4 | Complete |
| FED-04 | Phase 4 | Pending |
| FED-05 | Phase 4 | Pending |
| FED-06 | Phase 4 | Pending |
| MIGRATE-01 | Phase 4 | Pending |
| MIGRATE-02 | Phase 4 | Pending |
| MIGRATE-03 | Phase 4 | Pending |
| MIGRATE-04 | Phase 4 | Pending |
| TEST-06 | Phase 4 | Pending |
| CARRY-01 | Phase 4 | Pending |

**Coverage:** 85/85 v0.9 active requirements mapped to exactly one phase. No orphans. (Future and Out-of-Scope sections do not require phase mapping.) Total bumped from 84 → 85 by the D-12 HOOK-04 split landed in plan 02-12: HOOK-04 → HOOK-04a (Phase 2) + HOOK-04b (Phase 3).
