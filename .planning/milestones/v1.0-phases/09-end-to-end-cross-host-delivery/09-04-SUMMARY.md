---
phase: 09-end-to-end-cross-host-delivery
plan: 04
subsystem: infra
tags: [famp-gateway, tokio, ed25519, famp-transport-http, uds-bus, egress, ingress, wiring]

# Dependency graph
requires:
  - phase: 09-end-to-end-cross-host-delivery
    plan: 02
    provides: "egress::run_egress(name, Arc<Mutex<GatewayRegistry>>, Arc<HttpTransport>, FampSigningKey, TrustedVerifyingKey)"
  - phase: 09-end-to-end-cross-host-delivery
    plan: 03
    provides: "ingress::run_ingress(listen_addr, tls_cert_path, tls_key_path, Arc<Mutex<GatewayRegistry>>, Arc<Keyring>) -> io::Result<()>"
provides:
  - "famp-gateway bin: extended parse_args -> GatewayArgs{sock,names,listen,tls_cert,tls_key,peers,trust_cert}"
  - "famp-gateway bin: live bidirectional main() — loads identity+peers keyring, populates D-02 peer map, runs run_ingress + one run_egress per backed principal + ctrl_c via tokio::select!"
affects: [09-05-e2e]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Hand-rolled arg loop extended (no clap) — matches the crate's existing parse_args style"
    - "Per-egress-task identity reload via load_or_generate instead of Clone (FampSigningKey deliberately non-Clone for secret-key hygiene); idempotent load (T-08-12) makes this behaviorally identical to sharing one loaded key"
    - "tokio::task::JoinSet draining a while-let loop as one arm of tokio::select!, alongside run_ingress and ctrl_c"

key-files:
  created: []
  modified:
    - crates/famp-gateway/src/main.rs
    - crates/famp-gateway/tests/liveness.rs
    - crates/famp-gateway/tests/no_cross_talk.rs

key-decisions:
  - "GatewayArgs.peers is Vec<(String domain, url::Url)> (D-02); resolved to HttpTransport::add_peer entries via the cross product of every backed principal name x every --peer domain (agent:{domain}/{name}) rather than a 1:1 pairing, since HttpTransport's addr_map is keyed by full recipient Principal, not bare domain, and Phase 8's one-key-per-remote-principal-name limitation means this scope only ever backs a small, known set of names."
  - "--peer and --trust-cert are NOT required flags (only --listen/--tls-cert/--tls-key are); an unmapped backed principal's egress attempts surface as transport UnknownRecipient (logged, drain loop continues), never a silent drop, matching the plan's key_links contract."
  - "Per-egress-task FampSigningKey is obtained via a fresh load_or_generate(&identity_path) call per spawn rather than a single shared+cloned key, because FampSigningKey deliberately does not implement Clone (secret-key hygiene, famp-crypto). load_or_generate is idempotent (same path -> byte-identical key, T-08-12), so this is behaviorally equivalent to the plan's 'load once' framing while staying compile-clean."
  - "tokio::task::JoinSet (not a Vec<JoinHandle>) holds the per-principal run_egress tasks so they can be drained as a single future inside tokio::select!, alongside run_ingress and ctrl_c()."

patterns-established:
  - "Two-commit split within one file: Task 1 (parse_args/GatewayArgs, tested standalone with the old park loop still in place) then Task 2 (main() wiring) — keeps each task's commit independently green under its own verification command."

requirements-completed: [GW-01, GW-02, GW-03]

coverage:
  - id: D1
    description: "parse_args accepts --listen/--tls-cert/--tls-key/--peer (repeatable)/--trust-cert plus 1+ positional names, returning GatewayArgs; --listen/--tls-cert/--tls-key are required and missing any produces a distinct usage error; malformed --peer (no '=', empty domain, invalid url) is a parse error naming the expected shape; positional-name-only invocation still errors clearly."
    requirement: "GW-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/main.rs#tests (13 tests: parses_full_cross_host_flag_surface, peer_flag_is_repeatable, trust_cert_defaults_to_none, malformed_peer_missing_equals_is_a_parse_error, malformed_peer_empty_domain_is_a_parse_error, malformed_peer_invalid_url_is_a_parse_error, missing_listen_is_a_distinct_usage_error, missing_tls_cert_is_a_distinct_usage_error, missing_tls_key_is_a_distinct_usage_error, invalid_listen_address_is_a_parse_error, positional_names_only_still_errors_clearly, missing_names_is_a_usage_error, socket_defaults_when_omitted)"
        status: pass
    human_judgment: false
  - id: D2
    description: "main() loads the gateway signing identity + pinned peers keyring once at startup (fail-fast on load error) and populates the D-02 to_domain->URL peer map on the outbound HttpTransport client before spawning any relay task."
    requirement: "GW-02"
    verification:
      - kind: integration
        ref: "cargo build -p famp-gateway --all-targets"
        status: pass
      - kind: other
        ref: "just lint (cargo clippy --workspace --all-targets -- -D warnings)"
        status: pass
    human_judgment: true
    rationale: "Live identity/keyring load against a real $FAMP_HOME is only exercised by the 09-05 two-process E2E; this plan proves the wiring compiles/type-checks and fails loud on a bad path, and the two existing subprocess tests (liveness/no_cross_talk) prove the process starts and stays live end-to-end with a real (empty) keyring."
  - id: D3
    description: "The park-only ctrl_c().await is replaced by tokio::select! over run_ingress, a JoinSet of one run_egress task per backed principal, and ctrl_c — the sole ctrl_c reference lives inside the select!."
    requirement: "GW-02"
    verification:
      - kind: other
        ref: "grep -c 'ctrl_c' crates/famp-gateway/src/main.rs -> 2 (declaration comment + the select! arm); the only executable ctrl_c() call is tokio::signal::ctrl_c() inside tokio::select!"
        status: pass
    human_judgment: false
  - id: D4
    description: "Exactly ONE Arc<Mutex<GatewayRegistry>> is constructed and cloned (Arc::clone) into run_ingress and every spawned run_egress task — the GW-02 shared-connection contract."
    requirement: "GW-02"
    verification:
      - kind: other
        ref: "crates/famp-gateway/src/main.rs main() — single `let registry = Arc::new(Mutex::new(registry));`, then Arc::clone(&registry) at each of the run_ingress call and the per-name run_egress spawn"
        status: pass
    human_judgment: false
  - id: D5
    description: "Existing LIVE-02/GW-04 subprocess integration tests (07-03) still pass unmodified in behavior after the new required cross-host flags land — famp-gateway --socket-only invocations previously used by these tests now require --listen/--tls-cert/--tls-key plus an isolated FAMP_HOME with a loadable peers.keyring."
    requirement: "GW-01"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/liveness.rs#live02_gateway_exit_reaps_all_principals"
        status: pass
      - kind: integration
        ref: "crates/famp-gateway/tests/no_cross_talk.rs#gw04_no_cross_talk_between_proxied_principals"
        status: pass
    human_judgment: false

# Metrics
duration: ~20min
completed: 2026-07-27
status: complete
---

# Phase 9 Plan 4: famp-gateway Cross-Host Wiring Summary

**Composed 09-01/09-02/09-03's dormant relay halves into a live bidirectional `famp-gateway` process: extended arg parsing for the cross-host surface, then replaced the park-only `ctrl_c` loop with a concurrent `run_ingress` + per-principal `run_egress` `tokio::select!` sharing one `Arc<Mutex<GatewayRegistry>>`.**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-27
- **Tasks:** 2
- **Files modified:** 3 (`main.rs` + 2 pre-existing integration test fixtures fixed for the new required flags)

## Accomplishments

- `parse_args` now returns a `GatewayArgs` struct carrying `sock`, `names`, `listen: SocketAddr`, `tls_cert`/`tls_key: PathBuf`, `peers: Vec<(String, url::Url)>` (D-02's `to_domain` -> URL map, one entry per repeatable `--peer <domain>=<url>`), and `trust_cert: Option<PathBuf>`. `--listen`/`--tls-cert`/`--tls-key` are required; a malformed `--peer` (no `=`, empty domain, or an invalid URL) and each missing required flag produce distinct, named usage errors. Still a hand-rolled, I/O-free, pure function — no `clap` dependency added.
- `main()` is now the composition point: after backing every principal (unchanged), it resolves `$FAMP_HOME`, loads the gateway's persisted signing identity (`load_or_generate`) and pinned peers keyring (`Keyring::load_from_file`) — both fail loud on error before any relay task spawns — builds an `HttpTransport::new_client_only(trust_cert)` client, and populates its address map by resolving every backed principal name against every `--peer` domain (`agent:{domain}/{name}`).
- The park-only `tokio::signal::ctrl_c().await` is replaced by `tokio::select!` over `run_ingress` (09-03), a `tokio::task::JoinSet` draining one `run_egress` (09-02) task per backed principal, and `ctrl_c()` for graceful shutdown. Exactly one `Arc<Mutex<GatewayRegistry>>` is constructed and `Arc::clone`d into every task — the GW-02 shared-connection contract egress/ingress already implement structurally is now wired end-to-end.
- Fixed a regression the new required flags introduced in two pre-existing 07-03 subprocess integration tests (`liveness.rs`, `no_cross_talk.rs`) — see Deviations.

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend arg parsing for the cross-host surface** - `23ab7d4` (feat)
2. **Task 2: Concurrent ingress + egress runtime** - `a94427e` (feat)

## Files Created/Modified

- `crates/famp-gateway/src/main.rs` - `GatewayArgs`/extended `parse_args` (13 new unit tests) + `main()` rewritten as the live bidirectional composition point (identity/keyring load, peer map population, `tokio::select!` over `run_ingress`/egress `JoinSet`/`ctrl_c`)
- `crates/famp-gateway/tests/liveness.rs` - `spawn_gateway_subprocess` now passes `--listen 127.0.0.1:0 --tls-cert --tls-key` (shared `cross_machine` fixture certs) and sets `FAMP_HOME` to the test's own tempdir with a pre-created empty `peers.keyring`
- `crates/famp-gateway/tests/no_cross_talk.rs` - same fix as `liveness.rs`

## Decisions Made

- **`--peer`/`--trust-cert` are not required flags** — only `--listen`/`--tls-cert`/`--tls-key` gate on "a gateway with no inbound listener has no way to relay" per the plan's stated behavior. A backed principal whose domain has no matching `--peer` entry never gets added to the transport's address map; the resulting egress attempt surfaces as a `HttpTransportError::UnknownRecipient` (logged, drain loop continues) rather than a silent drop — matching the plan's `key_links` contract exactly.
- **Peer-map resolution is a cross product**, not a 1:1 domain-to-name pairing: for each `--peer <domain>=<url>` and each backed principal `name`, `main()` attempts `agent:{domain}/{name}`.parse::<Principal>() and calls `add_peer` on success. This was necessary because `HttpTransport`'s `addr_map` is keyed by the full recipient `Principal` (confirmed via `crates/famp-transport-http/src/transport.rs`'s `send()`), not a bare domain string, and Phase 8's "one signing key per remote principal name" scope limitation means this cross product only ever spans a small, known set of names in practice.
- **`FampSigningKey` is not `Clone`** (confirmed via `crates/famp-crypto/src/keys.rs` — deliberate secret-key hygiene, no `#[derive(Clone)]`). Rather than adding `Clone` to a security-sensitive type outside this task's scope, each spawned `run_egress` task calls `load_or_generate(&identity_path)` fresh. `load_or_generate` is documented and tested as idempotent (`load_or_generate_is_idempotent`, T-08-12: same path always returns the byte-identical key), so this reload is behaviorally indistinguishable from sharing one in-memory key — it just re-reads the same small on-disk file per spawn instead.
- **`tokio::task::JoinSet`** (not a `Vec<JoinHandle>>`) holds the per-principal `run_egress` tasks, drained via a `while ... .join_next().await.is_some() {}` loop that becomes one arm of the outer `tokio::select!` — this lets N tasks (N = number of backed principals) participate in the same 3-way `select!` as a single future without hand-rolling a `futures::future::select_all`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `--listen`/`--tls-cert`/`--tls-key` becoming required broke two pre-existing 07-03 subprocess integration tests**
- **Found during:** Task 2 verification (`cargo test -p famp-gateway --test liveness`)
- **Issue:** `tests/liveness.rs` and `tests/no_cross_talk.rs` (07-03, LIVE-02/GW-04) both spawn `famp-gateway --socket <sock> <names>...` with no cross-host flags — a shape this plan's Task 1 deliberately made an error ("positional-name-only invocation... still errors clearly"). Both tests timed out waiting for principals to appear live, since the gateway subprocess now exits(1) immediately on the missing-`--listen` usage error.
- **Fix:** Updated both tests' `spawn_gateway_subprocess` helpers to pass `--listen 127.0.0.1:0` (OS-assigned ephemeral port, never dialed by either test — they only exercise LIVE-02/GW-04 local-bus behavior) plus `--tls-cert`/`--tls-key` pointing at the shared `crates/famp/tests/fixtures/cross_machine/alice.{crt,key}` fixture pair (already used by the deferred v1 e2e test and referenced in 09-PATTERNS.md).
- **Files modified:** `crates/famp-gateway/tests/liveness.rs`, `crates/famp-gateway/tests/no_cross_talk.rs`
- **Verification:** `cargo test -p famp-gateway --test liveness` and `cargo test -p famp-gateway --test no_cross_talk` both pass.
- **Committed in:** `a94427e` (Task 2's commit, since it directly follows from Task 2's required-flags change)

**2. [Rule 1 - Bug] Same tests also needed `FAMP_HOME` isolation once `main()` started loading identity/keyring**
- **Found during:** Task 2 verification, immediately after fixing deviation #1
- **Issue:** With `--listen`/`--tls-cert`/`--tls-key` fixed, both subprocess tests still failed: `main()` now resolves `$FAMP_HOME` (defaulting to the real `$HOME/.famp` when unset) and requires `Keyring::load_from_file` to succeed against `~/.famp/gateway/peers.keyring` — a file that doesn't exist on a fresh machine, and which would be non-hermetic (and would create a real signing-identity file in the developer's actual home directory) even if it did.
- **Fix:** Both `spawn_gateway_subprocess` helpers now take a `home: &Path` parameter, set it as the subprocess's `FAMP_HOME` env var, and pre-create an empty `home/gateway/peers.keyring` file (a valid, zero-entry keyring — neither test relays anything, so no peers are needed). Both call sites pass the test's own `tempfile::TempDir` (`tmp.path()`), reusing the same tempdir already used for `--socket`'s bus isolation — matching 09-RESEARCH.md §7's "single tempdir serves both isolation axes" guidance.
- **Files modified:** `crates/famp-gateway/tests/liveness.rs`, `crates/famp-gateway/tests/no_cross_talk.rs`
- **Verification:** Both tests pass; full `cargo test -p famp-gateway` (27 tests total) and `just lint` (workspace clippy `-D warnings`) both green.
- **Committed in:** `a94427e` (same commit as deviation #1 — both fixes were needed together to make the tests pass again)

---

**Total deviations:** 2 auto-fixed, both Rule 1 (regressions directly caused by this task's required-flag change in existing 07-03 test fixtures, out of this plan's `files_modified` scope but squarely in-scope per the deviation rules' scope boundary — "directly caused by the current task's changes")
**Impact on plan:** Neither changes the plan's must-have behavior; both were necessary to keep the pre-existing LIVE-02/GW-04 test suite green under the new required cross-host CLI surface.

## Issues Encountered

- The plan's action text describes `sk.clone()`/`vk.clone()` when spawning each `run_egress` task; `TrustedVerifyingKey` does derive `Clone` (confirmed in `famp-crypto/src/keys.rs`) but `FampSigningKey` deliberately does not. Resolved via the `load_or_generate`-per-task approach documented above (Decisions Made) rather than adding `Clone` to a secret-key type, which would be an architectural change to a different crate outside this task's scope.
- `cargo fmt --all` (pre-commit hook) reformatted several multi-line `match`/`?`-chain expressions across both commits; re-staged and committed as-is per the hook's own fix instructions, no functional change.
- `clippy::doc_lazy_continuation` (part of `clippy::all`, deny-level) flagged a doc comment in `tests/liveness.rs` where a line started with `+` (interpreted as an unindented markdown list continuation) — reworded `+ broker reap` to `and broker reap` to clear it.

## User Setup Required

None — no external service configuration required. The gateway's own signing identity and peers keyring continue to be created/loaded from `$FAMP_HOME` exactly as documented in 09-RESEARCH.md §5; nothing new to provision for local development.

## Next Phase Readiness

- 09-05's two-process E2E can now spawn two real `famp-gateway` subprocesses with the full cross-host flag surface (`--socket`, `--listen`, `--tls-cert`/`--tls-key`, `--peer <domain>=<url>`, `--trust-cert`, isolated `FAMP_HOME` per side) and expect a live, bidirectional relay process on each side — this plan is the last piece of gateway-process wiring 09-05 needs before it can drive a real request -> commit -> deliver -> ack cycle across two hosts.
- `cargo test -p famp-gateway` (27 tests: 11 lib unit + 13 main.rs arg-parsing unit + 3 integration) and `just lint` (workspace clippy `-D warnings`) both green as of `a94427e`.
- No blockers identified.

---
*Phase: 09-end-to-end-cross-host-delivery*
*Completed: 2026-07-27*

## Self-Check: PASSED
- FOUND: crates/famp-gateway/src/main.rs
- FOUND: crates/famp-gateway/tests/liveness.rs
- FOUND: crates/famp-gateway/tests/no_cross_talk.rs
- FOUND: .planning/phases/09-end-to-end-cross-host-delivery/09-04-SUMMARY.md
- FOUND commit: 23ab7d4
- FOUND commit: a94427e
