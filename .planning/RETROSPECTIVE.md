# FAMP — Living Retrospective

Cross-milestone lessons and patterns. Appended after each shipped milestone; cross-milestone trends live at the bottom.

---

## Milestone: v0.6 — Foundation Crates

**Shipped:** 2026-04-13
**Phases:** 3 | **Plans:** 9 | **Tasks:** 16 | **Commits:** 34
**Timeline:** 2026-04-12 → 2026-04-13 (substrate shipped in one execution day after half a day of planning/research)

### What Was Built

- `famp-canonical` — RFC 8785 JCS wrapper over `serde_jcs 0.2.0` with a 12-test conformance gate (Appendix B/C/E, 100K cyberphone float corpus, UTF-16 supplementary-plane sort, NaN/Infinity rejection, duplicate-key rejection) wired into CI as a blocking prerequisite; nightly 100M-line full-corpus workflow on cron; 357-LoC from-scratch fallback plan committed on disk.
- `famp-crypto` — Ed25519 `verify_strict`-only public surface, `TrustedVerifyingKey` ingress newtype that rejects weak / small-subgroup keys, SPEC-03 domain-separation prefix prepended internally via `canonicalize_for_signature`, RFC 8032 KAT gate, PITFALLS §7.1c worked-example byte-exact against external Python reference, `sha256_artifact_id` with NIST FIPS 180-2 KATs, constant-time `subtle::ConstantTimeEq` for signature equality.
- `famp-core` — `Principal`/`Instance` with wire-string round-trip, distinct UUIDv7 ID newtypes (`MessageId`/`ConversationId`/`TaskId`/`CommitmentId`), `ArtifactId` enforcing `sha256:<hex>` at parse time, 15-variant flat `ProtocolErrorKind` covering all §15.1 categories with `ProtocolError` wrapper, `invariants::INV_1..INV_11` public doc anchors, 5-variant `AuthorityScope` ladder with hand-written 5×5 `satisfies()` truth table (no `Ord` derive), exhaustive consumer stub under `#![deny(unreachable_patterns)]` turning any future enum extension into a compile error across downstream crates.

### What Worked

- **External vectors as hard CI gates, from day one.** RFC 8785 Appendix B and the cyberphone float corpus were committed *before* `famp-canonical` had a public API. RFC 8032 Ed25519 vectors were wired before `sign`/`verify` had implementations. PITFALLS §7.1c bytes came verbatim from the spec. Every interop-critical claim is a blocking test, not a TODO.
- **SEED-001 fallback plan written before the decision.** The 357-LoC RFC 8785 from-scratch fallback plan landed on disk in Plan 01-01, *before* Plan 01-03 ran the conformance gate and decided to keep `serde_jcs`. Having the escape hatch pre-committed made the "keep" decision low-risk rather than path-locked.
- **Narrow, phase-appropriate error enums.** Each crate got its own ~5-variant error enum with `thiserror`. Compiler-checked exhaustive `match` caught missing arms during refactors that a single god-enum would have hidden behind `_ => …` fallthrough.
- **Free-function-primary + trait-sugar pattern.** `sign_value` / `verify_canonical_bytes` / `canonicalize_for_signature` are the sanctioned callable surface; `Signer`/`Verifier` traits are thin sugar. Downstream crates can mock the traits for tests without losing the guarantee that the production path used the prefix.
- **Same-day tech-debt closure.** The audit surfaced two real CI-parity gaps (rustfmt drift + integration-test clippy hygiene) and both got closed inside the audit pass, not punted to v0.7. Result: `just ci` was actually green when the milestone archived, not "green once we clean up."
- **Verification gap closure via a dedicated plan, not an extension of the previous plan.** CRYPTO-07 (SHA-256 content-addressing) was flagged by `02-VERIFICATION.md` and closed by Plan 02-04, additive-only, leaving every other crypto source file untouched. Plan 02-04 → commit → verification → audit was a clean cycle.
- **Phase numbering reset to 1 for v0.6** — keeping v0.5.1's docs-only phases separately numbered would have blurred the "first code milestone" boundary in every future trace.

### What Was Inefficient

- **`gsd-tools summary-extract` pulled literal "One-liner:" placeholder text** from several Phase 2 SUMMARY.md files because the front-matter pattern didn't include a proper one-liner line, polluting the auto-generated MILESTONE entry. Had to hand-rewrite the v0.6 accomplishments section during archival. Fixable by: (a) a SUMMARY lint gate that rejects placeholder strings, or (b) a richer template that forces a one-sentence `deliverable:` field.
- **Planning artifacts lived in two places pre-archive.** `.planning/ROADMAP.md` and `.planning/v0.6-MILESTONE-AUDIT.md` both existed at repo root alongside `.planning/milestones/` until archival. Two sources of "current" added friction during the audit pass. Consider moving audit reports into `milestones/` as soon as they're written.
- **`famp-canonical` integration-test clippy hygiene** (`unused_crate_dependencies`, `unwrap_used`, `expect_used`) was a known carried TODO from Plan 01-02 and should have been closed in 01-03, not in the audit pass. Lesson: close phase-local hygiene before running the verification agent, not after.

### Patterns Established

- **External vectors as hard blocking CI gates** — applies to every interop-critical primitive: canonical JSON, Ed25519, SHA-256, (future) envelope schemas. No exceptions, no `continue-on-error`.
- **Fallback plan committed before the decision** — SEED-001 pattern. Any "use upstream crate X or fork?" decision gets a written fork plan on disk first, so the decision is technical not path-dependent.
- **Free-function-primary + trait-sugar** — public callable surface is free functions; traits are thin wrappers. Mocking for tests doesn't break the production invariant.
- **Narrow, phase-appropriate error enums** — not one god enum. ~5 variants per crate, each exhaustively matchable.
- **Exhaustive consumer stub under `#![deny(unreachable_patterns)]`** — for every wire-facing enum (`ProtocolErrorKind`, `AuthorityScope`, future message-type enum), commit a stub that matches every variant explicitly, so adding a variant is a compile error everywhere.
- **Verification gap closure as a dedicated additive plan** — don't extend the previous plan; write Plan N+1 that touches only the gap, and re-verify. Keeps provenance clean.
- **Hand-written truth tables for non-total-order relations** — `AuthorityScope::satisfies()` is a 5×5 hand-written table, not a derived `Ord`. When the relation isn't a total order, don't pretend it is.

### Key Lessons

1. **Byte-exact guarantees cost less than they look like when you front-load the vectors.** Ed25519 + RFC 8785 + SHA-256 all landed byte-exact on first implementation attempt because the vectors were already green-or-red on day one. The hard part was the decision (SEED-001), not the code.
2. **The milestone audit should be non-negotiable before `complete-milestone`, not after.** The v0.6 audit caught two real gaps (format drift, clippy hygiene) that `just ci` would have surfaced on the next CI push. Running the audit first turned a potentially embarrassing post-archive fix into a same-day tech-debt closure.
3. **Phase-local TODOs carried across phase boundaries cost more than they save.** The Plan 01-02 clippy hygiene TODO sat open through two full phases. Closing it in 01-03 would have cost 10 minutes; closing it in the audit cost context re-loading.
4. **SUMMARY.md front-matter is an API, not a log.** It's consumed by `gsd-tools summary-extract` during milestone archival. Missing or placeholder one-liners leak into MILESTONES.md. Treat the SUMMARY as a machine-readable artifact, not freeform notes.

### Cost Observations

- **Model mix:** dominated by Sonnet for per-plan execution; Opus used sparingly for architectural decisions (profile split, SEED-001 decision framing, audit verdict)
- **Session count:** substrate shipped across ~1 planning session + ~1 execution day; phase work ran in yolo mode with auto-advance
- **Notable efficiency:** single-day execution of 9 plans was possible because every plan had RESEARCH.md + VALIDATION.md + external vectors committed before execution started. The up-front cost was ~half a day of planning/research; the payoff was zero in-flight rework.

---

## Milestone: v0.8 — Usable from Claude Code

**Shipped:** 2026-04-26
**Phases:** 5 (4 archived 2026-04-15 + 1 v0.8.x bridge 2026-04-26) | **Plans:** 18 | **Tests:** 419/419 green
**Timeline:** 2026-04-14 → 2026-04-26 (12 days; original 4-phase ship in 2 days, then a 10-day gap with quick-task hardening, then the bridge phase in 1 day)

### What Was Built

- **Phase 1 — Identity & CLI Foundation:** `famp init` produces persistent Ed25519 keys (0600), self-signed TLS cert via `rcgen`, `config.toml`, empty `peers.toml`. `FAMP_HOME` env override drives every subcommand. Six integration test binaries lock the contract; compile-fail rustdoc enforces no key-bytes leak in public API.
- **Phase 2 — Daemon & Inbox:** `famp listen` on axum + rustls, reusing v0.7's `FampSigVerifyLayer` byte-for-byte. Inbox writes are fsync-sealed before HTTP 200 (durability receipt, stricter than upstream's 202). Tail-tolerant reader survives crash mid-write. SIGINT/SIGTERM clean shutdown via `tokio::signal::unix`. `PortInUse` typed bind-collision error. New crate: `famp-inbox` (raw `&[u8]` to preserve byte-exactness, P3).
- **Phase 3 — Conversation CLI:** `famp send/await/inbox/peer add` over the v0.7 FSM unmodified — CONV-05 checkpoint proves v0.7 was expressive enough. Task records under `~/.famp/tasks/<id>.toml` survive daemon restarts. Advisory `inbox.lock` prevents double-consumption. INBOX-02/03/05 re-mapped from Phase 2 to Phase 3 (mid-flight requirements fix).
- **Phase 4 — MCP Server & Same-Laptop E2E:** `famp mcp` stdio JSON-RPC server (Content-Length framing, hand-rolled, no `rmcp` dep). Four tools wrap the CLI; exhaustive `CliError::mcp_error_kind()` (28 variants, no wildcard) so every misuse is a typed `famp_error_kind`. Multi-entry keyring + auto-commit handler enable two-daemon flows. `e2e_two_daemons.rs` automated test gates v0.8.
- **v0.8.x bridge — Session-bound MCP identity:** `famp_register` / `famp_whoami` tools added. `famp mcp` stops reading `FAMP_HOME` at startup; reads `FAMP_LOCAL_ROOT` only. Pre-registration tool calls return typed `not_registered`. **B-strict variant adopted** — no `legacy_famp_home` grace period. `scripts/famp-local` auto-rewrites legacy `.mcp.json` on touch via byte-exact `desired_template` + `cmp -s` idempotency gate. Two-MCP-server E2E test (`mcp_session_bound_e2e.rs`) drives two real subprocesses through full `request → commit → deliver × N → terminal`. Bonus fix surfaced by E2E: `await_cmd/mod.rs` FSM advance bug (terminal `deliver` receipts now correctly walk COMMITTED → COMPLETED).

### What Worked

- **Bridge phase as v0.9 pull-forward, not a v0.8 patch.** When dogfooding revealed that two-window onboarding required per-window startup configuration, the response wasn't a v0.8 hotfix or "wait for v0.9" — it was a 5-plan bridge phase that pulled the v0.9 MCP contract forward onto the v0.8 substrate. The user-visible promise ("two windows, one repo, two identities") was protected without churning the substrate.
- **B-strict over a permissive grace period.** The bridge could have shipped a `legacy_famp_home: { home }` `BindingSource` variant that mid-flight migrations would tolerate. Instead it shipped strict — `BindingSource::Explicit` only, pre-registration calls return `not_registered`, migration is auto-rewrite. Result: the production code has zero "transitional" branches to remove later, and the test surface tests one path, not two.
- **Same-day production fix surfaced by the new E2E test.** The two-window E2E test physically failed when terminal `deliver` receipts didn't advance COMMITTED → COMPLETED; tracing the failure revealed a real bug in `await_cmd/mod.rs` that was inert until the test arrived. Fixed inline as a Rule 1 deviation; documented in `01-VERIFICATION.md`.
- **Redirect-style Phase 4 verification.** Phase 4 shipped without a VERIFICATION.md. Rather than reconstruct one retroactively, the milestone-audit cleanup wrote `04-VERIFICATION.md` as a redirect note pointing at the bridge phase's `01-VERIFICATION.md` (which physically witnesses the milestone-goal flow). Cheap to write; future audits see Phase 4 coverage in one place.
- **Mid-flight requirements re-mapping (Phase 2 → 3).** `02-VERIFICATION.md` flagged INBOX-02/03/05 as labeled to Phase 2 but landing in Phase 3. The fix was a documentation-only re-map (no code churn) noted in REQUIREMENTS.md, preserving Phase 2's 5/5 ROADMAP success criteria. Audit-quality doc work paid off.

### What Was Inefficient

- **MCP-driven E2E witness gap shipped as "PASSED" for 11 days.** Phase 4's E2E-02 smoke test on 2026-04-15 fell back to CLI-driven exchange when "MCP server connection failed in Claude Code sessions." That fall-back was witnessed and accepted, but the actual MCP-driven flow that the milestone goal promised was unverified until the bridge phase landed on 2026-04-26. The `04-E2E-SMOKE.md` artifact correctly documented the fall-back, but the MCP gap should have triggered a follow-up sooner.
- **Test-gate parallelism flake discovered at audit time, not build time.** Default-`-j` `cargo nextest run` reproducibly fails 6 listen-subprocess tests on this hardware; only `-j 4` is reliably green. The bridge phase's `01-VERIFICATION.md` claims "419/419" without specifying parallelism. Should have pinned `[[profile.default.test-groups]]` from the first time a listen subprocess test landed.
- **Orphan REQ-IDs in plan frontmatter.** Bridge plans claim MCP-07..16 + E2E-04 in `requirements:` frontmatter; only MCP-10/11 actually got realized and tracked. Aspirational labeling in plan frontmatter creates dead audit references that the milestone close has to clean up.
- **`.planning/REQUIREMENTS.md` lagged the bridge phase by 11 days.** MCP-10 and MCP-11 were implemented and verified on 2026-04-26 but only added to REQUIREMENTS.md during the milestone audit. The bridge phase should have updated the master traceability table at execute-phase time.

### Patterns Established

- **Bridge phase pattern.** When a substrate is shipped but a forward-compatibility need surfaces, a 1-week numbered "v0.X.x bridge" phase that pulls the next-milestone contract back onto the current substrate beats both (a) leaving the substrate unfit-for-purpose and (b) waiting for the next milestone. Track separately, archive under the current milestone.
- **Strict-on-day-one over grace periods.** When changing a contract that touches user-facing onboarding (env var → register tool), ship strict and migrate. The migration code is a one-time write; grace-period code is forever.
- **Audit-time doc backfill is a real artifact category.** TD-2 (Phase 4 redirect VERIFICATION) and TD-5 (REQUIREMENTS.md MCP-10/11 backfill) are both legitimate audit-time outputs — they're cheap, they preserve the audit trail, and they belong in the milestone close, not in a follow-up phase.
- **Hand-rolled MCP framing over `rmcp` dependency.** Content-Length-framed JSON-RPC over stdio is ~50 lines of Rust; the Rust MCP SDK adds ~15 dependencies. For a single binary that ships exactly the FAMP tool surface, hand-rolling is correct.

### Key Lessons

- **The milestone goal is the witness, not the SUMMARY count.** v0.8 had 13 plans archived as "complete" by 2026-04-15 and the milestone goal (two-window MCP-driven exchange) was demonstrably unmet. Plan completion ≠ milestone completion. The bridge phase exists because the difference matters.
- **`disk_status: complete` lies if VERIFICATION.md is missing.** Phase 4 never had a `*-VERIFICATION.md` and the SDK happily called the phase complete because all SUMMARYs were present. Future milestones should treat "no VERIFICATION.md" as a phase-complete blocker, not a soft warning.
- **Auto-commit semantics are load-bearing for two-window flows.** The bridge phase's E2E only works because Phase 4's auto-commit handler fires-and-forgets a commit envelope back to the originator after inbox fsync. Without auto-commit, the originator never advances REQUESTED → COMMITTED and `famp_await --task <id>` returns nothing useful. v0.9 broker design must preserve this behavior.

### Cost Observations

- **Model mix:** Bridge phase ran on Opus 4.7 throughout (planning, execution, verification). No Haiku/Sonnet routing.
- **Sessions:** ~3 sessions for the bridge (plan, execute waves 1+2, verify+audit). Audit + complete-milestone in one continuation session (this one).
- **Notable efficiency:** the bridge phase landed in 1 day from plan to verified because the plan was structured as 5 small TDD plans (01-01 through 01-05) with explicit dependency chains, and waves 2 (01-04, 01-05) could run in parallel after wave 1.
- **Notable inefficiency:** the discovery loop on the `await_cmd` FSM bug consumed ~30 min of bridge phase 01-05 because the failing E2E test pointed at the wrong layer first. A unit-level repro for FSM advance on terminal deliver would have caught it directly.

---

## v0.9 Prep Sprint and the Sofer Field Report

**Recorded:** 2026-04-26 (same day as v0.8 ship)

### What happened

Hours after the v0.8 milestone closed, **Dr. Ben Sofer** (first wild user) shipped a 5-agent FAMP mesh on his medical-platform repos: `dk` (discreetketamine.com), `tovani` (tovanihealth.com), `dbs` (drbensoffer.com), `infra` (infrastructure-dashboard), `openheart` (api-monitor). One Mac, all signed-messaging via `famp-local`. `famp-local init dk tovani dbs infra openheart`, then 5× `famp-local wire`. Auto port allocation across 5 agents (8443 → 8447) just worked. End-to-end `dk → tovani` smoke landed in under a second.

Nine use cases shipped in hours, split cleanly into two layers:

**Layer 1 — Deterministic events via Claude Code `PostToolUse` hooks.** Bash script at `~/.famp-local/hooks/notify-on-edit.sh` reads hook payload from stdin, pattern-matches `file_path`, forks `famp send` to background. Self-sends skipped. Use cases: infra audit log on amplify.yml/Prisma schema edits, migration coord on `prisma/migrations/**`, Rx integration cross-site fan-out on drchrono/prescri/pharmacy/-rx file patterns.

**Layer 2 — Judgment events via CLAUDE.md instructions.** Each repo's CLAUDE.md tells its agent when to use `famp_send` for events that need judgment. Use cases: dk ↔ tovani feature port, junior-site bootstrap (dbs ASKS dk/tovani for patterns; dk/tovani proactively brief dbs on polished work), question routing across loaded contexts, end-of-session digest to `openheart`, discovery alerts, GSC opportunity routing in `/blog-today`.

Both layers ride the same MCP server, same identity, same trust model. Sofer's framing: *"the same MCP server backs both deterministic hooks and judgment-driven instructions, and they compose into a coherent multi-agent system."*

### What worked

- **Peer-card exchange + TOFU pinning** stayed invisible inside `famp-local`. Never thought about TLS certs once across a day.
- **MCP integration is the killer feature.** The agent doesn't need to know FAMP exists — `famp_send` / `famp_inbox` / `famp_await` / `famp_peers` show up as tools and it uses them naturally.
- **The 5-state FSM is visible and useful.** Validated round-trips with clean transitions, signed audit trail on both sides.
- **Auto port allocation, JSON-per-line inbox, two-layer pattern composability** all confirmed working under load.
- **Question routing across loaded contexts is the headline value.** Window A asks B from B's already-loaded repo context instead of reloading. Validated CLI end-to-end with full FSM round-trip.

### Rough edges (mapped to prep-sprint actions)

1. `famp-local wire` dies on first call (mesh size 1 → error). First-time UX is "click → error → re-read README." → **Prep sprint T2.**
2. No built-in hook layer. Sofer rolled bash + per-repo `settings.json` from scratch. He named `famp-local hook add --on Edit:<glob> --to <peer>` as biggest leverage gap. → **v0.9 Phase 2** (channels in v0.9 redefine the hook target model; building on v0.8 means rebuilding).
3. No fan-out primitive (notifying 2-3 sites = 2-3 `famp send` calls). → **v0.9 channels (already in design).**
4. Hooks duplicate `FAMP_LOCAL_ROOT` though `.mcp.json` already encodes it. → **Prep sprint T3.**
5. Inbox volume from auto-notifications. He proposed an "audit log" envelope class with auto-ack, separate from `request`, no FSM accumulation. → **Prep sprint T5** (spec amendment v0.5.1 → v0.5.2) **+ v0.9 Phase 1** (impl).
6. Mid-experiment he asked "could I use this for cross-site calendar sync?" Correct answer: no — FAMP needs an open Claude Code window actively reading inbox. Production data sync (appointments, customer state) must happen server-side. → **Prep sprint T4** (README boundary section).

### What this drove

A 3-day prep sprint between v0.8 ship and `/gsd-new-milestone v0.9`. Nine tasks (T1-T9). **No v0.8.2 release tag** — these are commits on `main` that unblock v0.9.

The vector pack drafted today as `WRAP-V0-5-1-PLAN.md` defers to v1.0 alongside the federation gateway; ships when a second implementer (likely Sofer from a different machine) commits to interop. CLAUDE.md "L2+L3 in one milestone" constraint — written for an earlier bet — revised in T6 to allow staged conformance.

Two parallel agent reviews (`the-architect` + `zed-velocity-engineer`) pressure-tested the plan. Both wanted vectors first; rejected on the grounds that vectors produce an artifact for nobody until a second implementer exists, and v0.9 doesn't break Layer 0 (vectors aren't blocked by v0.9; v0.9 doesn't block them). Both were right that hook subcommand is a yak-shave for v0.8.x — moved to v0.9 Phase 2. Architect was right that audit-log requires a spec amendment, not just an impl change — added as T5.

**v1.0 readiness trigger named explicitly** in PROJECT.md (T7): *"v1.0 federation milestone triggers when Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. If 4 weeks pass after v0.9.0 ships with no movement on this trigger, federation framing is reconsidered."*

### Key lesson

**Field reports change priorities; protocol-purity reasoning untethered from real users does not.** Both reviewers reached for "ship vectors first" because the value-prop language demanded it; neither named a real consumer. The Sofer report is the actual signal. The vector pack ships when interop becomes concrete, not when self-imposed protocol discipline says it should.

The full nine-use-case detail and Sofer's wishlist for post-v0.9 (hook subcommand, `famp tail` / `famp watch`, identity introspection, audit-log envelope class with auto-ack semantics, boundary docs) is the source of truth for v0.9 scope refinement. v0.9's MILESTONE-AUDIT will retrospectively check whether each rough edge was addressed.

### Pickup pointer

If context resets, durable trail: `.planning/V0-9-PREP-SPRINT.md` (ordered checklist), `.planning/STATE.md` (current status), this section (full report), auto-memory entries `project_v09_prep_sprint` / `project_sofer_field_report` / `project_vector_pack_decision` / `project_v10_trigger` / `project_v09_scope_additions`, and Brain decisions captured 2026-04-26 under project `famp`.

---

## Milestone: v0.9 — Local-First Bus

**Shipped:** 2026-05-04
**Phases:** 5 (1+2+3+4 + close-fix Phase 5) | **Plans:** 35 | **Commits:** 193
**Timeline:** 2026-04-27 → 2026-05-04 (8 days; the prep sprint accounts for the 2026-04-26 ship-of-v0.8 → 2026-04-27 milestone-open interval)

### What Was Built

- `famp-bus` Layer-1 substrate — pure `Broker::handle(BrokerInput, Instant) -> Vec<Out>`, zero `tokio` and zero I/O in core, length-prefixed canonical-JSON codec (4-byte BE, 16 MiB cap), nine `BusMessage` / eleven `BusReply` variants byte-exact through `famp-canonical`. Four RED-first TDD gates GREEN (codec fuzz, drain cursor atomicity, PID reuse race, EOF cleanup mid-await) and five proptest properties GREEN (DM fan-in ordering, channel fan-out, join/leave idempotency, drain completeness, PID-table uniqueness). `just check-no-tokio-in-bus` and `just check-spec-version-coherence` permanent CI gates.
- Atomic v0.5.1 → v0.5.2 spec bump in single commit `9ca6e13` — `MessageClass::AuditLog` + `AuditLogBody` + `Relation::Audits` + `BusEnvelope<B>` (BUS-11 sibling type, private inner, 2 `compile_fail` doctests) + `AnyBusEnvelope` 6-arm dispatch + `EnvelopeDecodeError::UnexpectedSignature` + `FAMP_SPEC_VERSION = "0.5.2"` + T5 lag-block deletion + `vector_1` worked example. AUDIT-05 atomic-bump invariant honored.
- `famp broker` UDS daemon at `~/.famp/bus.sock` with `posix_spawn`+`setsid` auto-spawn, `bind()`-IS-the-lock single-broker exclusion (no `flock`, no PID file), 5-minute idle exit with fsync+unlink, NFS-mount startup warning. 8-verb top-level CLI (`register`, `send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`); `~/.famp/mailboxes/<name>.jsonl` reuses `famp-inbox` JSONL with atomic temp-file+rename cursor advance.
- 8-tool stable MCP surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) — `famp mcp` drops `reqwest`+`rustls` from the startup path; `cargo tree -p famp` shows zero TLS reach for MCP. Error-mapping is exhaustive `match` over `BusErrorKind` (no wildcard).
- `famp install-claude-code` writes user-scope MCP config to `~/.claude.json` and drops 7 slash-command markdown files (`/famp-register`, `/famp-join`, `/famp-leave`, `/famp-send`, `/famp-channel`, `/famp-who`, `/famp-inbox`) into `~/.claude/commands/`. **README Quick Start passes the 12-line / 30-second acceptance test on a fresh macOS install.** Codex parity ships as MCP-only install/uninstall via TOML structural merge.
- `famp-local hook add --on Edit:<glob> --to <peer-or-#channel>` declarative wiring; `~/.famp-local/hooks.tsv` registry; `list`/`remove` round-trip; `hooks.Stop` execution runner via `~/.famp/hook-runner.sh` parameterized on `${FAMP_LOCAL_ROOT:-$HOME/.famp-local}`.
- Federation CLI hard-deleted (`famp setup`, `famp listen`, `famp init`, `famp peer add`, `famp peer import`, old TLS-form `famp send`); `famp-transport-http` + `famp-keyring` relabeled "v1.0 federation internals" in workspace `Cargo.toml`. **`e2e_two_daemons` refactored to library-API direct instantiation** (full signed `request → commit → deliver → ack` over real HTTPS in-process), runs green in `just ci` every commit — the **plumb-line-2 commitment** against mummification. Tag `v0.8.1-federation-preserved` cut at `debed78` BEFORE deletions land. `docs/MIGRATION-v0.8-to-v0.9.md` ships table-first; ~27 federation-coupled tests parked under `crates/famp/tests/_deferred_v1/` for v1.0 reactivation.

### What Worked

- **The Sofer field report became the forcing function.** Phase 4 federation unwire was scoped to the bone before the milestone opened because the Architect's "local-case black hole" risk had a concrete escape hatch (the `e2e_two_daemons` library-API refactor) and a named v1.0 trigger (Sofer-from-different-machine). No drift toward "let's also keep `famp listen` working from the CLI just in case."
- **Atomic spec-bump invariant in a single commit (`9ca6e13`).** The constant flip + impl + dispatch + body + doc-comment removal + Justfile recipe rode one commit. AUDIT-05 was the right invariant — bumping in a separate commit either lies (if before impl) or strands impl as v0.5.1-tagged (if after). Pattern carries forward to v0.5.3 / v0.5.4 if needed.
- **TDD gates RED-first, GREEN later.** Phase 1 Plan 01-01 shipped TDD-02/03/04 as compile-red gates *before* the broker existed. Plan 01-02 turning them green was structurally constrained — no broker design that broke a gate could ship. Pattern proven in v0.6 (vectors before impl) carries into multi-actor concurrency without modification.
- **Wave-based plan structure under v0.9 Phase 2.** 14 plans across 7 waves with explicit Wave-0 stub-file infrastructure (Plan 02-00) kept Plan 02-01 below the 15-file blocker threshold and gave Plans 02-10/11/12/13 a known-good landing pad to overwrite. The infrastructure plan was a discipline aid, not a tax.
- **Phase 5 as a milestone-close fix-pass.** v0.9-MILESTONE-AUDIT.md found two real gaps (CC-07 BROKEN, HOOK-04b PARTIAL) and one bookkeeping gap (Phase 3 retroactive verification). Phase 5 closed all three in 4 small plans with a re-audit confirming `passed`. Pattern (audit → planned-gap-closure phase → re-audit → ship) is the right way to handle "audit found gaps" without dragging the milestone or shipping broken.
- **Migration guide table-first, not prose-first.** `docs/MIGRATION-v0.8-to-v0.9.md` leads with a CLI verb mapping table because users don't read prose during migration — they grep for the verb that just disappeared and look at the right column.
- **Architect counsel re-read before relaying.** The 2026-04-30 catch (architect superseded an earlier verdict; I had been about to relay the stale one) reinforced the auto-memory rule: re-read the agent's mailbox before relaying counsel that locks scope. Pattern held through v0.9 Phase 4 scope decisions.

### What Was Inefficient

- **Auto-generated MILESTONE accomplishments pulled noise from SUMMARY headers.** `gsd-sdk query milestone.complete` extracted clippy findings, file paths, and "Rule 1 — Bug" prefixes as one-liners, polluting the v0.9 entry. Same root cause as v0.6's complaint: SUMMARY one-liner extraction has no template enforcement. Hand-rewrite cost ~10 minutes per milestone close. Lesson stays open: SUMMARY.md front-matter needs a `deliverable:` field that the extractor *only* pulls from.
- **`milestone_name: Federation Profile)` in STATE.md frontmatter** for the entire v0.9 milestone life — set incorrectly during `/gsd-new-milestone` and never noticed because it's not displayed anywhere user-visible. Caught at milestone close. Lesson: STATE.md frontmatter is consumed by the SDK; lints should validate `milestone_name` is non-default and matches the milestone in PROJECT.md.
- **Quick-task index drift.** 30 orphan quick-task slugs accumulated (federation-era + v0.9 prep-sprint residue) and surfaced only at milestone-close audit. The slugs reference no actual completion artifacts. Lesson: `/gsd-cleanup` should run automatically at phase boundaries, not only at milestone close, OR the quick-task SDK should refuse to keep "missing"-status entries past N days.
- **Two manual UAT scenarios (BROKER-02 broker-survives-SIGINT, BROKER-05 NFS warning positive path) parked open through Phase 2 close** — resolved 2026-04-30 in a single sweep but the parking should have been an explicit Phase 2 closer plan rather than ad-hoc. v1.0 should structure manual UATs as gated plan items with named owners, not "we'll get to it before the milestone closes."

### Patterns Established

- **Atomic spec-bump invariant** (AUDIT-05): when the protocol semantics change, the constant flip + impl + dispatch + body schema + doc-comment removal + CI guard recipe must ride one commit. Bump-then-impl strands. Impl-then-bump lies.
- **Plumb-line-2 against mummification**: when removing user-facing code that has unique-in-CI test coverage, refactor the test to library-API and keep it in CI on every commit. v0.9 Phase 4's `e2e_two_daemons` is the template — federation HTTPS coverage stays even after the user-facing CLI is gone.
- **`bind()` IS the lock for single-instance UDS daemons.** No `flock`, no PID file, no double-fork. `EADDRINUSE` → probe via `connect()` → live or stale → unlink+retry once. Pattern proven against `kill -9` mid-Send recovery and two-near-simultaneous-spawn race.
- **Wave-0 stub-file infrastructure plan.** When a multi-plan phase has plans that need to overwrite stub files (test fixtures, integration test scaffolds), ship a Plan 02-00 that creates `#[ignore]`-gated stubs first. Keeps later plans below blocker thresholds AND keeps test-name churn out of `git blame`.
- **Milestone-close fix-pass phase.** When the milestone audit finds gaps that aren't worth deferring (CC-07, HOOK-04b), open a numbered close-fix phase (Phase 5 in v0.9), close the gaps in small plans, and re-audit. Don't re-open closed phases; don't punt to the next milestone if the gap is small.
- **Table-first migration guides.** Lead with a CLI mapping table; prose is supplemental.

### Key Lessons

1. **A named human-with-different-machine is the only honest exit from local-case satisfaction.** The Architect's "local-case black hole" risk is unfalsifiable as long as the only humans running FAMP are co-resident. v0.9 closed by naming Sofer as the trigger and starting a 4-week clock. Lesson generalizes: any milestone whose value depends on someone outside the local mesh validating it must name that person and start a clock at ship.
2. **Refactor user-facing-deletion tests to library API in the same milestone that deletes the surface.** If the test stayed pinned to the deleted CLI, it would either rot to `#[ignore]` or get deleted alongside the CLI; either way the federation-grade coverage would die quietly. Pattern: when a milestone removes a user-facing surface that holds unique CI coverage, the surface deletion and the test refactor ride the same milestone. v1.0 federation-gateway work will inherit this pattern.
3. **Phase numbering reset per milestone is right.** v0.9 reset to Phase 1, same as v0.7 and v0.8. Cross-milestone phase-number continuity has zero practical value and obscures the "first phase of the new milestone" boundary in every future trace.
4. **Local bus + 8-tool stable MCP surface = the right shape for "two Claude Code windows on one Mac."** v0.8's per-identity TLS listener mesh was federation-grade overhead for same-host work. v0.9's UDS broker reduced onboarding from "8 manual steps + a `famp-local` bash wrapper" to "12-line README, 30-second second-window install." The federation-grade primitives didn't go away — they moved to v1.0 internals, exercised by one library-API test, ready for the gateway.
5. **Audit → planned-gap-closure phase → re-audit ships better milestones than audit → "we'll fix it later".** Phase 5 (4 small plans, half a day) closed the audit gaps in the same milestone. The alternative would have been "ship v0.9.0 with a known-broken `/famp-who` and HOOK-04b path mismatch." Numbered close-fix phases are a feature, not a smell.
6. **Channel mailboxes will grow unbounded** — `famp mailbox rotate` / `famp mailbox compact` is in v0.9.1 explicitly. Acceptable in v0.9 because interactive developer usage won't hit the limit for weeks. Don't add bounded-cache complexity that solves a problem nobody has yet.

### Cost Observations

- Model mix during v0.9: predominantly Opus 4.7 1M for execution; Haiku 4.5 for parallel auditor passes; Sonnet 4.6 for UI/docs polish.
- Sessions: ~30+ across 8 days, including ~6 multi-window mesh sessions for the 3-agent pressure test that drove backlog 999.3/999.4/999.5.
- 193 commits / 35 plans = 5.5 commits/plan average — within the v0.6 (3.8) and v0.8 (4.4) ranges. Phase 1's atomic v0.5.2 bump (1 commit) and Phase 5's surgical fix-pass (4 small commits) pulled the average down; Phase 2's 14-plan wave structure pulled it up.
- Notable: Phase 3 hit the 12-line / 30-second README acceptance gate on first attempt. The gate was the design forcing function from day one (locked into the design spec) — proof that hard acceptance gates set BEFORE the phase starts produce more focused execution than gates discovered during the phase.

### Pickup pointer

If context resets, durable trail: `.planning/milestones/v0.9-ROADMAP.md` (full archive), `.planning/milestones/v0.9-MILESTONE-AUDIT.md` (audit `passed`), `.planning/milestones/v0.9-REQUIREMENTS.md` (85/85 mapped), `.planning/milestones/v0.9-phases/` (35 plans + summaries + verifications), this section, and Brain decisions captured 2026-04-27 → 2026-05-04 under project `famp`.

---

## Cross-Milestone Trends

### Patterns that have held across ≥3 milestones

- **External spec vectors committed verbatim, never self-generated** (PITFALLS P10) — established in v0.5.1 (spec-lint anchors), reinforced in v0.6 (RFC 8785, RFC 8032, §7.1c, NIST FIPS 180-2 KATs), held in v0.7 (NIST KATs across HTTP transport adversarial cases), v0.8 (envelope wire stays byte-for-byte the v0.7 contract), and v0.9 (vector_1 audit-log fixture ships in same atomic commit as the spec bump).
- **`just ci` as a single blocking gate, `cargo tree -i openssl` empty as a hard gate** — held v0.5.1 → v0.9. v0.9 added `just check-no-tokio-in-bus` and `just check-spec-version-coherence`.
- **Spec version pinning via a Rust constant** — held; v0.9 was the first milestone to bump (`"0.5.1"` → `"0.5.2"`) and proved the AUDIT-05 atomic-bump pattern.
- **Narrow, phase-appropriate error enums** — held into v0.9 (`BusErrorKind` is exhaustive, no wildcard; MCP error-mapping fails compile until `BusErrorKind` extension is handled).
- **Free-function-primary + trait-sugar pattern** — held in v0.9 (`Mailbox`/`Liveness` are traits but `InMemoryMailbox`/`DiskMailboxEnv` are constructed as concrete types).
- **Defer-until-proven-needed for `stateright`** — re-evaluated at v0.9 multi-actor concurrency entry; proptest legality + the four TDD gates (drain cursor atomicity, PID reuse race, EOF cleanup) were sufficient. Position holds for v1.0; will be re-evaluated when federation negotiation FSM lands.
- **Phase numbering reset per milestone** — v0.7, v0.8, v0.9 all reset to Phase 1. Convention locked.

### Patterns established in v0.9 (watch for ≥2-milestone confirmation)

- **Atomic spec-bump invariant (AUDIT-05)** — first proven in v0.9. Re-validate at v0.5.3 / v1.0.
- **Plumb-line-2 against mummification** — first proven in v0.9 Phase 4. Re-validate when v1.0 deletes any other v0.9 surface.
- **Wave-0 stub-file infrastructure plans** — first proven in v0.9 Phase 2. Re-validate when next ≥10-plan phase ships.
- **Milestone-close fix-pass phase** — first proven in v0.9 Phase 5. Re-validate when next milestone audit finds non-deferrable gaps.
- **Trigger-gated next milestone with a 4-week clock** — first proven by v0.9.0 → v1.0 transition. Re-validate by 2026-06-01 (clock expiration).

### Open watch-items for v1.0

- **Will the named-human trigger fire?** Sofer-from-different-machine, 4-week clock. If 2026-06-01 passes without movement, federation framing is reconsidered — and the conformance vector pack stays deferred.
- **Will the v0.9 8-tool MCP surface stay stable through v1.0 federation-gateway work?** v0.9 promised the surface as the contract carried forward to v1.0. The federation gateway should *wrap* the local bus, not replace it; the gateway should be a separate process bridging UDS to remote HTTPS, not a new MCP surface.
- **Will `e2e_two_daemons` library-API coverage be sufficient for federation regression detection in v1.0?** Today it's one test exercising sig-verify on real HTTPS. v1.0 federation work will likely demand 5+ tests (Agent Card validation, replay defense, supersession, delegation forms). Plan the test surface ahead of the implementation.
- **Will the auto-extracted MILESTONES.md accomplishments noise problem (v0.6, repeated v0.9) be fixed before v1.0 close?** Three milestones with the same complaint is a pattern, not a one-off. Either fix `gsd-sdk query milestone.complete` extractor, or add a SUMMARY.md `deliverable:` field as a hard schema lint.
- **Will quick-task index drift continue accumulating across milestones?** v0.8 closed with 22 deferred; v0.9 closed with 30. If v1.0 closes with 40+, the SDK should refuse to keep "missing"-status entries past N days OR `/gsd-cleanup` should run at phase boundaries.

---
*Living retrospective — appended per milestone, cross-milestone trends below the last entry.*
