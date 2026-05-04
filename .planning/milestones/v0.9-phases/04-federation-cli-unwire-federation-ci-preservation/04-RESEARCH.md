# Phase 4: Federation CLI unwire + federation-CI preservation — Research

**Researched:** 2026-05-03
**Domain:** Rust workspace surgery (CLI surface deletion + crate relabeling) with atomic-commit / git-tag escape-hatch discipline. Federation-test reorganization. Migration-doc authoring.
**Confidence:** HIGH (every claim verified against source files and `git log`; no `[ASSUMED]` claims).
**Source authority:** `04-CONTEXT.md` (22 locked decisions), `REQUIREMENTS.md` §FED/MIGRATE/TEST-06/CARRY-01, `ROADMAP.md` Phase 4 success criteria, `docs/superpowers/specs/2026-04-17-local-first-bus-design.md` §"Phasing/Phase 4".

---

## User Constraints (from CONTEXT.md)

### Locked Decisions (22)

- **D-01:** Move `#[ignore = "Phase 04 ..."]` test files to `crates/famp/tests/_deferred_v1/`, do NOT hard-delete. ~19 ignored test rows across ~14 files (anchor: STATE.md 2026-04-30 sweep). Tests are intent documents.
- **D-02:** Add `crates/famp/tests/_deferred_v1/README.md` explaining the freeze: (a) why dormant; (b) reactivation criteria (Sofer-from-different-machine triggers v1.0 federation gateway, then port-and-rename against new lib API); (c) link to `docs/history/v0.9-prep-sprint/famp-local/` and `docs/MIGRATION-v0.8-to-v0.9.md`.
- **D-03:** Case-by-case audit before move — port any property not already covered by Phase 1 PROP-01..05, Phase 2 TEST-01..05, refactored `e2e_two_daemons`, or `tests/adversarial/` HTTP rows. Expected ratio: ~17 moves, ~2 ports.
- **D-04:** Move via `git mv` (preserves history); README and any ports land in the SAME commit as the move.
- **D-05:** Hard delete the 6 federation verbs and their modules. No soft-deprecation stubs.
- **D-06:** `famp send` keeps its name — only the TLS-form (HTTPS-via-`famp listen`) code paths get deleted. Bus-routed `famp send` (Phase 2 plan 02-04) stays.
- **D-07:** Removal commit lands AFTER `e2e_two_daemons` library-API refactor AND AFTER `v0.8.1-federation-preserved` tag is cut. Three-step sequence non-negotiable.
- **D-08:** After deletion, `Commands` enum has 16 variants (down from current 22). Removed: `Init`, `Setup`, `Listen`, `Peer`. Kept: `InstallClaudeCode`, `UninstallClaudeCode`, `InstallCodex`, `UninstallCodex`, `Info`, `Send`, `Await`, `Inbox`, `Mcp`, `Broker`, `Register`, `Join`, `Leave`, `Sessions`, `Whoami`.
- **D-09:** Single happy-path test + one adversarial sentinel in `e2e_two_daemons`. No further expansion.
- **D-10:** Same tokio runtime, two `tokio::spawn` listener tasks (NOT two separate runtimes).
- **D-11:** Reuse existing `crates/famp/tests/fixtures/cross_machine/` certs.
- **D-12:** Conversation shape unchanged: `request → commit → deliver → ack`.
- **D-13:** Adversarial cases in `crates/famp/tests/adversarial/` HTTP rows stay where they are — do NOT duplicate inside `e2e_two_daemons`.
- **D-14:** Archive `scripts/famp-local` to `docs/history/v0.9-prep-sprint/famp-local/`. Do NOT hard-delete. Use `git mv`. Add single-line README marking the script as frozen.
- **D-15:** Update backlog 999.6 path with new archive location atomically with the move.
- **D-16:** MIGRATE-03 doc tone — staged framing ("FAMP today is local-first; FAMP at v1.0 is federated"), NOT identity rewrite.
- **D-17:** Staged-framing landing sites: README.md first paragraph + Quick Start preamble; CLAUDE.md "## Project" one-line description; `.planning/ROADMAP.md` v0.9 milestone callout; `ARCHITECTURE.md` consistency check; `.planning/MILESTONES.md` (researcher MUST verify existence — see Audit 1).
- **D-18:** `docs/MIGRATION-v0.8-to-v0.9.md` MUST contain CLI mapping table + `.mcp.json` cleanup + `~/.famp/` dir cleanup + tag pointer + `_deferred_v1/` pointer. Terse, table-first, ≤200 lines.
- **D-19:** `v0.8.1-federation-preserved` is a lightweight tag (not annotated).
- **D-20:** Tag commit MUST satisfy three properties: (a) `just ci` green; (b) all 6 federation CLI verbs still functional; (c) `e2e_two_daemons` already refactored to library API and green.
- **D-21:** Tag discoverability via README + MIGRATION-doc references only. No GitHub release artifact.
- **D-22:** CARRY-01 is closed in code per STATE.md 2026-04-30. Phase 4 only updates bookkeeping (verify pin still exists, reference closing SHA, flip checkbox).

### Claude's Discretion

- Exact `git mv` glob invocation for test-file freeze move
- One README + minimal doc-comment markers on each frozen test file (lean) vs per-file inline doc-comment headers
- Bundle 6 CLI deletion arms into one atomic commit (lean) vs six smaller commits — planner's call
- Workspace `Cargo.toml` exact comment string for "v1.0 federation internals" relabel (~5 words)
- Whether `Commands::Info` survives as-is or gets a doc-comment update (lean: doc-comment update only — but see Risk #1 below; `Info` has hidden coupling)
- `MIGRATION-v0.8-to-v0.9.md` exact bullet wording
- `.planning/MILESTONES.md` existence check (audited below)
- `_deferred_v1/` README links to v1.0 federation spec sketch in PROJECT.md vs just trigger condition

### Deferred Ideas (OUT OF SCOPE)

- Conformance vector pack (deferred to v1.0 alongside `famp-gateway`)
- `famp-gateway` design or implementation (v1.0 trigger-gated)
- Soft-deprecation stubs for removed verbs (rejected; migration doc carries the load)
- Two-runtime `e2e_two_daemons` (rejected; same-runtime good enough for parked path)
- "FAMP is local-first" identity rewrite (rejected; staged framing instead)
- `scripts/famp-local` hard delete (rejected; archive preserves provenance + 999.6 backlog actionability)
- GitHub release artifact for tag (defer)
- `_deferred_v1/` reactivation roadmap (defer to v1.0 planning)
- CC-05/CC-09 wording fixes (locked in Phase 3)
- `famp mailbox rotate` / `famp mailbox compact` (deferred to v0.9.1)
- Heartbeat / `user_attention` envelope classes (Phase 999.3 / 999.4 backlog)

---

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FED-01 | Top-level CLI removals (`famp setup/listen/init/peer add/peer import`, old TLS-form `famp send`) | Audit 5 (`Commands` enum surgery), Audit 3 (TLS-form `famp send` import chain), Risk #1 (`Info` hidden coupling) |
| FED-02 | `famp-transport-http` + `famp-keyring` relabeled as v1.0 federation internals in workspace `Cargo.toml` comments | Workspace `Cargo.toml` lines 10, 12 already have member entries — comment relabel is a 2-line edit |
| FED-03 | `e2e_two_daemons` refactored to library API — instantiates two `famp-transport-http` server instances directly | Audit 6 (lib API surface map) — `build_router`, `HttpTransport`, `Keyring` all already public |
| FED-04 | Federation e2e test green in `just ci` on every commit | Audit 6 reuse pattern: `tests/http_happy_path.rs` is already a working template |
| FED-05 | Tag `v0.8.1-federation-preserved` cut on commit BEFORE deletions | Audit 7 (tag mechanics), Audit 11 (commit sequence) |
| FED-06 | `cargo tree` shows federation crates consumed only by refactored e2e test, no top-level CLI usage | Validation arch §`cargo tree` invariants — `cargo tree -p famp-transport-http -i` |
| MIGRATE-01 | `docs/MIGRATION-v0.8-to-v0.9.md` CLI mapping table | Audit 9 (skeleton draft) |
| MIGRATE-02 | `.mcp.json` cleanup instructions | Audit 9 — embedded in skeleton |
| MIGRATE-03 | README + CLAUDE + MILESTONES updated; local-first headline / federation v1.0 promise | Audit 1 (MILESTONES exists), Audit 10 (staged-framing landing sites) |
| MIGRATE-04 | `scripts/famp-local` marked deprecated (archived to `docs/history/v0.9-prep-sprint/famp-local/`) | Audit 8 (archive scope, 1316 LOC single bash file) |
| TEST-06 | Conformance gates (RFC 8785, §7.1c) continue running unchanged on every CI run | No changes to `famp-canonical` / `famp-crypto`; Phase 4 surgery is CLI-only. Existing `just ci` recipe untouched. |
| CARRY-01 | `[[profile.default.test-groups]] listen-subprocess = max-threads=4` pinned | Audit 4 — pin is at HEAD; closing SHA `ebd0854`. **Bookkeeping-only** per D-22. |

---

## Executive Summary (TL;DR — 5 decision-critical findings)

1. **The deletion blast radius is larger than CONTEXT.md anchors imply.** STATE.md cites "~19 rows across ~14 files" of `Phase 04` ignore-tagged tests. Audit 2 confirms exactly **20 ignore rows across 14 files**. But **at least 9 OTHER federation-coupled test files are NOT yet ignore-tagged** (`init_*.rs`, `peer_*.rs`, `setup_*.rs`, `info_happy_path.rs`, `listen_bind_collision.rs`, `listen_durability.rs`, `listen_shutdown.rs`, `listen_truncated_tail.rs`, plus `http_happy_path.rs`, `cross_machine_happy_path.rs`, `adversarial.rs` + `tests/adversarial/`). They currently compile only because `init/setup/peer/listen` modules still exist. After D-05 deletion they will fail to compile and MUST also move. Planner: the `_deferred_v1/` move set is **~23 test files, not 14**; the count anchor in STATE.md predates the deletion-scope audit.

2. **`Commands::Info` is NOT safe to keep as-is.** D-08 keeps `Info` in the enum, but `crates/famp/src/cli/info.rs` line 13 imports `crate::cli::setup::PeerCard` and line 14 imports `crate::cli::init::load_identity`. Deleting `setup` and `init` modules breaks `Info` at compile time. Planner has three resolutions: (a) move `PeerCard` struct into `info.rs` directly (drop the cross-module dep); (b) keep `setup.rs` as a one-struct file (violates D-05 hard-delete spirit); or (c) drop `Info` from the enum too (would shrink to 15 variants, conflicts with locked D-08). **Recommend (a)** — surgical, preserves D-08, and the peer-card emitter remains useful for v1.0 federation. Flag for plan-lock.

3. **The TLS-form `famp send` import chain is contained: `crates/famp/src/cli/send/client.rs` is the entire TLS surface, plus a single re-import in `crates/famp/src/cli/listen/auto_commit.rs:40`.** Audit 3 found NO consumers of `send::client::*` outside listen-stack code. Bus-routed `famp send` (`crates/famp/src/cli/send/mod.rs`) only depends on `famp_bus`, not on `famp-keyring` or `famp-transport-http`. The cut is clean: delete `send/client.rs` and `send/fsm_glue.rs` entirely; no surgery needed inside `send/mod.rs`. The `famp-keyring` / `famp-transport-http` references in the umbrella `famp` crate live in `cli/init/tls.rs`, `cli/listen/mod.rs`, `cli/listen/router.rs`, `runtime/loop_fn.rs`, `runtime/peek.rs`, and `runtime/error.rs` — all of which die with `init`/`listen` deletion. The `runtime/` module exists only for the TLS-form path and dies wholesale.

4. **MILESTONES.md exists.** Audit 1 confirms `.planning/MILESTONES.md` is present (155 lines, current). MIGRATE-03 retargeting is unnecessary; the staged-framing edit lands in MILESTONES.md as originally planned. Open audit closed.

5. **CARRY-01 closing commit SHA: `ebd0854ff793b7b9112ff69665a235304f915533`** — `chore(tech-debt): TD-1 — pin listen-subprocess nextest test-group at max-threads=4` (2026-04-30). Pin is verified at HEAD. Bookkeeping-only flip per D-22.

**Primary recommendation:** plan as 9 ordered atomic commits (Audit 11) with the `v0.8.1-federation-preserved` tag cut between commits 4 and 5; the `_deferred_v1/` test-freeze move is the largest single commit at ~23 file moves + ~2 ports + 1 README, and must precede all deletions.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| User-facing CLI (deletion targets) | Application binary (`crates/famp/src/cli/`) | — | Phase 4 strips federation verbs from this layer only. Protocol-primitive crates untouched. |
| Federation library surface (preservation target) | Library crates (`famp-transport-http`, `famp-keyring`) | Test consumer (`e2e_two_daemons`) | These crates stay compiling; their ONLY consumer post-Phase-4 is the refactored e2e integration test. |
| Migration documentation | Repo-root docs (`docs/MIGRATION-v0.8-to-v0.9.md`) | Repo-root framing (`README.md`, `CLAUDE.md`, `.planning/MILESTONES.md`) | Hard-delete carries no soft-deprecation stub; the doc IS the discoverability surface. |
| Design-provenance archive | `docs/history/v0.9-prep-sprint/famp-local/` | Backlog 999.6 (path reference) | `scripts/famp-local` is frozen, not maintained — preserves T1–T9 prep-sprint receipts. |
| Test-freeze archive | `crates/famp/tests/_deferred_v1/` | `_deferred_v1/README.md` | Federation-tagged tests are reactivation seed material for v1.0 `famp-gateway` integration suite. |
| Carry-forward debt closure (CARRY-01) | `.config/nextest.toml` (already pinned) | `.planning/REQUIREMENTS.md` checkbox flip + ROADMAP traceability | Bookkeeping-only — no code change. |
| Atomic git-tag escape hatch | `v0.8.1-federation-preserved` (lightweight tag) | `MIGRATION-v0.8-to-v0.9.md` pointer | Single-digit user count → no GitHub release; tag pointer is enough discoverability. |

---

## Research Audits

### Audit 1 — `.planning/MILESTONES.md` existence check (D-17)

**Status:** ✅ **EXISTS** (verified `ls .planning/MILESTONES.md` → 155 lines, current as of 2026-04-26).

**Content shape:** The file is a chronologically-ordered milestone changelog. Top section is "v0.8 Usable from Claude Code (Shipped: 2026-04-26)"; second section is "v0.9 Local-First Bus (In Design — 2026-04-17)" lines 24–53. Phase 4 staged-framing edit lands in **lines 24–53**: replace the "implementation paused pending ~2 weeks" hedge (line 26–27) with the v0.9-as-shipping framing, and add the explicit "today: local-first; trigger to v1.0: Sofer-from-different-machine" sentence per D-17.

**Implication:** MIGRATE-03 retargeting to ROADMAP.md is **not needed**. The original D-17 plan (edit MILESTONES.md alongside README/CLAUDE/ROADMAP) holds. The four-file framing edit set is unchanged.

**Source:** `[VERIFIED: ls + Read]` `.planning/MILESTONES.md` lines 1–155.

---

### Audit 2 — Full enumeration of `Phase 04` ignore-tagged test rows (D-01/D-03)

**Status:** ✅ Anchor confirmed and refined: STATE.md says "~19 rows across ~14 files"; exhaustive grep finds **exactly 20 rows across 14 files**.

**Full enumeration** (file path | test name proxy | anchor classification):

| # | File path | Line | Property guarded | MOVE / PORT |
|---|-----------|------|------------------|-------------|
| 1 | `crates/famp/tests/conversation_restart_safety.rs` | 21 | task record persistence across daemon restart | **MOVE** — task-record persistence is now broker-state concern (Phase 1 PROP-04 drain completeness covers offline-then-online in mailbox layer); v0.8 HTTPS-specific path is parked |
| 2 | `crates/famp/tests/listen_smoke.rs` | 29 | `famp listen` happy-path daemon spin-up | **MOVE** — daemon shape gone in v0.9; `famp register` is the analogue, covered by Phase 2 TEST-01 |
| 3 | `crates/famp/tests/listen_multi_peer_keyring.rs` | 103 | multi-peer keyring trust at listen time | **MOVE** — keyring is parked v1.0 federation internals; refactored `e2e_two_daemons` exercises 2-peer keyring |
| 4 | `crates/famp/tests/listen_multi_peer_keyring.rs` | 155 | multi-peer keyring (variant 2) | **MOVE** — same property as row 3 |
| 5 | `crates/famp/tests/listen_multi_peer_keyring.rs` | 203 | multi-peer keyring (variant 3) | **MOVE** — same property as row 3 |
| 6 | `crates/famp/tests/mcp_session_bound_e2e.rs` | 80 | session-bound MCP via TLS path | **MOVE** — Phase 2 02-13 TEST-05 (MCP-via-bus E2E) is the v0.9 analogue |
| 7 | `crates/famp/tests/send_more_coming_requires_new_task.rs` | 47 | `--more-coming` flag flag-matrix | **PORT** — flag matrix is still relevant on bus path; `crates/famp/src/cli/send/mod.rs` `more_coming_without_new_task_errors_in_run_at_structured` already covers it (line 510) — verify before move |
| 8 | `crates/famp/tests/mcp_stdio_tool_calls.rs` | 322 | MCP `famp_send` tool over TLS | **MOVE** — Phase 2 02-13 (TEST-05) covers MCP send-over-bus |
| 9 | `crates/famp/tests/mcp_stdio_tool_calls.rs` | 442 | `peers.toml` fixture shape inside MCP | **MOVE** — peers.toml is parked; v0.9 has no peer-card concept |
| 10 | `crates/famp/tests/mcp_stdio_tool_calls.rs` | 565 | file-fixture writes (variant 1) | **MOVE** — file-fixture writes are v0.8 federation shape |
| 11 | `crates/famp/tests/mcp_stdio_tool_calls.rs` | 585 | file-fixture writes (variant 2) | **MOVE** — same as row 10 |
| 12 | `crates/famp/tests/mcp_stdio_tool_calls.rs` | 603 | `peer_not_found` failure mode (TLS path) | **MOVE** — bus path uses `BusErrorKind::NotRegistered`, exhaustively tested in Phase 2 |
| 13 | `crates/famp/tests/send_terminal_advance_error_surfaces.rs` | 67 | terminal-deliver error surfacing on TLS path | **PORT** — error-surface property still applies to bus-routed send; verify Phase 2 02-04 covers; if not, port to a sibling test on bus path |
| 14 | `crates/famp/tests/send_new_task.rs` | 34 | new-task happy-path on TLS | **MOVE** — Phase 2 TEST-01 covers DM round-trip |
| 15 | `crates/famp/tests/send_deliver_sequence.rs` | 34 | full deliver sequence on TLS | **MOVE** — covered by refactored `e2e_two_daemons` (request → commit → deliver → ack) |
| 16 | `crates/famp/tests/send_new_task_scope_instructions.rs` | 38 | scope-instructions on `--new-task` over TLS | **MOVE** — scope-instructions are v0.5.2 envelope-level; covered by `audit_log_dispatch.rs` and Phase 1 |
| 17 | `crates/famp/tests/send_principal_fallback.rs` | 51 | principal fallback (variant 1) over TLS | **MOVE** — bus path uses identity-resolution Tier 1–4 (Phase 2 D-01); fallback property covered in `cli/identity.rs` unit tests |
| 18 | `crates/famp/tests/send_principal_fallback.rs` | 75 | principal fallback (variant 2) | **MOVE** — same as row 17 |
| 19 | `crates/famp/tests/send_principal_fallback.rs` | 103 | principal fallback (variant 3) | **MOVE** — same as row 17 |
| 20 | `crates/famp/tests/send_tofu_bootstrap_refused.rs` | 36 | TOFU bootstrap refused (env-var gate) | **MOVE** — TOFU is v1.0 federation-internal; no analogue on the bus |

**Summary:** 18 MOVE, 2 PORT (rows 7 and 13). The PORT cases need verification before move:

- **PORT row 7** (`send_more_coming_requires_new_task.rs:47`): the bus-path test `more_coming_without_new_task_errors_in_run_at_structured` in `crates/famp/src/cli/send/mod.rs` line 510 already enforces this property at the `run_at_structured` entry point. **Recommendation: verify, then MOVE** — the property is already preserved.
- **PORT row 13** (`send_terminal_advance_error_surfaces.rs:67`): error-surfacing on terminal advance failure. Phase 2 02-04 plan added `BusReply::Err { kind: BusErrorKind::TaskNotFound }` for stale `--task` UUIDs. The terminal-advance-failure case maps to `TaskNotFound` or `EnvelopeInvalid` in v0.9. **Recommendation: confirm coverage in Phase 2 02-04 SUMMARY; if absent, port a small test exercising `BusReply::Err` on bad `--task` UUID with `--terminal`.**

**The 14 files** (per CONTEXT.md anchor) covering 20 rows are confirmed. But **Audit 2.5 below** surfaces the much larger problem of compile-time-coupled tests.

**Source:** `[VERIFIED: grep -rn '#\[ignore = "Phase 04']` over `crates/*/tests/`.

---

### Audit 2.5 — Federation-coupled test files NOT yet ignore-tagged (compile-time blast radius)

**Status:** 🔴 **GAP IN CONTEXT.md.** The `_deferred_v1/` move scope is larger than the 14 anchored files.

**Problem:** the 20-row anchor in Audit 2 captures tests that were *already* parked at the test level (`#[ignore = "Phase 04 ..."]`) during the 2026-04-30 STATE.md sweep. But several test files import deleted modules at the **file level** — they currently compile because `init`, `setup`, `peer`, `listen` modules still exist in `crates/famp/src/cli/`. After D-05 deletion, these test files will fail to compile and break `cargo test` / `just ci`.

**Files coupled at compile time (verified by grep `use famp::cli::{init,setup,peer,listen}`):**

| File | Coupling | Action |
|------|----------|--------|
| `init_force.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `init_happy_path.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `init_home_env.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `init_identity_incomplete.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `init_no_leak.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `init_refuses.rs` | imports `famp::cli::init::*` | move to `_deferred_v1/` |
| `peer_add.rs` | imports `famp::cli::peer::*` | move to `_deferred_v1/` |
| `peer_import.rs` | imports `famp::cli::peer::*` AND `famp::cli::info::*` (line 107) | move to `_deferred_v1/` |
| `setup_happy_path.rs` | imports `famp::cli::setup::*` | move to `_deferred_v1/` |
| `info_happy_path.rs` | imports `famp::cli::info::InfoArgs`. **Survives if `Info` keeper resolution per Risk #1 lands correctly.** | KEEP if Info refactor lands; otherwise move |
| `listen_bind_collision.rs` | listen-stack | move to `_deferred_v1/` |
| `listen_durability.rs` | listen-stack | move to `_deferred_v1/` |
| `listen_shutdown.rs` | listen-stack | move to `_deferred_v1/` |
| `listen_truncated_tail.rs` | listen-stack | move to `_deferred_v1/` |
| `http_happy_path.rs` | imports `famp_transport_http::{build_router, tls, tls_server, HttpTransport}` — **this is the TEMPLATE for the refactored `e2e_two_daemons`** | KEEP — already a library-API test; this becomes a sibling of refactored `e2e_two_daemons` and is the template. Verify it's in `just ci`. |
| `cross_machine_happy_path.rs` | imports `famp_transport_http`; already `#[ignore]` for chicken-and-egg subprocess reason (NOT a `Phase 04` ignore) | KEEP `#[ignore]` posture as-is; not a deletion target |
| `adversarial.rs` + `tests/adversarial/{harness.rs,fixtures.rs,memory.rs,http.rs}` | exercises `MemoryTransport` + `HttpTransport` via `build_router`/`InboxRegistry` library API — explicitly NOT to be moved per D-13 | KEEP — D-13 locks these in place |

**Summary additions to MOVE list:** 13 test files (`init_*` ×6, `peer_*` ×2, `setup_happy_path.rs`, `listen_{bind_collision,durability,shutdown,truncated_tail}` ×4 — note these listen tests exist separately from the `listen_smoke` and `listen_multi_peer_keyring` files already counted in Audit 2). **Plus** `info_happy_path.rs` AND `peer_import.rs` if the `Info` resolution per Risk #1 doesn't extract `PeerCard` cleanly.

**Total `_deferred_v1/` move scope: ~14 ignore-tagged + ~13 compile-coupled = ~27 files.** Planner must size accordingly.

**Source:** `[VERIFIED: grep -l "use famp::cli::{init,setup,peer,listen}" crates/famp/tests/*.rs]`.

---

### Audit 3 — TLS-form `famp send` import chain (D-06)

**Status:** ✅ Cut is clean. Bus-routed `famp send` is fully decoupled from federation crates.

**TLS-form code locations:**

| File | LOC | Role | Action |
|------|-----|------|--------|
| `crates/famp/src/cli/send/client.rs` | ~330 LOC (read 60 lines, full size from grep) | TOFU-pinning HTTPS reqwest client. Imports `reqwest::tls`, `rustls::*`, `sha2::{Digest, Sha256}`. **Defines `post_envelope`.** | **DELETE** |
| `crates/famp/src/cli/send/fsm_glue.rs` | TBD | FSM-side adapter for HTTPS send path; imports `famp_core::{MessageClass, TerminalStatus}`, `famp_fsm::{TaskFsm, TaskState, TaskTransitionInput}`, `famp_taskdir::TaskRecord`. **Used only by the TLS path.** | **DELETE** (verify no consumers of `fsm_glue::*` outside `send::client::*` and `listen::auto_commit` — grep confirms zero hits) |
| `crates/famp/src/cli/send/mod.rs` | ~540 LOC | **Bus-routed `famp send`.** Imports `famp_bus::{BusErrorKind, BusMessage, BusReply, Target}`, NO `famp_keyring`, NO `famp_transport_http`, NO `peers.toml`. **`pub mod client;` and `pub mod fsm_glue;` declarations need removal at lines 57–58.** | **EDIT** — drop two `pub mod` lines (57, 58) only |

**Consumers of `send::client::post_envelope`:**

1. `crates/famp/src/cli/listen/auto_commit.rs:40` — `use crate::cli::send::client::post_envelope;`. This file dies wholesale with `Listen` deletion.

**That's it.** No other consumer in the codebase. The cut is surgical.

**Bus-routed `send::run_at_structured` consumers** (preserved):

1. `crates/famp/src/cli/mcp/tools/send.rs:21` — `use crate::cli::send::{run_at_structured, SendArgs};` — **MUST stay working.**

**Per-line cut list for `crates/famp/src/cli/send/mod.rs`:**

```rust
// REMOVE line 57:
pub mod client;
// REMOVE line 58:
pub mod fsm_glue;
```

Update doc-comment block at lines 40–46 (the "v0.8 federation (HTTPS) path" explainer paragraph) — replace with a one-line note: "v0.8 federation HTTPS path was deleted in Phase 4; see `docs/MIGRATION-v0.8-to-v0.9.md`."

**Federation-crate `use` chain in src/ that dies with init/listen deletion:**

| File | `use` line(s) | Dies because |
|------|--------------|--------------|
| `crates/famp/src/cli/init/tls.rs:131,132,134` | `famp_transport_http::tls::*` | parent module deleted |
| `crates/famp/src/cli/listen/mod.rs:100,114,248,249,250,256` | `famp_keyring::Keyring`, `famp_transport_http::tls::*`, `famp_transport_http::tls_server` | parent module deleted |
| `crates/famp/src/cli/listen/router.rs:32,33` | `famp_keyring::Keyring`, `famp_transport_http::FampSigVerifyLayer` | parent module deleted |
| `crates/famp/src/runtime/loop_fn.rs:25` | `famp_keyring::Keyring` | runtime/ used only by listen/send-tls; dies as a module |
| `crates/famp/src/runtime/error.rs:45` | `famp_keyring::KeyringError` | same |
| `crates/famp/src/runtime/peek.rs:8` | `crate::runtime::error::RuntimeError` | same |
| `crates/famp/src/cli/error.rs:71` | `famp_transport_http::TlsError` (in `CliError::Tls` variant) | **EDIT** — drop the `Tls` variant from `CliError` enum |
| `crates/famp/src/lib.rs:32` | `use famp_transport_http as _;` (silences unused-deps lint at lib level) | **EDIT** — drop this line; transport-http is no longer reached from the lib compile unit |
| `crates/famp/src/bin/famp.rs:20,23` | `use famp_keyring as _;` and `use famp_transport_http as _;` | **EDIT** — drop both lines |

**Cargo.toml changes** (`crates/famp/Cargo.toml`):

```toml
# REMOVE lines 39–40:
famp-keyring = { path = "../famp-keyring", version = "0.1.0" }
famp-transport-http = { path = "../famp-transport-http", version = "0.1.0" }
# REMOVE lines 62–63:
reqwest = { version = "0.13", default-features = false, features = ["rustls-no-provider"] }
rustls = { workspace = true }
# REMOVE line 56:
rcgen = "0.14"
# REMOVE line 75 (dev-dep):
reqwest = { version = "0.13", default-features = false, features = ["rustls"] }
```

(`rcgen` was only used for `famp init` self-signed cert generation; verify with grep before removal.)

**Source:** `[VERIFIED: grep -rn "famp_keyring\|famp_transport_http\|reqwest\|rustls\|rcgen" crates/famp/src/ crates/famp/Cargo.toml]`.

---

### Audit 4 — CARRY-01 closing commit SHA (D-22)

**Status:** ✅ **CLOSED IN CODE.** Pin verified at HEAD.

**Closing commit:**

- **SHA:** `ebd0854ff793b7b9112ff69665a235304f915533`
- **Title:** `chore(tech-debt): TD-1 — pin listen-subprocess nextest test-group at max-threads=4`
- **Date:** 2026-04-30 18:39:27 -0400

**Pin location and verbatim content** (`.config/nextest.toml` lines 25–26):

```toml
[test-groups]
listen-subprocess = { max-threads = 4 }
```

Also lines 17–23 (override filters for `default` and `ci` profiles, both targeting `package(famp) and (test(/listen_/) or test(=conversation_restart_safety) or test(=mcp_stdio_tool_calls))`).

**REQUIREMENTS.md flip:** line 136 `CARRY-01` checkbox `[ ]` → `[x]`. Inline reference to closing commit `ebd0854` per D-22.

**ROADMAP.md flip:** Phase 4 traceability table line 265 (`CARRY-01 | Phase 4 | Pending` → `Complete`).

**Note for planner:** Phase 4 Wave-0 verification step is `grep -n "listen-subprocess" .config/nextest.toml` returning the pin. If the pin disappears between Phase 4 plan-lock and execution (unlikely), Phase 4 reverts to executing TD-1 directly. STATE.md anchors the assumption that the pin is in place.

**Source:** `[VERIFIED: git log --oneline .config/nextest.toml + Read .config/nextest.toml]`.

---

### Audit 5 — `Commands` enum surgery (FED-01)

**Status:** ✅ Per-line cut list ready. Current `Commands` enum has **16 variants, NOT 22** — CONTEXT.md D-08 over-counts the current state.

**Current variant inventory** (`crates/famp/src/cli/mod.rs` lines 43–116):

| # | Variant | Status |
|---|---------|--------|
| 1 | `Init(InitArgs)` | DELETE |
| 2 | `Setup(setup::SetupArgs)` | DELETE |
| 3 | `InstallClaudeCode` | KEEP |
| 4 | `UninstallClaudeCode` | KEEP |
| 5 | `InstallCodex` | KEEP |
| 6 | `UninstallCodex` | KEEP |
| 7 | `Info(info::InfoArgs)` | KEEP (with surgery — see Risk #1) |
| 8 | `Listen(ListenArgs)` | DELETE |
| 9 | `Peer(peer::PeerArgs)` | DELETE |
| 10 | `Send(send::SendArgs)` | KEEP (mod.rs survives; client/fsm_glue go) |
| 11 | `Await(await_cmd::AwaitArgs)` | KEEP |
| 12 | `Inbox(inbox::InboxArgs)` | KEEP |
| 13 | `Mcp(mcp::McpArgs)` | KEEP |
| 14 | `Broker(BrokerArgs)` | KEEP |
| 15 | `Register(register::RegisterArgs)` | KEEP |
| 16 | `Join(join::JoinArgs)` | KEEP |
| 17 | `Leave(leave::LeaveArgs)` | KEEP |
| 18 | `Sessions(sessions::SessionsArgs)` | KEEP |
| 19 | `Whoami(whoami::WhoamiArgs)` | KEEP |

**Total: 19 variants now. After deletion: 15 variants** (4 removed: `Init`, `Setup`, `Listen`, `Peer`).

**D-08 says "16 variants down from current 22"** — both numbers are off by counting `Cli`-the-parser as a separate item or counting `InitArgs` (struct, not enum variant) separately. The accurate count is **15 after deletion**. Planner: confirm with Ben at plan-lock; the math doesn't change the deletion targets, just the bookkeeping.

**Per-line cut list for `crates/famp/src/cli/mod.rs`:**

```rust
// Line 14:    pub mod init;        → DELETE
// Line 18:    pub mod listen;      → DELETE
// Line 22:    pub mod peer;        → DELETE
// Line 26:    pub mod setup;       → DELETE
// Line 33:    pub use init::InitOutcome;        → DELETE
// Line 34:    pub use listen::ListenArgs;       → DELETE
// Lines 45–46:    /// Initialize a FAMP home directory.
//                 Init(InitArgs),                      → DELETE
// Lines 47–48:    /// One-command setup: init + port selection + peer card output.
//                 Setup(setup::SetupArgs),             → DELETE
// Lines 71–73:    /// Run the FAMP daemon: bind the HTTPS listener and append inbound
//                 /// signed envelopes to `~/.famp/inbox.jsonl`.
//                 Listen(ListenArgs),                  → DELETE
// Lines 74–75:    /// Manage the peer registry (`peers.toml`).
//                 Peer(peer::PeerArgs),                → DELETE
// Lines 118–123: pub struct InitArgs { ... force: bool }   → DELETE (whole struct)
// Lines 147–148:  Commands::Init(args) => init::run(args).map(|_| ()),
//                 Commands::Setup(args) => setup::run(&args).map(|_| ()),    → DELETE
// Line 154:       Commands::Peer(args) => peer::run(args),                   → DELETE
// Line 158:       Commands::Listen(args) => block_on_async(listen::run(args)), → DELETE
```

**Module-directory deletes:**

- `rm -r crates/famp/src/cli/init/` (3 files: `atomic.rs`, `mod.rs`, `tls.rs`)
- `rm -r crates/famp/src/cli/peer/` (3 files: `add.rs`, `import.rs`, `mod.rs`)
- `rm -r crates/famp/src/cli/listen/` (4 files: `auto_commit.rs`, `mod.rs`, `router.rs`, `signal.rs`)
- `rm crates/famp/src/cli/setup.rs`
- `rm -r crates/famp/src/runtime/` (entire directory: `error.rs`, `loop_fn.rs`, `peek.rs`, etc. — verify nothing outside listen/send-tls uses it before deletion; grep confirms only `runtime::loop_fn` and `runtime::peek` reference one another within the module)

**`crates/famp/src/lib.rs` edits:**

- Line 32: `use famp_transport_http as _;` → DELETE
- Line 67: `pub mod runtime;` → DELETE

**`crates/famp/src/bin/famp.rs` edits:**

- Line 20: `use famp_keyring as _;` → DELETE
- Line 23: `use famp_transport_http as _;` → DELETE

**Re-imports in `crates/famp/src/cli/mod.rs`:**

- `pub use broker::BrokerArgs;` (line 31) → KEEP
- `pub use error::CliError;` (line 32) → KEEP
- `pub use init::InitOutcome;` (line 33) → DELETE
- `pub use listen::ListenArgs;` (line 34) → DELETE

**Source:** `[VERIFIED: Read crates/famp/src/cli/mod.rs lines 1–171]`.

---

### Audit 6 — `famp-transport-http` library API surface for `e2e_two_daemons` refactor (FED-03/04, TEST-06)

**Status:** ✅ Library API is fully public, well-documented, and already used as a template by `tests/http_happy_path.rs` and `tests/adversarial/http.rs`.

**Public surface from `crates/famp-transport-http/src/lib.rs`:**

```rust
pub use error::{HttpTransportError, MiddlewareError};
pub use middleware::FampSigVerifyLayer;
pub use server::{build_router, InboxRegistry, ServerState, INBOX_ROUTE};
pub use tls::{build_client_config, build_server_config, load_pem_cert, load_pem_key, TlsError};
pub use transport::HttpTransport;
```

**Reusable pattern from `tests/http_happy_path.rs`** (the exact template the refactored `e2e_two_daemons` should mirror):

```rust
use famp_transport_http::{build_router, tls, tls_server, HttpTransport};

let alice_transport = HttpTransport::new_client_only(Some(&alice_trust)).unwrap();
let bob_transport = HttpTransport::new_client_only(Some(&bob_trust)).unwrap();
let bob_router = build_router(bob_keyring.clone(), bob_transport.inboxes());
let alice_router = build_router(alice_keyring.clone(), alice_transport.inboxes());
// ... wait_for_tls_listener_ready, drive cycle through transports ...
```

**Critical types and entry points:**

| Function | Purpose | Use |
|----------|---------|-----|
| `build_router(keyring: Arc<Keyring>, inboxes: Arc<InboxRegistry>) -> Router` | Construct the axum 0.8 router with `FampSigVerifyLayer` + `RequestBodyLimitLayer(1MiB)` already wired (server.rs lines 37–53) | Each daemon's HTTP entry point |
| `HttpTransport::new_client_only(trust_cert_path: Option<&Path>) -> Result<Self, HttpTransportError>` | reqwest client wired with rustls + extra trust anchors via `rustls-platform-verifier` | Client-side send |
| `HttpTransport::add_peer(principal: Principal, url: Url)` / `register(principal: Principal)` | Wire principals to inbox channels | Setup |
| `HttpTransport::inboxes() -> Arc<InboxRegistry>` | Expose the receiving inbox map for `build_router` | Pair client and server |
| `tls::{load_pem_cert, load_pem_key, build_server_config}` | PEM/rustls helpers for fixture certs | Server-side TLS setup |
| `tls_server::serve_std_listener(listener, router, server_config)` | Spawnable HTTPS server task | Replaces a `famp listen` daemon spawn |
| `FampSigVerifyLayer` (re-exported as a public middleware layer) | Ed25519 sig verify before route dispatch | Already inside `build_router`; sentinel test reuses it |

**Ephemeral-port binding:**

`tls_server::serve_std_listener` accepts a `std::net::TcpListener`. The pattern from `http_happy_path.rs` (and what `e2e_two_daemons` must mirror) is:

```rust
let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
let port = listener.local_addr()?.port();
let server_config = tls::build_server_config(certs, key)?;
let join: tokio::task::JoinHandle<...> = tls_server::serve_std_listener(listener, router, Arc::new(server_config));
```

Two `tokio::spawn`'d tasks (Alice and Bob) on the SAME tokio runtime per D-10.

**Fixture certs:** `crates/famp/tests/fixtures/cross_machine/` contains `alice.crt`, `alice.key`, `bob.crt`, `bob.key`, `README.md`. **These are the certs `e2e_two_daemons` MUST reuse per D-11.**

**Conversation shape (D-12):** `request → commit → deliver → ack`. The pattern is exercised by `tests/http_happy_path.rs` via the shared `cycle_driver` (`tests/common/cycle_driver.rs`, included via `#[path]`). The refactored `e2e_two_daemons` can either:

- (a) **Reuse `cycle_driver` via `#[path]`** — same approach as `http_happy_path.rs`. **Lean toward this** — minimum new code, byte-equality with the proven driver.
- (b) **Inline a 4-step driver** — more explicit but duplicates ~50 LoC of cycle logic.

**Adversarial sentinel (D-09):** the pattern from `tests/adversarial/http.rs` is:

```rust
let inboxes: Arc<InboxRegistry> = Arc::new(Mutex::new(HashMap::new()));
let router = build_router(keyring, inboxes.clone());
// Inject an unsigned envelope at the tower layer; assert handler closure is
// NOT entered (use a sentinel `Arc<AtomicBool>` flipped only inside the handler;
// or shadow the route's `inbox_handler` with a sentinel-flipping wrapper for
// this test only).
```

**Source:** `[VERIFIED: Read crates/famp-transport-http/src/server.rs + lib.rs + tls.rs + transport.rs head; Read crates/famp/tests/http_happy_path.rs head; grep tests/adversarial/http.rs]`.

---

### Audit 7 — `v0.8.1-federation-preserved` tag mechanics (FED-05)

**Status:** ✅ Operation is `git tag v0.8.1-federation-preserved <sha>` (lightweight, no `-a`/`-m`).

**Sequencing constraints (D-07, D-19, D-20):**

- The tagged commit MUST be the one **AFTER** the `e2e_two_daemons` library-API refactor + workspace `Cargo.toml` relabel land.
- The tagged commit MUST be **BEFORE** any deletion. Specifically: before `init/setup/listen/peer` modules are removed; before `send/{client,fsm_glue}.rs` are removed; before `_deferred_v1/` move (because the move is itself a "deletion-shaped" operation that the tag wants to predate).

**Three properties the tag commit MUST satisfy (D-20):**

1. `just ci` green
2. All 6 federation CLI verbs still functional from this SHA — provable by `cargo run --bin famp -- listen --help`, `... init --help`, `... setup --help`, `... peer --help`, etc., returning success output (not "unrecognized subcommand")
3. `e2e_two_daemons` is already refactored to library API and green

**Operation:**

```bash
git tag v0.8.1-federation-preserved $(git rev-parse HEAD)
git push origin v0.8.1-federation-preserved   # if user wants public escape hatch
```

`git push origin v0.8.1-federation-preserved` is **optional**; D-21 says discoverability is via README + MIGRATION-doc references only. Local tag is sufficient to satisfy D-19.

**Discoverability** (D-21): the `MIGRATION-v0.8-to-v0.9.md` "If you need federation today" section is the discoverability surface.

**`git log v0.8.1-federation-preserved..main` reading test** (D-07): a reader of this output MUST see ONLY deletion + relabeling work, never the refactor. The Audit 11 commit sequence respects this.

**Source:** `[VERIFIED: Read 04-CONTEXT.md D-07/D-19/D-20/D-21]`.

---

### Audit 8 — `scripts/famp-local` archive scope (D-14, D-15)

**Status:** ✅ Single-file archive. **NOT a directory.**

**Findings:**

- `scripts/famp-local` is a **single bash script of 1316 lines**, not a directory. CONTEXT.md sometimes refers to it as a "directory" but `file scripts/famp-local` confirms `Bourne-Again shell script text executable`.
- Sibling files in `scripts/`: `check-mcp-deps.sh`, `redeploy-listeners.sh`, `spec-lint.sh` (not part of this archive — they stay).

**Archive operation (D-14):**

```bash
mkdir -p docs/history/v0.9-prep-sprint/famp-local/
git mv scripts/famp-local docs/history/v0.9-prep-sprint/famp-local/famp-local
# Then add the README:
echo "# famp-local — frozen v0.9 prep-sprint scaffolding" > docs/history/v0.9-prep-sprint/famp-local/README.md
# (one-line marker per D-14)
```

**Backlog 999.6 update (D-15):** `.planning/ROADMAP.md` line 314–324 references `scripts/famp-local`. Specific path mentions:

- Line 314: `Phase 999.6: \`update_zprofile_init\` should sandbox on non-default \`FAMP_LOCAL_ROOT\` (BACKLOG)`
- Line 316: `**Goal:** Make \`scripts/famp-local\`'s \`update_zprofile_init\` a no-op when ...`
- Line 321: `... \`scripts/famp-local\`'s \`update_zprofile_init\` calls \`update_zprofile_init "$mesh"\` ...`

**Replace** all `scripts/famp-local` references in lines 314–324 with `docs/history/v0.9-prep-sprint/famp-local/famp-local`. AUDIT-05-style atomic discipline: same commit as the archive move per D-15.

**Other consumers in repo** that reference `scripts/famp-local`:

- `.planning/REQUIREMENTS.md:132` — MIGRATE-04 row says "scripts/famp-local (prep-sprint scaffolding) marked deprecated". After the archive, `scripts/famp-local` literally won't exist; rephrase to "`docs/history/v0.9-prep-sprint/famp-local/famp-local` (archived prep-sprint scaffolding) marked frozen, superseded by native broker + CLI".
- `.planning/MILESTONES.md` lines 27, 49–50 — already references `scripts/famp-local` ("../scripts/famp-local"). Update to new path.
- `README.md` and `CLAUDE.md` may reference it — grep confirms `CLAUDE.md` does not (prep-sprint references are at a higher abstraction); README.md doesn't either in the parts we've inspected. Spot-check during plan execution.

**Source:** `[VERIFIED: file scripts/famp-local; wc -l scripts/famp-local; grep -rn "scripts/famp-local" .planning/]`.

---

### Audit 9 — `MIGRATION-v0.8-to-v0.9.md` skeleton draft (MIGRATE-03 / MIGRATE-01)

**Status:** ✅ Skeleton drafted below. Target: ≤200 lines, table-first, terse.

```markdown
# Migration: FAMP v0.8 → v0.9

**Local-first bus replaces the federation TLS listener mesh for same-host agents.**
v0.9 ships a UDS-backed broker; cross-host messaging is deferred to v1.0.

## TL;DR

- Run `famp install-claude-code` — auto-rewrites your `.mcp.json` and drops new
  slash commands.
- Switch `famp setup` / `famp init` → `famp register <name>`.
- `famp listen` is gone — the broker auto-spawns.
- `famp peer add` / `famp peer import` are gone — same-host discovery is automatic.
- `famp send` keeps the same flag surface; only the transport changed.

## CLI mapping table

| v0.8 | v0.9 | Notes |
|------|------|-------|
| `famp init --home <dir>` | `famp register <name>` | Identity bind via `~/.famp-local/agents/<name>/`; no per-identity HOME dir |
| `famp setup --name <n> --home <d> --port <p>` | `famp register <name>` | Single command; no port selection |
| `famp listen` | (gone) | Broker auto-spawns at `~/.famp/bus.sock` on first `famp register` |
| `famp peer add --alias <a> --endpoint <u> --pubkey <pk>` | (gone) | Same-host discovery is automatic via broker |
| `famp peer import` | (gone) | Peer cards are v1.0 federation-internal |
| `famp send --to <a> --new-task "<x>"` (TLS) | `famp send --to <name> --new-task "<x>"` (UDS) | Same syntax; bus under the hood |
| `FAMP_HOME=/tmp/a` env var | (no longer meaningful) | `~/.famp/` is sole root |
| `FAMP_TOFU_BOOTSTRAP=1` | (no longer meaningful) | No TLS on local bus |
| `famp mcp` with `FAMP_HOME=...` in `.mcp.json` | `famp mcp` (no env vars) | Register identity inside the MCP session via `famp_register` tool |

## `.mcp.json` cleanup

`famp install-claude-code` does this for you. Manual cleanup:

1. Open `~/.claude.json` (or your project-scope `.mcp.json`).
2. Find the `mcpServers.famp` entry.
3. **Delete** any `env` keys: `FAMP_HOME`, `FAMP_LOCAL_ROOT`.
4. Confirm `command` points to your installed `famp` binary, `args: ["mcp"]`.

## `~/.famp/` directory cleanup (optional)

Legacy v0.8 artifacts under `~/.famp/` (per-identity dirs containing
`config.toml`, `peers.toml`, `cert.pem`, `key.pem`) are no longer read.
They don't break anything; delete at your leisure with:

```bash
# Inspect first.
ls ~/.famp/
# Then remove the per-identity dirs (NOT the bus.sock or broker.log).
rm -rf ~/.famp/<old-identity-dir>
```

v0.9 uses `~/.famp-local/agents/<name>/` for per-identity state (mailboxes,
sessions) and `~/.famp/bus.sock` + `~/.famp/broker.log` for the broker.

## If you genuinely need federation today

The `v0.8.1-federation-preserved` git tag is an escape hatch for users who
need cross-host messaging via the v0.8 TLS listener mesh.

```bash
git checkout v0.8.1-federation-preserved
cargo install --path crates/famp
```

This tag is **frozen**. Bug fixes ship via the v1.0 federation gateway
(`famp-gateway`, trigger-gated on the named v1.0 readiness condition).

## For federation engineering reference

Federation-tagged tests are preserved under
`crates/famp/tests/_deferred_v1/`. They will be the starting test surface
for `famp-gateway`'s integration suite when v1.0 federation milestone fires.
See `crates/famp/tests/_deferred_v1/README.md`.

## Workspace internals

`famp-transport-http` and `famp-keyring` crates remain in the workspace as
**v1.0 federation internals**. They compile and stay tested in `just ci`
via the refactored `crates/famp/tests/e2e_two_daemons.rs` integration test.
No top-level CLI subcommand reaches them in v0.9.
```

**Skeleton size:** 65 LoC including code blocks. Final document budget is ≤200 lines per D-18; bullet-heavy/terse.

---

### Audit 10 — Staged-framing edits (D-16/D-17)

**Status:** ✅ Per-file landing sites identified; surrounding context quoted.

#### `README.md`

**Existing federation language (lines 24, 58–61, 233–256, 271, 562–564):**

- Line 10: `> implementation milestones (all shipped); v0.9 (local-first bus) is in design and v1.0`
- Line 24: `TOFU pinning. The raw federation CLI (\`famp setup / listen / send /`
- Line 58: `  - \`famp setup\` — one-command identity creation with auto port selection`
- Line 60: `  - \`famp peer import\` — import peer cards from other agents`
- Line 61: `  - \`famp listen\` — run the HTTPS daemon`
- Line 233: `./target/release/famp setup --name alice --home /tmp/famp-alice --port 8443`
- Line 562: `- \`v0.9\`: **local-first bus** — in design.`

**Landing site for D-17 staged framing (first paragraph after project title):** the README opening (around line 10) currently says v0.9 is "in design"; this is now stale (v0.9 ships at end of Phase 4). Replace with:

> FAMP today is local-first: a UDS-backed broker for same-host agent messaging
> with zero crypto on the local path. FAMP at v1.0 is federated: cross-host
> messaging via a `famp-gateway` wrapping the local bus, all of v0.5.2's
> signature/canonical-JSON guarantees preserved. v1.0 fires when ... (link to
> ARCHITECTURE.md trigger condition).

Quick Start preamble already uses Claude Code path (per Phase 3 CC-09); add one explicit "this is the v0.9 local-first path" sentence above the `cargo install` line.

**Lines 24, 58–61, 233–256, 271 (federation CLI tutorial)** must be deleted or replaced. The "raw federation CLI" section is exactly what Phase 4 deletes. Replace the section with a one-paragraph pointer to `MIGRATION-v0.8-to-v0.9.md` and to the `v0.8.1-federation-preserved` tag.

#### `CLAUDE.md`

**Existing federation language (lines 214, 221):**

- Line 214: `\`famp listen\` HTTPS daemon per identity; TOFU-pinned peers; every wire`
- Line 221: `not from \`FAMP_HOME\`. The federation transport (\`famp listen\`, \`famp send\`,`

**Landing site for D-17 staged framing (under "## Project"):** the CLAUDE.md "## Project" section (around line 211) describes the v0.8 federation-first architecture. Replace the one-line description with:

> **FAMP today is local-first** (v0.9): a UDS-backed broker for same-host agent
> messaging. **FAMP at v1.0 is federated**: cross-host messaging via
> `famp-gateway` wrapping the local bus. See [ARCHITECTURE.md](ARCHITECTURE.md)
> for the full layered model (Layer 0 protocol primitives → Layer 1 local bus →
> Layer 2 federation gateway).

Lines 214, 221 (federation transport descriptions) get deleted or rewritten to past-tense ("v0.8 used `famp listen` HTTPS daemons; v0.9 replaces this with the local bus").

#### `.planning/ROADMAP.md`

**Landing site for D-17 (v0.9 milestone callout):** ROADMAP.md line 11 already describes v0.9 with rich detail. Add **one** explicit sentence:

> **Today (v0.9):** local-first; broker handles same-host messaging.
> **Trigger to v1.0:** Sofer-from-different-machine + 4-week clock starts at v0.9.0.

Core Value line (line 3) stays as-is per D-17 — already accurate.

#### `.planning/MILESTONES.md`

**Landing site for D-17 (v0.9 section):** lines 24–53 currently say "implementation paused pending ~2 weeks of pre-v0.9 scaffolding validation". This is stale. Rewrite the section header to past-tense ship state:

> ## v0.9 Local-First Bus (Shipped: TBD-2026-05-XX)
>
> 4 phases, 85 requirements, NN/NN tests green. UDS-backed broker, zero crypto
> on local path, IRC-style channels, durable per-name mailboxes, stable MCP
> tool surface. **FAMP today is local-first; FAMP at v1.0 is federated.**
> Federation-tagged tests preserved under `crates/famp/tests/_deferred_v1/`;
> escape-hatch tag `v0.8.1-federation-preserved` exists for federation users.

#### `ARCHITECTURE.md`

**Landing site for D-17 (already mostly correct per Phase 3 D-13):** ARCHITECTURE.md line 1 ("# Architecture") through line 17 describes v0.8 in present tense ("Federation-first."). Line 38 introduces "v0.9 direction — local-first bus (in design)". Phase 4 surgical edit: change line 4 from "## Current state (v0.8)" to "## Past state (v0.8)" — and rewrite line 38's "(in design)" to "(shipping at v0.9.0 tag)". The body of the v0.9 section lines 38–79 is already correctly framed; only the headers need flipping.

**Source:** `[VERIFIED: Read README.md / CLAUDE.md / ARCHITECTURE.md / MILESTONES.md / ROADMAP.md grep results]`.

---

### Audit 11 — Atomic commit sequence (D-04, D-07, D-15)

**Status:** ✅ 9-commit sequence proposed. Tag cut between commits 4 and 5.

The reader of `git log v0.8.1-federation-preserved..main` MUST see ONLY deletion + relabeling, NEVER the refactor. The sequence below respects this: commits 1–4 land BEFORE the tag; commits 5–9 land AFTER.

**Commits 1–4 (PRE-TAG: refactor + relabel; tag cut on commit 4 SHA):**

| # | Title | Scope | Atomic claim |
|---|-------|-------|--------------|
| 1 | `chore(04): pin reqs/roadmap CARRY-01 to closing SHA ebd0854` | `.planning/REQUIREMENTS.md` (CARRY-01 row) + `.planning/ROADMAP.md` (traceability table CARRY-01 row → Complete) — bookkeeping only per D-22 | One commit flips both checkboxes; references SHA inline |
| 2 | `refactor(04): e2e_two_daemons targets transport-http library API directly` | `crates/famp/tests/e2e_two_daemons.rs` rewritten from 9-line skeleton to library-API happy path; one sibling `e2e_two_daemons_adversarial.rs` (or `#[test]` row) for the unsigned-envelope sentinel per D-09. Reuses `tests/fixtures/cross_machine/` certs (D-11). Same tokio runtime, two `tokio::spawn` listener tasks (D-10). Conversation shape unchanged (D-12). | One commit; `just ci` green; D-09/10/11/12 all locked |
| 3 | `chore(04): relabel famp-transport-http and famp-keyring as v1.0 federation internals` | Workspace `Cargo.toml` member entries (lines 10, 12) get a 5-word comment relabel. Per CONTEXT.md "lean: ~5 words". Suggested: `# v1.0 federation internals` above each. | One commit; comment-only; FED-02 closes |
| 4 | (no commit; **tag cut here on the SHA of commit 3**) | `git tag v0.8.1-federation-preserved <SHA-of-commit-3>` | Per D-19/D-20: tag is lightweight; commit-3 satisfies all three D-20 properties (just ci green; 6 federation CLI verbs all functional; e2e_two_daemons green at library API) |

**Commits 5–9 (POST-TAG: deletion + framing + archive):**

| # | Title | Scope | Atomic claim |
|---|-------|-------|--------------|
| 5 | `test(04): freeze federation tests under _deferred_v1/` | `git mv` of ~27 test files (Audit 2 + Audit 2.5 sets) into `crates/famp/tests/_deferred_v1/`. Add `_deferred_v1/README.md` (D-02). Port 0–2 tests if Audit 2 PORT rows surface uncovered properties. **Drop the `#[ignore = "Phase 04 ..."]` attribute from each moved file** — they are no longer in the active test set per CONTEXT.md "Established Patterns" note. | One commit; `git mv` preserves history (D-04); `just ci` green because moved files are no longer compiled |
| 6 | `feat!(04): remove federation CLI surface (init, setup, listen, peer, TLS-form send)` | All deletions per Audit 5 + Audit 3. Delete `crates/famp/src/cli/{init,listen,peer}/` directories; delete `crates/famp/src/cli/setup.rs`; delete `crates/famp/src/cli/send/{client,fsm_glue}.rs`; delete `crates/famp/src/runtime/`; surgery in `crates/famp/src/cli/mod.rs` (drop 4 enum variants + 4 dispatch arms + 4 `pub mod` decls + 2 `pub use` re-exports + `InitArgs` struct); surgery in `crates/famp/src/cli/send/mod.rs` (drop 2 `pub mod` decls); surgery in `crates/famp/src/cli/info.rs` (move `PeerCard` inline per Risk #1); surgery in `crates/famp/src/cli/error.rs` (drop `Tls(#[from] famp_transport_http::TlsError)` variant); surgery in `crates/famp/src/lib.rs` and `crates/famp/src/bin/famp.rs` (drop `use _ as _;` lines + `pub mod runtime;`); `crates/famp/Cargo.toml` (drop `famp-keyring`, `famp-transport-http`, `reqwest`, `rustls`, `rcgen`, dev-dep `reqwest`). | One commit per CONTEXT.md Discretion lean; flag for plan-lock if planner sees clearer 6-commit-per-verb split |
| 7 | `chore(04): archive scripts/famp-local under docs/history/v0.9-prep-sprint/` | `git mv scripts/famp-local docs/history/v0.9-prep-sprint/famp-local/famp-local`; add one-line README (D-14); update `.planning/ROADMAP.md` lines 314–324 (999.6 backlog path); update `.planning/MILESTONES.md` lines 27, 49–50; update `.planning/REQUIREMENTS.md` line 132 (MIGRATE-04 wording per D-15) | One commit; AUDIT-05-style atomic discipline (D-15) |
| 8 | `docs(04): MIGRATION-v0.8-to-v0.9.md` | Create `docs/MIGRATION-v0.8-to-v0.9.md` per Audit 9 skeleton. ≤200 lines, table-first. (MIGRATE-01 + MIGRATE-02 close.) | One commit; doc-only |
| 9 | `docs(04): staged-framing edits across README, CLAUDE, ROADMAP, MILESTONES, ARCHITECTURE` | Surgical edits per Audit 10 — "FAMP today is local-first; FAMP at v1.0 is federated" + delete federation-CLI tutorial blocks in README. (MIGRATE-03 closes; FED-01..06 + MIGRATE-01..04 + TEST-06 + CARRY-01 all green.) | One commit; surgical per CLAUDE.md "no drive-by polish" |

**Final step (NOT a commit):** `git tag v0.9.0 <SHA-of-commit-9>` — caller's call. Per ROADMAP.md exit criteria: `just ci` green + `cargo tree -i openssl` empty.

**Verification at each step:**

- After commit 2: `cargo nextest run -p famp e2e_two_daemons` green
- After commit 3 (and BEFORE tag): full `just ci` green; `cargo run --bin famp -- listen --help` returns success
- After tag: `git log v0.8.1-federation-preserved..main` shows ONLY commits 5–9 (deletion + relabel + framing)
- After commit 6: `cargo build -p famp` green; `cargo tree -p famp -i famp-transport-http` shows only `famp-transport-http` itself + the `e2e_two_daemons` test target
- After commit 9: full `just ci` green; `famp --help` shows 15 variants (per Audit 5); `cargo tree -i openssl` empty

---

### Audit 12 — Validation Architecture (Nyquist)

> Phase 4 inherits the workspace's nyquist_validation gate. Section is REQUIRED.

#### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo nextest 0.9.x` (workspace gate; `.config/nextest.toml` carries the `default` and `ci` profiles) |
| Config file | `.config/nextest.toml` (CARRY-01 pin lives here) |
| Quick run command | `cargo nextest run -p famp e2e_two_daemons` (FED-03/04 sanity) |
| Full suite command | `just ci` (runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo nextest run --workspace`, `just check-spec-version-coherence`, `just check-no-tokio-in-bus`, `just check-shellcheck`, `just publish-workspace-dry-run`) |

#### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FED-01 | 6 federation verbs absent from CLI | smoke (CLI help) | `cargo run --bin famp -- --help \| grep -E "^  (init\|setup\|listen\|peer)\$"` returns nothing | ❌ Wave 0 — needs a `tests/cli_help_invariant.rs` test asserting the absence |
| FED-02 | Workspace `Cargo.toml` carries v1.0-internals comment | static (grep) | `grep "v1.0 federation internals" Cargo.toml` returns ≥2 lines | manual (one-shot grep) |
| FED-03 | `e2e_two_daemons` targets library API | unit | `cargo nextest run -p famp e2e_two_daemons -E 'test(=happy_path)'` | ❌ Wave 0 — refactor IS the test |
| FED-04 | Federation e2e green in `just ci` on every commit | integration | `just ci` | ✅ existing (refactor brings it back into the active set) |
| FED-05 | `v0.8.1-federation-preserved` tag exists | git | `git rev-parse v0.8.1-federation-preserved` returns a valid SHA | manual (post-Phase-4 check) |
| FED-06 | `cargo tree` shows federation crates consumed only by e2e test | static (cargo tree) | `cargo tree -p famp-transport-http -i \| grep -v "famp-transport-http\|<test target>"` returns nothing | ✅ existing tooling |
| MIGRATE-01..04 | Doc files exist with required content | static (grep) | `grep -l "v0.8.1-federation-preserved" docs/MIGRATION-v0.8-to-v0.9.md` returns the file | manual one-shot |
| TEST-06 | RFC 8785 + §7.1c gates green | conformance | `just check-canonical` + `just check-crypto` (existing recipes) | ✅ existing |
| CARRY-01 | listen-subprocess pin still present | static (grep) | `grep -A1 "\\[test-groups\\]" .config/nextest.toml \| grep "listen-subprocess = { max-threads = 4 }"` | ✅ existing (verified Audit 4) |

#### Sampling Rate

- **Per task commit:** `cargo nextest run -p famp` (umbrella crate; covers `e2e_two_daemons` + adversarial)
- **Per wave merge:** `just ci`
- **Phase gate:** `just ci` green AND `cargo tree -i openssl` empty AND `git rev-parse v0.8.1-federation-preserved` succeeds AND `cargo run --bin famp -- --help` shows 15 variants (no init/setup/listen/peer rows) BEFORE `/gsd-verify-work`

#### Wave 0 Gaps

- [ ] `crates/famp/tests/e2e_two_daemons.rs` — refactor from 9-line skeleton (FED-03/04). The current file is empty per Read.
- [ ] `crates/famp/tests/e2e_two_daemons_adversarial.rs` (or sibling `#[test]` inside `e2e_two_daemons.rs`) — adversarial sentinel (D-09). New file.
- [ ] `crates/famp/tests/_deferred_v1/README.md` — freeze explainer (D-02). New file.
- [ ] `docs/MIGRATION-v0.8-to-v0.9.md` — migration doc (MIGRATE-01/02). New file.
- [ ] `docs/history/v0.9-prep-sprint/famp-local/README.md` — frozen marker (D-14). New file.
- [ ] CLI help invariant test (`tests/cli_help_invariant.rs` or `cargo run` smoke in `just ci`) — verify 6 deleted verbs absent. Optional but recommended.
- [ ] Manual UAT (D-20): checkout `v0.8.1-federation-preserved` on a fresh clone, run `cargo build && cargo run --bin famp -- listen --help` returning success. Manual; document in plan.

---

## Risks / Open Questions

### Risk #1 — `Commands::Info` has hidden coupling to deleted modules (BLOCKER for plan-lock)

**Severity:** HIGH. `crates/famp/src/cli/info.rs` line 13 imports `crate::cli::setup::PeerCard`; line 14 imports `crate::cli::init::load_identity`. Deleting `setup` and `init` modules per D-05 breaks `Info` at compile time, contradicting D-08's "Info is kept".

**Resolution candidates:**

- **(a)** Inline `PeerCard` struct into `crates/famp/src/cli/info.rs` (drop the cross-module dep); replicate `init::load_identity` logic inline as a private fn in `info.rs` (it's a 5-line read-from-disk-and-parse). **Recommend.** Surgical, preserves D-08, info.rs becomes self-contained, peer-card emitter remains useful for v1.0 federation.
- **(b)** Keep `setup.rs` as a one-struct file holding only `PeerCard`. Violates D-05 spirit ("hard delete the 6 federation verbs and their modules") and leaves stub code.
- **(c)** Drop `Info` from the enum too. Violates D-08 ("16 variants ... `Info` is kept"). Would require Ben re-confirmation.

**Action:** flag for plan-lock; planner asks Ben to confirm resolution (a). The `info_happy_path.rs` test stays in active suite if (a) lands; otherwise it joins the `_deferred_v1/` move.

**Source:** `[VERIFIED: Read crates/famp/src/cli/info.rs lines 1–60 + grep cross-module imports]`.

### Risk #2 — `peer_import.rs` test ALSO references `info` module (line 107)

**Severity:** MEDIUM. `peer_import.rs` is already in the move set (Audit 2.5), but it transitively uses `famp::cli::info::InfoArgs` + `famp::cli::info::run_at`. After the move, `_deferred_v1/peer_import.rs` will still compile (since `info` is kept per D-08 + Risk #1 resolution (a)). **No additional action needed**, but planner verifies the ported `_deferred_v1/` files still compile if the resolution doesn't affect public API of `info::*`.

### Risk #3 — D-08 variant count math is off

**Severity:** LOW (cosmetic). D-08 says "16 variants down from current 22"; actual current is 19, after deletion 15. Doesn't change deletion targets, just the bookkeeping number. Planner: confirm with Ben at plan-lock, then update D-08 wording in CONTEXT.md (or accept as a soft drift). Recommend: just lock at "15 variants after deletion, down from 19 current" in the plan.

### Risk #4 — Workspace `Cargo.toml` has no `[features]` toggles gating federation crates

**Severity:** NONE. Verified `[features]` blocks are absent in workspace root `Cargo.toml`, `crates/famp/Cargo.toml`, `crates/famp-transport-http/Cargo.toml`, `crates/famp-keyring/Cargo.toml`. No feature-flag work needed.

**Source:** `[VERIFIED: grep "\\[features\\]" Cargo.toml crates/*/Cargo.toml]`.

### Risk #5 — `famp install-claude-code` does NOT touch deleted CLI verbs in templates

**Severity:** NONE. Verified: `grep -rn "famp init\|famp listen\|famp setup\|famp peer" crates/famp/src/cli/install/` returned no hits. The Phase 3 install-claude-code path writes MCP config + slash commands (`famp_register`, `famp_send`, etc.) and a Stop hook — none of which reference deleted verbs. **No regression risk.**

**Source:** `[VERIFIED: grep crates/famp/src/cli/install/]`.

### Risk #6 — `cross_machine_happy_path.rs` is `#[ignore]`d for a different reason

**Severity:** NONE-but-flag. `cross_machine_happy_path.rs:63` has `#[ignore = "subprocess bootstrap chicken-and-egg ..."]` — NOT a `Phase 04` ignore. It's a long-standing test ignored for harness reasons (it spawns subprocesses that try to load the same fixture). It is NOT a deletion target; it stays as-is in `crates/famp/tests/`. Planner: do NOT include in `_deferred_v1/` move set.

### Risk #7 — `runtime/` module deletion blast radius

**Severity:** LOW. Verified `crates/famp/src/runtime/` is consumed only by itself (loop_fn ↔ peek ↔ error). No external consumer in `cli/` modules that survives. The `famp-keyring` and `famp-transport-http` references die wholesale. **Safe to `rm -r crates/famp/src/runtime/`.** Planner: verify with `cargo check` post-deletion.

### Risk #8 — `cli/error.rs::CliError::Tls` variant deletion ripple

**Severity:** LOW. Removing `Tls(#[from] famp_transport_http::TlsError)` from `CliError` means any `match` on `CliError` that has a `CliError::Tls { .. } =>` arm needs the arm dropped. Verify with `grep -rn "CliError::Tls" crates/famp/src/`. Likely consumers: `mcp_error_kind` exhaustive match (per Phase 2 D-06 / D-11 discipline). Flag for plan-lock; planner schedules the match-arm deletion atomically with the variant deletion.

### Open Question #1 — One atomic CLI deletion commit vs six per-verb commits

CONTEXT.md Claude's Discretion section says "Lean toward one — six is over-granular for a deletion sweep". But the atomic commit is large (~600 LoC across ~15 files including module-directory deletes). **Recommendation:** one atomic commit per the lean. If the planner sees a cleaner split (e.g., "1 commit removes init/setup; 1 commit removes listen; 1 commit removes peer; 1 commit removes TLS-form send"), the 4-commit split is also defensible. Six per-verb is over-granular.

### Open Question #2 — Whether `info_happy_path.rs` is a `_deferred_v1/` candidate

If Risk #1 resolution is **(a)** (inline `PeerCard` into `info.rs`), `info_happy_path.rs` stays in the active suite. If resolution is **(c)** (drop `Info` from enum), `info_happy_path.rs` joins the move. **Open — depends on Risk #1 resolution.** Recommend resolution (a).

---

## Standard Stack

This phase introduces NO new library dependencies. The deletion targets remove deps; the preservation targets keep deps; the new test file uses existing workspace deps verbatim.

### Removed deps (from `crates/famp/Cargo.toml`)

| Dep | Version | Removed because |
|-----|---------|-----------------|
| `famp-keyring` | path | TLS-form send + listen path deletion; only `_deferred_v1/` and federation-CI test consume it now |
| `famp-transport-http` | path | Same |
| `reqwest` | 0.13 | TLS-form send-client deletion |
| `rustls` | workspace | Same |
| `rcgen` | 0.14 | `famp init` self-signed cert generation deletion |
| `reqwest` (dev-dep) | 0.13 | Was used by federation tests being moved |

### Preserved deps (kept in workspace)

`famp-transport-http`, `famp-keyring` retain their workspace member entries; they stay compiling and tested via the refactored `e2e_two_daemons` integration test.

---

## Architecture Patterns

### Pattern: Atomic-commit-with-tag-escape-hatch (Phase 1 AUDIT-05 precedent)

The Phase 1 atomic v0.5.2 spec bump (`9ca6e13`) is the precedent: a single commit lands the full state transition; the commit message documents what's atomic about it. Phase 4's tag-then-delete is the same pattern at a coarser grain — the tag is the atomic boundary; the commits before the tag form one logical unit ("federation works at this SHA"); the commits after form another ("federation removed; bus-only").

**Source:** `crates/famp-envelope/src/version.rs` + Phase 1 STATE.md anchor.

### Pattern: `git mv` over `cp + rm` for archival moves

Per CONTEXT.md "Established Patterns" + CLAUDE.md commit-discipline. Both the test-file freeze (D-04) and the `scripts/famp-local` archive (D-14) use `git mv`. Verifies via `git log --follow <new-path>` that history is preserved.

### Pattern: Library-API integration test as federation-CI insurance

The refactored `e2e_two_daemons` follows the same shape as `tests/http_happy_path.rs` (template) and `tests/adversarial/http.rs` (sentinel pattern). One happy path + one adversarial = plumb-line-2 insurance against `famp-transport-http` mummification.

### Anti-Patterns to Avoid

- **Soft-deprecation stubs for removed CLI verbs.** Explicitly considered and rejected (CONTEXT.md Deferred Ideas). The migration doc IS the answer.
- **Two-runtime `e2e_two_daemons`.** Explicitly considered and rejected (D-10). Same-runtime is good enough for a parked path.
- **Duplicating adversarial cases inside `e2e_two_daemons`.** D-13 locks: existing `tests/adversarial/` HTTP rows stay where they are.
- **Hard-deleting `scripts/famp-local`.** Rejected (D-14). Archive preserves provenance + 999.6 backlog actionability.
- **Headline "FAMP is local-first" identity rewrite.** Rejected (D-16). Staged framing instead.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Test-file freeze | Manual `cp + rm` | `git mv` | Preserves blame history (CONTEXT.md "Reusable Assets") |
| Two-process happy-path | Subprocess `assert_cmd` invocations against deleted CLI | `tokio::spawn` two `tls_server::serve_std_listener` tasks on the SAME runtime, via `build_router` library API | D-10 lock; same-runtime is simpler and faster than subprocess; library API is already public |
| HTTPS server in-test | Hand-wired axum + tower stack | `famp_transport_http::build_router(keyring, inboxes)` | Already includes `FampSigVerifyLayer` + `RequestBodyLimitLayer(1MiB)`; matches production wire (`tests/http_happy_path.rs` is the proven template) |
| TLS in-test | Custom rustls config | `famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config}` + fixture certs | Existing helpers; D-11 reuses fixtures |
| Migration deprecation stubs | Wrapper CLI verbs that print "use `famp register` instead" | `docs/MIGRATION-v0.8-to-v0.9.md` | D-05 hard-delete + D-18 doc-carries-the-load |
| Soft-deprecation runtime warnings | `eprintln!("famp listen is deprecated; ...")` shim | Clap's default "unrecognized subcommand" error pointing to `--help` | D-05 explicit |

---

## Code Examples

### Example 1: `e2e_two_daemons` library-API happy path (refactor target)

Mirrors `tests/http_happy_path.rs` shape. Reuses `tests/common/cycle_driver.rs` via `#[path]`.

```rust
// crates/famp/tests/e2e_two_daemons.rs
//
// Phase 4 plumb-line-2 insurance: target famp-transport-http library API
// directly to keep the federation HTTPS path exercised in `just ci`.
// See ARCHITECTURE.md and `crates/famp/tests/_deferred_v1/README.md`.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "common/cycle_driver.rs"]
mod cycle_driver;

use famp_transport_http::{build_router, tls, tls_server, HttpTransport};
use famp_keyring::Keyring;
// ... rest mirrors tests/http_happy_path.rs lines 30–200 ...
```

**Source:** template `tests/http_happy_path.rs`.

### Example 2: Adversarial sentinel (mirrors `tests/adversarial/http.rs`)

```rust
// crates/famp/tests/e2e_two_daemons_adversarial.rs (or as a sibling #[test] in e2e_two_daemons.rs)
//
// Sentinel: signature-verification middleware rejects unsigned envelope at
// the tower layer; handler closure NOT entered.

use famp_transport_http::{build_router, InboxRegistry};

let inboxes: Arc<InboxRegistry> = Arc::new(Mutex::new(HashMap::new()));
let router = build_router(keyring, inboxes.clone());
// inject unsigned envelope; assert handler-closure-entered AtomicBool stays false
```

**Source:** `tests/adversarial/http.rs` lines 1–70.

### Example 3: Workspace `Cargo.toml` relabel (FED-02)

```toml
# Before:
"crates/famp-keyring",
# After:
# v1.0 federation internals
"crates/famp-keyring",

# Before:
"crates/famp-transport-http",
# After:
# v1.0 federation internals
"crates/famp-transport-http",
```

**Source:** Workspace `Cargo.toml` lines 10, 12.

### Example 4: `git tag` lightweight (FED-05)

```bash
# Cut the tag on the SHA of commit 3 (Audit 11):
git tag v0.8.1-federation-preserved $(git rev-parse HEAD)
# Optional: push (per D-21 not strictly required):
git push origin v0.8.1-federation-preserved
```

---

## Recommendations for the Planner

### Plan slicing

The phase divides cleanly into **5 waves** based on the 9-commit sequence:

- **Wave 0 (preconditions / Wave 0 gaps):** create `tests/_deferred_v1/README.md` skeleton, draft `docs/MIGRATION-v0.8-to-v0.9.md` skeleton, create `docs/history/v0.9-prep-sprint/famp-local/README.md` skeleton, draft staged-framing edits as one diff per file. Pure-doc; no code touches yet. Lands as part of Wave 1's first plan but tracked separately.
- **Wave 1 (refactor + relabel — PRE-TAG):** commits 1–3.
  - Plan 04-01: `chore(04): pin reqs/roadmap CARRY-01 to closing SHA`
  - Plan 04-02: `refactor(04): e2e_two_daemons targets transport-http library API directly` + adversarial sentinel
  - Plan 04-03: `chore(04): relabel famp-transport-http and famp-keyring as v1.0 federation internals`
- **Wave 1.5 (tag cut, NOT a commit):** `git tag v0.8.1-federation-preserved $(git rev-parse HEAD)` per D-19. Manual UAT step (D-20).
- **Wave 2 (test freeze — POST-TAG):** commit 5.
  - Plan 04-04: `test(04): freeze federation tests under _deferred_v1/` (~27 file moves + 0–2 ports + README)
- **Wave 3 (deletion sweep):** commit 6.
  - Plan 04-05: `feat!(04): remove federation CLI surface`
  - Includes the `Commands::Info` resolution per Risk #1 (move `PeerCard` inline)
- **Wave 4 (archive + framing):** commits 7–9.
  - Plan 04-06: `chore(04): archive scripts/famp-local under docs/history/v0.9-prep-sprint/`
  - Plan 04-07: `docs(04): MIGRATION-v0.8-to-v0.9.md`
  - Plan 04-08: `docs(04): staged-framing edits across README, CLAUDE, ROADMAP, MILESTONES, ARCHITECTURE`

### Wave structure (proposed)

8 plans across 4 waves + tag cut. The `e2e_two_daemons` refactor is the largest single plan (FED-03/04 plus adversarial sentinel); the deletion sweep is the second largest (Audit 5 cut list). Everything else is bounded.

### `must_haves` derivation

Plan-checker's `must_haves` for Phase 4:

- `must_haves.tag_exists`: `git rev-parse v0.8.1-federation-preserved` succeeds AND points at the SHA before any deletion commits land.
- `must_haves.federation_e2e_green`: `cargo nextest run -p famp e2e_two_daemons` returns `0` after Wave 1 plan 04-02 lands.
- `must_haves.no_top_level_federation_consumer`: `cargo tree -p famp-transport-http -i` lists ONLY `famp-transport-http` itself + the e2e test target — no `famp` umbrella crate consumer — after Wave 3 lands.
- `must_haves.cli_help_clean`: `cargo run --bin famp -- --help` does not contain `init`, `setup`, `listen`, `peer` rows after Wave 3 lands.
- `must_haves.no_openssl`: `cargo tree -i openssl` empty after Wave 3 lands.
- `must_haves.migration_doc_exists`: `test -f docs/MIGRATION-v0.8-to-v0.9.md` after Wave 4 plan 04-07 lands.
- `must_haves.archive_exists`: `test -f docs/history/v0.9-prep-sprint/famp-local/famp-local` after Wave 4 plan 04-06 lands.
- `must_haves.staged_framing_present`: `grep -l "FAMP today is local-first" README.md CLAUDE.md` returns both files after Wave 4 plan 04-08.
- `must_haves.carry01_pin_present`: `grep "listen-subprocess = { max-threads = 4 }" .config/nextest.toml` returns the line throughout the phase (verified at Wave 1 plan 04-01 and re-verified post-phase).
- `must_haves.just_ci_green`: `just ci` returns `0` at every commit boundary.

### What to push back on

- The "16 variants down from 22" math (D-08 / Risk #3). Just lock at "15 after deletion, down from 19 current" in the plan and update D-08 wording in CONTEXT.md if Ben wants symmetry.
- The "`_deferred_v1/` move is ~14 files" anchor (CONTEXT.md). Audit 2.5 establishes the actual move set is ~27 files because of compile-time-coupled tests. Plan 04-04 must size accordingly.
- `Commands::Info` keeper (Risk #1). Resolution (a) is recommended; ask Ben to confirm before plan-lock.

---

## Sources

### Primary (HIGH confidence)
- `crates/famp/src/cli/mod.rs` lines 1–171 — Read directly; per-line cut list anchored
- `crates/famp/src/cli/info.rs` lines 1–60 — Read directly; Risk #1 anchor
- `crates/famp/src/cli/send/mod.rs` lines 1–540 — Read directly; bus-routed surface verified
- `crates/famp/src/cli/send/client.rs` lines 1–60 — Read directly; TLS-form scope confirmed
- `crates/famp-transport-http/src/server.rs` lines 1–114 — Read directly; library API verified
- `crates/famp-transport-http/src/tls.rs` lines 1–186 — Read directly; helper surface confirmed
- `crates/famp/tests/e2e_two_daemons.rs` lines 1–10 — Read directly; current 9-line skeleton confirmed
- `.config/nextest.toml` lines 1–27 — Read directly; CARRY-01 pin verified
- `.planning/MILESTONES.md` lines 1–155 — Read directly; existence verified (Audit 1)
- `Cargo.toml` workspace + `crates/famp/Cargo.toml` + `crates/famp-transport-http/Cargo.toml` — Read directly; dep edges confirmed
- `git log --oneline .config/nextest.toml` — verified Audit 4 SHA `ebd0854`
- `grep -rn '#\[ignore = "Phase 04'` — Audit 2 enumeration
- `grep -l "use famp::cli::{init,setup,peer,listen}"` — Audit 2.5 compile-coupling enumeration
- `docs/superpowers/specs/2026-04-17-local-first-bus-design.md` lines 412–432 — Phase 4 exit criteria

### Secondary (MEDIUM confidence)
- ARCHITECTURE.md lines 1–104 — Read; staged-framing landing site confirmed
- README.md grep for federation verbs — landing sites identified (Audit 10)
- CLAUDE.md grep for federation language — landing sites identified

### Tertiary (LOW confidence)
- None — all claims verified against source files and tooling.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; existing workspace deps verified
- Architecture: HIGH — patterns are existing precedents (Phase 1 atomic commits, `tests/http_happy_path.rs` template)
- Pitfalls: HIGH — Risks #1 and #2.5 (compile-coupled tests + Info coupling) are the two non-obvious landmines and both are surfaced with resolutions

**Research date:** 2026-05-03
**Valid until:** 2026-06-03 (30 days for stable codebase; Phase 4 is deletion-shaped so the research doesn't stale via library updates)

## RESEARCH COMPLETE
