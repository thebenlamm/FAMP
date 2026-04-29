---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 13
subsystem: tests/mcp + cli/mcp/tools
tags: [phase-2, wave-7, test-05, e2e, mcp-01, ship-gate]
requires:
  - 02-09 (eight-tool MCP surface; ToolError plumbing; per-instance bus socket)
  - 02-08 (cli::mcp::session::active_identity + ensure_bus)
  - 02-04..02-07 (cli::send/inbox/await/join/leave structured entries)
  - 02-02 (broker daemon + Hello.bind_as proxy semantics)
  - 02-01 (BusClient + Wave-0 mcp_bus_e2e.rs stub)
provides:
  - "TEST-05 GREEN — full bus-side E2E across two `famp mcp` stdio processes"
  - "MCP tool integration proof: `famp_register → famp_send → famp_await` round-trip over UDS"
  - "MCP-01 startup-path isolation confirmed at integration level (FAMP_HOME and FAMP_LOCAL_ROOT both env_remove'd, MCP processes still succeed end-to-end)"
  - "Plumb-through fix for MCP session active_identity → tool act_as: send/inbox/await_/join/leave now thread the session-bound name into `cli::*::run_at_structured` so D-01 identity resolution does not fall back to wires.tsv when called from MCP"
affects:
  - crates/famp/tests/mcp_bus_e2e.rs (Wave-0 stub → 322-line E2E test)
  - crates/famp/src/cli/mcp/tools/send.rs (act_as: session::active_identity().await)
  - crates/famp/src/cli/mcp/tools/inbox.rs (act_as: session::active_identity().await)
  - crates/famp/src/cli/mcp/tools/await_.rs (act_as: session::active_identity().await)
  - crates/famp/src/cli/mcp/tools/join.rs (act_as: session::active_identity().await)
  - crates/famp/src/cli/mcp/tools/leave.rs (act_as: session::active_identity().await)
tech-stack:
  added: []
  patterns:
    - "Two-process MCP harness: each `McpHarness` owns its child + stdin/stdout, and the test thread holds two of them sharing one `FAMP_BUS_SOCKET`. Belt-and-suspenders env hygiene: FAMP_HOME, FAMP_LOCAL_ROOT, AND FAMP_LOCAL_IDENTITY are all env_remove'd so the only path that can possibly succeed is the broker-via-session-active_identity path."
    - "Park-await-before-send pattern: the broker's `Await` handler ONLY parks; it does not scan the mailbox for pre-existing lines. So a single-threaded test that awaits AFTER send will time out even when the message is already in the mailbox. We split send_msg + recv_msg so bob's await frame is on the wire before alice's send hits the broker."
    - "Tool act_as plumb-through: `session::active_identity().await` is the canonical source of truth for proxy bind_as / D-01 tier-1 identity from inside MCP. Each delegating tool reads it just before constructing the `*Args`, never holding the session lock across the bus round-trip (preserves the `clippy::significant_drop_tightening` discipline 02-09 established)."
key-files:
  created:
    - crates/famp/tests/mcp_bus_e2e.rs (322 lines)
  modified:
    - crates/famp/src/cli/mcp/tools/send.rs
    - crates/famp/src/cli/mcp/tools/inbox.rs
    - crates/famp/src/cli/mcp/tools/await_.rs
    - crates/famp/src/cli/mcp/tools/join.rs
    - crates/famp/src/cli/mcp/tools/leave.rs
decisions:
  - "Adapted must-have shapes to the actual v0.9 tool surface. The plan must-haves sketched `famp_send({to:{kind:\"agent\",name:\"bob\"}, new_task:\"hello\"})` and `famp_await({timeout_ms:5000})`. The actual v0.9 surface is `famp_send({peer, mode:\"new_task\", title})` and `famp_await({timeout_seconds})`. The plan body's task-1 note explicitly authorizes this adaptation: 'JSON-RPC method name and reply shape may differ from the assumed... read crates/famp/tests/mcp_stdio_tool_calls.rs to confirm the exact MCP method name'. We chose what mcp_stdio_tool_calls.rs uses (NDJSON, tools/call, flat send args, timeout_seconds)."
  - "Park-await-before-send instead of poll-loop. Two viable patterns: (a) issue await BEFORE send and use send_msg/recv_msg split to interleave the JSON-RPC frames on a single thread, or (b) call `famp_inbox` action=list after send and assert on inbox contents. Option (a) is the proper test of `famp_await` (the must-have explicitly names `famp_await` as the receive path), so we built the harness around it. The 500 ms sleep between bob's await frame and alice's send is the same race mitigation `cli_dm_roundtrip::test_await_unblocks` uses."
  - "Plumb-through fix in scope. The act_as: None bug in tools/{send,inbox,await_,join,leave} is a real Rule 2 (missing critical functionality) deviation, not architectural. The fix is one-line surgery in each tool: replace `act_as: None` with `act_as: session::active_identity().await`. dispatch_tool already gates these tools behind `active_identity().is_some()`, so the value is guaranteed Some at the call site. No public API change, no new dep, no behavior change for v0.8 callers (the tools weren't reachable end-to-end before this plan)."
  - "Body assertion via substring match, not structural lookup. The Phase-2 envelope wraps the user's `title` ('hello from alice') in `body.details.summary` of an `audit_log` envelope. Phase 4's federation gateway will likely re-shape this. Two assertions guard against regression: (1) a permissive `body.to_string().contains(\"hello from alice\")` substring check that survives any reshape, and (2) a precise `body.details.summary == \"hello from alice\"` that pins the current Phase-2 shape. If Phase 4 shifts the field, assertion (1) keeps the test passing while (2) signals the contract change explicitly."
metrics:
  duration: ~50min
  completed_date: 2026-04-28
---

# Phase 2 Plan 13: TEST-05 Bus-Side E2E Summary

Fills in the Wave-0 stub at `crates/famp/tests/mcp_bus_e2e.rs` with the
real two-MCP-process register → send → await round-trip. This is the
v0.9 bus-side equivalent of v0.8's deleted `e2e_two_daemons` HTTPS test
and the plan that closes Phase 2's ship gate (the eight-tool MCP
surface is proven to work across two stdio processes).

## Final harness shape

| Aspect | Choice |
| ------ | ------ |
| Wire framing | NDJSON (newline-delimited JSON), NOT LSP `Content-Length` |
| Method name | `tools/call` with `{name, arguments}` params |
| Result shape | tool output JSON-encoded as a string in `result.content[0].text` |
| Initialize handshake | One `initialize` round-trip during `McpHarness::spawn` |
| Bus socket | One shared `$tmp/test-bus.sock` across both processes |
| Identity isolation | `FAMP_HOME`, `FAMP_LOCAL_ROOT`, `FAMP_LOCAL_IDENTITY` all env_remove'd |
| Race mitigation | 500 ms sleep between bob's parked await frame and alice's send |

The harness exposes a tiny three-method API:
- `spawn(sock, label) -> McpHarness` — child process + initialize.
- `tool_call(tool, &args) -> Value` — synchronous JSON-RPC call/return.
- `recv_msg(timeout) -> Value` — for the park-then-read pattern.

Plus `McpHarness::ok_result(reply, what) -> Value` to dig out and parse
the inner JSON document the tool wrote into `result.content[0].text`.

## Flakiness assessment

Test runs in ~1.5 seconds locally. No flakiness observed across
back-to-back runs during development. Mitigations in place:
1. 500 ms sleep before alice's send (same pattern as
   `cli_dm_roundtrip::test_await_unblocks`).
2. 10 second `timeout_seconds` on bob's await (absorbs broker-spawn
   jitter without making the happy path slow).
3. 15 second JSON-RPC reply read timeout (absorbs CI scheduling
   pauses without hanging the test if the broker dies silently).
4. `FAMP_BUS_SOCKET` is per-test under a `tempfile::TempDir`, so the
   broker is never shared with sibling tests (matching 02-09's
   per-instance bus socket isolation pattern).

If flakiness emerges in CI, the most likely fix is bumping the
inter-frame sleep from 500 ms to 1 s (the sleep guards the
broker-side ParkAwait insertion against alice's Send racing past it).

## Phase-2 requirements coverage

All 36 Phase-2 requirements green by end of this plan, per the plan
success criteria:

| Group | Requirements | Status |
| ----- | ------------ | ------ |
| BROKER-01..05 | broker daemon, sock, hello, register, ipc | GREEN (waves 0-2) |
| CLI-01..11    | register/send/inbox/await/peers/whoami/join/leave/sessions/hook/mcp | GREEN (waves 4-6) |
| MCP-01..10    | startup-path isolation, eight tools, error_kind exhaustive, etc | GREEN (waves 6-7) |
| HOOK-01..04   | `famp hook` subcommand for Stop hook integration | GREEN (wave 6) |
| TEST-01..04   | DM round-trip, channel fanout, hook subcommand, broker lifecycle | GREEN (waves 5-6) |
| TEST-05       | full bus-side E2E across two MCP processes | **GREEN (this plan)** |
| CARRY-02      | v0.8 hygiene carry-overs (Sofer field-report items) | GREEN (waves 1-3) |

## MCP-01 confirmation

The test removes BOTH `FAMP_HOME` and `FAMP_LOCAL_ROOT` from the env of
every spawned `famp mcp` (and `FAMP_LOCAL_IDENTITY` for good measure),
and the round-trip still completes. This is integration-level
confirmation of MCP-01 (no FAMP_HOME / FAMP_LOCAL_ROOT in MCP startup
path), complementing the source-import gate `scripts/check-mcp-deps.sh`
(which is also green: zero `use reqwest` / `use rustls` reachable from
`cli/mcp/`, `bus_client/`, or `broker/`).

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 2 - Missing critical functionality] MCP tools were passing `act_as: None`**

- **Found during:** Task 1 implementation, on first nextest run.
- **Issue:** `tools/{send,inbox,await_,join,leave}` constructed
  `*Args { ..., act_as: None }`, which made
  `cli::*::run_at_structured` call `resolve_identity(None)`. With
  `FAMP_LOCAL_IDENTITY` env_remove'd (per the test's MCP-01 isolation
  contract) and no `wires.tsv` match in cwd, `resolve_identity`
  returned `CliError::NoIdentityBound`. alice's `famp_send` then
  failed before reaching the broker, and bob's await timed out.
  This wasn't caught by 02-09's 17-test green sweep because all 5
  send/inbox/await tests in `mcp_stdio_tool_calls.rs` were `#[ignore]`'d.
- **Fix:** Each tool now calls `session::active_identity().await` to
  populate `act_as`. The `dispatch_tool` (server.rs) already gates
  every non-register/whoami tool behind `active_identity().is_some()`,
  so the value is guaranteed Some at the call site (no need to handle
  None). One-line surgery per tool; no public-API change.
- **Files modified:** `tools/send.rs`, `tools/inbox.rs`, `tools/await_.rs`,
  `tools/join.rs`, `tools/leave.rs`.
- **Commit:** `a24f76b`.

### No architectural deviations

No checkpoints reached. No auth gates. The plan's `files_modified`
field formally lists only `crates/famp/tests/mcp_bus_e2e.rs`, but
Rule 2 (missing critical functionality) auto-fixes are explicitly
in scope and the plan body's success criterion 3 ("MCP-01 confirmed
at integration level") is unreachable without the tool-level fix.

## Self-Check: PASSED

Verified all created files exist on disk:
- `crates/famp/tests/mcp_bus_e2e.rs` — FOUND (322 lines)

Verified all commits exist in `git log`:
- `a24f76b` (Rule 2 fix — tool act_as plumb-through) — FOUND
- `893258c` (Task 1 — TEST-05 implementation) — FOUND

Acceptance grep counts (all checks ≥ required threshold):
- `#[ignore` in mcp_bus_e2e.rs: 0 (stub fully replaced)
- `env_remove("FAMP_HOME")`: 1
- `env_remove("FAMP_LOCAL_ROOT")`: 1
- `FAMP_BUS_SOCKET`: 3 (module docs + spawn + module docs)
- `famp_register`: 5 lines (alice + bob + module docs)
- `famp_send`: 4 lines (call site + tool name + module docs)
- `famp_await`: 3 lines (call site + tool name + module docs)
- `"hello from alice"`: 3 lines (sent + 2 assertions)
- File length: 330 (≥ 200 min_lines requirement)

Verified no other test suite regressed:
- `cargo nextest run -p famp --lib`: 64/64 PASS
- `cargo nextest run -p famp --test mcp_register_whoami --test mcp_pre_registration_gating --test mcp_malformed_input --test mcp_stdio_tool_calls --test mcp_error_kind_exhaustive --test mcp_bus_e2e --test mcp_session_bound_e2e`: 18 passed, 6 skipped (pre-existing #[ignore]'s)
- `cargo clippy -p famp --tests -- -D warnings`: green
- `bash scripts/check-mcp-deps.sh`: green

## Commits

| Step | Commit    | Files | Insertions / Deletions |
| ---- | --------- | ----- | ---------------------- |
| Pre-fix (Rule 2) | `a24f76b` | 5 | +35 / -5 |
| TEST-05 impl | `893258c` | 1 | +322 / -4 |
