# Phase 4: MCP Server & Same-Laptop E2E - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous --from 2)

<domain>
## Phase Boundary

Two Claude Code sessions on the same laptop — each pointing at its own `famp` daemon via an MCP server — can open one task, exchange ≥4 `deliver` messages driven by real LLM conversation, and close the task with a terminal deliver that transitions COMPLETED on both sides.

Concretely this phase delivers:
- `famp mcp` — stdio JSON-RPC MCP server exposing four tools: `famp_send`, `famp_await`, `famp_inbox`, `famp_peers`
- Typed error discriminators (`famp_error_kind`) on every tool failure
- Multi-entry keyring: the listen daemon accepts inbound envelopes whose `from` is any registered peer principal, not just a hardcoded `agent:localhost/self`
- Two-home, two-daemon, two-peer loopback harness for E2E-01
- An automated nextest E2E integration test spawning both daemons, both CLIs, and driving a full `request → commit → deliver ×N → terminal → ack` round-trip
- The FSM glue correction: Phase 3's `advance_terminal` seeded TaskFsm at Committed; Phase 4 replaces that with a real commit-reply handshake so the FSM walks REQUESTED → COMMITTED → COMPLETED naturally
- Witnessed manual smoke (E2E-02) with live Claude Code sessions — result captured in VERIFICATION.md

Out of scope (defer to v0.9+):
- Federation profile semantics (causality, negotiation, delegation)
- Agent Cards peer discovery (TOFU stays the v0.8 answer)
- Non-stdio MCP transports (HTTP, SSE) — stdio is sufficient for Claude Code local
- Windows support

</domain>

<decisions>
## Implementation Decisions

### MCP Library Choice
- **Use the `rmcp` crate** (or the official Rust MCP SDK if newer; planner should verify latest at plan time). It is the Anthropic-blessed Rust SDK for MCP servers, pinned to tokio, supports stdio transport out of the box, derives JSON-RPC schemas from Rust types.
- No hand-rolled JSON-RPC. No TypeScript bindings. Pure Rust, stdio only.
- If the crate surface is unstable, the planner may fall back to a small hand-rolled stdio JSON-RPC loop, but MUST document that choice in PLAN with a link to the crate's README.

### MCP Server Surface
- Binary entry: `famp mcp` subcommand. No new bin target.
- Tools exposed (MCP-01..04):
  1. `famp_send(peer: string, task_id: Option<string>, body: string, terminal: bool) -> { task_id, state }` — thin wrapper over `cli::send::run` with structured input/output.
  2. `famp_await(timeout_seconds: u64, task_id: Option<string>) -> { offset, task_id, from, class, body }` — wraps `cli::await_cmd::run`.
  3. `famp_inbox(action: "list"|"ack", since: Option<u64>, offset: Option<u64>) -> {...}` — wraps `cli::inbox::{list,ack}`.
  4. `famp_peers(action: "list"|"add", alias: Option<string>, endpoint: Option<string>, pubkey: Option<string>, principal: Option<string>) -> {...}` — wraps `cli::peer`.
- Each tool reuses the Phase 3 CLI functions directly — no duplicated business logic. The MCP layer is a thin adapter that serializes typed errors.
- Input schemas derived via `serde` + `schemars` (or equivalent).

### Typed Error Discriminators
- Every MCP tool result that fails returns `{ error: string, famp_error_kind: string, details: object }`.
- `famp_error_kind` maps 1:1 to CliError variants: `peer_not_found`, `peer_duplicate`, `task_not_found`, `task_terminal`, `await_timeout`, `inbox_locked`, `send_failed`, `tls_failed`, `io`, etc.
- Implementation: a new `MCP_ERROR_KIND` function on CliError (in famp/src/cli/error.rs) returns a stable string for each variant. Adding a new variant forces a match arm update (exhaustive) — compile-time guarantee.

### Multi-Entry Keyring
- Phase 2's listen daemon hardcodes `agent:localhost/self` as the single keyring entry. Phase 4 replaces this with a proper keyring built from `peers.toml` at daemon startup.
- Listen command reads `peers.toml`, iterates entries, adds each `(principal, pubkey)` pair to the `famp-keyring::Keyring` passed to `FampSigVerifyLayer`.
- Daemon also adds its OWN principal+pubkey so it can receive replies addressed to itself.
- Backward compatible: empty `peers.toml` still works for solo tests — keyring just has one entry.

### Commit-Reply Handshake
- Phase 3 shortcut: `send --terminal` transitions the local FSM by seeding at Committed. This is incorrect for a real run — a task should walk REQUESTED → COMMITTED → COMPLETED via actual message round-trips.
- Phase 4 correction:
  1. `famp send --new-task` still creates local record in REQUESTED
  2. The receiving daemon, on seeing a `request` envelope, automatically sends back a `commit` reply (new logic in `listen/handler.rs` or equivalent)
  3. The originator's `famp await` sees the `commit` and advances its local task record to COMMITTED
  4. Subsequent `deliver` messages now step from COMMITTED via valid FSM arrows
  5. `send --terminal` steps COMMITTED → COMPLETED naturally; the Phase 3 `fsm_glue::advance_terminal` shortcut is removed
- The commit reply is a real signed envelope, not a fake. It uses the same send path with `class = "commit"`.
- If the simple "auto-commit every request" policy is too aggressive (e.g., the receiver might want to reject), narrow it to "auto-commit requests addressed to self" — still automatic for this phase's single-long-task scope.

### E2E Integration Test (E2E-01)
- New test file: `crates/famp/tests/e2e_two_daemons.rs` — spawns TWO `famp listen` subprocesses, each with its own FAMP_HOME, each with the other registered as a peer (mutual TOFU), then drives `famp send` / `famp await` from a third test process to walk the full lifecycle.
- Test asserts ≥4 non-terminal delivers plus a terminal, matching the Phase 4 ROADMAP success criterion.
- Reuses `listen_harness` + `conversation_harness` from Phases 2 and 3 as the base.

### Manual Witnessed Smoke (E2E-02)
- A `.planning/milestones/v0.8-phases/04-mcp-server-e2e/E2E-SMOKE.md` checklist document that the human (or a live Claude Code session) fills in during the manual test.
- VERIFICATION.md for Phase 4 MUST capture the E2E-02 outcome (pass/fail with notes) — the gsd-verifier should mark `status: human_needed` until the human confirms.
- A helper `just e2e-smoke` recipe starts two daemons in the background, prints the MCP wiring instructions, and tails both inboxes — to reduce ceremony for the operator.

### Claude Discretion
- Exact rmcp version pin (planner decides at plan time based on crates.io)
- Whether tool inputs accept positional or named args (named is more robust; MCP default)
- Log levels during daemon start for MCP debugging

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `cli::send::run`, `cli::await_cmd::run`, `cli::inbox::{list,ack}`, `cli::peer::add` — direct targets for the four MCP tools
- `famp-taskdir`, `famp-inbox`, `famp-envelope`, `famp-fsm`, `famp-keyring` — all the storage + signing + state machinery is in place
- Phase 2 `listen_harness`, Phase 3 `conversation_harness` — test infrastructure to extend
- `FampSigVerifyLayer::new(Arc<Keyring>)` — the middleware accepts a full keyring, we just need to build it properly now

### Established Patterns
- Narrow thiserror enums per crate
- Atomic file replace
- Integration tests named after ROADMAP success criteria
- cargo nextest, clippy -D warnings, no openssl, forbid(unsafe_code)

### Integration Points
- `crates/famp/src/cli/mod.rs` — dispatcher gains Mcp variant
- `crates/famp/src/bin/famp.rs` — clap subcommand
- `crates/famp/src/cli/listen/mod.rs` — keyring-from-peers-toml build
- `crates/famp/src/cli/listen/router.rs` — auto-commit handler for inbound requests
- `crates/famp/src/cli/mcp/` — new module tree
- `crates/famp/src/cli/send/fsm_glue.rs` — REMOVE the seed shortcut; the FSM should walk naturally

</code_context>

<specifics>
## Specific Ideas

- E2E-01 test asserts workspace-total test count still ends green (should be 343 + N new Phase 4 tests)
- The keyring-from-peers test: register peer B on daemon A, send signed message as B, daemon A accepts; send unsigned → rejected; send signed by unknown principal → rejected
- MCP tool parity test: call each tool over a mock stdio and assert structured output matches the CLI output byte-equivalent (modulo JSON envelope differences)
- A smoke script that prints the `.mcp.json` snippet the operator needs to paste into Claude Code for the manual E2E

</specifics>

<deferred>
## Deferred Ideas

- HTTP/SSE MCP transports — stdio is enough for v0.8
- Tool permissioning / sandboxing — Claude Code already gates MCP tool calls
- Federation-style trust negotiation — v0.9+
- Windows support — out of scope
- E2E test matrix across >2 daemons — future if needed

</deferred>
