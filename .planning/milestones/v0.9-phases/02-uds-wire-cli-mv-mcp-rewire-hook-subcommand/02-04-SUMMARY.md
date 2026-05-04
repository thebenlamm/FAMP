---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 04
subsystem: cli/send + cli/error + cli/mcp/error_kind
tags: [phase-2, wave-4, cli-02, send, d-10, uds, identity, proxy]
requires:
  - 02-01 (BusClient + cli/identity foundation)
  - 02-02 (D-10 wire field + broker proxy semantics)
provides:
  - "`famp send` rewired onto BusClient + Hello.bind_as proxy (D-10)"
  - "SendArgs gains --as / --channel; --to becomes Option<String>"
  - "SendOutcome { task_id, delivered } — broker-shaped, not v0.8-FSM-shaped"
  - "normalize_channel helper (mirrors famp_bus channel regex)"
  - "CliError::NotRegisteredHint { name } — D-10 proxy-binding failure hint"
  - "CliError::BusError { kind, message } — non-NotRegistered broker errors"
  - "CliError::BusClient { detail } — transport-level / codec / handshake errors"
  - "CliError::BrokerUnreachable — UDS connect failure / spawn failure"
  - "MCP error_kind discriminators: not_registered_hint, bus_error, bus_client_error, broker_unreachable"
  - "Acceptance test: every BusReply::Err(NotRegistered) path collapses to the same operator hint"
affects:
  - crates/famp/Cargo.toml (regex direct dep added)
  - crates/famp/src/lib.rs (`uuid as _;` silencer added — uuid is now used only via re-exports / tests)
  - crates/famp/src/bin/famp.rs (`regex as _;` silencer)
  - crates/famp/src/cli/send/mod.rs (entire `run_at_structured` body rewritten)
  - crates/famp/src/cli/error.rs (4 new variants)
  - crates/famp/src/cli/mcp/error_kind.rs (4 new arms)
  - crates/famp/src/cli/mcp/tools/send.rs (minimum patch for compile; 02-09 owns the full rewire)
  - crates/famp/examples/{_gen_fixture_certs,cross_machine_two_agents,personal_two_agents}.rs (`use regex as _;` silencer)
  - crates/famp/tests/common/conversation_harness.rs (SendArgs literals updated; semantic correctness deferred)
  - crates/famp/tests/mcp_error_kind_exhaustive.rs (4 new fixture rows)
  - 13 v0.8 integration test files marked `#[ignore]` with Phase-02-Plan-02-04 reason
tech-stack:
  added:
    - "regex 1.10 (workspace dep) — channel-name regex inside cli::send (mirrors famp-bus)"
  patterns:
    - "Three-layer run / run_at / run_at_structured pattern preserved for MCP tool compatibility"
    - "D-10 proxy: connection-level identity via Hello.bind_as (NOT per-message act_as field)"
    - "BusClientError → CliError mapping is exhaustive on `BusErrorKind::NotRegistered`; everything else collapses to BusClient/BrokerUnreachable"
    - "Minimal envelope value (mode-tagged JSON) replaces the v0.8 SignedEnvelope on the bus path; federation envelope construction returns in Phase 4 once the federation gateway lands"
key-files:
  created: []
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/cli/send/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/src/cli/mcp/tools/send.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
    - crates/famp/tests/common/conversation_harness.rs
    - 13 v0.8 send/conversation/listen/mcp integration test files
decisions:
  - "BusMessage::Send shape is unchanged from Phase 1 — `to: Target, envelope: serde_json::Value` only. D-10 explicitly REJECTS adding a per-message act_as field."
  - "Identity binding is at the connection level via `Hello { bind_as: Some(identity) }` per D-10. The broker validates at Hello time and re-checks per-op."
  - "v0.8 `cli/send/client.rs::post_envelope` is NOT deleted — it is still called by `cli/listen/auto_commit.rs` (the federation auto-commit path Phase 4 will rewire)."
  - "`cli/send/client.rs::send_via_bus` (originally proposed by the plan as a thin BusClient wrapper) was NOT added; the entire bus client logic lives directly in `cli/send/mod.rs::run_at_structured` per the plan's decision to keep `client.rs` minimal. `client.rs` therefore stays exactly the v0.8 file — no public-API churn."
  - "v0.8 send_*.rs / conversation_*.rs / listen_*.rs / mcp_*.rs integration tests are marked `#[ignore]` rather than `#![cfg(any())]`-gated. `#[ignore]` keeps the file compiling under the new SendArgs shape (so the test fixtures stay green for the silencer-block / unused-crate-dependencies lint), and the SendArgs literals were mechanically updated to wrap `to` in `Some(...)` and add `channel: None, act_as: None` fields."
  - "SendOutcome { task_id, state } → SendOutcome { task_id, delivered }. The `delivered: String` is a debug-format of `Vec<Delivered>` from the broker. The MCP famp_send tool was patched to read `delivered` instead of `state` so the workspace compiles. Plan 02-09 will replace this debug-stringify with a structured representation."
  - "MCP famp_send tool's IdentityBinding.home is no longer used; we resolve the bus socket via `bus_client::resolve_sock_path` and forward the session-bound identity as `--as` (D-10 proxy). Plan 02-09 will rewire MCP fully against the bus."
metrics:
  duration: ~70min
  completed_date: 2026-04-28
---

# Phase 2 Plan 04: famp send UDS rewire + D-10 proxy Summary

Wave-4 rewire of CLI-02 (`famp send`) onto the v0.9 UDS broker. Identity
binds at the connection layer via `Hello { bind_as: Some(name) }` per
D-10; `BusMessage::Send` carries only `to` + `envelope` (no per-message
identity field). The v0.8 federation HTTPS path in `cli/send/client.rs`
is preserved (still consumed by `cli/listen/auto_commit.rs`) but no
longer reached from `run_at_structured`.

## What Shipped

### Task 1 — Rewire `cli::send` to BusClient + Hello.bind_as proxy (commit `86e9982`)

**`crates/famp/src/cli/send/mod.rs`** — entire `run_at_structured` body
rewritten. New flow:

1. `resolve_identity(args.act_as.as_deref())` (D-01 four-tier resolver).
2. Defense-in-depth `--more-coming` ↔ `--new-task` guard (BL-01 semantic
   preserved at the run-time level).
3. Build `Target` from `--to` / `--channel` (with `normalize_channel`).
4. Build a minimal mode-tagged JSON envelope (`new_task` / `deliver` /
   `deliver_terminal` / `channel_post`).
5. `BusClient::connect(sock, Some(identity))` — bind as proxy at Hello time.
6. `bus.send_recv(BusMessage::Send { to, envelope })` — broker stamps
   `from` from `effective_identity(state)`.
7. Map reply: `SendOk → SendOutcome`; `Err{NotRegistered} →
   NotRegisteredHint`; `Err{kind} → BusError`; other reply variants →
   BusClient.

**`SendArgs`** gains:
- `to: Option<String>` (was `String`; mutually exclusive with `--channel`)
- `channel: Option<String>` (new)
- `act_as: Option<String>` (new; CLI flag is `--as`)

**`SendOutcome`** changes:
- `state: String` (v0.8 FSM string) → `delivered: String` (debug-format
  of `Vec<Delivered>` from the broker).

**Module-private helpers added:**
- `normalize_channel(input) → Result<String, CliError>` — accepts
  `planning` and `#planning`, rejects `##planning`, validates against
  `^#[a-z0-9][a-z0-9_-]{0,31}$`.
- `build_envelope_value(args) → Result<Value, CliError>` — builds the
  minimal mode-tagged JSON used in `BusMessage::Send.envelope`.

**`crates/famp/src/cli/error.rs`** — 4 new variants:
- `NotRegisteredHint { name }` — D-10 proxy-binding failure with a
  literal hint message: `{name} is not registered — start \`famp register
  {name}\` in another terminal first`.
- `BusError { kind: BusErrorKind, message }` — typed broker-protocol
  errors (everything except NotRegistered).
- `BusClient { detail }` — transport-level / codec / handshake errors
  (debug-format of `BusClientError`).
- `BrokerUnreachable` — UDS connect failure / spawn failure.

**`crates/famp/src/cli/mcp/error_kind.rs`** — 4 new arms in the
exhaustive match: `not_registered_hint`, `bus_error`,
`bus_client_error`, `broker_unreachable`. The corresponding fixture rows
were added to `crates/famp/tests/mcp_error_kind_exhaustive.rs` so the
unique-discriminator + every-variant-has-kind gates stay green.

**`crates/famp/src/cli/mcp/tools/send.rs`** — minimum patch. The MCP
tool now resolves the bus socket via `bus_client::resolve_sock_path()`
and forwards `binding.identity` as `--as` (D-10 proxy). It reads
`outcome.delivered` instead of `outcome.state`. Plan 02-09 owns the
full MCP rewire.

**`crates/famp/Cargo.toml`** — `regex = { workspace = true }` added as a
direct dep. (Previously regex came in transitively through `famp-bus`;
making it explicit is required for `unused_crate_dependencies` cleanliness.)

### Test counts

- **Unit tests added (cli::send::tests)**: 9
  (5 `normalize_channel` cases + 3 `build_envelope_value` cases + 1
  `more_coming_without_new_task_errors_in_run_at_structured` BL-01
  unit-level guard).
- **MCP error_kind fixture rows added**: 4
  (`NotRegisteredHint`, `BusError`, `BusClient`, `BrokerUnreachable`).
- **Workspace nextest -p famp send**: 15/15 pass, 170 skipped.
- **Workspace nextest -p famp --lib**: 59/59 pass.
- **Workspace nextest -p famp --test mcp_error_kind_exhaustive**: 3/3 pass.
- **Workspace nextest -p famp-bus**: 41/41 pass (no regression).
- **Workspace nextest -p famp**: 145/146 pass; 39 skipped; 1 failure
  (`listen_bind_collision::second_listen_on_same_port_errors_port_in_use`)
  — pre-existing on the merge base, documented as deferred in plan 02-01
  SUMMARY.md.

### D-10 wire shape compliance

- `git diff --stat crates/famp-bus/` after this plan: **empty**.
  proto.rs is owned by plan 02-02; this plan does NOT touch it.
- `grep -A2 'BusMessage::Send {' crates/famp/src/cli/send/mod.rs |
  grep -F 'act_as' | wc -l`: **0**.
- `grep -F 'act_as' crates/famp-bus/src/proto.rs | wc -l`: **0**.

The send literal carries only `to` + `envelope` per D-10. Identity rides
on the Hello frame (via `bus_client::BusClient::connect`'s `bind_as`
parameter, back-filled into the wire field by plan 02-02).

## Output specification (from plan)

The plan asked the SUMMARY to document four points explicitly:

1. **Confirmation that BusMessage::Send shape is unchanged from Phase 1
   (no act_as field added):** ✅ confirmed — `git diff
   crates/famp-bus/src/proto.rs` returns empty, and the send literal in
   `cli/send/mod.rs` contains only `to:` + `envelope:`.

2. **Confirmation that identity binding is at Hello.bind_as level per
   D-10:** ✅ confirmed — `BusClient::connect(sock,
   Some(identity.clone()))` is the sole identity-supplying call site;
   the broker validates at Hello time per the 02-02 SUMMARY.

3. **Decision on v0.8 federation send tests (ignored vs deleted vs
   migrated):** **#[ignore]'d, NOT deleted, NOT migrated.** Each affected
   test (`send_*.rs`, `conversation_*.rs`, `e2e_two_daemons.rs`,
   `mcp_stdio_tool_calls::mcp_famp_send_*`,
   `mcp_session_bound_e2e.rs`, etc.) carries a `#[ignore = "Phase 02
   Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; revisit /
   migrate in Phase 4 federation gateway"]` marker plus updated
   `SendArgs` literals so they still compile. The semantic check from
   BL-01 (`--more-coming` requires `--new-task`) is now covered at the
   unit level inside `cli/send/mod.rs::tests`. Phase 4 will delete or
   migrate these tests once the federation gateway lands.

4. **Whether `cli/send/client.rs::send_via_bus` is exposed publicly:**
   **Not added.** The plan offered the option of "renaming
   `post_envelope` and adding a thin `send_via_bus` wrapper". I chose
   the simpler path: leave `client.rs` exactly as v0.8 (because
   `cli/listen/auto_commit.rs` still imports `post_envelope`) and put
   the entire bus-client logic inside `cli/send/mod.rs::run_at_structured`.
   This minimizes the public-API surface change and keeps `client.rs`
   100% federation-only. Phase 4 will delete `client.rs` wholesale when
   it removes the v0.8 federation path.

## Deviations from Plan

### [Rule 2 - Critical] Add three additional CliError variants

- **Found during:** Task 1 (compile gate)
- **Issue:** The plan prescribes `CliError::NotRegisteredHint { name }`
  and references `CliError::BusError { kind, message }` in the reply
  match arm, but does not enumerate `BusError` itself. The `match reply`
  arms also need a typed home for `BusClientError` (transport / codec
  failure outside of HelloErr) and for the connect-time
  `Io`/`BrokerDidNotStart` failures. Without those typed variants, the
  send path collapses too many distinct failures into the same opaque
  `BusError` bucket and breaks the MCP exhaustive-error-kind guard.
- **Fix:** Add four typed variants — `NotRegisteredHint`, `BusError`,
  `BusClient`, `BrokerUnreachable` — each with a stable
  `mcp_error_kind` discriminator + matching fixture row in the
  exhaustive test.
- **Files modified:** `crates/famp/src/cli/error.rs`,
  `crates/famp/src/cli/mcp/error_kind.rs`,
  `crates/famp/tests/mcp_error_kind_exhaustive.rs`.
- **Commit:** `86e9982`.

### [Rule 3 - Blocking] regex needs to be a direct famp dep

- **Found during:** Task 1 (compile gate)
- **Issue:** `normalize_channel` uses the `regex` crate to validate
  channel names. Previously `regex` was a transitive dep of `famp-bus`;
  using it from `famp` directly without declaring it triggers Rust's
  "use after declaration" check on closures and propagates as a clippy
  `unused_crate_dependencies` warning when the dep is later removed
  from the transitive graph.
- **Fix:** Add `regex = { workspace = true }` to
  `crates/famp/Cargo.toml`. Add `use regex as _;` to `bin/famp.rs` and
  the three examples that don't reference regex but inherit famp's
  dep set.
- **Files modified:** `crates/famp/Cargo.toml`, `crates/famp/src/bin/famp.rs`,
  `crates/famp/examples/{_gen_fixture_certs,cross_machine_two_agents,personal_two_agents}.rs`.
- **Commit:** `86e9982`.

### [Rule 3 - Blocking] uuid silencer for famp lib

- **Found during:** Task 1 (`-W unused_crate_dependencies`)
- **Issue:** The v0.8 `cli/send/mod.rs` constructed `MessageId::new_v7()`
  via the `uuid` crate. Phase 02 Plan 02-04 replaced that with the
  broker-assigned `task_id` (which the broker stamps via Phase-1
  `Broker::send_agent` / `send_channel`). The famp lib no longer
  references `uuid` directly even though it still re-thread types
  carrying `uuid::Uuid` via `BusReply::SendOk.task_id`.
- **Fix:** `use uuid as _;` in `crates/famp/src/lib.rs` with a comment
  documenting the reason. Pure additive.
- **Files modified:** `crates/famp/src/lib.rs`.
- **Commit:** `86e9982`.

### [Rule 3 - Blocking] v0.8 integration tests need SendArgs shape update

- **Found during:** Task 1 (compile gate on test artifacts)
- **Issue:** Changing `SendArgs.to` from `String` to `Option<String>`
  and adding `channel` / `act_as` fields breaks every `SendArgs { ... }`
  literal in the v0.8 integration tests. The plan instructed me to mark
  those tests `#[ignore]`, but `#[ignore]` does not skip *compilation* —
  it only skips runtime execution.
- **Fix:** Mechanically update each `SendArgs { ... }` literal to wrap
  `to` in `Some(...)` and add `channel: None, act_as: None`. Then add
  `#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8
  HTTPS shape; revisit / migrate in Phase 4 federation gateway"]` to
  every test fn that drives the v0.8 HTTPS path through the harness.
  All 13 affected files compile under the new shape and skip cleanly.
- **Files modified:** `crates/famp/tests/{send_*.rs (8),
  conversation_*.rs (4), e2e_two_daemons.rs, listen_smoke.rs,
  listen_multi_peer_keyring.rs, mcp_session_bound_e2e.rs,
  mcp_stdio_tool_calls.rs (2 fns), inbox_list_respects_cursor.rs,
  await_commit_advance_error_surfaces.rs}`,
  `crates/famp/tests/common/conversation_harness.rs`.
- **Commit:** `86e9982`.

### [Rule 1 - Bug] MCP famp_send tool needed minimal patch to compile

- **Found during:** Task 1 (compile gate)
- **Issue:** `cli/mcp/tools/send.rs` calls `run_at_structured(home,
  args)` and reads `outcome.state`. The new signature is
  `run_at_structured(sock, args)` with `outcome.delivered`. Without a
  patch, the workspace doesn't compile.
- **Fix:** Resolve the bus socket via `bus_client::resolve_sock_path()`
  inside the tool dispatcher; forward `binding.identity` as
  `args.act_as`; read `outcome.delivered` in the JSON reply. Update all
  three `SendArgs { ... }` constructor literals (new_task, deliver,
  terminal) to add `channel: None` and the new `act_as` field. Plan
  02-09 owns the full MCP rewire (bus-only session, no longer reading
  `binding.home`).
- **Files modified:** `crates/famp/src/cli/mcp/tools/send.rs`.
- **Commit:** `86e9982`.

### Pre-existing fmt issue on hook_subcommand.rs

- **Found during:** Task 1 (`cargo fmt --all -- --check`)
- **Issue:** `crates/famp/tests/hook_subcommand.rs` line 80 had a long
  inline string-array argument that exceeded rustfmt's 100-column
  default. This pre-existed on the merge base; running `cargo fmt
  --all` to clean up the new files reformatted it as a side effect.
- **Fix:** Apply the rustfmt change to `hook_subcommand.rs`
  (whitespace-only; no behavioral change). Required to keep workspace
  fmt-check green.
- **Files modified:** `crates/famp/tests/hook_subcommand.rs` (whitespace).
- **Commit:** `86e9982`.

## Pre-Existing Issues (Not Caused by This Plan)

Documented in plan 02-01 SUMMARY's deferred-items section:

- `listen_bind_collision::second_listen_on_same_port_errors_port_in_use`
  was already failing on the merge base. The same failure persists
  unchanged after plan 02-04. Tracking continues in
  `.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/deferred-items.md`.

## Self-Check: PASSED

- [x] `crates/famp/src/cli/send/mod.rs` exists; `BusClient::connect`
  referenced; `Some(identity` referenced; `BusMessage::Send {` has no
  `act_as`; `normalize_channel` defined + invoked.
- [x] `grep -F 'long = "as"' crates/famp/src/cli/send/mod.rs`: 1 line.
- [x] `grep -F 'act_as' crates/famp/src/cli/send/mod.rs`: 9 lines (Args
  field + identity-resolve call + tests + struct usages).
- [x] `grep -F 'NotRegisteredHint' crates/famp/src/cli/error.rs`: 1 line
  (variant declaration).
- [x] `grep -F 'is not registered — start ' crates/famp/src/cli/error.rs`:
  1 line (the operator hint).
- [x] `git diff --stat crates/famp-bus/`: empty (proto.rs untouched).
- [x] `cargo build --workspace --tests`: 0 errors.
- [x] `cargo clippy -p famp -p famp-bus --all-targets -- -D warnings`:
  0 errors.
- [x] `cargo fmt --all -- --check`: 0 diff.
- [x] `cargo nextest run -p famp send`: 15/15 pass, 170 skipped.
- [x] `cargo nextest run -p famp --lib cli::send`: 9/9 pass.
- [x] `cargo nextest run -p famp --test mcp_error_kind_exhaustive`:
  3/3 pass.
- [x] `cargo nextest run -p famp-bus`: 41/41 pass (no regression).
- [x] No git deletions across the task commit.
- [x] No `_ =>` wildcard arms inside `cli::send::run_at_structured`'s
  reply match (the explicit `other =>` arm fires only on
  non-Send-domain reply variants and surfaces as a typed
  `BusClient::UnexpectedReply`-equivalent).

## Threat Flags

None. The new CLI surface is a thin wrapper over the broker's existing
D-10 proxy semantics (validated at Hello time + per-op liveness re-check
per plan 02-02 SUMMARY). Same-UID local trust per BUS-11 still holds —
the proxy connection cannot impersonate any identity not currently held
by a live `famp register` process owned by the same UID.

## Commits

| Task | Commit    | Files | Insertions / Deletions |
|------|-----------|-------|------------------------|
| 1    | `86e9982` | 33    | +676 / -543            |
