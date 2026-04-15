---
phase: 04-mcp-server-e2e
plan: 02
subsystem: mcp
tags: [mcp, json-rpc, stdio, content-length-framing, error-kind, tdd]

requires:
  - phase: 04-01
    provides: multi-entry keyring + auto-commit handler wired into famp listen
  - phase: 03-04
    provides: conversation_harness with spawn_listener / stop_listener / add_self_peer helpers

provides:
  - CliError::mcp_error_kind() exhaustive match (compile-time gate — adding a variant without mapping it is a build error)
  - SendOutcome / AwaitOutcome structured return types wrapping existing CLI entry points
  - famp mcp subcommand: Content-Length-framed stdio JSON-RPC server (hand-rolled, no rmcp)
  - Four MCP tools: famp_send, famp_await, famp_inbox, famp_peers
  - All tool errors carry famp_error_kind discriminator in JSON-RPC error.data
  - mcp_stdio_tool_calls.rs: 4 subprocess integration tests (all passing)
  - mcp_error_kind_exhaustive.rs: 3 exhaustive mapping tests (all passing)

affects: [04-03, any MCP client integration, conformance testing]

tech-stack:
  added: []
  patterns:
    - "Hand-rolled MCP stdio server: tokio::io::stdin/stdout + BufReader, read_line loop until blank line, read_exact for body"
    - "Tool dispatch: match name string -> async fn call(home, input) -> Result<Value, CliError>"
    - "Error propagation: CliError -> mcp_error_kind() -> JSON-RPC error.data.famp_error_kind"
    - "Structured CLI refactor: run_at() calls run_at_structured() to avoid duplication"
    - "Subprocess test with in-process daemon: tokio::runtime::Builder in blocking test + spawn_listener"

key-files:
  created:
    - crates/famp/src/cli/mcp/mod.rs
    - crates/famp/src/cli/mcp/server.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/src/cli/mcp/tools/mod.rs
    - crates/famp/src/cli/mcp/tools/send.rs
    - crates/famp/src/cli/mcp/tools/await_.rs
    - crates/famp/src/cli/mcp/tools/inbox.rs
    - crates/famp/src/cli/mcp/tools/peers.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
    - crates/famp/tests/mcp_stdio_tool_calls.rs
  modified:
    - crates/famp/src/cli/mod.rs (added Commands::Mcp arm)
    - crates/famp/src/cli/send/mod.rs (added SendOutcome + run_at_structured)
    - crates/famp/src/cli/await_cmd/mod.rs (added AwaitOutcome + run_at_structured)
    - crates/famp/src/cli/error.rs (added mcp_error_kind() method)
    - crates/famp/Cargo.toml (added io-std tokio feature)

key-decisions:
  - "Rejected rmcp 1.4.0: requires schemars::JsonSchema derives + proc macros incompatible with async CLI entry points sharing Arc<PathBuf> home context"
  - "Hand-rolled MCP server chosen: 150-line Content-Length framing loop matches protocol spec exactly with zero extra deps"
  - "mcp_error_kind() is const fn with NO _ => wildcard arm — compile-time exhaustiveness guarantee (T-04-13 mitigation)"
  - "Structured refactor: run_at_structured() returns SendOutcome/AwaitOutcome; old run_at() delegates to it — zero behavior change for CLI callers"
  - "mcp_famp_send_new_task test uses tokio::runtime::Builder in blocking #[test] to run in-process spawn_listener alongside blocking subprocess I/O"

patterns-established:
  - "MCP tool handler signature: pub async fn call(home: &Path, input: &Value) -> Result<Value, CliError>"
  - "Error discrimination: every CliError variant maps to a snake_case famp_error_kind string, tested exhaustively"
  - "TDD RED (test commit f2fb5ff) -> GREEN (impl commits 7005886)"

requirements-completed: [MCP-01, MCP-02, MCP-03, MCP-04, MCP-05, MCP-06]

duration: ~150min
completed: 2026-04-15
---

# Phase 04 Plan 02: MCP stdio server with four tools — all 4 subprocess integration tests passing

**Hand-rolled Content-Length-framed stdio JSON-RPC server exposing famp_send/famp_await/famp_inbox/famp_peers, with exhaustive compile-time CliError -> mcp_error_kind() mapping.**

## Performance

- **Duration:** ~150 min (across two sessions due to context limit)
- **Started:** 2026-04-15T00:00:00Z
- **Completed:** 2026-04-15T02:30:00Z
- **Tasks:** 2 of 2
- **Files modified:** 14

## Accomplishments

- Exhaustive `CliError::mcp_error_kind()` method: 28 variants, no `_ =>` fallback, compile error if any variant is added without mapping
- `SendOutcome` and `AwaitOutcome` structured return types added to CLI entry points without breaking existing callers
- Complete MCP stdio server: `initialize`, `tools/list`, `tools/call`, `ping`, notifications — all Content-Length-framed
- All 4 subprocess integration tests pass: `mcp_initialize_lists_four_tools`, `mcp_famp_send_new_task_returns_structured`, `mcp_famp_peers_list_returns_entries`, `mcp_error_has_famp_error_kind`
- `mcp_famp_send_new_task` uses in-process `spawn_listener` from `conversation_harness` so `famp_send` actually POSTs to a live FAMP daemon
- 353/354 workspace tests pass (1 pre-existing failure in `send_new_task` unrelated to this plan — confirmed via git stash test)
- Zero OpenSSL dependency (`cargo tree -i openssl` returns no match)

## Task Commits

1. **Task 1: CliError::mcp_error_kind() + send/await structured refactor** - `f2fb5ff` (feat)
2. **Task 2: MCP stdio server + subprocess integration tests (GREEN)** - `7005886` (feat)

## Files Created/Modified

- `crates/famp/src/cli/mcp/mod.rs` — Module root, McpArgs clap struct, run() dispatcher
- `crates/famp/src/cli/mcp/server.rs` — Async Content-Length-framed MCP stdio server (read_line loop + read_exact body)
- `crates/famp/src/cli/mcp/error_kind.rs` — Exhaustive mcp_error_kind() with 28 arms, const fn, no wildcard
- `crates/famp/src/cli/mcp/tools/mod.rs` — Tool module declarations
- `crates/famp/src/cli/mcp/tools/send.rs` — famp_send tool: maps mode to SendArgs, calls run_at_structured
- `crates/famp/src/cli/mcp/tools/await_.rs` — famp_await tool: timeout_seconds -> humantime string, calls run_at_structured
- `crates/famp/src/cli/mcp/tools/inbox.rs` — famp_inbox tool: list/ack actions
- `crates/famp/src/cli/mcp/tools/peers.rs` — famp_peers tool: list/add actions via read_peers / run_add_at
- `crates/famp/tests/mcp_error_kind_exhaustive.rs` — 3 tests: all variants have kind, kinds are unique, spot checks
- `crates/famp/tests/mcp_stdio_tool_calls.rs` — 4 subprocess integration tests (all passing)
- `crates/famp/src/cli/mod.rs` — Added Commands::Mcp + rt.block_on(mcp::run(args))
- `crates/famp/src/cli/send/mod.rs` — Added SendOutcome, run_at_structured (run_at delegates to it)
- `crates/famp/src/cli/await_cmd/mod.rs` — Added AwaitOutcome, run_at_structured
- `crates/famp/src/cli/error.rs` — Added mcp_error_kind() method
- `crates/famp/Cargo.toml` — Added "io-std" to tokio features

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] rmcp rejected; hand-rolled server used instead**
- **Found during:** Task 2 design phase
- **Issue:** rmcp 1.4.0 requires `schemars::JsonSchema` derives and proc macros (`#[tool_router]`, `#[tool_handler]`) that conflict with the async CLI entry point pattern where tool handlers need `Arc<PathBuf>` home context
- **Fix:** Hand-rolled 150-line Content-Length-framing server in server.rs (matches wire spec exactly)
- **Files modified:** crates/famp/src/cli/mcp/server.rs
- **Commit:** 7005886

**2. [Rule 1 - Bug] mcp_famp_send_new_task test needed real in-process daemon**
- **Found during:** Task 2 verification
- **Issue:** Original test used hardcoded `https://127.0.0.1:8443` which has no listener — famp_send returned `send_failed`
- **Fix:** Rewrote test to build tokio runtime in blocking #[test], call `conversation_harness::spawn_listener` for real ephemeral port, add peer at actual address, then run MCP subprocess I/O
- **Files modified:** crates/famp/tests/mcp_stdio_tool_calls.rs
- **Commit:** 7005886

**3. [Rule 2 - Missing functionality] tokio io-std feature not enabled**
- **Found during:** Task 2 compilation
- **Issue:** `tokio::io::stdin/stdout` not available without `io-std` feature
- **Fix:** Added `"io-std"` to tokio features in Cargo.toml
- **Files modified:** crates/famp/Cargo.toml
- **Commit:** 7005886

### Pre-existing Issue (Not Fixed)

**send_new_task_creates_record_and_hits_daemon** fails with `expected one inbox line, got 2`. Confirmed pre-existing via `git stash` test — the test was failing before any Task 2 changes. Out of scope for this plan; logged for deferred resolution.

## Known Stubs

None. All four MCP tools are wired to real CLI entry points; no hardcoded/placeholder values flow to tool responses.

## Threat Flags

None. No new network endpoints beyond `famp mcp` (which is a stdio server, not a network listener). No new auth paths or file access patterns beyond what the underlying CLI tools already do.

## Self-Check: PASSED

- `crates/famp/src/cli/mcp/server.rs` — exists (committed 7005886)
- `crates/famp/tests/mcp_stdio_tool_calls.rs` — exists (committed 7005886)
- `f2fb5ff` — confirmed in git log (Task 1)
- `7005886` — confirmed in git log (Task 2)
- 353/354 workspace tests pass (`cargo nextest run --workspace --no-fail-fast`)
- Clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`)
- `famp mcp --help` works and describes the four tools
