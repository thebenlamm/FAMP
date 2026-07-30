# Phase 10: Test Reactivation + Setup Docs - Research

**Researched:** 2026-07-27
**Domain:** Rust integration-test triage (cargo/nextest), CI-gate verification, developer-facing setup documentation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Triage each of the 27 `_deferred_v1/` tests into RETIRE-with-rationale / ALREADY-COVERED / REACTIVATE. Reactivate ONLY tests encoding still-live, not-otherwise-covered intent re-expressible against the current `famp-bus`/`famp-gateway` API. Ledger location: `_deferred_v1/TRIAGE.md` or updated `README.md` (Claude's discretion).
- **D-02:** Do NOT resurrect the deleted v0.8 CLI (`famp init/setup/listen/peer add`) to satisfy a test. The bar for "reactivate" is: the behavior still exists on a shipping surface today.
- **D-03:** Promote Phase 9's `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` as the TEST-02 artifact. Do not author a second E2E.
- **D-04 [CRUX]:** Verify whether the Phase 9 E2E actually runs under `cargo nextest run --workspace` (what `just ci` invokes). If it hangs/skips: add a nextest test-group mirroring the existing `inspect-subprocess` precedent. Do NOT fix via `#[ignore]` or a manual recipe.
- **D-05:** Keep the E2E hermetic + `ChildGuard`-clean: ephemeral ports, isolated `FAMP_HOME` per process, fixture certs, no reliance on a developer's `~/.famp/` daemon.
- **D-06:** New standalone guide at `docs/GATEWAY-SETUP.md`, linked from README's federation/quickstart section. Own-machines-first framing, no public relay. Sections: prerequisites → gateway identity → out-of-band key exchange (`peer export`/`import`) with TOFU pin + fingerprint check → start each gateway → connect/verify via `famp inspect tasks`. Planner reads `crates/famp-gateway/src/main.rs` for exact flag spellings.
- **D-07:** Doc accuracy is gated against the binary (grep-gate mirroring v0.11 Phase 6's pattern). The true two-physical-machine walkthrough is a HUMAN-UAT acceptance Ben performs, captured in `10-HUMAN-UAT.md`. Do not claim DOC-04 done on the grep-gate alone.

### Claude's Discretion

- Triage ledger location/format (`_deferred_v1/TRIAGE.md` vs README update) — D-01.
- Exact nextest serialization mechanism for the gateway E2E (test-group vs `[[profile...test-groups]]` pin) — D-04, against the existing `nextest.toml` + `inspect-subprocess` precedent.
- Guide filename (`docs/GATEWAY-SETUP.md` vs a README `## Cross-host setup` section) — D-06, against the existing docs layout.

### Deferred Ideas (OUT OF SCOPE)

- Automated two-*physical*-machine CI runner — TEST-02's CI artifact is the two-process loopback E2E; the real two-machine run is Ben's DOC-04 human UAT.
- Public-internet relay / directory / cross-person trust / inbound-taint — v1.1 (RELAY-01/DIR-01/PEER-01/TAINT-01).
- FAMP-Sec plane — v2.0+ (SEC-01..N).
- `v1.0.0` tag + milestone archival — `/gsd-complete-milestone` action, not a Phase 10 task.
- Conformance vector pack (Gate B) — event-driven, not this phase.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TEST-01 | The ~27 parked federation tests in `crates/famp/tests/_deferred_v1/` are triaged — still-valid tests run green in CI, obsolete tests removed with documented rationale. | Per-file triage table below: all 27 files read and classified. **Finding: 27/27 RETIRE** (stronger than CONTEXT.md's 18/27 estimate) — 12 have a named, already-shipped covering test; the remaining 15 have no live salvageable intent because their subject code (`cli::init/setup/listen/peer::add`, v0.8 `cli::send::run_at`) no longer exists. Zero REACTIVATE candidates found, including the two CONTEXT.md flagged as candidate-salvage. |
| TEST-02 | A live two-process end-to-end test exercises the full signed cross-host task cycle and runs in `just ci`. | **CRUX answered empirically**: `cargo nextest run --workspace` (exactly what `just ci` invokes) was run live in this session — 969/969 tests passed including `famp-gateway::e2e_cross_host_delivery gw01_gw02_gw03_two_process_cross_host_delivery` in 9.3s. TEST-02 is **already satisfied** by Phase 9's shipped test; no nextest test-group is required for this specific test. See CRUX section below for full command evidence and an important correction to a locked assumption. |
| DOC-04 | A setup guide documents standing up the gateway on two machines — bind address, out-of-band key exchange, connect/verify. | Exact CLI flag surface extracted verbatim from `crates/famp-gateway/src/main.rs` and `crates/famp/src/cli/peer/{export,import}.rs` below. Identity/keyring disk paths confirmed. README insertion point identified (Quick Start line 170-171, currently pointing at a stale migration doc). Accuracy-gate mechanics identified (`famp-gateway` has no `--help`; gate must diff against the `usage:` string or `famp peer export/import --help`, which IS clap-generated). |

</phase_requirements>

## Summary

This phase is almost entirely a "read the code, run the command, report the fact" phase — there is very little design freedom. Three findings dominate:

1. **TEST-01 is more retirement-dominant than CONTEXT.md's scout estimated.** Every one of the 27 files in `crates/famp/tests/_deferred_v1/` depends, directly or via a shared `common::` harness, on a CLI surface (`famp init`, `famp setup`, `famp listen`, `famp::cli::listen::run_on_listener`, `famp::cli::peer::add`, or the pre-Phase-4 `famp::cli::send::run_at` HTTPS shape) that no longer exists in `crates/famp/src/cli/mod.rs`'s `Commands` enum or module tree. None of the 27 files compile against `main` today. Of those 27, 12 have a **named, already-shipped, currently-green test** that independently proves the same intent against the *current* API (`peer_roundtrip.rs`, `crates/famp-inbox/tests/truncated_tail.rs`, `mcp_tool_schema_invariants.rs`, `mcp_error_kind_exhaustive.rs`, `mcp_register_whoami.rs`, `mcp_bus_e2e.rs`, `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`, plus one unit test inside `cli/send/mod.rs::tests`). The two files CONTEXT.md flagged as "candidate-salvage" (`send_principal_fallback.rs`, `conversation_restart_safety.rs`) are, on inspection, both bound to the dead CLI with no rewrite target — their harness dependencies (`common::conversation_harness::spawn_listener`, `famp::cli::peer::add::run_add_at`) simply don't exist to rewrite against.

2. **TEST-02's CRUX question has a definitive, empirically-verified answer: yes, it already runs green under `cargo nextest run --workspace`.** This was verified live in this session (not assumed) — see the CRUX section. This corrects the locked assumption in CONTEXT.md D-04 that the E2E "was only ever run via plain `cargo test`" and might hang; it does not hang, and no nextest test-group is needed for it. The pre-existing `nextest.toml` test-groups (`listen-subprocess`, `inspect-subprocess`) already show the exact syntax to copy if a *future* reactivated subprocess test needs serialization — but nothing in this phase's TEST-01 output requires that, since 0 tests are being reactivated.

3. **DOC-04's CLI surface is small, hand-rolled (not clap) for `famp-gateway`, and clap-based for `famp peer export/import`.** `famp-gateway` has no `--help` — the accuracy gate has to diff against the `usage:` string embedded in `main.rs`'s `parse_args`, obtainable by running the binary with no args. `famp peer export --help` / `famp peer import --help` work normally via clap.

**Primary recommendation:** For TEST-01, write the ledger as a 27-row RETIRE table (12 with `ALREADY-COVERED:` pointer, 15 plain rationale), delete all 27 files (+ the `.deferred` file) in one commit, and do not move anything into the active glob. For TEST-02, add one paragraph to the ledger/CI docs asserting the fact with the command that proves it — do not build a nextest test-group. For DOC-04, write `docs/GATEWAY-SETUP.md` against the flag/path facts below, link it from README's Quick Start (replacing the stale `docs/MIGRATION-v0.8-to-v0.9.md` pointer for the federation case) and from `## Advanced: v0.8 federation CLI`, and gate it with a new integration test that (a) runs `famp-gateway` with no args and asserts the guide's flag list is a subset of the printed `usage:` string, and (b) runs `famp peer export --help` / `famp peer import --help` and asserts the same for those two subcommands.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Deferred-test triage / deletion | Test infrastructure (crate-local, `crates/famp/tests/`) | — | Pure test-suite hygiene; no runtime code changes |
| CI-gated E2E verification | CI / build tooling (`justfile`, `.config/nextest.toml`) | Test infrastructure | Already-shipped `famp-gateway` test target; the "primary tier" here is the CI pipeline configuration proving it runs, not new product code |
| Setup guide (DOC-04) | Docs (`docs/`) | CLI / Backend (`famp-gateway`, `famp peer`) | The doc's *content* is owned by docs, but its *accuracy* is owned by the CLI surface it describes — hence the grep-gate |
| Accuracy grep-gate | Test infrastructure | Docs | A new `crates/famp/tests/*.rs` (or `famp-gateway` test) asserting doc/binary parity, following the `cli_help_invariant.rs` precedent |

## Standard Stack

No new external dependencies are introduced by this phase (test triage + CI config + Markdown docs). All crates/tools used already exist in the workspace:

| Tool | Version (verified in-repo) | Purpose |
|------|------|---------|
| `cargo-nextest` | workspace-pinned via CI (`taiki-e/install-action@... tool: cargo-nextest`) | Test runner — `just test` / `just ci` |
| `assert_cmd` | already a dev-dependency (used by `cli_help_invariant.rs`) | Subprocess `--help`/no-args accuracy-gate pattern to mirror for DOC-04 |
| `clap` | already the CLI framework for `famp` (not `famp-gateway`, which is hand-rolled) | `famp peer export/import --help` |

**No `Package Legitimacy Audit` section is required** — this phase installs no new external packages.

## Architecture Patterns

### TEST-01 Triage Table (all 27 files read in full)

| File | Disposition | Rationale |
|------|-------------|-----------|
| `init_refuses.rs` | RETIRE | `famp::cli::init::run_at` / `CliError::AlreadyInitialized` — `cli::init` module deleted (not in `Commands` enum or `cli/mod.rs`'s module list) |
| `init_home_env.rs` | RETIRE | `famp::cli::init::run` / `InitArgs` — same, `FAMP_HOME`-driven `init` gone |
| `init_identity_incomplete.rs` | RETIRE | `famp::cli::init::load_identity` / `CliError::IdentityIncomplete` — module gone |
| `init_no_leak.rs` | RETIRE | `famp::cli::init::run_at` — module gone |
| `init_force.rs` | RETIRE | `famp::cli::init::run_at(..., force=true)` — module gone |
| `init_happy_path.rs` | RETIRE | `famp::cli::init::run_at` + TLS PEM cross-check against `famp_transport_http` — module gone |
| `listen_truncated_tail.rs` | **RETIRE — ALREADY-COVERED** | Uses `common::init_home_in_process` (wraps dead `cli::init`), but its actual assertion target — `famp_inbox::read::read_all` tail-tolerance (INBOX-04/05) — is independently and currently proven by `crates/famp-inbox/tests/truncated_tail.rs` (`tail_tolerant_when_file_lacks_trailing_newline`, active, in the live glob) |
| `listen_durability.rs` | RETIRE | Spawns `famp listen` subprocess (`spawn_listen` helper) — `listen` subcommand deleted |
| `info_happy_path.rs` | RETIRE | Depends on `famp::cli::setup::{PeerCard,SetupArgs}` — `setup` module deleted (kills the whole file even though `cli::info` itself still exists) |
| `listen_bind_collision.rs` | RETIRE | Spawns `famp listen` subprocess — deleted subcommand |
| `listen_smoke.rs` | RETIRE | `famp::cli::listen::run_on_listener` in-process — module deleted |
| `send_more_coming_requires_new_task.rs` | **RETIRE — ALREADY-COVERED** | `famp::cli::send::run_at_structured` (pre-Phase-4 HTTPS-shaped `SendArgs`, taking `home: &Path`) — current `send/mod.rs` has a different `SendArgs` shape (`sock: &Path`, bus-routed) and does not define this function signature at all. File's own header states the semantic check is already covered by unit test `more_coming_without_new_task_errors_in_run_at_structured` in `crates/famp/src/cli/send/mod.rs::tests` (confirmed still present) |
| `listen_shutdown.rs` | RETIRE | Spawns `famp listen` subprocess, sends SIGINT — deleted subcommand |
| `conversation_restart_safety.rs` | RETIRE | Uses `common::conversation_harness::{spawn_listener, stop_listener, ...}`, which wraps `famp::cli::listen::run_on_listener` — deleted module. **This was CONTEXT.md's #1 salvage candidate; confirmed on inspection to have no rewrite target** — the harness it depends on has no current-API equivalent (no `famp-bus`/`famp-gateway` "restart a listener and resume a task on a new port" surface exists to test against directly; the closest current concept, gateway process restart, is untested territory but a *new* test, not a rewrite of this one) |
| `peer_import.rs` | **RETIRE — ALREADY-COVERED** | `famp::cli::peer::import::run_import_at` — old TOML `peers.toml` shape, deleted in Phase 4 and replaced in Phase 8 by Ed25519 TOFU-pin `cli::peer::import` (different function signature, different keyring file). Current shape's export/import/pin round-trip is proven by active `crates/famp/tests/peer_roundtrip.rs` (TRUST-01) |
| `setup_happy_path.rs` | RETIRE | `famp::cli::setup::{run_with_io, SetupArgs, PeerCard}` — module deleted entirely |
| `send_tofu_bootstrap_refused.rs` | **RETIRE — ALREADY-COVERED** | Old HTTPS-shaped `cli::send::run_at` TOFU refusal test — the *equivalent* current-surface guarantee (unpinned peer key rejected before any bus/local-mailbox write) is proven by `famp-gateway`'s active `verify::tests::rejects_unpinned_key` and `verify_inbound_any_rejects_unpinned_key_for_every_class` (TRUST-02) |
| `send_new_task_scope_instructions.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `famp::cli::listen::run_on_listener` in-process — both deleted |
| `send_principal_fallback.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `famp::cli::peer::add::run_add_at` (deleted `peer add`). **CONTEXT.md's #2 salvage candidate; confirmed no rewrite target** — the "silent principal fallback" concern was about the old `config.toml`-resident self-principal; the current bus-routed send path resolves identity through `famp register`'s canonical-holder mechanism, a structurally different code path with no equivalent function to retarget the assertions at |
| `send_new_task.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + in-process listener — deleted |
| `peer_add.rs` | **RETIRE — ALREADY-COVERED** | `famp::cli::peer::add::run_add_at` (deleted TOML `peer add`) — current peer-trust CRUD is `cli::peer::export`/`import` (Ed25519), proven by `peer_roundtrip.rs` |
| `send_deliver_sequence.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `peer::add` — both deleted |
| `e2e_two_daemons.rs.deferred` | **RETIRE — ALREADY-COVERED** | Uses `famp_transport_http::{build_router, tls_server, HttpTransport}` directly (not the deleted CLI), but its intent — a full two-daemon signed cross-host `request→commit→deliver→ack` cycle — is exactly what Phase 9's `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` now proves against the *current* `famp-gateway` architecture (this file predates the gateway and drives `famp-transport-http` bare, without the gateway/broker proxy layer Phase 7-9 built). README's `_deferred_v1/README.md` already flags it as flaky (0/7 passes, oversubscribed-scheduler root cause) — an additional reason not to reactivate as-is even setting aside the coverage overlap |
| `mcp_session_bound_e2e.rs` | **RETIRE — ALREADY-COVERED** | Uses `common::two_daemon_harness::spawn_two_daemons_under_local_root`, which spawns `famp::cli::listen::run_on_listener` — deleted. Intent (two-MCP-window session-bound identity, full request→commit→deliver→terminal cycle) is proven against the current bus by active `mcp_bus_e2e.rs` (TEST-05) and `mcp_register_whoami.rs` |
| `send_terminal_advance_error_surfaces.rs` | RETIRE | `famp::cli::send::run_at` (v0.8 shape) + `peer::add`. Also: the specific bug class it sentinel-tests (`persist_post_send`/`advance_terminal`/swallowed-error-on-`tasks.update`) has **no equivalent code path in current `send/mod.rs`** — grepped for `advance_terminal`/`try_update`/`persist_post_send`, zero hits; the current bus-routed send module doesn't touch `TaskDir` directly at all. Not salvageable as a rewrite; if the underlying lost-update-on-error concern is still live somewhere in the current write path, it needs a *new* test authored from scratch, out of this phase's retirement-only scope |
| `mcp_stdio_tool_calls.rs` | **RETIRE — ALREADY-COVERED** | Spawns `famp init` as a subprocess (`Command::new(...).args(["init"])`) — `init` subcommand deleted, so this file cannot even reach its MCP assertions. Its actual assertion surface (8-tool enumeration: `famp_send/await/inbox/peers/register/whoami/join/leave`; error-kind mapping; register handshake) is fully covered today by active `mcp_tool_schema_invariants.rs`, `mcp_error_kind_exhaustive.rs`, `mcp_register_whoami.rs`, and `mcp_malformed_input.rs` |
| `listen_multi_peer_keyring.rs` | RETIRE | `common::conversation_harness::setup_home` + `common::listen_harness::init_home_in_process` + `famp::cli::peer::add::run_add_at` — all bound to the deleted daemon/keyring shape |

**Totals: 27/27 RETIRE.** 12 of the 27 carry an explicit `ALREADY-COVERED` pointer to a named, currently-green test. **0 REACTIVATE.** This is a stronger (more retirement-dominant) result than CONTEXT.md's scout estimate of 18/27-with-3-salvage — both CONTEXT.md-flagged salvage candidates (`send_principal_fallback.rs`, `conversation_restart_safety.rs`) were traced to harness dependencies with no live rewrite target, consistent with D-02's "the behavior still exists on a shipping surface today" bar not being met.

**Mechanics of exclusion (for the planner):** cargo's default integration-test discovery treats every `.rs` file directly under `crates/famp/tests/` as its own test binary; files one level deeper (`crates/famp/tests/_deferred_v1/*.rs`) are invisible to `cargo test`/`cargo nextest` — they are simply never listed as targets, with no `#[cfg]` gate or Cargo.toml exclusion needed. This is *why* they're dormant, not `#[ignore]`. Deleting the 27 files (and the now-pointless dead nextest.toml filter references below) needs no Cargo.toml change. If the planner ever needed to *reactivate* a file, "moving into the active glob" means `git mv crates/famp/tests/_deferred_v1/foo.rs crates/famp/tests/foo.rs` — and since `mod common;` at the top of these files resolves relative to the file's own directory, moving up to `crates/famp/tests/` makes `mod common;` correctly resolve against the already-existing `crates/famp/tests/common/mod.rs` (no import path rewrite required for that part) — but note `common/mod.rs` currently only does `pub mod mcp_harness;` (the `listen_harness`/`conversation_harness`/`two_daemon_harness`/`cycle_driver`/`child_guard` files still physically exist on disk in `crates/famp/tests/common/` but are **not** `pub mod`-exported from `common/mod.rs` today — a moved file using `common::conversation_harness::...` would need that line added back, on top of fixing the harness's own dead-CLI calls).

**Dead nextest.toml filter references (informational, not a fix required by this phase):** `.config/nextest.toml`'s `listen-subprocess` test-group filter literally names `test(/listen_/)`, `test(=conversation_restart_safety)`, and `test(=mcp_stdio_tool_calls)` — a residue from when these tests were active. Since 0 tests are being reactivated under those names, this filter currently matches nothing (harmless no-op). The planner may leave it as-is or clean it up in the same commit that deletes `_deferred_v1/`; either is acceptable, but leaving it is zero-risk and avoids scope creep into `nextest.toml` beyond what TEST-02 requires.

### TEST-02 CRUX: Empirical nextest verification (command evidence)

**Question:** Does `cargo nextest run --workspace` (= `just test` = the `test:` recipe `just ci` chains) actually list, execute, and pass the Phase 9 E2E?

**Answer: YES**, verified live in this session, cold-cache and warm-cache both checked:

```bash
$ cargo nextest list -p famp-gateway    # confirms --list phase does not hang for famp-gateway
famp-gateway::e2e_cross_host_delivery:
    gw01_gw02_gw03_two_process_cross_host_delivery
# ... (30 tests total, listed instantly)

$ cargo nextest run -p famp-gateway -E 'test(gw01_gw02_gw03_two_process_cross_host_delivery)'
PASS [   8.947s] (1/1) famp-gateway::e2e_cross_host_delivery gw01_gw02_gw03_two_process_cross_host_delivery
Summary [   8.948s] 1 test run: 1 passed, 30 skipped

$ cargo nextest run --workspace          # the exact command `just test`/`just ci` invoke
...
PASS [   9.333s] (969/969) famp-gateway::e2e_cross_host_delivery gw01_gw02_gw03_two_process_cross_host_delivery
Summary [  27.417s] 969 tests run: 969 passed, 5 skipped
```

**No `#[ignore]`, no test-group, no workaround — the full workspace run is green today, on this machine, including the gateway E2E.** GitHub Actions CI (`.github/workflows/ci.yml`'s `test (ubuntu-latest)` / `test (macos-latest)` jobs, `cargo nextest run --workspace --profile ci`) is independently confirmed green on the most recent run predating this phase's E2E commit (`gh run view 30049100024`: both `test` jobs ✓, only the exogenous `audit` job — known-flapping RustSec advisory, per memory — failed). Given the E2E individually passes under nextest with the identical invocation shape CI uses, and the full local workspace run (which now includes it) is green, **TEST-02 is (a) already satisfied.**

**Important correction to a locked assumption (flagged, not silently overridden — D-04 itself invited this verification):** CONTEXT.md D-04 cites the auto-memory `nextest_list_hang` ("`cargo nextest -p famp` hangs in the test-binary `--list` phase") as the reason to expect a problem. This session reproduced that exact symptom on a **cold** build (`timeout 60 cargo nextest list -p famp` → exit 124, timeout) — but a **warm-cache** re-run of the identical command completed in 0.78s with a full, correct listing. The "hang" was the ~40-60s of first-time compilation of `famp`'s ~600+ test functions across its many binaries exceeding a 60s timeout, not an actual infinite stall in the list logic. **Recommendation:** update/soften the `nextest_list_hang` memory to note it is a cold-build-time issue observable with a short timeout, not a genuine hang — but that correction is out of this phase's scope (it's a `~/.claude/memory/` edit, not a FAMP repo change). For TEST-02 itself, no code or config change is needed. If the planner wants defense-in-depth against a future *slow* first list (e.g., a CI cache-miss), documenting `cargo nextest run --workspace` with a generous timeout in any wrapper script is sufficient — no test-group.

**The pre-existing serialization precedent (for reference, not action needed this phase):**

```toml
# .config/nextest.toml — verbatim, already in the repo
[[profile.default.overrides]]
filter = "package(famp) and (test(/inspect_broker/) or test(/inspect_identities/) or test(/inspect_tasks/) or test(/inspect_messages/) or test(/inspect_cancel_1000/) or test(/inspect_load_test/))"
test-group = "inspect-subprocess"

[test-groups]
inspect-subprocess = { max-threads = 1 }
```

If a *future* phase reactivates a genuinely subprocess-spawning test that flakes under parallel execution, this is the exact syntax to copy: a `[[profile.default.overrides]]` (and matching `[[profile.ci.overrides]]`) block with a `filter` selecting the test(s) by package+name, a `test-group = "<name>"`, and a `[test-groups]` entry capping `max-threads`. Not needed for TEST-02 in this phase.

### DOC-04: Exact CLI Surface (verbatim from source)

**`famp-gateway`** (`crates/famp-gateway/src/main.rs`) — hand-rolled arg parser, **no clap, no `--help` flag**:

```
usage: famp-gateway [--socket <path>] --listen <addr> --tls-cert <path> \
       --tls-key <path> [--peer <domain>=<url>]... [--trust-cert <path>] \
       <principal-name>...
```

| Flag | Required | Value format | Notes |
|------|----------|---------------|-------|
| `--socket <path>` | No | filesystem path | Defaults to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock` via `famp::bus_client::resolve_sock_path` |
| `--listen <addr>` | **Yes** | `SocketAddr` (e.g. `127.0.0.1:8443` or `0.0.0.0:8443`) | Errors: `--listen: invalid address '<raw>': <err>` if unparseable; `--listen <addr> is required, e.g. --listen 127.0.0.1:8443` if omitted |
| `--tls-cert <path>` | **Yes** | filesystem path | `--tls-cert <path> is required` if omitted |
| `--tls-key <path>` | **Yes** | filesystem path | `--tls-key <path> is required` if omitted |
| `--peer <domain>=<url>` | No, repeatable | `<domain>=<base-url>`, e.g. `--peer other-host.local=https://other-host.local:8443` | Malformed (no `=`, empty domain, invalid URL) errors name the exact reason and echo the offending raw value |
| `--trust-cert <path>` | No | filesystem path | Passed to `HttpTransport::new_client_only`; omit for system trust store |
| `<principal-name>...` | **Yes, ≥1 positional** | bare name(s), e.g. `bob` | Any non-flag token is collected as a backed principal name; zero names → `usage:` error |

**Startup identity resolution** (no flag — env/`$HOME`-driven, same as the rest of the CLI): `$FAMP_HOME` or `$HOME/.famp` via `famp::cli::home::resolve_famp_home`. From that home:

- **Gateway signing key:** `<home>/gateway/identity.ed25519` (`gateway_identity_path`) — generated on first use via `load_or_generate` (idempotent: same path always yields the same key).
- **Gateway peers keyring:** `<home>/gateway/peers.keyring` (`gateway_peers_keyring_path`) — the file `famp peer import` writes to and `verify_inbound` reads from at the ingress boundary.

**Full example (two flags + one peer + two principal names):**
```bash
famp-gateway --listen 0.0.0.0:8443 --tls-cert /path/tls.cert.pem --tls-key /path/tls.key.pem \
             --peer other-host.local=https://other-host.local:8443 alice bob
```

**`famp peer export`** (`crates/famp/src/cli/peer/export.rs`, clap-based, `famp peer export --help` works):

```
famp peer export --as <principal>
```
- `--as <principal>`: required, e.g. `agent:my-mbp.local/gateway`. Prints one line to stdout: `<principal> <pubkey-b64url> <key_id>\n` (3 whitespace-separated fields, trailing newline, no PEM/multi-line blob).
- Reads/generates the gateway signing key from `<home>/gateway/identity.ed25519` (same path `famp-gateway` uses) — export and the running gateway share one identity.

**`famp peer import`** (`crates/famp/src/cli/peer/import.rs`, clap-based, `famp peer import --help` works):

```
famp peer import [<source>]      # source defaults to "-" (stdin)
```
- Positional `source`: a file path, or `-`/omitted for stdin.
- Parses `<principal> <pubkey-b64url> [<key_id>]` (2 or 3 fields; a mismatching 3rd-field fingerprint prints a `warning:` to stderr but still imports — 3rd field is advisory, not a hard gate).
- TOFU-pins into `<home>/gateway/peers.keyring`. **Fails closed** on a conflicting re-pin for the same principal (`CliError::PeerKeyConflict`) — re-importing a *different* key for an already-pinned principal is rejected, not silently overwritten.

**Two-machine bootstrap sequence (for the guide's structure), derived directly from the above:**
1. On A: `famp peer export --as agent:hostA.example/gateway` → copy the printed line out-of-band (Signal/clipboard).
2. On B: paste A's line into `famp peer import` (stdin or a file).
3. On B: `famp peer export --as agent:hostB.example/gateway` → copy to A.
4. On A: `famp peer import` with B's line.
5. Start each gateway: `famp-gateway --listen <bind-addr> --tls-cert <cert> --tls-key <key> --peer <other-domain>=<other-base-url> <local-principal-name>...`
6. Verify: send a message addressed to the remote principal, then `famp inspect tasks --id <task_id> --json` on both sides to confirm state advances (mirrors Phase 9's E2E assertion pattern — `poll_terminal_state`/`famp inspect tasks`).

### Accuracy-gate pattern to mirror (D-07)

The v0.11 Phase 6 precedent is `crates/famp/tests/cli_help_invariant.rs`, which does exactly the pattern needed: `Command::cargo_bin("famp").args(["--help"]).output()`, then asserts on stdout content (presence/absence of specific verb tokens). `onboarding_line_count_gate.rs` / `readme_line_count_gate.rs` show the doc-content-assertion half of the pattern (substring `.contains(...)` checks against `docs/ONBOARDING.md` / `README.md`).

**Adaptation needed for `famp-gateway` specifically:** it has no `--help`. Running it with zero args (or with recognized flags but zero positional names) triggers the `names.is_empty()` branch, which prints the full `usage: famp-gateway ...` string to stderr and exits 1 — this is the closest thing to `--help` and is what a new accuracy-gate test should invoke (`Command::cargo_bin("famp-gateway").output()`, assert stderr contains each flag token also present in `docs/GATEWAY-SETUP.md`). For `famp peer export`/`import`, use the same `cli_help_invariant.rs`-style `--help` invocation since those ARE clap subcommands.

### Recommended Project Structure (docs, no new dirs)

```
docs/
├── GATEWAY-SETUP.md          # NEW (D-06) — two-machine runbook
├── MIGRATION-v0.8-to-v0.9.md # existing — currently the (stale) target of README's federation link
crates/famp/tests/
├── _deferred_v1/              # DELETED in this phase (27 files + README, or README becomes TRIAGE.md)
├── cli_help_invariant.rs      # existing precedent for the accuracy-gate pattern
├── (new) gateway_setup_doc_accuracy.rs   # or similar — D-07 grep-gate
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|--------------|-----|
| Serializing a flaky subprocess test group | A custom `#[serial]`-style mutex/lock in test code | nextest's `[test-groups]` + `[[profile.*.overrides]]` (already in `.config/nextest.toml`) | It's the existing, CI-integrated mechanism; a hand-rolled lock would duplicate what nextest already does and wouldn't apply under `--profile ci` |
| Doc/binary drift detection | Manual periodic doc review | `Command::cargo_bin(...)` + assert on stdout/stderr, following `cli_help_invariant.rs` | Compiles the check into CI; catches drift the next time the flag surface changes, not just at authoring time |

**Key insight:** every piece of "don't hand-roll" machinery this phase needs already exists in the repo (nextest test-groups, the `cli_help_invariant.rs` accuracy pattern, `ChildGuard`, fixture certs). This phase is composition, not new infrastructure.

## Common Pitfalls

### Pitfall 1: Mistaking a cold-build compile for a nextest hang
**What goes wrong:** `cargo nextest list -p famp` (or any package-scoped list) with a short timeout (e.g. 60s) appears to hang and returns a timeout exit code.
**Why it happens:** On a cold `target/` cache, nextest must compile every test binary in the package before it can emit a list — `famp` has 60+ integration test binaries. This can take well over a minute.
**How to avoid:** Never conclude "hang" from a single short-timeout run. Re-run with a longer timeout, or run once to warm the cache, then re-check.
**Warning signs:** The command prints `Finished 'test' profile ... target(s) in Xs` and then nothing further for a while — that's a slow list phase settling in, not a stall (a genuine stall in the list logic would not print the `Finished` compile line at all, since compilation itself already completed).

### Pitfall 2: `mod common;` resolution breaks silently on file move
**What goes wrong:** Moving a file from `_deferred_v1/foo.rs` up to `tests/foo.rs` and expecting `mod common;` to keep working without checking `common/mod.rs`'s current `pub mod` list.
**Why it happens:** `common/mod.rs` today only exports `mcp_harness` — `listen_harness`, `conversation_harness`, `two_daemon_harness`, `cycle_driver`, `child_guard` files exist on disk but are unexported. This phase doesn't need to fix this (nothing is being reactivated), but a future phase that does must add the `pub mod` line back, on top of fixing the harness's own dead-CLI calls.
**How to avoid:** Not applicable to this phase's RETIRE-only outcome — flagged for completeness in case scope changes.

### Pitfall 3: `just lint` vs plain clippy (CLAUDE.md / project memory)
**What goes wrong:** Running plain `cargo clippy` and declaring lint-clean, missing nursery lints `just lint` promotes.
**How to avoid:** Run `just lint` for any Rust-touching change in this phase (deleting the 27 files still requires a `just lint` pass since it changes what compiles in the workspace; a new accuracy-gate test file definitely does).

### Pitfall 4: `.planning/` is gitignored
**What goes wrong:** An isolated-worktree executor loses uncommitted `.planning/` docs (PLAN.md, SUMMARY.md) on worktree cleanup.
**How to avoid:** Per project memory, run this phase's executors non-isolated on `main` (matches Phases 7-9's precedent, already reflected in this repo's git history).

### Pitfall 5: The `audit` CI job is exogenous noise
**What goes wrong:** Treating the `audit` job's failure in a CI run as a blocker for TEST-02's "runs green in `just ci`" claim.
**Why it happens:** Per project memory, the `audit` job flaps on exogenous RustSec advisories unrelated to code changes — confirmed in this session's `gh run view`, where `audit` was the only failing job out of 9 while both `test` jobs (the ones that matter for TEST-02) passed.
**How to avoid:** TEST-02's success criterion is the `test (ubuntu-latest)` / `test (macos-latest)` jobs (`cargo nextest run --workspace --profile ci`), not the `ci:` justfile recipe's full chain (which also runs `audit`-adjacent checks via `publish-workspace-dry-run` etc., but not `audit` itself — `audit` is CI-only, not in the `justfile`'s `ci:` recipe at all, so local `just ci` is unaffected either way).

## Code Examples

### Accuracy-gate pattern (mirror this, from `crates/famp/tests/cli_help_invariant.rs`)

```rust
// Source: crates/famp/tests/cli_help_invariant.rs (existing, in-repo)
use std::process::Command;
use assert_cmd::cargo::CommandCargoExt;

#[test]
fn famp_help_omits_deleted_federation_verbs() {
    let out = Command::cargo_bin("famp").unwrap().args(["--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // ... substring assertions against the live --help output
}
```

For `famp-gateway` (no `--help`), the equivalent probe is a no-args invocation, which exits 1 and prints the full usage string to stderr:

```rust
// New pattern needed for DOC-04 (famp-gateway has no clap, no --help)
let out = Command::cargo_bin("famp-gateway").unwrap().output().unwrap();
assert!(!out.status.success());
let stderr = String::from_utf8_lossy(&out.stderr);
assert!(stderr.contains("--listen"));
assert!(stderr.contains("--tls-cert"));
assert!(stderr.contains("--tls-key"));
assert!(stderr.contains("--peer"));
assert!(stderr.contains("--trust-cert"));
```

### Peer export/import round trip (already proven, mirror for the guide's example output)

```rust
// Source: crates/famp/tests/peer_roundtrip.rs (existing, active, TRUST-01)
// Shows the exact call shape the guide should describe in prose:
// famp peer export --as <principal>  ->  famp peer import (stdin/file)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `famp init/setup/listen/peer add` HTTPS-daemon CLI, `_deferred_v1/`'s subject | `famp-gateway` process + `famp peer export/import` (Ed25519 TOFU) + local UDS bus | v0.9 Phase 4 (deletion), Phase 7-9 (v1.0 gateway build) | All 27 deferred tests target a surface with no live equivalent to rewrite against 1:1 |
| GW-03 terminal-state proof via appended `control`/`cancel` envelope (workaround) | Genuine `COMPLETED` state derived from the deliver envelope's own `terminal_status` field | Commit `785b8c2` (fix(09-gw03)), 2026-07-27, same day as this research | The E2E test currently in the active glob is the POST-fix, workaround-free version — Phase 10 promotes this final form, not an earlier draft |

**Deprecated/outdated:** README.md's Quick Start (line ~170) still points cross-host-federation readers at `docs/MIGRATION-v0.8-to-v0.9.md` — that doc describes the v0.8→v0.9 CLI removal, not how to use the v1.0 gateway. DOC-04's guide should become the linked target for that sentence (or be added alongside it, at the planner's discretion per D-06).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | GitHub Actions CI (`test (ubuntu-latest)`/`test (macos-latest)`) will pass with the Phase 9 E2E included, matching this session's local `cargo nextest run --workspace` result | TEST-02 CRUX | LOW — the most recent CI run predates the E2E test's commit, so this is inferred from (a) the local full-workspace pass including the E2E, and (b) CI's identical `cargo nextest run --workspace --profile ci` invocation passing on a prior commit. The planner should still treat "confirm green in an actual CI run after this phase's changes land" as a real verification step, not skip it because this research says so |

**All other claims in this research are `[VERIFIED]`** — obtained by directly reading the named source files or running the named commands in this session, not from training-data assumption or unauthenticated web search.

## Open Questions

1. **Should the `nextest.toml` dead `listen-subprocess` filter (naming `conversation_restart_safety`/`mcp_stdio_tool_calls`, both being retired) be cleaned up in this phase?**
   - What we know: It currently matches zero tests (harmless no-op) since neither name will exist post-retirement.
   - What's unclear: Whether leaving stale-but-harmless config in place violates any hygiene norm the project cares about.
   - Recommendation: Leave it — cleaning it up is optional polish, not required for TEST-01/TEST-02 compliance, and touching `nextest.toml` beyond what's needed risks an unnecessary CI-config diff. Flag as Claude's discretion if the planner wants to include it as a one-line cleanup task.

2. **Where exactly should `docs/GATEWAY-SETUP.md` be linked from — the Quick Start sentence only, or also `## Advanced: v0.8 federation CLI`?**
   - What we know: Both sections currently reference federation in some form; Quick Start points at the stale migration doc, "Advanced" describes the deleted CLI's escape hatch.
   - What's unclear: Whether "Advanced: v0.8 federation CLI" should be renamed/restructured to also link the new guide, or left untouched as v0.8-specific history.
   - Recommendation: Link from Quick Start (replaces/supplements the stale pointer) as the primary entry point; leave "Advanced: v0.8 federation CLI" as historical/escape-hatch documentation for `v0.8.1-federation-preserved` users, unmodified. Planner's call at the CONTEXT.md-granted discretion level (D-06 doesn't specify).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|--------------|-----------|---------|----------|
| `cargo-nextest` | TEST-02 verification | ✓ | (workspace-resolved; CI installs via `taiki-e/install-action`) | — |
| `just` | Running `just ci`/`just test`/`just lint` | ✓ (used throughout this session) | — | — |
| `gh` CLI | Checking CI run history (this research only) | ✓ | — | — |

No missing dependencies. This phase has no new external service or tool dependency beyond what Phases 7-9 already required.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo-nextest (workspace), plain `cargo test` for doctests |
| Config file | `.config/nextest.toml` |
| Quick run command | `cargo nextest run -p famp-gateway -E 'test(gw01_gw02_gw03_two_process_cross_host_delivery)'` (~9s) |
| Full suite command | `just test` (= `cargo nextest run --workspace`, ~27s warm) or `just ci` (full CI-parity chain) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|--------------------|--------------|
| TEST-01 | Deferred tests triaged, dead ones removed with rationale | Manual/doc verification (ledger review) + `cargo build --workspace` (proves nothing references the deleted files) | `cargo build --workspace --all-targets` | N/A — deletion + doc, no new test asserts triage completeness beyond "it still compiles" |
| TEST-02 | Gateway E2E runs green in `just ci` | integration | `cargo nextest run -p famp-gateway -E 'test(gw01_gw02_gw03_two_process_cross_host_delivery)'` then `just test`/`just ci` | ✅ (`crates/famp-gateway/tests/e2e_cross_host_delivery.rs`, already shipped Phase 9) |
| DOC-04 | Guide's flags/commands match the shipping binary | integration (new) | `cargo test -p famp --test gateway_setup_doc_accuracy` (name TBD by planner) | ❌ — Wave 0 gap, planner must create |

### Sampling Rate
- **Per task commit:** targeted `cargo nextest run -p famp-gateway` / `cargo nextest run -p famp --test <new_accuracy_test>`
- **Per wave merge:** `just test` (full workspace)
- **Phase gate:** `just ci` full chain green before `/gsd-verify-work`; a real GitHub Actions CI run after push (per Assumption A1)

### Wave 0 Gaps
- [ ] New accuracy-gate test file (name TBD, e.g. `crates/famp/tests/gateway_setup_doc_accuracy.rs` or under `crates/famp-gateway/tests/`) — covers DOC-04's D-07 grep-gate requirement. No existing file covers this; `cli_help_invariant.rs` is the pattern to copy, not a file to extend (different binary, different — no `--help` — surface).
- [ ] `crates/famp/tests/_deferred_v1/TRIAGE.md` (or equivalent) — the TEST-01 ledger itself; doesn't exist yet.

*(Framework install: none needed — `cargo-nextest` already present.)*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | No | Out of scope — this phase touches no auth surface (test triage + docs) |
| V3 Session Management | No | Out of scope |
| V4 Access Control | No | Out of scope |
| V5 Input Validation | Marginal — Yes for the new accuracy-gate test | Standard Rust string/subprocess-output handling; no new parser |
| V6 Cryptography | No new surface | DOC-04 *documents* existing Ed25519 TOFU trust bootstrap (Phase 8, already ASVS-reviewed there) but adds no new crypto code |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| Setup guide instructs an insecure practice (e.g., pasting private key material instead of the public export line) | Information Disclosure | The `famp peer export` blob is, by design, public-key-only (`<principal> <pubkey-b64url> <key_id>`, no secret material) — the guide must never instruct copying `<home>/gateway/identity.ed25519` itself; confirmed the export command never touches that file's contents, only derives a public verifying key from it |
| Doc drifts from binary, developer follows stale flag names, connects insecurely or fails silently | Tampering (of trust, via doc/binary mismatch) | The D-07 accuracy grep-gate directly mitigates this — CI fails if the doc and binary diverge |

## Sources

### Primary (HIGH confidence — all `[VERIFIED]`, read/run directly in this session)
- `crates/famp/tests/_deferred_v1/*.rs` (27 files) and `README.md` — read in full or to sufficient depth to confirm every dependency
- `crates/famp/src/cli/mod.rs` — confirmed current `Commands` enum and module tree (no `init`/`setup`/`listen`/`peer::add`)
- `crates/famp-gateway/src/main.rs` — exact flag parser, usage string, identity/keyring paths
- `crates/famp/src/cli/peer/{mod,export,import}.rs` — exact `famp peer export/import` shape
- `.config/nextest.toml` and `justfile` — exact `test:`/`ci:` recipe chain and existing test-group precedent
- Live command execution: `cargo nextest list -p famp-gateway`, `cargo nextest run -p famp-gateway -E '...'`, `cargo nextest list -p famp` (cold, timed out; warm, 0.78s), `cargo nextest run --workspace` (969/969 passed) — all run in this session, this repo state
- `gh run view 30049100024` — GitHub Actions CI job breakdown (9 jobs, 8 green, `audit` exogenous failure)
- `crates/famp/tests/{cli_help_invariant,peer_roundtrip,onboarding_line_count_gate,readme_line_count_gate}.rs`, `crates/famp-inbox/tests/truncated_tail.rs`, `crates/famp/tests/mcp_{tool_schema_invariants,error_kind_exhaustive,register_whoami,bus_e2e,malformed_input}.rs` — read to confirm ALREADY-COVERED claims
- `.planning/phases/09-end-to-end-cross-host-delivery/{09-05-SUMMARY.md,09-VERIFICATION.md}` — GW-03 fix history and post-fix verified state
- `README.md` — Quick Start / Platform support / Advanced federation sections, exact line ranges

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all tooling already in-repo and exercised live
- Architecture (TEST-01 triage): HIGH — every one of the 27 files was read and its dependency chain traced to a specific deleted symbol
- Architecture (TEST-02 CRUX): HIGH — the exact command `just ci` invokes was run live, twice (isolated + full workspace), both green
- DOC-04 CLI surface: HIGH — read verbatim from source, cross-checked against the files' own unit tests
- Pitfalls: HIGH — each pitfall was directly observed in this session (the cold-build timeout, the `audit` job flap) or sourced from project memory

**Research date:** 2026-07-27
**Valid until:** Until the next code change to `crates/famp-gateway/src/main.rs`, `crates/famp/src/cli/peer/`, or `.config/nextest.toml` — this is a code-grounded snapshot, not a stable API surface; DOC-04's own D-07 accuracy-gate is the durable defense against drift going forward.
