# Phase 1: Broker Diagnosis & Identity Inspection - Context

**Gathered:** 2026-05-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Three new crates (`famp-inspect-proto`, `famp-inspect-client`, `famp-inspect-server`) plus `famp inspect broker` and `famp inspect identities` end-to-end — RPC, server handlers, and CLI rendering — shipped in a single merge that closes the orphan-listener incident class.

</domain>

<spec_lock>
## Requirements (locked via SPEC.md)

**16 requirements are locked.** See `01-SPEC.md` for full requirements, boundaries, and acceptance criteria.

Downstream agents MUST read `01-SPEC.md` before planning or implementing. Requirements are not duplicated here.

**In scope (from SPEC.md):**
- `BusMessage::Inspect { kind, ... }` variant added to `famp-bus/proto.rs` with codec round-trip + `deny_unknown_fields` parity
- `famp-inspect-proto` crate (RPC types, no I/O) — InspectKind + 4 request/reply struct pairs
- `famp-inspect-client` crate (UDS client, no clap)
- `famp-inspect-server` crate (handlers for Broker + Identities; Tasks + Messages return `NotYetImplemented`)
- `famp inspect broker` end-to-end (RPC dispatch + 4 connect-handshake-based down-states + CLI rendering + `--json`)
- `famp inspect identities` end-to-end (RPC dispatch + in-memory BrokerState read + mailbox metadata via BrokerEnv + CLI table + `--json`)
- `just check-no-io-in-inspect-proto`, `just check-inspect-readonly`, `just check-inspect-version-aligned` recipes wired into `just ci`

**Out of scope (from SPEC.md):**
- `famp inspect tasks` / `famp inspect messages` end-to-end (Phase 2)
- 500ms latency budget and cancellable-handler discipline (Phase 2)
- Load test / starvation proof (Phase 3)
- Orphan-listener E2E integration scenario (Phase 3)
- Migration docs (Phase 3)
- Mutation tools, browser SPA, per-identity double-print counter (v0.10.x+)

</spec_lock>

<decisions>
## Implementation Decisions

### Client cwd Tracking (INSP-IDENT-01)
- **D-01:** Extend `Register { name: String, pid: u32 }` with `cwd: Option<String>` using `#[serde(default, skip_serializing_if = "Option::is_none")]` — client sends its own cwd; broker stores it in `ClientState`. Follow the exact field pattern used by `Hello.bind_as` and `Inbox.since` for backward compat.
- **D-02:** Capture cwd at register time and never refresh. If the client `chdir`'s after registering, the inspect row reflects where the agent was born. Document in the Register field doc comment.
- **D-03:** Add `cwd: Option<String>` to `ClientState` (alongside existing `name`, `pid`, `bind_as` fields). Populated in the `Register` handler arm in `Broker::handle()`.

### ORPHAN_HOLDER Pid Discovery (INSP-BROKER-03)
- **D-04:** Use `SO_PEERCRED` / `LOCAL_PEERPID` first (platform-specific but in-process, zero subprocess): Linux uses `SO_PEERCRED` returning a `ucred` struct; macOS uses `getsockopt(SOL_LOCAL, LOCAL_PEERPID)` returning a `pid_t`. Fall back to `lsof -U <socket_path>` (macOS) or `ss -lx` / `/proc/net/unix` (Linux) subprocess when the socket option fails or returns 0.
- **D-05:** Surface the discovery method in the evidence row as a `pid_source` field: `peercred`, `lsof`, or `unknown`. Helps operators know whether to trust the PID during an incident. Return type: `(Option<u32>, PidSource)` from a single `peer_pid(socket_path) -> Result<(Option<u32>, PidSource), _>` function in `famp-inspect-client`.

### `famp inspect tasks` / `messages` CLI in Phase 1
- **D-06:** These sub-subcommands are **absent from the Phase 1 CLI**. `famp inspect tasks` returns `unrecognized subcommand` until Phase 2 adds the commands. The proto-level `InspectKind::Tasks` and `InspectKind::Messages` variants are internal forward-compat seams — they do not surface in `--help` in Phase 1. Adding subcommands in Phase 2 is a non-breaking change; shipping stubs would calcify their flag shape before the server answers.

### broker started_at Tracking (INSP-BROKER-01)
- **D-07:** Add `started_at: std::time::SystemTime` to `BrokerState`, set via `SystemTime::now()` in the `BrokerState` constructor (or broker startup path). `BrokerState` currently derives `Default` — add an explicit `BrokerState::new()` or move to a `Default` impl that captures startup time.
- **D-08:** Do not use socket file mtime. Socket mtime reflects file creation, not process startup; it lies after restart-with-reused-socket, `touch` from external tools, and filesystem quirks. `started_at` must be set by the process that's answering.

### Claude's Discretion
- Concrete cargo-tree test form for `check-inspect-version-aligned` (INSP-CRATE-03 Acceptance) — SPEC defers this to plan-phase.
- Whether `BrokerState` moves from `derive(Default)` to an explicit `new()` constructor or keeps `Default` with a custom impl — both satisfy D-07; planner picks the cleanest Rust shape.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Locked Requirements
- `.planning/phases/01-broker-diagnosis-identity-inspection/01-SPEC.md` — 16 locked requirements, down-state definitions, crate architecture, acceptance criteria, and the Phase 1↔2 cut rationale. Read this first.

### Protocol Wire Shape
- `crates/famp-bus/src/proto.rs` — `BusMessage` enum; the new `Inspect` variant MUST follow the existing `#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]` pattern. Also contains `BusReply` and existing codec round-trip tests that the `Inspect` variant's property test must mirror.

### State Structs to Extend
- `crates/famp-bus/src/broker/state.rs` — `ClientState` (add `cwd: Option<String>`) and `BrokerState` (add `started_at: SystemTime`). Confirms existing field shapes; D-01 and D-07 both require additions here.

### Broker Handler (Dispatch Mount Point)
- `crates/famp/src/bus_client/mod.rs` — UDS client used by existing CLI commands; `famp-inspect-client` follows this pattern.
- `crates/famp-bus/src/` — broker handle logic where `BusMessage::Inspect` dispatch arm goes. Read to understand the existing dispatch pattern before adding the new arm.

### CLI Convention
- `crates/famp/src/cli/mod.rs` — `Subcommand` enum and module declarations; `inspect` module plugs in here.
- `crates/famp/src/cli/register.rs` — exemplar for the Register message extension (D-01). Read to understand how `Register` args are currently constructed and how to add `cwd`.

### CI Gate Pattern
- `justfile` — existing `ci:` target line and `check-no-tokio-in-bus` recipe (lines 121+, 146); three new `check-*` recipes follow this exact pattern and wire into the same `ci:` target line.

### Workspace Dependency Version Alignment
- `Cargo.toml` (root) — `[workspace.dependencies]` section; `famp-canonical`, `famp-envelope`, `famp-fsm` are already listed. New inspector crates MUST use `workspace = true` for these deps to satisfy INSP-CRATE-03 and `check-inspect-version-aligned`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/famp/src/bus_client/mod.rs` — existing UDS connect + codec; `famp-inspect-client` reuses this pattern (connect, Hello, send frame, decode reply). Read before designing the client crate's API.
- `crates/famp/src/cli/sessions.rs` — existing tabular output example; `famp inspect identities` table format follows this precedent for column alignment and header printing.
- `crates/famp-bus/src/proto.rs` round-trip property tests (lines 250+) — `famp-inspect-proto`'s codec test must be a parallel sibling of these.

### Established Patterns
- `#[serde(default, skip_serializing_if = "Option::is_none")]` on optional wire fields — used by `Hello.bind_as`, `Inbox.since`, `Await.task`. D-01's `Register.cwd` follows this exact pattern.
- `#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]` on `BusMessage` — the new `Inspect` variant must use identical serde attributes.
- `pub(super)` visibility for internal broker types (`ClientState`, `BrokerState`) — `famp-inspect-server` must access state via a broker-owned dispatch function, not by reaching into internals directly.
- Workspace-level dep declaration: `famp-canonical = { version = "...", path = "crates/famp-canonical" }` in `[workspace.dependencies]`, then `famp-canonical.workspace = true` in each crate's `Cargo.toml`.

### Integration Points
- `Broker::handle()` in `crates/famp-bus/src/broker/` — new `BusMessage::Inspect { kind, ... }` match arm dispatches to `famp_inspect_server::dispatch(&state, kind)`.
- `crates/famp/src/cli/mod.rs` `Subcommand` enum — `Subcommand::Inspect(InspectArgs)` wired in; `inspect/mod.rs` + `inspect/broker.rs` + `inspect/identities.rs` follow the existing subcommand module layout.
- `justfile` `ci:` target — three new recipe names appended in the same space-separated list.

</code_context>

<specifics>
## Specific Ideas

- **`PidSource` enum in evidence row:** Matt recommended surfacing how holder_pid was discovered (`peercred | lsof | unknown`) in the ORPHAN_HOLDER evidence row — gives operators confidence in the PID during an incident (D-05).
- **`peer_pid()` function signature:** `peer_pid(socket_path: &Path) -> Result<(Option<u32>, PidSource), _>` — a single platform-conditional function in `famp-inspect-client` that abstracts the SO_PEERCRED / lsof forking. The rest of the diagnostic path is platform-agnostic.
- **Register.cwd documentation:** The field doc comment on `Register.cwd` should explicitly note that this is the client's cwd at registration time and is never updated — guards against a well-intentioned refresh in a future contribution.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-broker-diagnosis-identity-inspection*
*Context gathered: 2026-05-10*
