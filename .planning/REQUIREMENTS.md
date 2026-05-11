# Requirements: FAMP — v0.10 Inspector & Observability

**Defined:** 2026-05-09
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later. v0.10 makes that substrate's runtime state legible to the operator running it.

**Milestone goal:** Make conversation state in the v0.9 broker observable without grep-and-guess via a read-only inspector RPC + `famp inspect` CLI consumer.

## v1 Requirements

Requirements for v0.10 release. Each maps to a roadmap phase below.

### Broker Health Diagnostics — `famp inspect broker`

- [ ] **INSP-BROKER-01**: Operator can run `famp inspect broker` against a running broker and receive `state: HEALTHY` plus pid, socket path, started-at timestamp, and build version on a single line of human-readable output.
- [ ] **INSP-BROKER-02**: Operator can run `famp inspect broker` against a non-running broker and receive a state of `DOWN_CLEAN`, `STALE_SOCKET`, `ORPHAN_HOLDER`, or `PERMISSION_DENIED` (exactly one), plus the evidence row used to decide (socket path presence, connect() result, Hello handshake result, current socket-holder PID, fs permissions). Detection is connect-handshake-based — v0.9 uses bind()-exclusion with no PID file: `DOWN_CLEAN` = no socket file; `STALE_SOCKET` = socket present but `connect()` returns `ECONNREFUSED`; `ORPHAN_HOLDER` = `connect()` succeeds but Hello rejected; `PERMISSION_DENIED` = `EACCES`.
- [ ] **INSP-BROKER-03**: When `famp inspect broker` returns `ORPHAN_HOLDER`, the evidence row includes the holder's PID (via `SO_PEERCRED`/`LOCAL_PEERPID` or `lsof` fallback); if the PID cannot be discovered, the row says `holder_pid=unknown reason=<short>` — the field is never silently omitted.
- [ ] **INSP-BROKER-04**: `famp inspect broker` exits 0 only on `state: HEALTHY`; all four down-states exit 1 with the diagnosis printed to stdout (not stderr) so callers can capture both the verdict and the evidence in one stream.

### Identity Introspection — `famp inspect identities`

- [ ] **INSP-IDENT-01**: Operator can run `famp inspect identities` and receive a row per registered session identity with: name, listen-mode (bool), registered-at, last-activity timestamp, originating cwd.
- [ ] **INSP-IDENT-02**: Each identity row also includes mailbox unread-count, mailbox total-count, last-sender (name or "(none)"), and last-received-at timestamp.
- [ ] **INSP-IDENT-03**: The output explicitly does NOT include any per-identity "double-print" / "received vs surfaced" counter (deferred — wrong instrument; see Out of Scope).

### Task FSM Visibility — `famp inspect tasks`

- [x] **INSP-TASK-01**: Operator can run `famp inspect tasks` and receive a list grouped by task_id, each group showing current FSM state (one of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`), envelope count, last-transition-age.
- [x] **INSP-TASK-02**: Task records with `task_id == 0` or missing surface in a top-level `--orphans` bucket, recency-sorted (newest first); the bucket is rendered above the per-task-id groups.
- [x] **INSP-TASK-03**: `famp inspect tasks --id <task_id>` prints the envelope chain that drove each FSM transition, summary fields only by default: envelope_id, sender, recipient, fsm_transition, timestamp, sig_verified.
- [x] **INSP-TASK-04**: `famp inspect tasks --id <task_id> --full` emits each envelope in canonical JCS (RFC 8785) form so that piping the output through `jq` reproduces the exact canonical bytes that fed the signature input.

### Message Metadata — `famp inspect messages`

- [x] **INSP-MSG-01**: Operator can run `famp inspect messages --to <name>` to list envelope metadata for that identity's mailbox; no message bodies are returned in the response.
- [x] **INSP-MSG-02**: Each envelope row shows: sender, recipient, task_id, MessageClass, FSM state, timestamp, body byte length, body sha256 prefix (first 12 hex chars).
- [x] **INSP-MSG-03**: `famp inspect messages --to <name> --tail N` limits the response to the most-recent N envelopes (newest first); default tail is 50 if `--tail` omitted.

### RPC Surface and Transport Discipline

- [x] **INSP-RPC-01**: The broker exposes a `famp.inspect.*` RPC namespace on the same UDS socket it already serves bus messages on (no separate inspector socket).
- [x] **INSP-RPC-02**: Every `famp.inspect.*` handler is read-only, enforced by two complementary mechanisms: (1) handler signatures take `&BrokerState` (not `&mut BrokerState`) so the borrow checker rejects mutation at compile time; (2) a workspace dep-graph gate (`just check-inspect-readonly`) fails CI if `famp-inspect-server` transitively imports any mailbox-write, taskdir-write, or broker `&mut self` mutation surface. Together these prove read-only without a runtime property test (rejected as ceremony for a compile-time invariant per matt-essentialist + zed-velocity-engineer review 2026-05-09).
- [x] **INSP-RPC-03**: Inspect handlers run under a bounded latency budget (default: 500 ms per call); a handler exceeding the budget is dropped and the client receives a `BudgetExceeded` reply, not a queue stall on the message path.
- [x] **INSP-RPC-04**: Inspect handlers are cancellable from the broker side without leaking file descriptors, mailbox locks, or in-flight allocations; verified by a test that issues 1000 concurrent inspect calls and cancels them mid-flight.
- [x] **INSP-RPC-05**: A `famp.inspect.*` RPC call cannot starve a concurrent bus message; verified by a load test that sustains bus message throughput under inspect-call pressure.

### Crate Architecture

- [x] **INSP-CRATE-01**: A new `famp-inspect-proto` crate ships in the workspace containing only RPC request/response types, with no I/O dependencies (no tokio, no axum, no reqwest, no clap); enforced by a `just` recipe parallel to the existing `check-no-tokio-in-bus` gate.
- [x] **INSP-CRATE-02**: A new `famp-inspect-client` crate ships in the workspace, depending on `famp-inspect-proto`, performing UDS calls; the client crate must NOT depend on `clap` or any CLI parser, so future non-CLI consumers (SPA, `famp doctor`, external tooling) can link it cleanly.
- [x] **INSP-CRATE-03**: A new `famp-inspect-server` crate ships in the workspace, depending on `famp-inspect-proto`, mounted by the broker process; the server crate shares the broker's `famp-canonical`, `famp-envelope`, and `famp-fsm` dependency versions exactly (no Cargo-resolved version skew between inspector and broker).

### CLI Consumer

- [x] **INSP-CLI-01**: `famp inspect` is a subcommand of the existing `famp` binary (not a separate `famp-inspect` binary). **Phase 1** ships two sub-subcommands: `broker` and `identities`. **Phase 2** adds `tasks` and `messages`. (The four sub-subcommands are all shipped by the end of v0.10.)
- [x] **INSP-CLI-02**: Every `famp inspect <subcommand>` invocation accepts a `--json` flag that emits a stable, documented JSON shape suitable for piping to `jq` or consumption by tests, CI, or future SPA/`famp doctor` consumers.
- [x] **INSP-CLI-03**: Default human-readable output for each `famp inspect <subcommand>` is a fixed-width column-aligned table (not Rust `Debug` format); column headers are explicit on every invocation.
- [x] **INSP-CLI-04**: When the broker is not running, every `famp inspect` subcommand other than `broker` exits 1 with stderr `"error: broker not running at <socket-path>"` (no stack trace, no retry loop). `famp inspect broker` is the one command that must work against a dead broker (per INSP-BROKER-02).

## v2 Requirements

Deferred to future release (v0.10.x or later). Tracked but not in current roadmap.

### Mutation / Doctor Tools

- **INSP-DOCTOR-01**: `famp doctor` subcommand for read-write maintenance (replay, force-FSM-transition, mailbox compaction). Gated on what the read-only inspector reveals as actually-needed mutations after ~2 weeks of CLI use.

### Browser Dashboard

- **INSP-SPA-01**: Browser SPA consuming `famp-inspect-client` over a localhost HTTP+SSE bridge. Punted from v0.10 because the CLI is expected to cover ~70% of the pain; SPA reconsidered only if CLI use after ~2 weeks shows the gap.

### Body Fetch

- **INSP-MSG-BODY-01**: `famp inspect message <envelope_id> --body` to fetch the full body of a single envelope. Deferred — body inspection during v0.10-era incidents overlaps with reading the mailbox file directly; revisit only if usage signals the reach.

### Double-Print Diagnostics

- **INSP-DBLPRINT-01**: A diagnostic surface for the double-print context-cost failure mode (token-attribution at the model boundary OR static audit of `famp_await` notification payload). Deferred — broker-side counter was rejected as wrong-instrument; the right surface is at the MCP/model boundary, a separate investigation.

## Out of Scope

Explicitly excluded from v0.10. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Mutation of broker state from the inspector | Read-only is the v0.10 discipline. Any mutation surface (`famp doctor`) ships only after the read-only view tells us *which* mutations we actually keep reaching for — preventing castle-building on hypothetical surgeries. |
| Federation-wide inspector view | v0.10 observes one node — the local broker. v1.0 federation gateway can expose its own inspector surface later; pre-building a multi-node UI before federation ships is premature. |
| Remote / non-local network access | UDS local-trust only. The dashboard inherits the broker's existing trust boundary. No HTTP listener, no TLS, no auth in v0.10. |
| Browser SPA + SSE event stream | Punted to v0.10.x at earliest, only after ~2 weeks of CLI use proves the read-only RPC + CLI is insufficient. Building both the surface and a frontend before either is exercised would over-commit the design. |
| Per-identity double-print counter | Wrong instrument. The double-print failure mode (wake-up notification + inbox fetch each carrying the body, doubling token cost) is observable only at the model boundary, not the broker. A broker-side counter that purports to detect it would mislead users and outlive the diagnostic that retires it. |
| `famp inspect message <envelope_id> --body` | Body fetch overlaps with reading the on-disk mailbox file directly during a v0.10-era incident. Add in v0.10.x only if observed CLI usage shows operators reach for it. |
| Polling-based event stream / SSE / WebSocket | No event-stream protocol in v0.10. The CLI is a snapshot tool; subsequent invocations show the new state. Stream-shaped consumption is a v0.10.x decision driven by SPA demand. |
| Separate `famp-inspect` binary | Subcommand of `famp` instead. A separate binary would duplicate the dependency graph for `famp-canonical`, `famp-envelope`, `famp-fsm` and create version-skew failure modes (inspector decoding envelopes with a different canonicalizer than the broker that wrote them) — unacceptable for a byte-exactness protocol. |

## Traceability

Which phases cover which requirements. Filled in during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| INSP-BROKER-01 | Phase 1 | Pending |
| INSP-BROKER-02 | Phase 1 | Pending |
| INSP-BROKER-03 | Phase 1 | Pending |
| INSP-BROKER-04 | Phase 1 | Pending |
| INSP-IDENT-01 | Phase 1 | Pending |
| INSP-IDENT-02 | Phase 1 | Pending |
| INSP-IDENT-03 | Phase 1 | Pending |
| INSP-RPC-01 | Phase 1 | Complete |
| INSP-RPC-02 | Phase 1 | Complete |
| INSP-RPC-03 | Phase 2 | Complete |
| INSP-RPC-04 | Phase 2 | Complete |
| INSP-RPC-05 | Phase 3 | Complete |
| INSP-CRATE-01 | Phase 1 | Complete |
| INSP-CRATE-02 | Phase 1 | Complete |
| INSP-CRATE-03 | Phase 1 | Complete |
| INSP-CLI-01 | Phase 1 | Complete |
| INSP-CLI-02 | Phase 1 | Complete |
| INSP-CLI-03 | Phase 1 | Complete |
| INSP-CLI-04 | Phase 1 | Complete |
| INSP-TASK-01 | Phase 2 | Complete |
| INSP-TASK-02 | Phase 2 | Complete |
| INSP-TASK-03 | Phase 2 | Complete |
| INSP-TASK-04 | Phase 2 | Complete |
| INSP-MSG-01 | Phase 2 | Complete |
| INSP-MSG-02 | Phase 2 | Complete |
| INSP-MSG-03 | Phase 2 | Complete |

**Coverage:**
- v1 requirements: 26 total
- Mapped to phases: 26 ✓
- Unmapped: 0 ✓

**Per-phase totals:**
- Phase 1 (Broker Diagnosis & Identity Inspection — closes orphan-listener incident class): 16 requirements (BROKER 4 + IDENT 3 + RPC 2 + CRATE 3 + CLI 4)
- Phase 2 (Task FSM & Message Visibility — enrichment + I/O-bound handlers): 9 requirements (TASK 4 + MSG 3 + RPC 2)
- Phase 3 (Load Verification & Integration Hardening): 1 requirement (RPC 1)

---
*Requirements defined: 2026-05-09 — derived from locked SPEC questions in the v0.10 brainstorm conversation, after matt-essentialist + hamming-research-scientist adversarial reviews.*
*Last updated: 2026-05-10 — (1) phase mapping recut: Phase 1 ships `inspect broker` + `inspect identities` end-to-end (RPC + CLI); Phase 2 ships `inspect tasks` + `inspect messages`; INSP-RPC-02 reworded from runtime property test to compile-time+build-time enforcement. (2) plan-checker correction 2026-05-10: INSP-BROKER-02 corrected to use connect-handshake down-states (STALE_SOCKET not STALE_PID — v0.9 has no PID file); INSP-CLI-01 phased (broker+identities in Phase 1, tasks+messages in Phase 2).*
