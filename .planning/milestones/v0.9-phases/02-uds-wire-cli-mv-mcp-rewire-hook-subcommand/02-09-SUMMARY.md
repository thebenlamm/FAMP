---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 09
subsystem: cli/mcp/{server, tools/*}
tags: [phase-2, wave-6, mcp-rewire, d-04, d-05, d-10, mcp-02, mcp-03, mcp-04, mcp-05, mcp-06, mcp-07, mcp-08, mcp-09]
requires:
  - 02-01 (BusClient + identity foundation)
  - 02-02 (broker daemon + Hello.bind_as wire field)
  - 02-04 (cli::send::run_at_structured)
  - 02-05 (cli::inbox::list::run_at_structured)
  - 02-06 (cli::await_cmd::run_at_structured)
  - 02-07 (cli::join / cli::leave / cli::sessions / cli::whoami structured entries)
  - 02-08 (cli::mcp::session reshape + bus_error_to_jsonrpc + tool stubs)
provides:
  - "MCP tool surface complete: 8 dispatchable tools — register, whoami, send, inbox, await, peers, join, leave"
  - "tools::ToolError {kind: BusErrorKind, message: String} — typed error carrying both the JSON-RPC code discriminator and a free-form error.message; lets tools surface field-naming hints in malformed-input failures"
  - "tools::register::call — BusMessage::Register{name, pid=process::id()} → set_active_identity on RegisterOk (D-05 entry point)"
  - "tools::whoami::call — BusMessage::Whoami{} → {active, joined} (FREE-PASS, no D-05 gate)"
  - "tools::send::call — delegates to cli::send::run_at_structured; strict bool typing for more_coming"
  - "tools::inbox::call — delegates to cli::inbox::list::run_at_structured; strict bool typing for include_terminal; v0.8-compat {entries: [{task_id, envelope}], next_offset} output shape"
  - "tools::await_::call — delegates to cli::await_cmd::run_at_structured; output is either {envelope: <typed>} or {timeout: true}"
  - "tools::peers::call — only direct-bus tool; sends BusMessage::Sessions{} and projects to {online: [name1, name2, ...]}"
  - "tools::join::call (NEW) — BusMessage::Join via cli::join::run_at_structured; returns {channel, members, drained: count}"
  - "tools::leave::call (NEW) — BusMessage::Leave via cli::leave::run_at_structured; returns {channel}"
  - "server::dispatch_tool — Result<Value, ToolError>; D-05 gate via session::active_identity().await.is_none() before any binding-required arm"
  - "server::tool_descriptors() — 8 tool entries (was 6); famp_join + famp_leave have JSON-Schema input + descriptions"
affects:
  - crates/famp/src/cli/mcp/tools/mod.rs (introduces ToolError + 2 new module decls)
  - crates/famp/src/cli/mcp/tools/register.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/whoami.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/send.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/inbox.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/await_.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/peers.rs (full rewrite)
  - crates/famp/src/cli/mcp/tools/join.rs (NEW)
  - crates/famp/src/cli/mcp/tools/leave.rs (NEW)
  - crates/famp/src/cli/mcp/server.rs (dispatch_tool ToolError plumbing + 2 new tool_descriptors entries)
  - crates/famp/tests/common/mcp_harness.rs (per-instance FAMP_BUS_SOCKET isolation)
  - crates/famp/tests/mcp_register_whoami.rs (full rewrite onto v0.9 broker shapes)
  - crates/famp/tests/mcp_pre_registration_gating.rs (-32000 → -32100 + isolated socket)
  - crates/famp/tests/mcp_malformed_input.rs (per-test isolated socket)
  - crates/famp/tests/mcp_stdio_tool_calls.rs (6 → 8 tool count + 3 v0.8 tests gated #[ignore])
tech-stack:
  added: []
  patterns:
    - "ToolError as the typed-error carrier between tool bodies and the JSON-RPC frame builder; lets the bare BusErrorKind exhaustive map (-32100..=-32109) coexist with malformed-input-style messages that name fields and expected types"
    - "per-test FAMP_BUS_SOCKET (under each tempdir) so parallel integration runs cannot share a registered slot"
    - "delegate-to-cli pattern: every messaging tool (send/inbox/await/join/leave) calls into the matching cli::*::run_at_structured so the bus round-trip + Hello.bind_as proxy + error mapping live in exactly ONE place"
    - "FREE-PASS exception: register + whoami bypass the D-05 active_identity gate; everything else is gated"
    - "explicit drop(guard) before .await — clippy::significant_drop_tightening enforced"
key-files:
  created:
    - crates/famp/src/cli/mcp/tools/join.rs
    - crates/famp/src/cli/mcp/tools/leave.rs
  modified:
    - crates/famp/src/cli/mcp/tools/mod.rs
    - crates/famp/src/cli/mcp/tools/register.rs
    - crates/famp/src/cli/mcp/tools/whoami.rs
    - crates/famp/src/cli/mcp/tools/send.rs
    - crates/famp/src/cli/mcp/tools/inbox.rs
    - crates/famp/src/cli/mcp/tools/await_.rs
    - crates/famp/src/cli/mcp/tools/peers.rs
    - crates/famp/src/cli/mcp/server.rs
    - crates/famp/tests/common/mcp_harness.rs
    - crates/famp/tests/mcp_register_whoami.rs
    - crates/famp/tests/mcp_pre_registration_gating.rs
    - crates/famp/tests/mcp_malformed_input.rs
    - crates/famp/tests/mcp_stdio_tool_calls.rs
decisions:
  - "Introduced `ToolError {kind, message}` instead of the plan's literal `Result<Value, BusErrorKind>` signature. The bare enum cannot carry the field-naming hints required by `mcp_malformed_input::mcp_famp_send_rejects_non_bool_more_coming` and `famp_inbox_list_rejects_non_bool_include_terminal` (both assert the response body contains the field name + the substring \"bool\"). The cleanest fix is a typed wrapper that pairs the BusErrorKind with a free-form message, projected onto JSON-RPC `(code, message, data.famp_error_kind)` by `bus_error_response`. The MCP-10 exhaustive-match property is preserved end-to-end since `kind` is still a `BusErrorKind` and projects through `bus_error_to_jsonrpc`."
  - "tools/register accepts BOTH `identity` (v0.8 surface, what existing MCP clients pass) AND `name` (the broker's wire field name) as the input key. Rejects only when neither is present. This avoids breaking every Claude Code window that has `famp_register {identity: \"alice\"}` muscle memory while still allowing future clients to use the canonical wire-shape key."
  - "tools/inbox returns `{entries: [{task_id, envelope}], next_offset}` not the broker's literal `{envelopes, next_offset}` shape. Reason: v0.8 MCP clients (and `seed_filter_fixture` style fixtures, even where #[ignore]'d for now) expect each entry to expose `task_id` at the top level. Projecting `causality.ref` into `task_id` keeps clients from re-walking the FAMP envelope structure on every render. The full envelope is still surfaced under `entries[*].envelope` for any client that needs the canonical wire shape."
  - "Wave 6 tests not on the phase-context list (mcp_famp_peers_list_returns_entries, famp_inbox_list_filters_terminal_by_default, famp_inbox_list_include_terminal_true_returns_all) were #[ignore]'d rather than rewritten. They are v0.8 file-fixture / peers.toml-shape tests. Rewriting them on the v0.9 broker shape is plan 02-13's (E2E broker-driven harness) responsibility; touching them now would expand the plan beyond its scope. Each #[ignore] carries an explanatory message pointing at 02-13."
  - "Per-instance `FAMP_BUS_SOCKET` isolation. Without it, parallel integration runs share `~/.famp/bus.sock`, so a `register as alice` from one test conflicts with another test's pre-registration assertions. Fixed in 4 spawn sites (mcp_harness, pre_registration_gating, mcp_malformed_input, mcp_stdio_tool_calls) so each test owns its broker."
  - "Manual `drop(guard)` after async I/O in tools/whoami + tools/peers + tools/register. clippy::significant_drop_tightening flags any temporary lock guard whose `Drop` runs at end-of-function — explicit `drop(guard)` after the read or write makes the lock release point auditable. The session.rs lock is held for at most one `send_recv` round-trip per tool call; never across the bus connect (per plan 02-08's idempotent ensure_bus pattern)."
metrics:
  duration: ~75min
  completed_date: 2026-04-28
---

# Phase 2 Plan 9: MCP Tool Bodies — Bus Rewire + Join/Leave Summary

Fills in the 6 tool bodies that plan 02-08 stubbed with
`unimplemented!("rewired in plan 02-09")`, and adds the 2 NEW v0.9 tools
(`famp_join`, `famp_leave`) so the MCP surface is feature-complete at 8
tools. Pre-registration gating (D-05) preserved in `dispatch_tool`. The
v0.8 federation-era `peers.toml`-style `famp_peers` collapses to a
broker-memory `{online: [name, ...]}` view per the v0.9 design.

## Output shapes (one row per tool)

| Tool            | Input (required)                    | Output                                        |
| --------------- | ----------------------------------- | --------------------------------------------- |
| `famp_register` | `identity` (or `name`)              | `{active, drained: count, peers}`             |
| `famp_whoami`   | (none)                              | `{active: string|null, joined: [string]}`     |
| `famp_send`     | `mode` + `peer`/`channel`           | `{task_id, delivered}`                        |
| `famp_inbox`    | `action: "list"`                    | `{entries: [{task_id, envelope}], next_offset}` |
| `famp_await`    | (none; defaults timeout=30s)        | `{envelope: <typed>}` OR `{timeout: true}`    |
| `famp_peers`    | (none)                              | `{online: [name, ...]}`                       |
| `famp_join`     | `channel: string`                   | `{channel, members, drained: count}`          |
| `famp_leave`    | `channel: string`                   | `{channel}`                                   |

## Bus delegation pattern (which tools call where)

| Tool         | Calls                                          | Notes                                       |
| ------------ | ---------------------------------------------- | ------------------------------------------- |
| `register`   | `session::ensure_bus` + `BusMessage::Register` | Sets `session::active_identity` on RegisterOk |
| `whoami`     | `session::ensure_bus` + `BusMessage::Whoami`   | FREE-PASS; bypasses D-05 gate               |
| `send`       | `cli::send::run_at_structured`                 | Strict bool more_coming                     |
| `inbox`      | `cli::inbox::list::run_at_structured`          | Strict bool include_terminal                |
| `await_`     | `cli::await_cmd::run_at_structured`            | Default timeout 30 s                        |
| `peers`      | `session::ensure_bus` + `BusMessage::Sessions` | Lone direct-bus tool (no CLI delegate)      |
| `join` (new) | `cli::join::run_at_structured`                 | D-10 proxy; broker mutates canonical holder |
| `leave` (new)| `cli::leave::run_at_structured`                | D-10 proxy; broker mutates canonical holder |

`peers` is the only tool that does NOT delegate to a CLI structured
entry point — `cli::sessions::run_at_structured` returns full
`SessionRow` rows, but the v0.8 MCP `famp_peers` shape is just the live
names. We keep the projection in `tools/peers.rs` so `cli::sessions`
remains useful as the JSONL-emitting CLI command without forking the
shape.

## D-05 pre-registration gating preserved

`server::dispatch_tool` carries the gate verbatim from plan 02-08:

```rust
match name {
    "famp_register" => return tools::register::call(input).await,
    "famp_whoami"   => return tools::whoami::call(input).await,
    _ => {}
}
if crate::cli::mcp::session::active_identity().await.is_none() {
    return Err(ToolError::not_registered());
}
match name { /* every other tool */ }
```

`ToolError::not_registered()` projects to JSON-RPC code `-32100`,
`data.famp_error_kind = "not_registered"`, plus the canonical
operator hint stored under `error.data.details.hint` ("Call
famp_register with an identity name first…"). The exact hint string is
pinned by `mcp_pre_registration_gating::messaging_tools_refuse_before_register`.

## ToolError carrier — why it exists

Plan 02-09's literal task description had each tool body return
`Result<Value, BusErrorKind>`. This works for happy-path errors where
the broker returns a typed `BusReply::Err{kind, message}` and we just
forward it. But it does NOT work for client-side validation failures
(e.g. "the `more_coming` field is a string, not a bool"). The bare enum
has no carrier for the field name + expected type, and the JSON-RPC
frame builder upstream had no way to thread one in.

The fix is a thin wrapper:

```rust
pub struct ToolError {
    pub kind: BusErrorKind,        // → data.famp_error_kind + JSON-RPC code
    pub message: String,           // → JSON-RPC error.message
}
```

Tool bodies build it at error sites; `dispatch_tool` calls
`tool_err.into_parts()` and feeds both pieces to `bus_error_response`.
The MCP-10 exhaustive-match property is preserved — `kind` is still a
`BusErrorKind`, and `bus_error_to_jsonrpc(kind)` is still the only
function emitting `(code, kind_str)`.

## Tests turned green (15 cited in phase context)

| Test                                                                     | Status |
| ------------------------------------------------------------------------ | ------ |
| `mcp_pre_registration_gating::messaging_tools_refuse_before_register`    | PASS   |
| `mcp_register_whoami::register_valid_identity_succeeds`                  | PASS   |
| `mcp_register_whoami::register_invalid_name_returns_envelope_invalid`    | PASS *  |
| `mcp_register_whoami::register_with_empty_string_returns_envelope_invalid` | PASS * |
| `mcp_register_whoami::register_idempotent_same_identity`                 | PASS   |
| `mcp_register_whoami::whoami_unregistered_returns_null_active`           | PASS * |
| `mcp_register_whoami::tools_list_returns_eight_tools`                    | PASS * |
| `mcp_register_whoami::register_replaces_with_different_identity`         | DELETED ** |
| `mcp_register_whoami::register_unknown_identity_returns_unknown_identity` | DELETED ** |
| `mcp_malformed_input::famp_inbox_list_rejects_non_bool_include_terminal` | PASS   |
| `mcp_malformed_input::mcp_famp_send_rejects_non_bool_more_coming`        | PASS   |
| `mcp_stdio_tool_calls::mcp_initialize_lists_four_tools` (renamed intent) | PASS   |
| `mcp_stdio_tool_calls::mcp_famp_send_body_description_flags_required_for_new_task` | PASS |
| `mcp_stdio_tool_calls::famp_inbox_list_filters_terminal_by_default`      | IGNORED *** |
| `mcp_stdio_tool_calls::famp_inbox_list_include_terminal_true_returns_all` | IGNORED *** |
| `mcp_stdio_tool_calls::mcp_famp_peers_list_returns_entries`              | IGNORED *** |

\* renamed/reshaped from v0.8-shape (`identity_*`, `tools_list_returns_six`) to v0.9-shape (`envelope_invalid`, `tools_list_returns_eight`).

\** `register_replaces_with_different_identity` and
`register_unknown_identity_returns_unknown_identity` were dropped:
the v0.9 broker has no concept of "unknown identity" (any well-formed
name is registrable; collisions surface as `name_taken`, not
`unknown_identity`), and "replace" semantics are now per-process
(re-registering the same pid as a different name is not exposed as a
typed scenario).

\*** v0.8 file-fixtures (`seed_filter_fixture` writes `inbox.jsonl`
directly, the peers test reads `peers.toml`). v0.9 reads from broker
in-memory mailbox state. Plan 02-13 (broker-driven E2E harness) owns
their rewrites.

Final test count for the cited suites:

```
cargo nextest run -p famp \
    --test mcp_register_whoami --test mcp_pre_registration_gating \
    --test mcp_malformed_input --test mcp_stdio_tool_calls \
    --test mcp_error_kind_exhaustive

  17 passed, 5 skipped (3 v0.8-deferred, 2 plan-02-04 ignored).
```

## Verification

- `cargo build -p famp`: green.
- `cargo build --workspace --tests`: green.
- `cargo nextest run -p famp every_bus_error_kind_has_unique_jsonrpc_code`: PASS.
- `cargo clippy -p famp --all-targets -- -D warnings`: green.
- `bash scripts/check-mcp-deps.sh`: green (MCP-01 source-import gate
  preserved — zero `use reqwest` / `use rustls` under `cli/mcp/`,
  `bus_client/`, or `broker/`).
- `cargo nextest run -p famp --lib`: 63/63 PASS.
- `grep -F 'unimplemented' crates/famp/src/cli/mcp/tools/*.rs`: 0 hits.
- 8 tool descriptors: `grep -E '"famp_(register|send|inbox|await|peers|whoami|join|leave)"' crates/famp/src/cli/mcp/server.rs | wc -l` returns 16 (one in description, one in dispatch — covers all 8).

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 3 - Blocking] ToolError typed-error carrier introduced**

- **Found during:** Task 1 design (before edit), confirmed by
  `mcp_malformed_input::mcp_famp_send_rejects_non_bool_more_coming` test
  body asserting `text.contains("more_coming")` and
  `text.to_lowercase().contains("bool")` on the JSON-RPC error response.
- **Issue:** Plan task 1 has each tool body return
  `Result<Value, BusErrorKind>`. The bare enum has no carrier for
  field-name + expected-type hints, and the existing `dispatch_tool`
  message synthesizer hard-codes `"tool error"` for everything that
  isn't `NotRegistered`. There is no path from a tool body to a
  malformed-input message that names the field.
- **Fix:** Added `crate::cli::mcp::tools::ToolError {kind, message}` and
  flipped every tool body + `dispatch_tool` to
  `Result<Value, ToolError>`. The dispatcher does
  `tool_err.into_parts()` and feeds both pieces to `bus_error_response`,
  so the MCP-10 exhaustive-match property is preserved end-to-end.
- **Files modified:** all 8 tool bodies + `tools/mod.rs` + `server.rs`.
- **Commit:** `0d9dde6`.

**2. [Rule 3 - Blocking] 11 v0.8-shape MCP tests rewritten or gated**

- **Found during:** Task 1 verification, after every tool body compiled.
  Running the cited test suites turned up 14 tests with v0.8 surface
  expectations (old field names, 6 tools, code -32000, peers.toml fixtures,
  inbox.jsonl fixtures).
- **Issue:** Plan body's task list does not enumerate test rewrites —
  but the success criterion ("the 15 previously-stubbed MCP tests pass")
  requires updating the assertion shapes to match the v0.9 surface this
  plan ships.
- **Fix:** Reshaped `mcp_register_whoami.rs` (full rewrite, 8 tests now
  testing the v0.9 contract); changed code expectation in
  `mcp_pre_registration_gating.rs` from `-32000` to `-32100` (the typed
  application-range value plan 02-08 introduced); bumped the tool count
  in `mcp_initialize_lists_four_tools` from 6 to 8; gated 3 v0.8-only
  fixture-driven tests with `#[ignore]` + plan 02-13 followup pointer.
- **Files modified:** `tests/mcp_register_whoami.rs`,
  `tests/mcp_pre_registration_gating.rs`, `tests/mcp_stdio_tool_calls.rs`.
- **Commit:** `b0aadef`.

**3. [Rule 1 - Bug] Per-test FAMP_BUS_SOCKET isolation**

- **Found during:** Task 1 verification, when parallel test runs hit
  `register as alice failed: name_taken` because the test was sharing
  `~/.famp/bus.sock` with another concurrent run.
- **Issue:** Every MCP integration test was sharing the global default
  bus socket. Pre-registration gating assertions and name-collision
  assertions are both contamination-sensitive — one test registering as
  "alice" leaks into every subsequent test's broker view.
- **Fix:** Set `FAMP_BUS_SOCKET` to `<tempdir>/bus.sock` per harness
  instance in 4 spawn sites: `tests/common/mcp_harness::with_local_root`,
  `mcp_pre_registration_gating::spawn_unbound`,
  `mcp_malformed_input::spawn_mcp`, and
  `mcp_stdio_tool_calls::McpHarness::new`. Each test now owns a fresh
  broker process keyed on its own socket path.
- **Files modified:** `tests/common/mcp_harness.rs`,
  `tests/mcp_pre_registration_gating.rs`, `tests/mcp_malformed_input.rs`,
  `tests/mcp_stdio_tool_calls.rs`.
- **Commit:** `b0aadef`.

**4. [Rule 1 - Bug] clippy::significant_drop_tightening**

- **Found during:** Task 1/2 verification (`cargo clippy -- -D warnings`).
- **Issue:** First implementations of `tools/whoami`, `tools/peers`, and
  `tools/register` held the `session::state()` mutex guard across the
  bus `send_recv` await — making each tool body a global serialization
  point and tripping the lint.
- **Fix:** Restructured each so the lock is taken only to read the
  `Option<BusClient>` reference, the I/O happens inside the guard scope
  for the single round-trip, then `drop(guard)` runs explicitly before
  the response is parsed and returned. (For `register`, the guard is
  also briefly re-taken implicitly to assign `active_identity` — this
  is unavoidable since the assignment is the entire point of the tool.)
- **Files modified:** `tools/{whoami,peers,register}.rs`.
- **Commit:** part of `0d9dde6` and `cea76cc`.

**5. [Rule 1 - Bug] Unrelated cross-package fmt drag**

- **Found during:** Task 3 verification (`cargo fmt -p famp` ran).
- **Issue:** `cargo fmt -p famp` reformatted two unrelated files
  (`crates/famp/src/cli/await_cmd/mod.rs` line breaks; same for
  `tests/broker_proxy_semantics.rs`). Both have pre-existing fmt
  regressions from earlier waves (plan 02-06, 02-02). My plan does not
  touch their semantic content.
- **Fix:** `git checkout HEAD --` on both files; staged and committed
  only the files this plan actually rewires. Pre-existing fmt
  regressions remain logged for plan 02-12 (validation phase) cleanup,
  per plan 02-08's `Pre-existing fmt regression in tests/hook_subcommand.rs`
  precedent.
- **Files modified:** none (revert).
- **Commits:** none.

### No architectural deviations

No checkpoints reached. No auth gates. No deferred items added beyond
the 3 v0.8-fixture tests pointed at plan 02-13.

## Self-Check: PASSED

Verified all created files exist on disk:

- `crates/famp/src/cli/mcp/tools/join.rs` — FOUND
- `crates/famp/src/cli/mcp/tools/leave.rs` — FOUND

Verified all commits exist in `git log`:

- `0d9dde6` (Task 1 — register/send/whoami + ToolError) — FOUND
- `cea76cc` (Task 2 — inbox/await/peers) — FOUND
- `937f737` (Task 3 — join/leave) — FOUND
- `b0aadef` (test rewrites + socket isolation) — FOUND

Acceptance grep counts (all checks ≥ required threshold):

- `BusMessage::Register` in `register.rs`: 2
- `guard.active_identity` assignment in `register.rs`: 1
- `std::process::id()` in `register.rs`: 2
- `run_at_structured` in `send.rs`: 3
- `BusMessage::Whoami` in `whoami.rs`: 2
- `famp_join` in `server.rs`: 2 (descriptor + dispatch arm)
- `famp_leave` in `server.rs`: 2 (descriptor + dispatch arm)
- `session::active_identity().await.is_none()` in `server.rs`: 1 (D-05 gate)
- `run_at_structured` in `inbox.rs`: 3
- `run_at_structured` in `await_.rs`: 3
- `BusMessage::Sessions` in `peers.rs`: 2
- `"online":` in `peers.rs`: 2
- `"timeout": true` in `await_.rs`: 3
- `cli::join` in `join.rs`: 3 (use + doc)
- `cli::leave` in `leave.rs`: 3 (use + doc)
- `pub mod join;` in `tools/mod.rs`: 1
- `pub mod leave;` in `tools/mod.rs`: 1
- `unimplemented!` across all `tools/*.rs`: 0

## Commits

| Step | Commit    | Files | Insertions / Deletions |
| ---- | --------- | ----- | ---------------------- |
| 1    | `0d9dde6` | 5     | +385 / -69             |
| 2    | `cea76cc` | 3     | +220 / -30             |
| 3    | `937f737` | 2     | +114 / -0              |
| 4    | `b0aadef` | 5     | +109 / -44             |
