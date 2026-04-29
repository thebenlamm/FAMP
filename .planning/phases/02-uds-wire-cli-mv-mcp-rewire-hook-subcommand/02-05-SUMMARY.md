---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 05
subsystem: cli/inbox + cli/error + cli/mcp/tools/inbox
tags: [phase-2, wave-4, cli, inbox, d-10, bus-client]
requires:
  - 02-01 (BusClient + identity foundation)
  - 02-02 (broker daemon + D-10 wire + cursor_exec)
provides:
  - "`famp inbox list [--since <off>] [--include-terminal] [--as <name>]`
    — bus-backed JSONL dump via `Hello { bind_as: Some(identity) }` (D-10)
    + `BusMessage::Inbox`; emits one JSONL line per typed envelope
    followed by a `{\"next_offset\":N}` footer."
  - "`famp inbox ack --offset <N> [--as <name>]` — pure local atomic
    cursor advance via `cli::broker::cursor_exec::execute_advance_cursor`.
    NO broker round-trip per RESEARCH §6 (client is authoritative on
    per-session cursor)."
  - "`cli::inbox::list::run_at_structured(sock, ListArgs) -> ListOutcome`
    — typed-envelope MCP entry point reused by plan 02-09's
    `famp_inbox` tool."
  - "`cli::inbox::ack::run_at_structured(sock, AckArgs) -> AckOutcome`
    — local-only ack entry point for the MCP wrapper."
  - "`CliError::NotRegisteredHint { name }` — D-10 proxy validation
    failure surface (Hello-time and per-op liveness re-check)."
  - "`CliError::BrokerUnreachable` — Hello/IO failure other than
    NotRegistered."
  - "`CliError::BusError { kind, message }` — typed broker error reply
    pass-through."
affects:
  - crates/famp/src/cli/inbox/list.rs (rewritten — bus client transport)
  - crates/famp/src/cli/inbox/ack.rs (rewritten — client-side cursor only)
  - crates/famp/src/cli/inbox/mod.rs (dispatcher rewired; FAMP_HOME removed)
  - crates/famp/src/cli/error.rs (3 new variants)
  - crates/famp/src/cli/mcp/error_kind.rs (3 new exhaustive arms)
  - crates/famp/src/cli/mcp/tools/inbox.rs (shim to new structured entry points)
  - crates/famp/tests/mcp_error_kind_exhaustive.rs (3 fixture rows)
  - crates/famp/tests/cli_inbox.rs (test_inbox_ack_cursor wired GREEN)
  - crates/famp/tests/e2e_two_daemons.rs (helper inlined; reads inbox.jsonl
    directly for the v0.8 federation path)
  - crates/famp/tests/inbox_list_filters_terminal.rs (DELETED — v0.8)
  - crates/famp/tests/inbox_list_respects_cursor.rs (DELETED — v0.8)
tech-stack:
  added: []
  patterns:
    - "D-10 proxy: `BusClient::connect(sock, Some(identity))` carries
      identity at the connection level via `Hello.bind_as`; broker
      validates against live `famp register <name>` holder."
    - "Typed envelope wire shape: `BusReply::InboxOk { envelopes:
      Vec<serde_json::Value>, next_offset: u64 }` (Phase-1 D-09 evolved
      past raw `Vec<Vec<u8>>`). On-disk file is still raw bytes per
      line; the broker decodes via `AnyBusEnvelope::decode` between
      disk and wire."
    - "JSONL framing with `next_offset` footer: each envelope line +
      `{\"next_offset\":N}` final line, so the user can pipe to
      `famp inbox ack --offset $(... | tail -1 | jq .next_offset)`."
    - "Client-authoritative cursor: `inbox ack` is a pure local file
      write. NO `BusClient::connect`, NO Hello, NO `BusMessage` is
      sent. The cursor file lives at
      `<bus_dir>/mailboxes/.<identity>.cursor`."
    - "`run_at(sock, args, &mut (dyn Write + Send))` signature so the
      future is `Send` (clippy `future_not_send`); avoids the non-Send
      `StdoutLock` guard."
key-files:
  created:
    - "(none)"
  modified:
    - crates/famp/src/cli/inbox/list.rs
    - crates/famp/src/cli/inbox/ack.rs
    - crates/famp/src/cli/inbox/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/src/cli/mcp/tools/inbox.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
    - crates/famp/tests/cli_inbox.rs
    - crates/famp/tests/e2e_two_daemons.rs
  deleted:
    - crates/famp/tests/inbox_list_filters_terminal.rs
    - crates/famp/tests/inbox_list_respects_cursor.rs
decisions:
  - "`--offset` on `inbox ack` is REQUIRED (no implicit `ack everything
    just read` mode). Per RESEARCH §6 the value comes from the
    `next_offset` field of a prior `famp inbox list` output that the
    user feeds in explicitly. There is no implicit mode in v0.9."
  - "JSONL footer format is `{\"next_offset\":N}` on its own line.
    Plan 02-09's MCP `famp_inbox` tool surfaces this differently —
    as a structured field `{envelopes, next_offset}` in the tool
    response — but the CLI's footer-line shape is the canonical
    pipe-friendly form, designed for `tail -1 | jq .next_offset`."
  - "`BusReply::InboxOk` wire shape is `envelopes: Vec<serde_json::
    Value>` (typed envelopes), NOT `lines: Vec<Vec<u8>>` (raw bytes).
    Phase-1 D-09 evolved past raw bytes on the wire; the on-disk
    `mailboxes/<name>.jsonl` is still raw bytes per line, but the
    broker decodes via `AnyBusEnvelope::decode` between disk and wire."
  - "Connection identity uses `Hello { bind_as: Some(identity) }` per
    D-10. The broker reads the inbox of the bound identity, NOT the
    proxy connection's name. CLI subcommand never sends `Register`;
    the canonical holder is whichever process is running
    `famp register <name>` in another terminal."
  - "Three `CliError` variants added in this plan: `NotRegisteredHint
    { name: String }` (D-10 proxy validation failure with the
    user-visible hint that suggests starting `famp register <name>`),
    `BrokerUnreachable` (UDS Hello/IO failure other than
    NotRegistered), `BusError { kind: BusErrorKind, message: String }`
    (typed broker error reply pass-through). Plan 02-03 (parallel
    wave-3 `famp register`) plans the same variants with matching
    shapes; the merge will converge."
  - "v0.8 file-reader inbox tests deleted, not migrated. Plan 02-12
    owns the new bus-backed integration tests
    (`test_inbox_list`, `test_dm_roundtrip`); plan 02-09 owns the
    MCP tool integration tests. The deleted tests targeted the
    deleted `cli::inbox::list::run_list(home, since, include_terminal,
    out)` API and have no v0.9 analog."
  - "MCP tool wrapper (`cli/mcp/tools/inbox.rs`) shimmed in this plan
    to call `list::run_at_structured` and `ack::run_at_structured`
    directly. Plan 02-09 will further refactor to use a
    session-bound `BusClient`. Doing the shim now keeps the build
    green and reduces 02-09's surface."
metrics:
  duration: ~30min
  completed_date: 2026-04-28
---

# Phase 2 Plan 05: `famp inbox` Bus Rewire Summary

Wave-4 rewire of `famp inbox list` + `famp inbox ack` from v0.8
file-reader semantics (`run_list(home, since, include_terminal, out)`)
to the local UDS broker. Identity binding uses D-10 `Hello.bind_as`
proxy semantics; cursor management remains client-side per
RESEARCH §6 (the broker does not track per-session Inbox cursors).
CLI-03 wired (full E2E in plan 02-12), CLI-04 + CLI-10 GREEN
(atomic-write level on the ack path).

## What Shipped

### Task 1 — Rewire `inbox list` to `BusMessage::Inbox` + JSONL footer (commit `dae4011`)

`crates/famp/src/cli/inbox/list.rs` rewritten:

- `ListArgs { since: Option<u64>, include_terminal: bool, act_as:
  Option<String> }` with `--as` D-01 override flag.
- `run_at_structured(sock, args) -> ListOutcome` — resolves identity
  via `cli::identity::resolve_identity`, opens
  `BusClient::connect(sock, Some(identity))` (D-10 Hello.bind_as
  proxy), sends `BusMessage::Inbox { since, include_terminal:
  Some(_) }`, returns `ListOutcome { envelopes: Vec<Value>,
  next_offset: u64 }` from the `BusReply::InboxOk` reply (Phase-1
  D-09 typed-envelope wire shape; not raw `Vec<Vec<u8>>`).
- `run_at(sock, args, &mut (dyn Write + Send))` writes one JSONL
  line per typed envelope to `out` followed by a
  `{"next_offset":N}` footer.
- Hello-time `HelloFailed { kind: NotRegistered, .. }` →
  `CliError::NotRegisteredHint { name: identity }`. Per-op
  `BusReply::Err { kind: NotRegistered, .. }` → same hint
  (D-10 per-op liveness re-check fired). `BusReply::Err { kind,
  message }` → `CliError::BusError { kind, message }`. Other Hello
  failures → `CliError::BrokerUnreachable`. Unexpected reply →
  `CliError::Io { unexpected }`.

`crates/famp/src/cli/inbox/ack.rs` rewritten:

- `AckArgs { offset: u64, act_as: Option<String> }` — `--offset`
  REQUIRED (no implicit "ack everything just read" mode).
- `run_at_structured(sock, args) -> AckOutcome` — pure local file
  write via `cli::broker::cursor_exec::execute_advance_cursor`
  (atomic temp+rename, 0o600). NO `BusClient::connect`, NO Hello,
  NO `BusMessage`. The cursor file lives at
  `<bus_dir>/mailboxes/.<identity>.cursor`.
- `run` prints `{"acked":true,"offset":N}` to stdout.

`crates/famp/src/cli/inbox/mod.rs`: dispatcher rewired to the new
arg shapes; `home::resolve_famp_home` removed (the bus owns identity
resolution via `bus_client::resolve_sock_path`).

`crates/famp/src/cli/error.rs` + `crates/famp/src/cli/mcp/error_kind.rs`
(Rule 2 — critical, missing variants the plan body uses):

| Variant | MCP discriminator | Purpose |
| --- | --- | --- |
| `NotRegisteredHint { name }` | `not_registered_hint` | D-10 proxy validation failure with the operator hint |
| `BrokerUnreachable` | `broker_unreachable` | UDS Hello/IO failure (non-NotRegistered) |
| `BusError { kind: BusErrorKind, message }` | `bus_error` | typed broker error reply pass-through |

Plan 02-03 (parallel wave-3 register) plans the same variants with
matching shapes; the merge converges.

`crates/famp/src/cli/mcp/tools/inbox.rs`: shim to call
`list::run_at_structured` and `ack::run_at_structured` directly.
Output shape matches MCP-04 (`{envelopes, next_offset}` for list,
`{acked, offset}` for ack). Plan 02-09 will further refactor.

`crates/famp/tests/mcp_error_kind_exhaustive.rs`: three fixture rows
added; `every_variant_has_mcp_kind`, `mcp_kinds_are_unique`,
`mcp_kind_mapping_spot_checks` all pass.

`crates/famp/tests/e2e_two_daemons.rs`: federation E2E helper
inlined to read `<home>/inbox.jsonl` directly with the same
`terminal`-filter logic the deleted `run_list` had. The v0.8
federation listener still writes that file; Phase 4's FED-04
will rewrite this whole test against the bus or delete it.

### Task 2 — Wire `test_inbox_ack_cursor` GREEN (commit `23e7027`)

`crates/famp/tests/cli_inbox.rs::test_inbox_ack_cursor`:

1. Set `FAMP_BUS_SOCKET=$tmp/test-bus.sock`.
2. Pre-create the mailbox tree with three fake JSONL lines for
   alice (the lines are not actually read — the fixture is
   realistic only).
3. Run `famp inbox ack --offset 99 --as alice`. NO broker is
   running because `inbox ack` does NOT contact the broker.
4. Assert process exit 0 with stdout `{"acked":true,"offset":99}`.
5. Assert `<bus_dir>/mailboxes/.alice.cursor` exists, has mode
   0o600, and contains `99\n`.

Bundled clippy fixes against `-D pedantic`:

- `cli/inbox/list.rs::run_at` takes `&mut (dyn Write + Send)` so
  the future composes inside multi-threaded runtimes
  (`future_not_send`). Was `impl Write` with a non-`Send`
  `StdoutLock` guard.
- Doc-comment backtick + first-paragraph-too-long fixes in
  `cli/error.rs`, `cli/inbox/{ack,list}.rs`, `cli/mcp/tools/inbox.rs`.
- `tests/e2e_two_daemons.rs`: `extract_task_id_from_envelope` and
  `is_terminal_task` helpers hoisted to module scope
  (`items_after_statements`).

## Test Counts

- New integration test: 1 (`test_inbox_ack_cursor`) — GREEN.
- Deleted v0.8 file-reader tests: 2 (`inbox_list_filters_terminal.rs`,
  `inbox_list_respects_cursor.rs`).
- Exhaustive `mcp_error_kind` tests: 3 — all GREEN with the 3 new
  fixture rows.
- `cargo build -p famp`: green.
- `cargo build -p famp --tests`: green.
- `cargo nextest run -p famp --test cli_inbox`: 1/1 PASS.
- `cargo nextest run -p famp --test mcp_error_kind_exhaustive`:
  3/3 PASS.
- `cargo clippy -p famp --all-targets -- -D warnings`: green.

## D-10 Wire Compliance

Confirmed in commit `dae4011`:

- `grep -F 'BusClient::connect' crates/famp/src/cli/inbox/list.rs`
  → 1 line; the call passes `Some(identity)` as the second arg
  (D-10 Hello.bind_as proxy).
- `grep -F 'BusMessage::Inbox' crates/famp/src/cli/inbox/list.rs`
  → 2 lines (doc + send_recv site).
- `grep -F 'BusReply::InboxOk' crates/famp/src/cli/inbox/list.rs`
  → 2 lines (doc + match arm).
- `grep -F 'envelopes' crates/famp/src/cli/inbox/list.rs`
  → 7 lines (struct + destructure + iterate + footer JSON + docs).
- `grep -F 'next_offset' crates/famp/src/cli/inbox/list.rs`
  → 6 lines (footer + structured outcome + docs).
- `grep -F 'lines: Vec' crates/famp/src/cli/inbox/list.rs`
  → 0 lines (typed wire, NOT raw `Vec<Vec<u8>>`).
- `grep -F 'Vec<Vec<u8>>' crates/famp/src/cli/inbox/list.rs`
  → 0 lines.
- `grep -F 'base64' crates/famp/src/cli/inbox/list.rs`
  → 0 lines (no base64 encoding; raw `serde_json::Value`).

## ack-is-Local Compliance

Confirmed in commit `dae4011`:

- `grep -F 'BusClient::connect' crates/famp/src/cli/inbox/ack.rs`
  → 0 lines (no Hello, no proxy, no broker round-trip).
- `grep -F 'BusMessage::' crates/famp/src/cli/inbox/ack.rs`
  → 0 lines (no wire frame sent).
- `grep -F 'execute_advance_cursor' crates/famp/src/cli/inbox/ack.rs`
  → 1 line (the only side effect).
- The `test_inbox_ack_cursor` integration test runs WITHOUT
  spawning a broker — the cursor file write is synchronous, atomic
  (temp+rename), and confirmed mode 0o600.

## Deviations from Plan

### [Rule 2 - Critical] Add `CliError::NotRegisteredHint`, `BrokerUnreachable`, `BusError`

- **Found during:** Task 1 (compile gate)
- **Issue:** Plan 02-05 body references `CliError::NotRegisteredHint
  { name }`, `CliError::BrokerUnreachable`, and `CliError::BusError
  { kind, message }`, but none of those variants exist on the
  worktree base. Plan 02-03 (parallel wave-3) plans the same
  variants but had not run yet.
- **Fix:** Added the three variants to `cli/error.rs` with the
  shapes plan 02-03 specifies (verbatim — `BrokerUnreachable` is
  unit; `BusError` carries `famp_bus::BusErrorKind` and
  `String`; `NotRegisteredHint` carries `name: String` per the
  user-visible hint string the plan enforces). Added matching arms
  to `cli/mcp/error_kind.rs` (`not_registered_hint`,
  `broker_unreachable`, `bus_error`) and three fixture rows to
  `tests/mcp_error_kind_exhaustive.rs`. The `mcp_kinds_are_unique`
  test passes.
- **Files modified:** `crates/famp/src/cli/error.rs`,
  `crates/famp/src/cli/mcp/error_kind.rs`,
  `crates/famp/tests/mcp_error_kind_exhaustive.rs`.
- **Commit:** `dae4011`.

### [Rule 3 - Blocking] Delete obsolete v0.8 file-reader tests

- **Found during:** Task 1 (`cargo build -p famp --tests`)
- **Issue:** `tests/inbox_list_filters_terminal.rs` and
  `tests/inbox_list_respects_cursor.rs` import the deleted
  `cli::inbox::list::run_list(home, since, include_terminal, out)`
  API, plus `cli::inbox::list::extract_task_id_for_test`. Both
  helpers no longer exist on the bus-backed shape.
- **Fix:** Deleted both test files. Plan 02-12 owns the new
  bus-backed integration tests (`test_inbox_list`,
  `test_dm_roundtrip`) and plan 02-09 owns the MCP tool tests; the
  deleted tests had no v0.9 analog. Documented intent in the
  Task 1 commit body.
- **Files modified (deleted):** `crates/famp/tests/inbox_list_filters_terminal.rs`,
  `crates/famp/tests/inbox_list_respects_cursor.rs`.
- **Commit:** `dae4011`.

### [Rule 3 - Blocking] `tests/e2e_two_daemons.rs` helper inlined

- **Found during:** Task 1 (`cargo build -p famp --tests`)
- **Issue:** The federation E2E test `e2e_two_daemons_full_lifecycle`
  calls a helper that imports the deleted `cli::inbox::list::run_list`.
  The federation E2E targets the v0.8 listener, which still writes
  `<home>/inbox.jsonl`, so the read path is still meaningful — just
  no longer exposed via `cli::inbox::list`.
- **Fix:** Inlined the helper to read `inbox.jsonl` directly via
  `famp_inbox::read::read_from` and reproduced the
  `extract_task_id` + `terminal`-filter logic the v0.8 helper
  applied. Hoisted the inner `extract_task_id` and `is_terminal`
  closures to module-scope helpers (clippy
  `items_after_statements`). Test continues to assert the same
  filter properties without depending on the deleted API.
- **Files modified:** `crates/famp/tests/e2e_two_daemons.rs`.
- **Commit:** `dae4011` (initial inline) + `23e7027` (clippy hoist).

### [Rule 3 - Blocking] MCP `tools/inbox.rs` shim to new entry points

- **Found during:** Task 1 (compile gate after deleting the v0.8 API)
- **Issue:** The MCP `famp_inbox` tool wrapper at
  `cli/mcp/tools/inbox.rs` calls `list::run_list(home, ...)` and
  `ack::run_ack(home, offset)`. Both helpers were deleted by the
  plan. Plan 02-09 owns the MCP rewire but had not run yet, so the
  build was broken in the meantime.
- **Fix:** Rewrote `tools/inbox.rs` to call
  `list::run_at_structured(&sock, args)` and
  `ack::run_at_structured(&sock, args)` directly. Pulls the
  identity from the `IdentityBinding` (via the existing
  `act_as: Some(binding.identity)` field on the new args). Output
  shape now matches MCP-04 directly (`{envelopes, next_offset}` for
  list, `{acked, offset}` for ack — no JSONL parse-back needed).
  Plan 02-09 can further refactor when it lands the
  session-bound `BusClient`.
- **Files modified:** `crates/famp/src/cli/mcp/tools/inbox.rs`.
- **Commit:** `dae4011`.

### [Rule 1 - Bug] `run_at` future is not Send (StdoutLock guard)

- **Found during:** Task 2 (`cargo clippy -- -D warnings`)
- **Issue:** `run_at` initially took `mut out: impl std::io::Write`
  and the top-level `run` function passed `std::io::stdout().lock()`.
  The `StdoutLock` guard is `!Send` (it owns a
  `ReentrantLockGuard<RefCell<…>>`); the resulting future cannot
  cross thread boundaries on tokio multi-thread runtimes. Clippy's
  `future_not_send` flagged it.
- **Fix:** Changed signature to `run_at(sock, args, out: &mut (dyn
  Write + Send))` and pass `&mut std::io::stdout()` from `run`
  (the stdout handle itself is `Send`; the per-write locking
  happens internally). Mirrors the `cli::await_cmd::run_at`
  precedent (`out: &mut (dyn Write + Send)`).
- **Files modified:** `crates/famp/src/cli/inbox/list.rs`.
- **Commit:** `23e7027`.

### Worktree base sync (no-op)

- **Found during:** Executor startup
- **Issue:** Initial `git merge-base HEAD d84d83d` returned
  `e9e4e333` (HEAD), meaning HEAD was an ANCESTOR of the expected
  base, not a descendant — i.e. the worktree was 20 commits behind
  the orchestrator-expected base.
- **Fix:** `git merge --ff-only d84d83d` fast-forwarded the
  worktree branch to the expected base (BLOCKED `git reset --hard`
  in the sandbox forced the alternative). HEAD is now at the
  Wave-3-merged base; all Task 1/2 work proceeds on top.
- **Files modified:** None (working-tree state restored to expected
  base).

## Pre-Existing Issues (Not Caused by This Plan)

Documented in
`.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/deferred-items.md`
already, plus one new entry:

- `crates/famp/tests/hook_subcommand.rs:77` and `:114` fail
  `cargo fmt -- --check` on the worktree base. These lines were
  authored by a previous wave; not touched by plan 02-05. The
  `tests/cli_inbox.rs` fmt diff produced by Task 2 was fixed in
  the Task 2 commit.

## Self-Check: PASSED

- [x] `crates/famp/src/cli/inbox/list.rs` exists; references
  `BusMessage::Inbox`, `BusReply::InboxOk`, `BusClient::connect`,
  `next_offset`, `--as`. No `lines: Vec`, no `Vec<Vec<u8>>`, no
  `base64`.
- [x] `crates/famp/src/cli/inbox/ack.rs` exists; references
  `execute_advance_cursor`. No `BusClient::connect`, no
  `BusMessage::`.
- [x] `crates/famp/src/cli/inbox/mod.rs` rewired; dispatches
  `InboxCommand::List` → `list::run` and
  `InboxCommand::Ack` → `ack::run`.
- [x] `crates/famp/tests/cli_inbox.rs::test_inbox_ack_cursor` PASS
  (cursor file mode 0o600, body `99\n`, stdout
  `{"acked":true,"offset":99}`).
- [x] `crates/famp/tests/mcp_error_kind_exhaustive.rs`: 3/3 PASS
  with three new fixture rows.
- [x] `cargo build -p famp` exits 0.
- [x] `cargo build -p famp --tests` exits 0.
- [x] `cargo clippy -p famp --all-targets -- -D warnings` exits 0.
- [x] No git deletions across either commit other than the two
  documented v0.8 test files.

## Commits

| Task | Commit | Files | Insertions / Deletions |
|------|--------|-------|------------------------|
| 1    | `dae4011` | 10 | +335 / -743 |
| 2    | `23e7027` | 6  | +114 / -55  |
