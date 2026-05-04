---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 01
subsystem: bus_client + cli/identity + cli/broker
tags: [phase-2, wave-2, bus-client, identity, broker, foundation]
requires:
  - 02-00 (Wave-0 test stubs)
provides:
  - BusClient::connect (async UDS Hello handshake; D-10 proxy parameter
    held; wire-side bind_as field arrives in 02-02)
  - bus_client::codec::{write_frame, read_frame} (async wrappers around
    famp_bus::codec)
  - bus_client::spawn::spawn_broker_if_absent (portable Command::new +
    pre_exec(setsid) — RESEARCH Q1)
  - bus_client::resolve_sock_path / bus_dir
  - cli::identity::resolve_identity (D-01 four-tier resolver)
  - cli::broker::nfs_check::is_nfs (platform-conditional NFS detector)
  - CliError::NoIdentityBound (with mcp_error_kind = "no_identity_bound")
  - scripts/check-mcp-deps.sh wired into `just ci` via the new
    `check-mcp-deps` recipe
affects:
  - crates/famp/Cargo.toml (nix 0.31, dirs 5, famp-bus path dep,
    assert_cmd 2.0, tokio test-util feature)
  - crates/famp-bus/Cargo.toml (tokio dev-dep with macros/rt/test-util
    for plan 02-11's start_paused tests)
  - crates/famp/src/lib.rs (`#![deny(unsafe_code)]` instead of `forbid`
    so `bus_client::spawn` can opt in via a single-module
    `#[allow(unsafe_code)]`)
  - Justfile (new `check-mcp-deps` recipe, wired into `ci:`)
  - examples + tests silencer lists for the new transitive deps
tech-stack:
  added:
    - "nix 0.31 (process + fs features) — sys::statfs, unistd::setsid"
    - "dirs 5 — home_dir resolution"
    - "assert_cmd 2.0 — dev-dep for shelled CLI integration tests"
    - "tokio test-util feature in famp + famp-bus — start_paused / advance"
  patterns:
    - "Async length-prefixed canonical-JSON frame codec (sync core wrapped
      with tokio AsyncRead/AsyncWriteExt)"
    - "Portable broker spawn via Command::new + pre_exec(setsid) — locked
      Q1 answer; POSIX_SPAWN_SETSID intentionally not used"
    - "D-01 four-tier identity resolution with cwd → wires.tsv exact
      match (canonicalize both sides)"
    - "Single narrowly-scoped #[allow(unsafe_code)] on the spawn module;
      crate-level deny posture preserved everywhere else"
key-files:
  created:
    - crates/famp/src/bus_client/mod.rs
    - crates/famp/src/bus_client/codec.rs
    - crates/famp/src/bus_client/spawn.rs
    - crates/famp/src/cli/broker/mod.rs
    - crates/famp/src/cli/broker/nfs_check.rs
    - crates/famp/src/cli/identity.rs
    - scripts/check-mcp-deps.sh
  modified:
    - crates/famp/Cargo.toml
    - crates/famp-bus/Cargo.toml
    - crates/famp-bus/src/lib.rs
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
    - Justfile
    - crates/famp/examples/{personal_two_agents,_gen_fixture_certs,cross_machine_two_agents}.rs
    - crates/famp/tests/{broker_lifecycle,cli_dm_roundtrip,hook_subcommand}.rs (fmt only)
    - Cargo.lock
decisions:
  - "BusClient::connect carries `bind_as: Option<String>` today even
    though the wire field lands in 02-02. The parameter is held on the
    client and surfaced via `bind_as()`; the Hello frame still uses the
    pre-02-02 shape (no field) until 02-02 adds it to proto.rs. Plan
    02-02 only needs to back-fill the assignment inside `connect`."
  - "Workspace `forbid(unsafe_code)` is downgraded to `deny` for the
    famp crate so `bus_client::spawn` can opt in via a single
    `#[allow(unsafe_code)]`. Every other module keeps the deny posture."
  - "macOS NFS detection compares `filesystem_type_name().as_bytes()`
    against the magic prefix `b\"nfs\"` (handles `nfs`, `nfs3`, `nfs4`)."
  - "MCP error_kind discriminator for the new `NoIdentityBound` variant
    is `\"no_identity_bound\"` (snake_case, unique; gate green via
    `mcp_error_kind_exhaustive`)."
metrics:
  duration: ~75min
  completed_date: 2026-04-28
---

# Phase 2 Plan 01: BusClient + Identity + NFS Check Foundation Summary

Wave-2 substantive foundation for Phase 02: workspace dependencies
(`nix 0.31`, `assert_cmd 2.0`, `dirs 5`, `tokio test-util`), and the
three foundational modules — `bus_client/`, `cli/identity.rs`,
`cli/broker/nfs_check.rs` — that every later CLI plan and the broker
plan depend on. The 9 Wave-0 stub files compile cleanly against the
new dev-dependencies and continue to report as IGNORED; `BusClient::
connect` and the portable broker spawn helper are usable from any
subcommand starting now.

## What Shipped

### Task 1 — Module declarations + dependencies (commit `2ffdc35`)
Wired `nix 0.31` (process + fs features), `dirs 5`, `famp-bus` path
dep, `assert_cmd 2.0`, and the tokio `test-util` feature into
`crates/famp/Cargo.toml`. Added `tokio` (macros + rt + test-util) to
`crates/famp-bus` dev-dependencies for plan 02-11's `start_paused`
time-forward tests; `just check-no-tokio-in-bus` stays green because
the BUS-01 gate filters `--edges normal`. Registered `pub mod
bus_client;`, `pub mod cli::broker;`, and `pub mod cli::identity;`,
created the `cli::broker::nfs_check` skeleton, shipped
`scripts/check-mcp-deps.sh` (D-11 source-import grep) and wired it
into `just ci` via the new `check-mcp-deps` recipe.

### Task 2 — Real bodies + unit tests (commit `3d8d37f`)
- **`bus_client::codec`** — async `write_frame` / `read_frame` over
  the sync `famp_bus::codec::{encode_frame, try_decode_frame}` core,
  preserving the BUS-06 length-prefixed canonical-JSON wire shape.
- **`bus_client::mod`** — `BusClient { stream, bind_as }` with
  `connect(sock, bind_as)` (Hello handshake), `send_recv`, `bind_as()`
  accessor, `shutdown`, plus the `BusClientError` enum and helpers
  `resolve_sock_path` (`$FAMP_BUS_SOCKET` → `~/.famp/bus.sock`) and
  `bus_dir`.
- **`bus_client::spawn`** — portable broker spawn via
  `Command::new(current_exe).pre_exec(|| setsid())`. Single
  narrowly-scoped `#[allow(unsafe_code)]`; `lib.rs` `forbid` → `deny`.
  10×200ms post-spawn poll for socket-up.
- **`cli::identity::resolve_identity`** — D-01 four-tier hybrid
  resolver. Tier 1 `--as`, tier 2 `$FAMP_LOCAL_IDENTITY`, tier 3 cwd
  → `~/.famp-local/wires.tsv` exact match (canonicalize both sides),
  tier 4 `CliError::NoIdentityBound` with the locked hint message.
  4 hermetic unit tests (one per tier) using temp `HOME` dirs.
- **`cli::broker::nfs_check::is_nfs`** — platform-conditional via
  `nix::sys::statfs`. Linux compares `filesystem_type() ==
  NFS_SUPER_MAGIC`; macOS matches `filesystem_type_name().as_bytes()
  .starts_with(b"nfs")`. `statfs` errors fall through to `false`.

## Test Counts
- **Unit tests added**: 11 across the new modules (4 identity, 2
  codec, 1 spawn, 2 bus_client, 2 nfs_check). All passing.
- **Exhaustive-test fixture row added**: 1 (`NoIdentityBound`) keeps
  `mcp_error_kind_exhaustive` green.
- **Wave-0 stub tests still IGNORED**: 19 (count unchanged from
  pre-plan-02-01 baseline).
- **Workspace clippy** with `-D warnings`: green.
- **`cargo fmt --all -- --check`**: green (Wave-0 stub fmt
  reformatted in place — bodies unchanged).

## D-Q1 Compliance
The locked Q1 portable spawn pattern is in place:
- `grep -F 'pre_exec' crates/famp/src/bus_client/spawn.rs` → 3 lines
  (closure + comment + safety doc).
- `grep -F 'nix::unistd::setsid' crates/famp/src/bus_client/spawn.rs`
  → 2 lines (module doc + closure body).
- `grep -F 'POSIX_SPAWN_SETSID' crates/famp/src/bus_client/spawn.rs`
  → **0 lines** (the macOS-only flag is intentionally NOT used).
- `grep -F 'posix_spawnp' crates/famp/src/bus_client/spawn.rs` → 0 lines.

Confirmation: the spawn helper uses the portable `Command::new` +
`pre_exec(setsid)` pattern (RESEARCH Q1) and not the non-portable
macOS-only "set new session" `posix_spawn` flag.

## Deviations from Plan

### [Rule 3 - Blocking] Crate-level lint override for unsafe_code
- **Found during:** Task 2 (compile gate)
- **Issue:** Workspace lints set `unsafe_code = "forbid"` and
  `crates/famp/src/lib.rs` had `#![forbid(unsafe_code)]`. The locked
  Q1 `pre_exec` pattern requires `unsafe { cmd.pre_exec(...) }`,
  which `forbid` cannot relax (only `deny` accepts module-level
  `#[allow]`).
- **Fix:** Replaced `[lints] workspace = true` in
  `crates/famp/Cargo.toml` with explicit `[lints.rust]` /
  `[lints.clippy]` blocks that mirror the workspace defaults *except*
  `unsafe_code = "deny"`. Changed `crates/famp/src/lib.rs` from
  `#![forbid(unsafe_code)]` to `#![deny(unsafe_code)]`. Added
  `#![allow(unsafe_code)]` to `bus_client/spawn.rs` only (with a
  module-doc rationale and a `// SAFETY:` block on the `unsafe`
  call). Every other famp module keeps the `deny` posture, and the
  `famp` binary's `#![forbid(unsafe_code)]` is unchanged because the
  bin doesn't compile any `unsafe` code itself.
- **Files modified:** `crates/famp/Cargo.toml`, `crates/famp/src/lib.rs`,
  `crates/famp/src/bus_client/spawn.rs`.
- **Commits:** `3d8d37f`.

### [Rule 2 - Critical] Add CliError::NoIdentityBound + MCP discriminator
- **Found during:** Task 2 (compile gate)
- **Issue:** D-01 tier-4 hard-error required `CliError::NoIdentityBound`
  but the variant did not exist. Adding it then triggered the
  exhaustive-match guard inside `cli::mcp::error_kind` (T-04-13
  mitigation: every variant must have a stable `mcp_error_kind`
  string).
- **Fix:** Added `CliError::NoIdentityBound { reason: String }` with
  `#[error("{reason}")]`, an arm in `mcp::error_kind` returning
  `"no_identity_bound"`, and a fixture row in
  `mcp_error_kind_exhaustive.rs` so the unique-discriminator and
  every-variant-has-kind tests stay green.
- **Files modified:** `crates/famp/src/cli/error.rs`,
  `crates/famp/src/cli/mcp/error_kind.rs`,
  `crates/famp/tests/mcp_error_kind_exhaustive.rs`.
- **Commits:** `3d8d37f`.

### [Rule 3 - Blocking] Examples silencer expansion
- **Found during:** Task 2 (`cargo clippy --workspace --all-targets`)
- **Issue:** Adding `assert_cmd`, `dirs`, `famp-bus`, and `nix` as
  famp dependencies tripped `unused_crate_dependencies` for the three
  examples (`personal_two_agents`, `_gen_fixture_certs`,
  `cross_machine_two_agents`) which don't reference them.
- **Fix:** Extended each example's `use … as _;` silencer block to
  include the four new transitive deps. Pure additive.
- **Files modified:** `crates/famp/examples/*.rs`.
- **Commits:** `3d8d37f`.

### Pre-existing fmt expansion of Wave-0 stub bodies
- **Found during:** Task 1 (`cargo fmt --all -- --check`)
- **Issue:** Wave-0 stub files used single-line bodies
  (`fn test_x() { unimplemented!(...); }`) that fail
  `rustfmt --check`. Pre-existing from the 02-00 merge.
- **Fix:** Plan 02-01 task 2 ran `cargo fmt --all` so the new
  BusClient/identity sources could ride a green `fmt-check` gate.
  Test bodies are unchanged (still `unimplemented!(...)` under
  `#[ignore]`); only brace style was reformatted. Wave-0 stub
  ownership and `#[ignore]` discipline are unaffected.
- **Files modified:** `crates/famp/tests/{broker_lifecycle,
  cli_dm_roundtrip, hook_subcommand}.rs` (whitespace only).
- **Commits:** `3d8d37f`.

### bind_as parameter held but not on the wire (intentional)
- **Found during:** Task 2 (compile gate)
- **Issue:** Plan 02-01 must not modify `crates/famp-bus/src/proto.rs`
  (owned by 02-02 per D-10). `BusMessage::Hello` therefore does not
  yet carry a `bind_as` field. The plan acknowledges this in lines
  99-101 ("If 02-01 lands first temporally, leave bind_as as `None`
  until 02-02's proto edit; then back-fill the parameter").
- **Fix:** `BusClient::connect` accepts `bind_as: Option<String>`,
  stores it on the client, and exposes it via `BusClient::bind_as()`.
  The Hello frame still uses the pre-02-02 shape (`bus_proto`,
  `client` only). When 02-02 lands, the only change required is
  passing `client.bind_as.clone()` into the constructed Hello — the
  CLI-side public API is unchanged.
- **Files modified:** `crates/famp/src/bus_client/mod.rs`.
- **Commits:** `3d8d37f`.

## Pre-Existing Issues (Not Caused by This Plan)
Documented in
`.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/deferred-items.md`:
- The `listen_bind_collision second_listen_on_same_port_errors_port_in_use`
  test was already failing on the merge base (`68b2a2c`) before any
  plan 02-01 changes. Aligns with the 8 pre-existing listener / TLS
  loopback timeouts noted in `STATE.md` issues.

## Self-Check: PASSED

- [x] `crates/famp/src/bus_client/mod.rs` exists, exports
  `pub struct BusClient`, references `BusMessage::Hello`, references
  `bind_as` (16 lines).
- [x] `pre_exec`, `nix::unistd::setsid` present in
  `bus_client/spawn.rs`; `POSIX_SPAWN_SETSID` and `posix_spawnp`
  return 0 lines.
- [x] `famp_bus::codec::encode_frame` referenced from
  `bus_client/codec.rs`.
- [x] `wires.tsv`, `FAMP_LOCAL_IDENTITY` referenced in
  `cli/identity.rs`.
- [x] `NFS_SUPER_MAGIC` and `b"nfs"` present in
  `cli/broker/nfs_check.rs`.
- [x] `pub fn resolve_sock_path` exists in `bus_client/mod.rs`.
- [x] `cargo build --workspace --tests` exits 0.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [x] `cargo fmt --all -- --check` exits 0.
- [x] `cargo nextest run -p famp --lib bus_client cli::identity
  cli::broker::nfs_check` passes 12 tests.
- [x] 19 Wave-0 stub tests still report as IGNORED.
- [x] `bash scripts/check-mcp-deps.sh` exits 0.
- [x] `just check-no-tokio-in-bus` exits 0.
- [x] No `_ =>` wildcard arms under `bus_client/`,
  `cli::identity`, or `cli::broker::nfs_check`.

## Commits

| Task | Commit | Files | Insertions / Deletions |
|------|--------|-------|------------------------|
| 1    | `2ffdc35` | 16 | +294 / -3   |
| 2    | `3d8d37f` | 17 | +790 / -45  |
