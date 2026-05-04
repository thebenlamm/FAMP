---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 07
subsystem: cli
tags: [cli, channels, sessions, identity, d-10-proxy]
requires:
  - cli/identity::resolve_identity (D-01) — plan 02-03
  - bus_client::BusClient::connect(sock, Some(name)) (D-10) — plan 02-02
  - famp_bus::{BusMessage, BusReply, SessionRow} — Phase 1
provides:
  - famp join <#channel> [--as <name>] (CLI-06)
  - famp leave <#channel> [--as <name>] (CLI-06)
  - famp sessions [--me] (CLI-08, CLI-11)
  - famp whoami [--as <name>] (CLI-07)
  - cli::util::normalize_channel — promoted from cli/send/mod.rs to a shared module
  - cli::join::run_at_structured / JoinOutcome — MCP-tool reuse entry (plan 02-09)
  - cli::leave::run_at_structured / LeaveOutcome — MCP-tool reuse entry (plan 02-09)
  - cli::sessions::run_at_structured / SessionsOutcome — MCP-tool reuse entry (plan 02-09)
  - cli::whoami::run_at_structured / WhoamiOutcome — MCP-tool reuse entry (plan 02-09)
  - cli::block_on_async — shared multi-thread runtime helper for async dispatch arms
affects:
  - crates/famp/src/cli/mod.rs (Commands enum, dispatcher)
  - crates/famp/src/cli/send/mod.rs (relocated normalize_channel import)
tech-stack:
  added: []
  patterns:
    - D-10 Hello.bind_as proxy for every identity-bound subcommand
    - Channel-name auto-prefix + regex validation shared across send/join/leave
    - Structured run_at_structured entry per subcommand for MCP reuse
key-files:
  created:
    - crates/famp/src/cli/util.rs
    - crates/famp/src/cli/join.rs
    - crates/famp/src/cli/leave.rs
    - crates/famp/src/cli/sessions.rs
    - crates/famp/src/cli/whoami.rs
  modified:
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/send/mod.rs
decisions:
  - Refactored cli::run dispatcher to use a shared block_on_async helper rather than copy-pasting the multi_thread runtime construction across every async arm — prevents a class of copy-paste-typo risk and keeps the function under the clippy::pedantic too_many_lines threshold.
  - normalize_channel relocated to cli/util.rs (with its 5 unit tests) rather than re-imported via a `pub use` from send/mod.rs — establishes cli/util as the canonical home for any future cross-subcommand helper and avoids the implicit "send owns this" coupling the plan flagged.
  - Sessions arm with `bind_as = None` is an explicit observer-only connection. The error mapping returns `NotRegisteredHint` only when --me was set; without --me, all HelloFailed kinds collapse to BrokerUnreachable.
metrics:
  duration: ~30min (single agent, sequential tasks)
  completed_date: "2026-04-28"
---

# Phase 2 Plan 7: Top-level CLI subcommands (join / leave / sessions / whoami) Summary

Round out the v0.9 CLI surface with the four channel + introspection primitives, all riding on the D-10 Hello.bind_as proxy. Identity binding is uniformly connection-level (never per-message); join/leave mutations land on the canonical live registered holder so one-shot CLI processes can exit without auto-leaving the channel.

## What shipped

### Task 1 — `cli/util.rs`, `cli/join.rs`, `cli/leave.rs` (commit `b46f68d`)

- `cli/util.rs`: `pub fn normalize_channel(input: &str) -> Result<String, CliError>` — promoted from `cli/send/mod.rs`. Accepts `planning` and `#planning`, rejects `##*`, validates against `^#[a-z0-9][a-z0-9_-]{0,31}$` (byte-equivalent to `famp_bus::proto::CHANNEL_PATTERN`). 5 unit tests relocated alongside the helper.
- `cli/join.rs`: `JoinArgs { channel, act_as }`, `JoinOutcome { channel, members, drained: Vec<serde_json::Value> }`, `run` (CLI), `run_at_structured` (MCP). On `BusReply::JoinOk`, the structured outcome carries the typed envelopes verbatim (Phase-1 D-09 wire shape); the CLI surface prints only the count for ergonomics.
- `cli/leave.rs`: `LeaveArgs { channel, act_as }`, `LeaveOutcome { channel }`, same `run` / `run_at_structured` split.
- `cli/mod.rs`: `pub mod util; pub mod join; pub mod leave;` + `Commands::Join(JoinArgs)` and `Commands::Leave(LeaveArgs)` variants + matching tokio-runtime dispatch arms.
- `cli/send/mod.rs`: now imports `normalize_channel` from `cli::util`; the inline copy and its 5 tests deleted (no logic change).

### Task 2 — `cli/sessions.rs`, `cli/whoami.rs`, dispatcher refactor (commit `5a1c3fb`)

- `cli/sessions.rs`: `SessionsArgs { me: bool }`, `SessionsOutcome { rows: Vec<SessionRow> }`, `run`/`run_at`/`run_at_structured`. `--me` resolves identity via D-01 and uses Hello.bind_as proxy; without `--me` the connection is unbound (observer-only). Output: one `SessionRow` JSONL line per filtered row.
- `cli/whoami.rs`: `WhoamiArgs { act_as }`, `WhoamiOutcome { active: Option<String>, joined: Vec<String> }`. Always opens a Hello.bind_as proxy — `whoami` without an identity-bound connection is a useless echo. Output: `{"active":"<name>","joined":[...]}`.
- `cli/mod.rs`: `pub mod sessions; pub mod whoami;` + `Commands::Sessions` / `Commands::Whoami` variants and dispatch arms. Extracted shared `block_on_async<F>(fut)` helper so every async dispatch arm is a single-line call. The dispatcher dropped from 116 lines to ~25 (back under `clippy::pedantic` `too_many_lines = 100`).

## D-10 proxy semantics — confirmed

Every identity-bound subcommand (`join`, `leave`, `whoami`, `sessions --me`) opens its `BusClient` via `BusClient::connect(sock, Some(identity))`. Per plan 02-02 broker logic:

- `Hello { bind_as: Some(name) }` triggers liveness validation against the canonical registered holder → `HelloErr { NotRegistered }` if the holder is gone.
- Per-op (`Send`/`Inbox`/`Join`/`Leave`/`Whoami`) liveness re-check returns `Err { NotRegistered }` if the holder dies between Hello and the op.
- Both surface as `CliError::NotRegisteredHint { name }` with the operator hint `"<name> is not registered — start `famp register <name>` in another terminal first"`.

The invariant from plan 02-02 — **the broker mutates the canonical holder's `joined` set on Join/Leave, NOT the proxy connection's** — is documented in the module docs of `join.rs` / `leave.rs`. Plan 02-11 will verify it at integration level by running `famp join --as alice #x` from a one-shot process and asserting alice still appears in `#x`'s member list after the CLI exits.

`sessions` without `--me` is the only command in the new set that opens an unbound observer connection (`bind_as: None`) — read-only ops do not require a bound identity.

## Output shapes

| Subcommand | stdout JSONL |
|---|---|
| `famp join #planning` | `{"channel":"#planning","members":["alice","bob"],"drained":3}` |
| `famp leave #planning` | `{"channel":"#planning"}` |
| `famp sessions` | `{"name":"alice","pid":12345,"joined":["#planning"]}\n{"name":"bob",...}` |
| `famp sessions --me` | (filtered to caller's identity only) |
| `famp whoami --as alice` | `{"active":"alice","joined":["#planning","#standup"]}` |

`run_at_structured` callers (the future MCP `famp_join` / `famp_leave` / `famp_sessions` / `famp_whoami` tools in plan 02-09) get the typed outcomes directly — `Vec<serde_json::Value>` for join's drained envelopes, `Vec<SessionRow>` for sessions, etc.

## Verification

- `cargo build -p famp`: green (1m 05s clean build, 4s incremental after Task 2).
- `cargo run -p famp -- {join,leave,sessions,whoami} --help`: all four exit 0 with documented flag surface.
- `cargo clippy -p famp --all-targets -- -D warnings`: green after the dispatcher extraction + doc-backtick polish.
- `cargo test -p famp --lib cli::util`: 5/5 normalize_channel tests pass at the new location.
- Acceptance grep checks (Task 1 + Task 2): all green except `Commands::Join`/`Commands::Leave` count-of-2 — the variant declaration inside the enum body uses bare `Join(...)` / `Leave(...)` syntax rather than `Commands::Join` / `Commands::Leave`, so the literal grep returns 1 (variant) + 1 (dispatch arm) on separate searches. The intent (variant + dispatch both present) is satisfied; the plan's `grep -F 'Commands::Join'` count assumed an enum body that prefixed the type name, which is not how Rust enum syntax renders.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] cli::run dispatcher exceeded clippy::pedantic too_many_lines after adding 4 new arms**

- **Found during:** Task 2 verification (`cargo clippy -p famp --all-targets -- -D warnings`)
- **Issue:** Adding Sessions + Whoami dispatch arms pushed `pub fn run` from ~110 lines to 116 lines, tripping `clippy::too_many_lines = 100`. The plan's verification step explicitly mandates clippy green, so this blocked task completion.
- **Fix:** Extracted the repeated `tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(...)?; rt.block_on(fut)` boilerplate into a private `block_on_async<F>(fut: F) -> Result<(), CliError>` helper. Each async dispatch arm now reads `Commands::X(args) => block_on_async(x::run(args))`, dropping the function below the threshold and removing copy-paste-typo risk for any future subcommand arms.
- **Files modified:** `crates/famp/src/cli/mod.rs`
- **Commit:** `5a1c3fb`

**2. [Rule 1 - Bug] Three doc comments tripped clippy::doc_markdown by writing `Hello.bind_as` without backticks**

- **Found during:** Task 2 clippy run.
- **Issue:** `clippy::doc_markdown` flags identifier-shaped tokens in doc comments that lack backticks — three sites in `sessions.rs`, `whoami.rs`, and `mod.rs` rendered `Hello.bind_as` as plain prose.
- **Fix:** Added backticks at all three sites (no behavior change; doc-only).
- **Files modified:** `cli/sessions.rs`, `cli/whoami.rs`, `cli/mod.rs`.
- **Commit:** `5a1c3fb`.

No architectural deviations. No checkpoints. No auth gates. No deferred items.

## Self-Check

Verified all created files exist on disk:

- `crates/famp/src/cli/util.rs` — FOUND
- `crates/famp/src/cli/join.rs` — FOUND
- `crates/famp/src/cli/leave.rs` — FOUND
- `crates/famp/src/cli/sessions.rs` — FOUND
- `crates/famp/src/cli/whoami.rs` — FOUND

Verified all commits exist:

- `b46f68d` (Task 1) — FOUND in `git log`
- `5a1c3fb` (Task 2) — FOUND in `git log`

## Self-Check: PASSED
