# Phase 11 — Cross-AI Plan Reviews

**Generated:** 2026-07-28
**Reviewers:** codex CLI (grounded, ran in-tree) · Fable-5 independent adversarial agent (grounded, ran in-tree)
**Unavailable:** gemini CLI (`IneligibleTierError` — Google dropped individual Code Assist for this account) · cursor-agent (auth required — `CURSOR_API_KEY` unset). Self (claude) skipped for independence.
**Verdict:** Both reviewers independently rate the phase **HIGH risk / NOT execution-ready**. The C2/C5 split-addressing design is correctly executed at the mechanism level; the risk is in **completeness and one trust-boundary hole**, not the addressing approach.

---

## Synthesis (convergent findings, ranked by blast radius)

Orchestrator note: the two starred findings below were **source-verified** by the orchestrator against actual code (not taken on the reviewers' word). The rest are catalogued for the `--reviews` replanning pass to verify/incorporate/reject per finding.

### ★ HIGH — Remote path can never reach a terminal FSM state (BOTH reviewers, VERIFIED)
Plan 03 Task 2 constructs `UnsignedEnvelope::<RequestBody>` for **every** remote send, including `--task` continuations and `--task --terminal` closes. The FSM only advances `REQUESTED → COMMITTED` on a `Commit` and reaches terminal on a `Deliver` carrying `terminal_status` (`crates/famp-fsm/src/engine.rs:25-50`; inspector fold `crates/famp-inspect-server/src/lib.rs:638-648,690-710` — a `request` stays `REQUESTED`). The proven E2E drives request→commit→deliver→ack explicitly (`e2e_cross_host_delivery.rs:707-774`, `build_deliver` + `with_terminal_status(Completed)` at `:465-487`). **Machine B's reply via the shipping client emits `class:"request"` → the fold never advances → Plan 06's "terminal FSM on both sides" gate (UAT-01) is unreachable as planned.** Research Open Q3 flagged this precise question; no plan resolved it.
**Fix direction:** branch the remote class on send mode — `--new-task`→`RequestBody`, `--task`→`CommitBody`/`DeliverBody`(interim), `--task --terminal`→`DeliverBody` + `with_terminal_status`. OR explicitly re-scope UAT-01's DoD with Ben's sign-off. Do one, in the plans, before execute.

### ★ HIGH — Authenticated local identity is not bound to envelope `from` (codex, VERIFIED — trust-boundary)
The broker gates on a live effective identity but **deliberately does not stamp/verify** the envelope `from` (`crates/famp-bus/src/broker/handle.rs:388-395`: "identity … is not currently stamped onto the envelope here — that responsibility is left to the CLI/MCP caller for v0.9"). Egress then signs that caller-controlled `from` with the gateway key (`crates/famp-gateway/src/egress.rs:198-211`), no check that `from` matches the authenticated sender. **A locally-registered agent can ask the gateway to sign as another pinned agent.**
**Severity scoping:** under v1.0's stated threat model (own two machines, all local agents are the same trusted person) this is self-forgery — low real-world impact NOW. But this phase introduces the remote-signing path that makes it cross-host exploitable, and it becomes a real vuln at v1.1 (relay + cross-person trust). **Decision needed:** add the `envelope.from.name() == effective_identity` (broker) + `from.authority() == own-domain` (egress) checks now, or explicitly record the deferral in the plan/threat-model. Neither check needs local signatures or reopens BUS-11.

### HIGH — "Single-source" own-domain does not structurally prevent drift (BOTH)
`--domain > FAMP_OWN_DOMAIN > file` lets different processes/invocations resolve different domains, and `famp-gateway` is not modified to read/validate own-domain. It centralizes lookup code but does not *guarantee* `envelope.from == peer-export label`. Fable-5 frames it as "enforced-only-when-configured"; codex notes egress just derives `from_domain` from whatever envelope it gets (`egress.rs:104-117`). **Fix:** make `$FAMP_HOME/own-domain` the canonical persisted source; treat env/CLI as overrides that must *agree* with the file; add the egress `from.authority()==own-domain` assertion (ties into finding #2).

### HIGH — Plans 02/03 omit required compile-time touchpoints (codex, spot-verified)
- MCP `error_kind.rs` is exhaustively matched with **no wildcard** (`crates/famp/src/cli/mcp/error_kind.rs:18-20`) — new `CliError` variants break the build; the uniqueness fixture `crates/famp/tests/mcp_error_kind_exhaustive.rs` too.
- Adding `domain` to `SendArgs` breaks the MCP struct-literal constructor `crates/famp/src/cli/mcp/tools/send.rs:210-222` — not in Plan 03 `files_modified`.
- The own-domain/export coupling affects `crates/famp/tests/peer_roundtrip.rs:27-33` and the E2E's own-domain-unset export (`e2e_cross_host_delivery.rs:274-288`) — **as planned the untouchable E2E will not stay green.**

### HIGH — Plan 04 harness reuse is mechanically impossible (BOTH)
Rust integration-test binaries are separate crates; `Side`/`famp_cmd`/`spawn_gateway`/`poll_inbox_contains` are private to `e2e_cross_host_delivery.rs`; only `common/child_guard.rs` is shared. A new `e2e_shipping_surface.rs` cannot import them while the plan also prohibits editing the E2E. **Fix:** extract the harness into `tests/common/gateway_harness.rs` (with a "no behavioral edits, stays green" guard on the E2E), or add shipping cases to the existing test.

### HIGH — Plan 05 documents the ready-line fix but does not implement it (codex, VERIFIED-plausible)
`famp-gateway` prints "ready" before resolving home / loading signing key / loading peer keyring (`crates/famp-gateway/src/main.rs:200-246`). Plan 05 edits only the guide + doc-accuracy test → **finding #5 remains in shipping code.** Add `crates/famp-gateway/src/main.rs` to Plan 05 and move the ready print after keyring+transport+listener init.

### MEDIUM — Negative-test observable unspecified / names wrong slug (BOTH)
Ingress returns typed slug `unpinned_key` (`crates/famp-gateway/src/ingress.rs:141-151`); egress consumes it as `ServerStatus`, logs, and continues while `famp send` already got local-bus `SendOk`. "Match on `UnpinnedKey`/`unknown_sender`" is cross-process and names the wrong slug. **Fix:** capture gateway-A stderr and assert the enriched relay-failure line contains `unpinned_key`/`403`, plus assert non-delivery on B.

### MEDIUM — D-08 falsification control has no branch on its outcome (Fable-5)
Head `c91e794` is CI-green and the macOS matrix leg already runs the full workspace incl. this E2E. If the pre-regen control PASSES on macOS, post-regen green proves nothing and the real finding-#5 failure mode (live dogfood certs, system-trust path) has no in-CI net. **Fix:** add an explicit branch — if pre-regen passes, add a dedicated test exercising the no-extra-root/system-trust client path before claiming TEST-03 closed. (Per the project's own falsification-needs-a-control rule.)

### MEDIUM — Error-chain fix still flattens the typed error (codex) / muddled causal note (Fable-5)
`format!("{e:?}")` stores another `String` in `RelayError::Transport` (`egress.rs:168-180`) — improves logs but doesn't preserve `Error::source()`; `reqwest`'s Display may still omit the TLS cause even after the leaf Display fix. Load-bearing capture is the `{e:?}`/source-walk at egress; retain `HttpTransportError` as the `RelayError` source and test the final gateway-visible text.

### MEDIUM — `just install` doesn't install the gateway (codex)
`Justfile:273-277` installs only `crates/famp`, not `famp-gateway`. Plan 06 dogfoods the gateway from PATH → stale-binary risk on both hosts. **Fix:** add `just install-gateway`/`install-all` and record binary hashes/commit IDs on both UAT machines.

### MEDIUM — Semantic doc gate still mostly token-matching (codex); `/gateway` negative grep false-positives (BOTH)
Proposed additions don't concretely assert "A backs bob, B backs alice" or ready-after-keyring ordering; a broad `/gateway` absence check collides with legit `~/.famp/gateway/identity.ed25519`. Scope the guard to the pin-label shape (regex `agent:[^\s]*/gateway`).

### LOW (convergent / catalogued)
- `agent:`-prefixed targets that fail `Principal` parse silently fall back to **local** addressing — should be a typed invalid-target error (`identity.rs:53-79`). (BOTH)
- `validate_authority` is **private** (`crates/famp-core/src/identity.rs:208`) — the probe-Principal-parse fallback is the only compliant route; make it the primary instruction (or make the fn `pub` as a reviewed API change). (BOTH)
- Env-race: Plan 03's missing-domain send test may consult `FAMP_OWN_DOMAIN` concurrently with `own_domain.rs`'s serial test → flaky; use `temp_env`/Option-injection. (Fable-5)
- "byte-for-byte" local-regression criteria are unfalsifiable (fresh `Uuid::now_v7()`+ts per call); reword as "identical modulo id/ts". (Fable-5)
- Leaf-name collision: `--to agent:hostb/bob` routes the bus frame to a local holder named `bob` if one exists — silent mis-delivery, untested. (Fable-5)
- Plan 02 `peer export` `--domain` tier is dead code (`PeerExportArgs` has only `--as`). (Fable-5)

---

## Reviewer 1 — codex CLI (full)

<full_review reviewer="codex">
[Overall: HIGH]

## Summary
The plans are well researched and correctly preserve C2/C5 split-addressing, INV-10 signing, and BUS-11 unsigned-local-bus semantics. However, they are not execution-ready for a v1.0.0 gate. The largest gap is functional: Plan 03 emits `request` for every remote send, while the FSM requires `commit → terminal deliver`; therefore Plan 06 cannot reach a terminal state using the shipping client as written. There is also a trust-boundary vulnerability: the broker authenticates a local connection but does not bind that identity to the envelope's `from`, and the gateway signs the caller-supplied value. Several plans also omit required compile-time touchpoints, propose an unusable cross-test harness reuse, and document—but do not implement—the ready-line fix.

### Strengths
- Split-addressing mechanism correct — local `Target::Agent{name:"bob"}` while retaining full remote principal (`e2e_cross_host_delivery.rs:519-535`).
- Qualifying both `from` and `to` necessary — ingress peeks `from`, keyring lookup, strict decode (`verify.rs:58-66`).
- BUS-11 preserved — bus decode rejects `signature` (`bus.rs:47-57`); ingress strips federation fields (`ingress.rs:91-120,197-209`).
- Plan 01 targets real observability loss (`error.rs:59-70` + `egress.rs:204-211`).
- TLS fixture finding source-confirmed (loopback SANs, no BasicConstraints/EKU; CI both OSes `ci.yml:104-119`).
- Setup-guide corrections grounded (`GATEWAY-SETUP.md:56-67,122-138`).
- Wave ordering sensible.

### Concerns
- HIGH — Plan 03 cannot drive FSM to terminal (`fsm/src/engine.rs:25-50`; e2e `:707-774`).
- HIGH — Authenticated local identity not bound to envelope `from` (`broker/handle.rs:378-400,1016-1034`; egress `:191-211`; `bus.rs:1-6`).
- HIGH — "single source" doesn't structurally prevent drift (gateway unmodified; egress `:104-117`).
- HIGH — Plans 02/03 omit compile-time edits (MCP `error_kind.rs:18-20,46-103`; `mcp_error_kind_exhaustive.rs`; `mcp/tools/send.rs:210-222`; `peer_roundtrip.rs:27-33`; e2e `:274-288`).
- HIGH — Plan 04 harness reuse mechanically impossible (`e2e:81-83,146-183`).
- HIGH — Plan 05 documents ready-line fix without implementing (`main.rs:200-246`).
- MEDIUM — error-chain still flattens typed error (`egress.rs:168-180`).
- MEDIUM — negative E2E lacks defined observable (`ingress.rs:141-151`).
- MEDIUM — semantic doc gate still token-matching (`gateway_setup_doc_accuracy.rs:77-96`).
- MEDIUM — `just install` doesn't install gateway (`Justfile:273-277`).
- LOW — malformed principal-looking target silently local (`identity.rs:53-79`).
- LOW — references private `validate_authority` (`identity.rs:208-243`).

### Suggestions
1. Revise Plan 03 around the full shipping lifecycle (commit/terminal deliver/ack) + full-cycle test reaching COMPLETED.
2. Enforce sender binding before federation signing (broker `from.name()==effective_identity`; egress `from.authority()==own-domain`; adversarial tests). No local sigs, no BUS-11 reopen.
3. Make `$FAMP_HOME/own-domain` canonical; env/CLI must agree with file.
4. Expand Plan 02/03 file lists (MCP error mappings, exhaustive tests, all constructors, peer round-trip, e2e config).
5. Extract two-host harness into `tests/common/gateway_harness.rs`; capture gateway stderr for exact `403`+`unpinned_key`.
6. Add `main.rs` to Plan 05; move ready print after init; test readiness behavior.
7. Plan 01: retain `HttpTransportError` as `RelayError` source; explicit source-chain formatter; test gateway-visible text.
8. Add gateway install/deploy command; record binary hashes/commit IDs on both UAT machines.
9. Plan 04 control: require evidence of the expected TLS/EKU failure specifically (child stderr currently discarded, `e2e:189-217`).

### Risk Assessment
Overall: HIGH. Addressing design sound, but plans fail two v1.0 gates (no terminal-FSM shipping path; gateway signs locally-forged `from`), plus omitted compile-time deps and infeasible E2E harness. Address before implementation or tagging v1.0.0.
</full_review>

---

## Reviewer 2 — Fable-5 independent adversarial agent (full)

<full_review reviewer="fable-5">

## Summary
The six plans are unusually well-grounded: nearly every cited line number checks out (verified `error.rs:63`, `egress.rs:211/273`, `verify.rs:62-63`, `send/mod.rs:413/425/494-507`, `main.rs:201/229/255`, `e2e:86-92/406-436/519-543`, `ci.yml:104-119`, flag-grep doc gate, ECDSA/stale-README fixtures). The settled C2/C5 approach is executed correctly at the mechanism level — both `from` AND `to` stamped, bus `Target` stays bare leaf, class upgrade gated on the domain-qualified branch, prohibitions present as checkable must_haves. One HIGH completeness gap: plans build only the request leg while Plan 06 requires terminal FSM on both sides (inspector derives it from `class:"deliver"`+`terminal_status`, `inspect-server/src/lib.rs:636-686`). A remote reply emits `class:"request"` → fold never advances. Research Open Q3 flagged it; no plan resolved it. Several test-harness contradictions will force executor improvisation.

### Strengths
- D-02 correctly executed not just asserted (`verify.rs:62-63` → `keyring.get(&from)`).
- Split-addressing traced to ground truth (`e2e:519-543`; route-map `main.rs:255-261`).
- Sign-then-strip byte-correct (`e2e:406-436`, `[42u8;32]`, `AuthorityScope::Advisory`, bounds≥2).
- D-06 genuinely sequenced first; swallow sites real (`error.rs:63`, `egress.rs:211`); also spots `InvalidUrl` Display-drop `error.rs:69-70`.
- D-05 mirrors `home.rs:1-6` single-env idiom + `peer/export.rs:31` seam.
- Honest CONTEXT corrections (macOS CI leg exists `ci.yml:111`; injector untracked).
- Doc corrections match actual broken doc (self-signed §1; `/gateway` pin §3; A backs alice §4; ready before keyring `main.rs:201`/`229`).
- Prohibitions checkable across plans.

### Concerns
- HIGH — remote path never terminal; Plan 06 gate unsatisfiable (`11-03-PLAN.md:100-101`; `inspect-server/src/lib.rs:636-641,688`; research Open Q3).
- MEDIUM-HIGH — Plan 02 Task 2 collides with untouchable e2e (`e2e:276` exports full principal, no own-domain set); only reconcilable semantic is "validate only when configured", silently downgrading the must_have.
- MEDIUM — Plan 04 harness-reuse internally contradictory (`common/` only has `child_guard.rs`; `Side` L150/`famp_cmd` L174/`spawn_gateway` L318/`poll_inbox_contains` L553 live inside the prohibited-to-edit e2e).
- MEDIUM — D-08 control has no branch on outcome; if pass-before-regen, post-regen proves nothing (research Open Q2: `--trust-cert` extra-root may mask EKU check).
- MEDIUM — Plan 03 `files_modified` omits guaranteed compile break at `mcp/tools/send.rs` (struct literal); silent on MCP `to` description update.
- MEDIUM — negative test observation point unspecified/unobservable across two subprocess boundaries (`verify.rs:63-64`→401; sender sees only `RelayError::Transport` string in gateway-A stderr).
- MEDIUM — `validate_authority` private (`identity.rs:208-212`); probe-principal fallback is the only compliant route and should be primary.
- LOW-MEDIUM — env-race in Plan 03 missing-domain test (needs `temp_env` like `identity.rs:221`).
- LOW — Plan 01 Task 2 causal note backwards (Debug never consults Display; reqwest Display omits TLS cause; load-bearing capture is `{e:?}` at egress).
- LOW — "byte-for-byte" local criteria unfalsifiable (fresh uuid/ts `send/mod.rs:437-461`).
- LOW — Plan 05 `/gateway` negative grep false-positives on legit content; scope to pin-label regex.
- LOW — `grep -n sleep` acceptance conflicts with the blessed poll helper (`e2e:589` uses sleep).
- LOW — leaf-name collision unhandled (real local `bob` → silent mis-delivery).
- LOW — Plan 02 export `cli_domain` tier dead code (`export.rs:15-20` only `--as`).

### Suggestions
1. Close terminal-leg gap before execution — branch remote class on send mode (`RequestBody`/`DeliverBody`+`with_terminal_status`), OR re-scope UAT-01 DoD with Ben's sign-off — in the plans, not at the dogfood.
2. Specify Plan 02 unset-domain export semantic (accept verbatim + warn when unset; typed error on mismatch-when-configured).
3. Resolve harness contradiction (permit mechanical helper-extraction to `tests/common/` with "stays green" guard, or drop "no copy-paste").
4. Give the falsification control teeth (branch on pre-regen pass; add system-trust/no-extra-root client-path test).
5. Add `mcp/tools/send.rs` to Plan 03 `files_modified` (+ optional `to`-description update).
6. Make probe-`Principal` parse the primary validation instruction (or `pub validate_authority` as reviewed change).
7. Pin negative-test observation (capture gateway-A stderr, assert `unknown_sender`/`401`; assert non-delivery on B).
8. Reword regression criteria "identical modulo id/ts"; use `temp_env` for the missing-domain send test.

### Risk Assessment
MEDIUM. Trust-boundary core (both from+to stamped, bus leaf-routed+unsigned, single own-domain source, hard-reject unpinned) is correctly designed/sourced/guarded — no defect that ships a wrong signature or provenance. Risk concentrated in **completeness not correctness**: the phase's own DoD (terminal FSM both sides) is unreachable with plans 01–05 (the HIGH finding), and three test-plan contradictions will surface mid-execution. Fixing class-selection + Plan-02 unset-domain semantic before execute converts this to LOW.
</full_review>

---

## Recommended action
Run `/gsd-plan-phase 11 --reviews` to replan incorporating this feedback. The planner must, at minimum, resolve the two verified HIGH findings (FSM terminal class-selection; sender-`from` binding decision) and the compile-time/harness gaps before `/gsd-execute-phase 11`. The two reviewers diverge only on overall rating (codex HIGH, Fable-5 MEDIUM) — the delta is whether the security finding is in-scope-now; that is a call for Ben.
