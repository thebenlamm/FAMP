# Phase 12: v1.0.0 Release Gate - Research

**Researched:** 2026-07-29
**Domain:** Release engineering / documentation-accuracy / adversarial source review / release-record hygiene — NOT new federation logic
**Confidence:** HIGH (every claim below is source-grounded: file:line citations, live `git`/`gh` command output, or verbatim PDF quotes — no library research was needed because this phase adds no new dependencies)

## Summary

Phase 12 closes design review C's §16 nine-item `v1.0.0` tag checklist. Six items (1,2,3,4,5,7) are already satisfied and re-verified in `11-VERIFICATION.md`; this phase closes the remaining three — item 8 (REL-01, doc states what `send` confirms), item 9 (REL-02, post-fix adversarial source review), and item 6 (REL-03, CI green **at the exact tag commit**) — plus release-record hygiene (REL-04) and the tag itself (REL-05).

The single highest-risk finding: **`docs/GATEWAY-SETUP.md`, `README.md`, and `.planning/REQUIREMENTS.md`/`ROADMAP.md` are all `docs/**`/`.planning/**`/`**/*.md` paths, and `.github/workflows/ci.yml` has `paths-ignore` on exactly those globs.** A commit that touches only those files triggers **zero CI runs** — not a skip, not a pass, literally no check-run object exists for that SHA. This is empirically confirmed: `ca59f48` and `73085aa` (the two most recent commits, both docs-only) have **zero** GitHub check-runs, while `269b748` (the last commit that touched `.rs` files) has a full green run (fmt-check, clippy, build×2, test×2, doc-test, audit, canonical, crypto, smoke-test — all `success`). REQUIREMENTS.md's own text ("CI's green run is on `ca59f48`") is **factually wrong** — `ca59f48` was never CI-checked at all. REL-03 must be satisfied by ordering the phase's commits so the SHA that receives the `v1.0.0` tag touches at least one non-ignored path (the version bump naturally does this — see §"Sequencing").

REL-01's second-highest-risk finding: the exit code of a remote `famp send` represents **local-broker acceptance into the gateway-backed proxy's mailbox only** — egress relay to the remote host is a fully decoupled async drain loop (`crates/famp-gateway/src/egress.rs::run_egress`, ~1s poll interval) that the CLI process never waits on. This is the true fire-and-forget boundary and must be stated precisely, not hand-waved.

REL-05's release-note risk: §16's own proposed limitation wording ("`famp send` … does not initiate or complete the task FSM") **predates Phase 11 and is now false** — ADDR-02 (shipped) makes remote sends typed and FSM-driving. Do not ship that sentence verbatim.

**Primary recommendation:** Sequence phase-12 work as (1) REL-01 doc+test commit [touches `.rs`, triggers CI], (2) REL-02 adversarial review [report is `.md`, may or may not need a code fix], (3) REL-04 hygiene fixes [pure `.md`/`.planning`, no CI trigger — do this **before** the version bump, not after], (4) REL-05 version bump [touches `Cargo.toml`/`Cargo.lock`/`crates/famp/src/cli/mod.rs`/`README.md` — triggers CI], confirm that commit's CI run green via `gh api`, then tag `v1.0.0` at that exact SHA.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Send-confirmation documentation (REL-01) | Docs / CLI help text | Test (`famp` crate, doc-accuracy) | Pure documentation + a regression test pinning it; no runtime behavior changes |
| Post-fix trust-boundary review (REL-02) | API/Backend (`famp-gateway` egress/ingress) | Database/Storage n/a | Source-level review of the federation trust boundary already implemented in Phase 11; any fix lands in `famp-gateway` |
| CI-green-at-tag-commit attestation (REL-03) | CI/CD (`.github/workflows/ci.yml`) | — | Process/release-engineering concern, not application code |
| Release-record hygiene (REL-04) | Docs/Process (`REQUIREMENTS.md`, `ROADMAP.md`) | — | Pure documentation consistency, zero runtime surface |
| Version bump + tag (REL-05) | Build/Release (`Cargo.toml`, `Cargo.lock`, CLI banner) | Docs (`README.md`, tag annotation) | Release-engineering; touches the CLI's compiled-in version string |

This phase has **no Browser/Client, Frontend-SSR, or CDN tier involvement whatsoever** — everything is CLI/backend/release-process. Flag to the plan-checker: if any plan proposes new federation runtime logic outside REL-02's fix-if-needed scope, that is a tier/scope violation of the phase's own hard constraint ("No federation logic changes unless REL-02 surfaces a real defect").

## Package Legitimacy Audit

**N/A — this phase installs no new external packages.** No `Cargo.toml` dependency additions are anticipated; the only `Cargo.toml` edits are the workspace `version` bump (`1.0.0-rc.1` → `1.0.0`) and its ripple to internal path-dependency `version = "..."` pins (all first-party crates already in the workspace — see REL-05 detail below). Skip the Package Legitimacy Gate protocol entirely for this phase.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REL-01 | Doc states what `send` confirms; `gateway_setup_doc_accuracy.rs` pins it | §"REL-01" below: exact exit-code semantics traced through `send/mod.rs` and `egress.rs`; exact insertion points in `docs/GATEWAY-SETUP.md` §6, `SendArgs` doc comments, README; existing test file structure to extend (not create) |
| REL-02 | Independent post-fix adversarial review of shipped trust boundary | §"REL-02" below: exact file:line bounded surface (egress.rs, ingress.rs, handle.rs, main.rs) for the reviewer; D-02's pre-fix claim, quoted, to distinguish "already reconciled" from "needs re-litigating" |
| REL-03 | CI green **at the exact tag commit** | §"REL-03" below: `ci.yml`'s job list, the `paths-ignore` trap (proven with `gh api` evidence), the corrected CI-green commit (`269b748`, not `ca59f48`), the `gh` incantations to verify any candidate commit |
| REL-04 | Release-record hygiene | §"REL-04" below: exact file:line of all three hygiene defects, each independently confirmed via `grep`/`git log` |
| REL-05 | Version bump + tag | §"REL-05" below: exhaustive grep of every `1.0.0-rc.1` occurrence (13 `Cargo.toml` files + root + `cli/mod.rs` const+test + `README.md` + `docs/GETTING-STARTED.md`); prior-tag annotation convention (quoted); §16's checklist verbatim for the tag body |
</phase_requirements>

## REL-01 — Send-Confirmation Documentation

### What a remote `famp send`'s zero exit code actually confirms (traced from source)

`famp send`'s CLI entry point (`crates/famp/src/cli/send/mod.rs:152-157`, `run()`) calls `run_at_structured()`, prints the JSON-Line outcome, and returns `Ok(())` **as soon as the local broker returns `BusReply::SendOk`** (`send/mod.rs:368`). For a remote-addressed send (`--to agent:<domain>/<name>`), that `SendOk` means the envelope was accepted into the **local UDS broker's mailbox for the gateway-backed proxy holder** — the same proxy connection `famp-gateway` uses to back that remote principal (Phase 7's `ProxiedPrincipal`/D-10 mechanism). It does **not** mean the envelope has left the machine.

Actual relay is a fully decoupled background process:
- `crates/famp-gateway/src/egress.rs::run_egress` (`egress.rs:315`) is an independent async loop per backed principal that re-acquires the registry lock, issues a **short (1000ms, `AWAIT_TIMEOUT_MS`, `egress.rs:38`) `Await`** against that principal's own mailbox, and only THEN signs (`sign_federation_fields`, `egress.rs:109`) and HTTPS-POSTs to the remote gateway (`relay_one`, `egress.rs:248`).
- This loop has **no connection whatsoever** to the CLI process that called `famp send` — the CLI has already exited by the time the drain loop even wakes up to notice the new envelope.
- Verification/delivery into the *remote* agent's real mailbox happens even later, inside the remote gateway's `ingress.rs::inbox_handler` (`ingress.rs:225`), after a full network round trip.

**Precise fire-and-forget boundary:** the zero exit code of `famp send --to agent:<domain>/<name>` confirms **"accepted by the local broker into the gateway-backed proxy's outbound mailbox"** — nothing more. It does NOT confirm: (a) the gateway has drained/signed/POSTed it yet, (b) the remote gateway received or verified it, (c) it landed in the real remote agent's mailbox, or (d) the task FSM advanced on the remote side. `famp inspect tasks --id <task_id> --json` on **both** hosts is the only way to confirm end-to-end delivery — which is exactly what `docs/GATEWAY-SETUP.md` §6 already instructs (`docs/GATEWAY-SETUP.md:258-269`), but without ever stating what the send's own exit code represents by itself. `[VERIFIED: source, crates/famp/src/cli/send/mod.rs:152-401, crates/famp-gateway/src/egress.rs:38,109,248,315]`

Note: this fire-and-forget characterization is symmetric for **local** sends too (a local `famp send` to a bare name also just confirms local-mailbox acceptance, not that the recipient process has read it) — but the local case has no cross-host trust/network hop, so the ambiguity is lower-stakes. The documentation gap specifically matters for the federated path because a user could reasonably assume "it printed success, so my peer got it," which is false until the drain loop runs.

### Exact insertion points

1. **`docs/GATEWAY-SETUP.md` §6 "Connect / verify"** (`docs/GATEWAY-SETUP.md:249-269`) — this section already tells the user to run `famp send`, then separately check `famp inspect tasks`. Add one paragraph, directly after the `famp send --to agent:hostB.example/bob --new-task ...` code block (after line 256) and before "Then confirm the task's FSM reached a terminal state" (line 258), stating explicitly: *`famp send`'s exit code / JSON-Line output confirms only that the local broker accepted the envelope into the gateway's outbound mailbox — it does not confirm remote delivery, signature verification, or FSM advancement. The `famp inspect tasks` check below is what actually confirms end-to-end delivery.* This is the natural home — it's already the section that walks through send-then-verify.
2. **`famp send` clap help text** — `SendArgs`'s doc comments become the `--help` text verbatim (clap convention used throughout this codebase — see `crates/famp/src/cli/send/mod.rs:60-109`). Add a doc comment above the struct or above the `to` field noting the fire-and-forget boundary in one line (clap help text should stay terse — one sentence, not the full explanation).
3. **README** — README currently has **no dedicated remote-path section at all**. The only remote-send mentions are prose fragments in the "Not Shipped Yet" list (`README.md:78`, which is now **stale/inaccurate** — it still lists `famp-gateway bridging the local bus to remote FAMP-over-HTTPS` as not-shipped, even though Phase 7-11 shipped exactly that) and scattered federation references (`README.md:11,15-16,27,31,78-79,170-172,285-291,634,715,718,738-739`). REQUIREMENTS.md's REL-01 text says "the README remote-path section" as if one exists — it does not. **Decision needed at plan time:** either (a) add a minimal new "Remote (federation) send" subsection to README with the confirmation-semantics sentence and a pointer to `docs/GATEWAY-SETUP.md`, or (b) correct the stale "Not Shipped Yet" entry and add a one-line pointer to `GATEWAY-SETUP.md` rather than duplicating full semantics in two places. Option (b) is cheaper and avoids doc drift between two copies of the same sentence — recommend (b), flagged here as `[ASSUMED]` since it's a judgment call, not verified against any stated convention.
4. **`crates/famp/tests/gateway_setup_doc_accuracy.rs`** — **this file already exists** (228 lines, created by Phase 11 plan 11-05 for DOC-05). REQUIREMENTS.md's phrasing ("A `crates/famp/tests/gateway_setup_doc_accuracy.rs` assertion pins the statement") describes **extending** this file, not creating it. The file's established pattern (`gateway_setup_doc_accuracy.rs:104-227`): whitespace-normalize the doc (`let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");`, line 112), then `assert!(normalized.contains("<exact anchor phrase>"), "update the guide or the code: <reason>")` for each semantic claim, always with a fail message starting `"update the guide or the code:"`. A new assertion block for REL-01 should follow this exact convention — assert the doc contains the chosen anchor phrase for the confirmation-semantics sentence (whatever exact wording is chosen at plan time), consistent with how findings #1-#8 were each pinned individually. `[VERIFIED: source, crates/famp/tests/gateway_setup_doc_accuracy.rs:1-227]`

## REL-02 — Post-Fix Adversarial Source Review

### Bounded surface (exact files/functions for the reviewer)

| Concern | File | Function/const | Lines |
|---------|------|-----------------|-------|
| Egress `from`-stamping / own-domain enforcement | `crates/famp-gateway/src/egress.rs` | `relay_one` (rejects foreign-domain `from` before signing), `sign_federation_fields` | `relay_one` ~L248-306; `sign_federation_fields` ~L109; `FromDomainMismatch` variant ~L221 |
| Ingress destination/domain validation | `crates/famp-gateway/src/ingress.rs` | `inbox_handler`, `envelope_federation_format_ok` | `inbox_handler` ~L225-290 (`MisaddressedRecipient` ~L259, `ForeignDomain` ~L272, `federation_format_ok` check ~L284); `envelope_federation_format_ok` ~L120-127 |
| Federation-owned-field ownership (one writer) | `crates/famp-gateway/src/egress.rs` | `FEDERATION_OWNED_FIELDS` const, `sign_federation_fields` | const ~L58-71 (7 fields: `from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry`, `capability`, `approval`); enforcement inside `relay_one` before `sign_federation_fields` runs |
| Route config parsing / fail-closed | `crates/famp-gateway/src/main.rs` | `GatewayArgs` parser, `build_route_map` | arg parsing ~L87-200 (duplicate `--backs` rejected ~L166-186); `build_route_map` ~L269-303 |
| Broker-side `from`-binding to authenticated identity | `crates/famp-bus/src/broker/handle.rs` | `send`, `is_self_authored` (via `drain_walk`) | `send` ~L378-437 (`effective_identity` resolve ~L392, `is_self_authored` gate ~L419) |

This is the **shipped** code at `v1.0.0-rc.1` (`ba6b166`) — the same functions independently re-run and confirmed by `11-VERIFICATION.md`'s SEC-01..04 rows (`11-VERIFICATION.md:38-46`). REL-02 is a **second, independent pass** over this exact surface looking for anything the first pass missed, not a re-confirmation of Phase 11's own claims.

### What D-02 (the pre-fix correction) actually said, and what "reconciliation" means

From `11-CONTEXT.md:33` (D-02, quoted verbatim): *"A `to`-only rewrite is INSUFFICIENT and strictly worse — ingress verifies on `from` (`crates/famp-gateway/src/verify.rs:62-63`, `peek_sender` returns `from`); a `local.bus` `from` → `UnpinnedKey`, trading `UnknownRecipient` for a symptom that looks like a trust-bootstrap bug. So a remote send MUST stamp `from = agent:{own-domain}/{identity}` too. (Proven by zed's source control during the dogfood.)"* This was zed's finding **before** the fix landed, and it directly drove the both-`from`-and-`to` rewrite (ADDR-01/ADDR-02). `11-VERIFICATION.md` truth #1 (`11-VERIFICATION.md:24`) confirms the shipped code does stamp both. So D-02 itself is already closed — **reconciliation for REL-02 means running a genuinely new adversarial pass against the shipped result**, not re-verifying D-02's original claim. §16 item 9 names this explicitly as "Zed's independent source verdict is reconciled" — the review's own confidence-assessment section (page 20-21 of the PDF, quoted in full below) lists open uncertainties that were NOT resolved by Phase 11 and are candidate focus areas for this pass: *"Whether the broker's stamped sender identity is available to gateway egress independently of client JSON," "Whether `BusMessage::Send` already separates mailbox target from envelope `to`," "Whether v0.5.2 explicitly requires globally qualified sender and recipient authorities," "Whether delivery failures are durable, retried, or dead-lettered," "Whether the gateway key is per gateway, per domain, or per agent," "Whether a user-facing terminal task workflow was independently committed for v1.0."* Every finding from this new pass must be triaged to fixed-in-code or documented-accept-with-rationale (per the phase's success criterion #2) — a fix, if any, is the ONLY circumstance under which this phase is allowed to touch federation logic (hard constraint).

## REL-03 — CI Green at the Exact Tag Commit

### `.github/workflows/ci.yml` job inventory

| Job | What it covers | Matrix |
|-----|-----------------|--------|
| `fmt-check` | `cargo fmt --all -- --check` | ubuntu |
| `clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | ubuntu |
| `build` | `cargo build --workspace --all-targets` + no-OpenSSL-in-dep-tree gate | ubuntu + macos |
| `test-canonical` | `just test-canonical-strict` — **the RFC 8785 / canonicalization gate** (SEED-001) | ubuntu |
| `test-crypto` | `just test-crypto` — Ed25519 §7.1c worked example + RFC 8032 | ubuntu |
| `test` (needs `test-canonical`, `test-crypto`) | `cargo nextest run --workspace --profile ci` — **includes `e2e_cross_host_delivery` and `e2e_shipping_surface`** (both live in `crates/famp-gateway/tests/`, bounded to `max-threads = 2` via the `gateway-subprocess` nextest test-group, `.config/nextest.toml:23-33`) | ubuntu + macos |
| `doc-test` | `cargo test --workspace --doc` | ubuntu |
| `audit` | `cargo audit` (prebuilt binary via `taiki-e/install-action`, switched 2026-07-29 from a source-build that had been broken 13+ consecutive runs — see `ci.yml:138-153` comment) | ubuntu |

`[VERIFIED: source, .github/workflows/ci.yml:1-154, .config/nextest.toml:1-39]`

### The `paths-ignore` trap — confirmed empirically, this is the phase's single biggest risk

`ci.yml:5-15` sets `paths-ignore: ["docs/**", ".planning/**", "**/*.md"]` on both `push` and `pull_request`. A commit (or a solo `git push` of one commit) that touches **only** files under those globs triggers **no workflow run at all** — not a green check, not a skipped check, literally zero `check-runs` objects for that SHA.

Live evidence (`gh api /repos/thebenlamm/FAMP/commits/<sha>/check-runs`, run 2026-07-29):

| Commit | Touches | Check-runs |
|--------|---------|------------|
| `269b748` (`fix: close UAT-01 follow-ups...`) | `.rs` source (F-A/F-B fixes) | **11 check-runs, ALL `success`** — fmt-check, clippy, build×2, test×2, doc-test, audit, canonical, crypto, smoke-test |
| `ca59f48` (`docs(phase-11): evolve PROJECT.md...`) | docs-only | **ZERO check-runs** |
| `73085aa` (`docs(roadmap): add Phase 12...`) — current `HEAD` | docs-only | **ZERO check-runs** |

**REQUIREMENTS.md line 89 ("`v1.0.0-rc.1` sits on `ba6b166` while CI's green run is on `ca59f48`") is factually incorrect** — `ca59f48` was never run through CI. The correct, current fact: **the last commit with a real, fully-green CI run is `269b748`**; every commit after it (including `ba6b166`'s successor commits `f8506cb`... wait, `ba6b166` predates `269b748`; the chain is `ba6b166` → `f8506cb`(also has code, check separately if needed) → `d85c6d3` → `b8f8f32` → `269b748` → `ca59f48` → `73085aa`) has either a real failing/passing run (code commits) or no run at all (docs commits). Use `gh run list --commit <sha>` or the `check-runs` API for any candidate SHA before trusting a written claim about "CI is green here" — this exact mistake is already sitting in a locked requirements doc.

### `gh` incantations for the planner/executor

```bash
# Full run history for the ci.yml workflow, most recent first
gh run list --workflow=ci.yml --limit 15 --json databaseId,headSha,conclusion,displayTitle,createdAt

# Per-commit check-run status (works even for commits with no gh run list match,
# since it queries the Commits API directly rather than the Actions run list)
gh api "/repos/thebenlamm/FAMP/commits/<full-or-short-sha>/check-runs" \
  --jq '.check_runs[] | {name, status, conclusion}'

# Confirm a specific SHA is what a given branch/tag points at before trusting its CI status
git rev-parse <ref>
```

Record the `databaseId` (run ID) of the passing `ci` job run for the exact SHA that receives the `v1.0.0` tag in the phase record — REL-03's success criterion explicitly requires "the run IDs recorded in the phase record," not just a prose claim.

### The audit job — is it flaky, and how should this phase treat it?

Per `ci.yml:130-153`'s own comment (added 2026-07-29 in commit `269b748`, one of this phase's immediate predecessors): the `audit` job **was** broken for 13+ consecutive runs due to an upstream `cargo-audit` source-build failure (unrelated to any real code regression), and was just fixed by switching to a prebuilt binary via `taiki-e/install-action`. As of `269b748`'s run, `audit` reports `success`. **Recommendation: do not pre-emptively treat `audit` as flaky for this phase** — the fix already landed and the most recent run is green. If a future run in this phase turns red, first check whether it's a genuine new RustSec advisory (real, must triage) vs. a reintroduction of the build-failure mode (infra, not a code regression) — do not assume it's noise without checking `gh run view <id> --log` first, per the project's "measure before severity" rule.

## REL-04 — Release-Record Hygiene (three exact defects)

### (a) UAT-01's stale checklist box + traceability row

- `.planning/REQUIREMENTS.md:81` — checklist line still reads `- [ ] **UAT-01**: ...` (unchecked), despite `11-HUMAN-UAT.md` recording `verdict: PASS` (frontmatter, `11-HUMAN-UAT.md:9`) and `11-VERIFICATION.md` truth #7 confirming it (`11-VERIFICATION.md:30`).
- `.planning/REQUIREMENTS.md:153` — traceability table row still reads `| UAT-01 | Phase 11 | Pending |`.
- **Fix:** flip the checkbox to `- [x]` and the traceability status to `Complete`. Both are pure `.md` edits in `REQUIREMENTS.md`.

### (b) The dangling `ADDR-04` reference

- Origin: commit `04171bd99101027ce562ccbdbdc489a7a569c599`, subject line `docs(11-07): plan metadata + Report C findings (SEC-01, ADDR-04)` — confirmed via `git log --grep=ADDR-04 --all --oneline`, which returns exactly one commit. `ADDR-04` never existed as a defined requirement anywhere in `REQUIREMENTS.md`'s history.
- **This is already resolved in `.planning/REQUIREMENTS.md:49-56`** — a standing note (`> **`ADDR-04` does not exist — resolved, do not go looking for it.**`) already documents the slip, names the commit, and explains the trust-boundary work it referred to is covered by `SEC-01`. This note appears to have been added as part of the same prep work that scoped Phase 12 (REQUIREMENTS.md's footer says "Last updated: 2026-07-29 — Phase 12 ... added").
- **What remains for the planner to decide:** whether this existing note fully discharges REL-04(b), or whether the `11-VERIFICATION.md:112` WARNING text itself (which still describes it as "an unresolved dangling reference — low-impact, informational only") should also be updated to point at the resolution note, so a future reader of the verification report doesn't re-open a closed question. Recommend a one-line addendum to `11-VERIFICATION.md`'s WARNING pointing at `REQUIREMENTS.md:49-56` — cheap, closes the loop completely. `[VERIFIED: source, git log --grep, .planning/REQUIREMENTS.md:49-56, 11-VERIFICATION.md:112]`

### (c) Phase 11's ROADMAP entry — missing plan 11-08, unchecked top-level phase box

- `.planning/ROADMAP.md:28` — the top-level Phases checklist still shows `- [ ] **Phase 11: Shipping-Client Remote Addressing + Setup Hardening**` (unchecked), even though `11-VERIFICATION.md` records `status: passed`, `score: 7/7 must-haves verified` (`11-VERIFICATION.md:3-5`).
- `.planning/ROADMAP.md:269-289` (the `### Phase 11` detail section) — its "Plans:" list enumerates Wave 1 (`11-01`, `11-02`), Wave 2 (`11-03`, `11-07`), Wave 3 (`11-04`, `11-05`), Wave 4 (`11-06`) — **`11-08` never appears**, even though `11-08-PLAN.md`/`11-08-SUMMARY.md` exist on disk (`.planning/phases/11-.../`), STATE.md's decision log cites `[11-08]` four times (STATE.md:144-147), and `11-VERIFICATION.md`'s requirements-coverage table attributes `SEC-01..04` to `"11-07, 11-08"` (`11-VERIFICATION.md:104`). ROADMAP.md's own summary line even says `"**Plans:** 7 plans"` (`ROADMAP.md:269`) — should be 8.
- **Fix:** add `11-08-PLAN.md` to the Wave list (it depends on 11-07 per the trust-boundary sequencing — SEC-02/03/04 build on SEC-01), correct the "7 plans" count to 8, and check the top-level Phase 11 box.
- **Related but out-of-scope finding (flag, don't fix under REL-04):** Phase 9 (`ROADMAP.md:26`) and Phase 10 (`ROADMAP.md:27`) top-level boxes are **also** unchecked despite being fully complete (all their sub-plans show `[x]`) — this is a broader staleness pattern in the top-level Phases list, not just a Phase 11 issue. REL-04's text scopes explicitly to "Phase 11's ROADMAP entry" — recommend fixing only Phase 11's box under REL-04, and separately flagging Phase 9/10 as a quick follow-up (or fixing all three in the same commit if the executor judges it in-scope-enough; either is defensible, but Phase 11 is the one REL-04 actually requires).

## REL-05 — Version Bump + Tag

### Every location the `1.0.0-rc.1` string appears (must all become `1.0.0`)

**Root:** `Cargo.toml:25` — `[workspace.package] version = "1.0.0-rc.1"`. All 15 member crates use `version.workspace = true` (confirmed via `grep -rn "^version" crates/*/Cargo.toml`), so **member-crate `[package]` blocks need no edit** — bumping the root is sufficient for their own version.

**But every internal path-dependency declaration pins an explicit version string too** (a Cargo convention for path deps that also declare a publishable version) — these do NOT auto-follow the workspace bump and must be edited individually:

| File | Count of `version = "1.0.0-rc.1"` occurrences |
|------|------|
| `crates/famp/Cargo.toml` | 14 |
| `crates/famp-gateway/Cargo.toml` | 8 |
| `crates/famp-transport-http/Cargo.toml` | 7 |
| `crates/famp-inspect-server/Cargo.toml` | 5 |
| `crates/famp-bus/Cargo.toml` | 4 |
| `crates/famp-keyring/Cargo.toml`, `crates/famp-envelope/Cargo.toml`, `crates/famp-inspect-client/Cargo.toml` | 2-3 each |
| `crates/famp-fsm/Cargo.toml`, `crates/famp-crypto/Cargo.toml`, `crates/famp-transport/Cargo.toml`, `crates/famp-inspect-proto/Cargo.toml` | 1 each |

A single global find/replace (`1.0.0-rc.1` → `1.0.0`) across all `Cargo.toml` files in the repo is the correct, low-risk approach — every occurrence found is a version pin, none are unrelated strings.

**Compiled-in banner + its own pinning regression test:** `crates/famp/src/cli/mod.rs:40` — `const BANNER_ABOUT: &str = "FAMP 1.0.0-rc.1 (spec v0.5.2)";` and a `#[cfg(test)]` test `version_strings_unified` (`cli/mod.rs:232-243`) that **hard-asserts** `env!("CARGO_PKG_VERSION") == "1.0.0-rc.1"` and `BANNER_ABOUT.contains("1.0.0-rc.1")`. **This test will fail after the Cargo.toml bump unless its own literals are updated in the same commit** — do not bump the workspace version without updating both the const and the test in one atomic commit, or CI's `test` job will go red on this specific assertion (which is actually the intended safety net — VER-02's whole purpose — just make sure the plan accounts for updating it, not "discovers" the resulting red CI as a surprise).

**Docs with the literal string:** `README.md:12` ("...is unified to `1.0.0-rc.1` (`famp -V` → `famp 1.0.0-rc.1`)") and `docs/GETTING-STARTED.md:53` (`# famp 1.0.0-rc.1` example output). Both need updating to `1.0.0`.

**`Cargo.lock`:** will auto-regenerate on the next `cargo build`/`cargo check`/`cargo test` after the `Cargo.toml` edits — confirm it's committed with the updated `version = "1.0.0"` lines (currently 13+ `famp-*` crate entries pin `1.0.0-rc.1` in `Cargo.lock`, e.g. lines 655, 703, 721, 732, 742, 761, 776, 787, 812, 823, 838, 848, 864 as of this research). Do not hand-edit `Cargo.lock` — let `cargo` regenerate it.

**Deployed-binary staleness (Runtime State Inventory item, see below):** the actual `~/.cargo/bin/famp` / `~/.cargo/bin/famp-gateway` binaries on Ben's machines (per `11-HUMAN-UAT.md`'s topology: `bens-macbook-air` + `home-devbox`) were built at commit `0184f01` / SHA-256-pinned in that report — they will continue reporting `1.0.0-rc.1` until someone runs `just install-all` (or the raw `cargo install --path ... --locked --force` equivalent on the devbox, which lacks `just`) again post-bump. Not required by REL-05's success criteria (which is about the tag, not live redeployment), but worth a one-line callout in the phase record so it isn't mistaken for a bug later.

### Prior-tag annotation convention (quoted, so the planner doesn't have to `git show` five tags)

Every prior milestone tag (`v0.9`, `v0.11`, `v1.0.0-rc.1`) follows the same annotated-tag shape: a one-line title, a `Delivered:` paragraph, a `Key accomplishments:` bullet list, a phase/plan/requirement count line, any deferred/known-gap callout, and (for the older tags) a pointer to `.planning/MILESTONES.md`. `v1.0.0-rc.1`'s annotation (`git show v1.0.0-rc.1`, full text captured during this research) additionally ends with: *"v1.0.0 proper still requires design review C's section 16 nine-item checklist."* — the `v1.0.0` tag should close that loop explicitly.

**REL-05's success criterion requires the §16 checklist reproduced in the tag annotation** — not just referenced. Reproduce it verbatim (see next section) as a checklist with each item marked satisfied and its evidence citation, inside the tag body.

### §16 "Exact release ruling" — verbatim, so the planner never has to re-open the 21-page PDF

From `DESIGN-REVIEW-C-final.pdf`, page 18-19, section "16. Exact release ruling":

> **Current commit — Do not ship v1.0.0.** "A release whose central new plane cannot be addressed by any released client is not a release with a limitation. It is a release whose primary feature is unreachable."
>
> **After C5 — Ship a release candidate first:** `v1.0.0-rc.1`. Run the documented two-machine test.
>
> **Tag `v1.0.0` only when:**
> 1. the shipping client accepts a complete remote principal;
> 2. the signed envelope contains globally qualified `from` and `to`;
> 3. `from` is bound to broker-authenticated identity;
> 4. no `local.bus` authority crosses federation;
> 5. the remote gateway verifies and delivers the same signed bytes;
> 6. existing canonicalization and E2E gates remain green;
> 7. ambiguous route configuration fails closed;
> 8. the documentation states what `send` confirms;
> 9. Zed's independent source verdict is reconciled.
>
> **Then ship with this limitation if necessary:** *"`famp send` demonstrates signed, fire-and-forget federated envelope delivery. It does not initiate or complete the task FSM; federated task initiation is not exposed through the v1.0 client interface."* "That is a real, bounded limitation. 'No shipping client can address a remote principal' is not."

**This proposed limitation sentence is stale and must NOT ship verbatim** — it was written before Phase 11's ADDR-02, which makes remote sends typed and FSM-driving (`--new-task`→Request, `--task`→Commit, `--task --terminal`→Deliver+terminal_status; live-proven in `11-HUMAN-UAT.md` §4, task `019fab97-d3e0-7d63-92ba-39f1ce171b83` reaching `COMPLETED` on both hosts). The "does not initiate or complete the task FSM" clause is now **false**. The "fire-and-forget… demonstrates signed… delivery" framing is still **true** at the CLI-exit-code level (see REL-01 above — the exit code still doesn't confirm remote/FSM completion, even though the FSM itself IS reachable through the remote path). Recommendation: either drop the limitation sentence entirely (since the FSM gap it described no longer exists) or replace it with the precise fire-and-forget-exit-code statement from REL-01 — do not reuse the old sentence unedited. This is exactly what REL-05's own success criterion #6 already instructs; this research confirms the staleness is real by tracing ADDR-02's shipped behavior, not just repeating the requirement text.

### §16's confidence-assessment appendix (page 20-21) — useful REL-02 scope-check

"Very high confidence" items are all already satisfied by Phase 11 (C1/C4 addressing model settled, complete recipient principals required, shipping-client E2E mandatory, current-commit-should-not-tag). "High confidence" items (gateway derives sender from trusted local identity + configured domain; federation-owned fields have one writer; peer/name cross-product replaced) map 1:1 to SEC-01/03/04, also already shipped. The "still requires source or spec confirmation" list (quoted in full under REL-02 above) is the actual open surface for REL-02's new adversarial pass — the review's own words: *"None of those uncertainties justify C1 or justify shipping the current implementation. They determine the exact patch shape around the same recommended model."* i.e., these are refinement questions, not blockers discovered after the fact — treat any REL-02 finding in this territory as documented-accept unless it surfaces an actual defect.

## Sequencing Constraint (the phase's single biggest execution risk)

**The problem:** REL-03 requires CI green **at the exact commit that receives the `v1.0.0` tag**. REL-01 (doc + test) and REL-04 (hygiene) both add commits. If REL-04's hygiene commit (pure `.md`/`.planning` edits) is the last commit before tagging, **no CI run will ever exist for that SHA** — REL-03 becomes literally unsatisfiable by any CI evidence, not just difficult.

**Recommended ordering:**
1. **REL-01** — extend `crates/famp/tests/gateway_setup_doc_accuracy.rs` (a `.rs` file, not ignored) in the SAME commit as the `docs/GATEWAY-SETUP.md`/README edits. Bundling a doc change with its pinning test in one commit is already this repo's established pattern (every Phase-11 `DOC-05`/`TEST-03` commit did this). This commit **will** trigger CI — confirm green.
2. **REL-02** — run the adversarial review. If it surfaces a real defect, the fix is `.rs` — commit and confirm CI green. If no defect (accept-with-rationale only), the review's own writeup can be `.md` and ride along in a later bundle; it doesn't need its own CI-triggering commit.
3. **REL-04** — hygiene fixes are pure `.md`/`.planning` (`REQUIREMENTS.md`, `ROADMAP.md`, `11-VERIFICATION.md` addendum). Commit these **before** the version bump, not after — they will not trigger CI, and that's fine as long as they aren't the last commit before the tag.
4. **REL-05** — the version bump touches `Cargo.toml`, `Cargo.lock`, `crates/famp/src/cli/mod.rs` (const + test) — all non-ignored paths. This is naturally the **last** commit and **will** trigger CI. Confirm this exact commit's run is green via `gh api .../check-runs` (or `gh run list --commit <sha>`), record the run ID(s) in the phase record, THEN create the annotated tag pointing at that SHA.

If the executor's workflow pushes commits one at a time (per this project's standing convention — "commit after each logical change... push freely"), each push's `paths-ignore` evaluation applies to that push's own diff, so this ordering is safe under normal single-commit-per-push behavior. If multiple commits are ever batched into one `git push`, GitHub evaluates `paths-ignore` against the **union** of all files changed in that push — bundling a docs-only commit together with a code commit in one push could accidentally "ride along" and get a CI run anyway, but do not rely on this; the ordering above is correct regardless of push batching.

## Common Pitfalls

### Pitfall 1: Trusting a written "CI is green on commit X" claim without re-checking
**What goes wrong:** REQUIREMENTS.md itself contains a false claim ("CI's green run is on `ca59f48`") that this research disproved with a direct API call.
**Why it happens:** `ca59f48` post-dates the last real CI run (`269b748`) chronologically, so it's easy to assume "the latest thing" is also "the latest verified thing" — but `paths-ignore` breaks that assumption silently.
**How to avoid:** Always re-verify the exact tag-candidate SHA's check-runs via `gh api /repos/<owner>/<repo>/commits/<sha>/check-runs` immediately before tagging, never trust a prior doc's claim.
**Warning signs:** A commit whose diff is entirely under `docs/`, `.planning/`, or `*.md`.

### Pitfall 2: Bumping `Cargo.toml`'s workspace version without updating `version_strings_unified`'s literals in the same commit
**What goes wrong:** CI's `test` job goes red on a self-inflicted assertion failure that looks like a real regression.
**Why it happens:** The version string is duplicated in three places (Cargo.toml, the `BANNER_ABOUT` const, and the test that pins both) by design (VER-02's whole point is to catch drift) — but that means a bump requires touching all three atomically.
**How to avoid:** Grep for the old version string across the whole repo (not just `Cargo.toml`) before considering the bump commit done; the exhaustive list is in REL-05 above.
**Warning signs:** `cargo test -p famp cli::mod::tests::version_strings_unified` failing after a `Cargo.toml`-only edit.

### Pitfall 3: Shipping §16's proposed limitation sentence unedited
**What goes wrong:** The `v1.0.0` release notes / tag annotation ship a factually false claim ("does not initiate or complete the task FSM") that a careful reader (or a future adversarial reviewer) will catch immediately, undermining trust in the rest of the release notes.
**Why it happens:** It's tempting to copy the review's own suggested wording verbatim since it's already written and "approved-sounding."
**How to avoid:** Cross-check any proposed limitation statement against the actual shipped behavior (ADDR-02, `11-HUMAN-UAT.md`) before it ships — this is exactly REL-05's own success criterion #6, already flagging this risk in the requirements text.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo-nextest` (workspace-wide) + `cargo test` for doctests |
| Config file | `.config/nextest.toml` (test-group concurrency bounds; `gateway-subprocess` group caps `famp-gateway` integration tests at 2 concurrent) |
| Quick run command | `cargo test -p famp --test gateway_setup_doc_accuracy` (REL-01); `cargo test -p famp cli::mod::tests::version_strings_unified` (REL-05) |
| Full suite command | CI-only: `.github/workflows/ci.yml`'s `test` job (`cargo nextest run --workspace --profile ci`). **`just ci` / full local `cargo test --workspace` are documented-unusable on this machine** (nextest `--list`-phase hang, `project_nextest_list_hang`) — CI is the only admissible full-suite evidence, exactly as REL-03's own text states. |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REL-01 | Doc states send-confirmation semantics; pinned against drift | integration (doc-accuracy) | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✅ exists — extend with a new assertion block |
| REL-02 | Adversarial trust-boundary review of shipped code | source review (not a compiled test); any resulting fix gets its own unit/integration test in `crates/famp-gateway/src/egress.rs` / `ingress.rs` tests | N/A for the review itself; `cargo test -p famp-gateway --lib` + `cargo test -p famp-gateway --test inbound_destination_validation --test route_config_fail_closed` for any fix's regression coverage | ✅ existing test files cover the fix surface if a fix is needed |
| REL-03 | CI green at the exact tag commit | CI-run attestation (GitHub Actions), not a local test | `gh api /repos/thebenlamm/FAMP/commits/<sha>/check-runs` | N/A — process evidence, not a repo test file |
| REL-04 | Release-record hygiene (3 defects) | manual doc edit + review (human/agent-verified, not compiled) | `grep -n "UAT-01" .planning/REQUIREMENTS.md`; `grep -n "11-08" .planning/ROADMAP.md` as a manual sanity check | N/A — no automated gate exists or is warranted for prose hygiene |
| REL-05 | Version bump consistent everywhere; tag created | regression test (`version_strings_unified`) + manual `git tag`/`famp -V` check | `cargo test -p famp cli::mod::tests::version_strings_unified`; `famp -V` (after `just install-all`); `git tag -v v1.0.0` (if GPG-signed; this repo's prior tags are NOT GPG-signed — plain annotated tags, confirmed via `git cat-file -p` on `v0.9`/`v0.11`) | ✅ test exists — literals need updating, not new test infra |

### Sampling Rate
- **Per task commit:** the specific targeted `cargo test -p ...` command from the table above for whatever REL-item that commit closes.
- **Per wave merge:** `cargo clippy --workspace --all-targets -- -D warnings` (`just lint`) + the two named e2e tests: `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery`.
- **Phase gate:** the version-bump commit's real CI run (all 8 jobs) must show `success` before the `v1.0.0` tag is created — verified via `gh api`, run ID recorded in the phase record, per REL-03.

### Wave 0 Gaps

None — every test file this phase needs already exists and is already wired into CI:
- `crates/famp/tests/gateway_setup_doc_accuracy.rs` (extend, don't create)
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`, `e2e_shipping_surface.rs`, `inbound_destination_validation.rs`, `route_config_fail_closed.rs` (regression net for any REL-02 fix)
- `.github/workflows/ci.yml` (all 8 jobs already defined)
- `.config/nextest.toml` (concurrency bounds already tuned for the gateway subprocess tests)

No framework installs, no new fixtures, no new CI jobs needed for this phase.

## Runtime State Inventory

**Trigger:** the version bump (`1.0.0-rc.1` → `1.0.0`) is a string-replacement operation across the repo, which is this template's trigger condition. Answered explicitly per category:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **None.** No datastore (mailbox JSONL, keyring, taskdir) persists the version string as a key, ID, or content field. | None |
| Live service config | **None.** The launchd/systemd daemon units invoke `~/.cargo/bin/famp broker --no-idle-exit` by path, not by version-pinned string; no config embeds `1.0.0-rc.1`. | None |
| OS-registered state | **None.** No Task Scheduler/launchd/systemd registration references the version string. | None |
| Secrets/env vars | **None.** `FAMP_OWN_DOMAIN` and friends are unrelated to the release version. | None |
| Build artifacts / installed packages | **Yes — real, but non-blocking.** `~/.cargo/bin/famp` and `~/.cargo/bin/famp-gateway` on Ben's two dogfooded machines (per `11-HUMAN-UAT.md`'s topology) are compiled from pre-bump source and will keep reporting `famp 1.0.0-rc.1` via `famp -V` until reinstalled. `Cargo.lock` is also a build artifact that must regenerate (not hand-edit) after the `Cargo.toml` bump. | Run `just install-all` (Mac) / the raw `cargo install --path crates/famp --locked --force` + `cargo install --path crates/famp-gateway --locked --force` (devbox, no `just`) post-tag if live redeployment matters to Ben; not required by REL-05's own success criteria, which only govern the tag, not live binaries. Flag this as a note in the phase record so it isn't later mistaken for "the bump didn't work." |

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `gh` CLI | REL-03 (CI status queries) | ✓ | authenticated as `thebenlamm` | — |
| `git` | all REL items | ✓ | — | — |
| `cargo` / `cargo-nextest` | REL-01/02/05 targeted tests | ✓ (targeted `cargo test -p <crate>` commands work; full-workspace `cargo nextest run` / `just ci` hangs locally per documented `project_nextest_list_hang` — use CI for full-suite evidence) | — | CI (GitHub Actions) is the fallback for full-suite verification |
| `just` | convenience recipes (`just lint`, `just install-all`) | ✓ on the Mac; **not installed on `home-devbox`** (per `11-HUMAN-UAT.md` §2 — raw `cargo install` commands were substituted there) | — | raw `cargo`/`cargo install` equivalents, already proven to work |

No missing dependencies block this phase's execution.

## Security Domain

`security_enforcement` is absent from `.planning/config.json` → treated as enabled.

### Applicable ASVS Categories

This phase adds **no new runtime code paths** (unless REL-02 surfaces a defect, in which case the fix inherits whatever ASVS category the existing trust-boundary code already maps to). No new ASVS surface is introduced.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No (unchanged) | Ed25519 identity via TOFU keyring — already shipped, not touched this phase |
| V3 Session Management | No | N/A — no session concept changes |
| V4 Access Control | Conditionally (only if REL-02 finds a defect) | `from`/`to` domain binding at egress/ingress (SEC-01/02) — already shipped; REL-02 is a review of this surface, not a rebuild |
| V5 Input Validation | Conditionally (only if REL-02 finds a defect) | Envelope/`Principal` parsing via the canonical `famp_core::Principal` type — already shipped |
| V6 Cryptography | No (unchanged) | Ed25519 via `ed25519-dalek`, `FAMP-sig-v1\0` domain prefix — already shipped, never hand-rolled, not touched this phase |

### Known Threat Patterns Already Mitigated (REL-02's actual scope — for orientation, not new work)

| Pattern | STRIDE | Standard Mitigation (already shipped, Phase 11) |
|---------|--------|---------------------------------------------|
| Spoofed `from` domain at egress | Tampering / Spoofing | SEC-01 — `egress.rs::relay_one`'s `FromDomainMismatch` check before signing |
| Open relay / misaddressed mailbox at ingress | Tampering / Elevation of Privilege | SEC-02 — `ingress.rs::inbox_handler`'s `MisaddressedRecipient` + `ForeignDomain` checks |
| Client-injected federation-owned fields | Tampering | SEC-03 — `egress.rs`'s `FEDERATION_OWNED_FIELDS` pre-check, gateway is sole writer |
| Ambiguous/last-write-wins route config | Spoofing (wrong destination) | SEC-04 — `main.rs::build_route_map`, duplicate `--backs` fails startup |
| Local agent impersonating another local identity in `from` | Spoofing | broker `handle.rs::send`'s `is_self_authored` gate |

REL-02's adversarial pass should treat this table as "confirm these still hold under adversarial pressure against the shipped code," not as a to-do list — none of these are open gaps per `11-VERIFICATION.md`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | README should get a corrected pointer to `GATEWAY-SETUP.md` rather than a duplicated confirmation-semantics sentence (REL-01 insertion-point option (b) over (a)) | REL-01, "Exact insertion points" item 3 | Low — either choice satisfies REL-01's literal text ("README remote-path section states..."); the risk is only doc-drift between two copies of the same sentence if option (a) is chosen instead and the two ever diverge later |
| A2 | `11-VERIFICATION.md`'s ADDR-04 WARNING should get a one-line addendum pointing at REQUIREMENTS.md's existing resolution note, rather than the existing note alone being sufficient | REL-04(b) | Low — cosmetic either way; both leave the underlying fact (ADDR-04 never existed, commit-message slip) equally well-documented |
| A3 | Phase 9/10's also-unchecked top-level ROADMAP boxes are out of REL-04's literal scope and should only be flagged, not fixed, under this phase | REL-04(c) | Low — fixing them too is harmless and arguably tidier; leaving them is also defensible since REL-04's text names only Phase 11 |

**All three are LOW-risk judgment calls, not factual uncertainties** — every load-bearing factual claim in this document (exit-code semantics, CI paths-ignore behavior, exact file:line locations, §16's verbatim text, the version-string occurrence list) was directly verified via source reads, live `git`/`gh` commands, or the PDF itself, not assumed.

## Open Questions

1. **Does REL-02's adversarial review need to be a separate sub-agent/external pass, or can it be performed in-line during plan execution?**
   - What we know: Phase 11 used external design-review passes (zed, a second and third external reviewer) for the pre-fix analysis. REL-02's own text says "independent, source-grounded adversarial review."
   - What's unclear: whether "independent" requires a literal separate reviewing agent/session (mirroring Phase 11's `matt-essentialist`/Fable-5/external-AI pattern per this project's general practice) or whether a rigorous self-review against the bounded surface table above satisfies the requirement, given six of nine §16 items are already closed and the remaining review surface is narrow.
   - Recommendation: given the project's general "Agent-First Methodology" preference for independent adversarial review on judgment-weight decisions, and that this is the FINAL gate before an outward-facing `v1.0.0` tag, recommend treating REL-02 as a `checkpoint:human-verify`-adjacent task that explicitly invokes a second reviewing perspective (matching Phase 11's own pattern for SEC-01..04's origin) rather than a same-agent self-check.

2. **Should `.planning/MILESTONES.md` gain a `## v1.0 Federation Profile` entry as part of this phase, or is that a separate milestone-close step?**
   - What we know: every prior shipped milestone (`v0.9`, `v0.11`, etc.) has a `MILESTONES.md` section, and prior tag annotations point to it ("See .planning/MILESTONES.md for full details").
   - What's unclear: whether REL-05's success criteria implicitly require this (not explicitly named in REQUIREMENTS.md's REL-05 text) or whether it's handled by a separate `/gsd-complete-milestone` step after Phase 12 ships.
   - Recommendation: leave `MILESTONES.md` out of Phase 12's plan scope — REL-05's literal text only requires the version bump + tag + §16-in-annotation; a milestone-close pass is a natural follow-up, not a blocker for this phase.

## Sources

### Primary (HIGH confidence — direct source reads / live command output this session)
- `crates/famp/src/cli/send/mod.rs` (full file structure, `SendArgs`, `run`/`run_at_structured`, output shape)
- `crates/famp-gateway/src/egress.rs`, `ingress.rs`, `main.rs` (exact function/line locations for REL-02's bounded surface)
- `crates/famp-bus/src/broker/handle.rs` (`send`, `is_self_authored`)
- `crates/famp/tests/gateway_setup_doc_accuracy.rs` (existing test pattern to extend)
- `docs/GATEWAY-SETUP.md`, `README.md`, `docs/GETTING-STARTED.md` (exact insertion points, version-string occurrences)
- `.github/workflows/ci.yml`, `.config/nextest.toml` (CI job inventory, paths-ignore, test-group bounds)
- `Cargo.toml` + all 15 `crates/*/Cargo.toml` (version-string occurrence count)
- `git log`, `git tag -l`, `git show v1.0.0-rc.1`, `git cat-file -p v0.9/v0.11`, `git log --grep=ADDR-04` (live command output)
- `gh auth status`, `gh run list`, `gh api .../check-runs` (live GitHub Actions state — the paths-ignore finding)
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md` (locked scope, traceability defects)
- `.planning/phases/11-.../11-VERIFICATION.md`, `11-CONTEXT.md`, `11-HUMAN-UAT.md` (evidence base, D-02 quote, UAT-01 live record)
- `.planning/phases/11-.../DESIGN-REVIEW-C-final.pdf` pages 15-21 (§16 verbatim, confidence-assessment appendix)

### Secondary / Tertiary
None — every claim in this document traces to a primary source read or a live command this session; no WebSearch was needed since this phase introduces no new external libraries or unfamiliar technology.

## Metadata

**Confidence breakdown:**
- REL-01 (send-confirmation semantics): HIGH — traced through actual source (`send/mod.rs`, `egress.rs`), not inferred
- REL-02 (bounded surface): HIGH — every file:line cited and cross-checked against `11-VERIFICATION.md`'s own independently-re-run test list
- REL-03 (CI-at-tag-commit): HIGH — the `paths-ignore` finding is directly falsifiable and was falsified/confirmed via live `gh api` calls, not assumed
- REL-04 (hygiene defects): HIGH — all three defects independently confirmed via `grep`/`git log`, not merely repeated from REQUIREMENTS.md's framing
- REL-05 (version bump + tag): HIGH — exhaustive grep of every occurrence; tag-annotation convention quoted from actual prior tags

**Research date:** 2026-07-29
**Valid until:** This research is tied to specific commit SHAs (`73085aa` HEAD, `269b748` last-green-CI, `ba6b166` rc.1) and live CI/GitHub state — re-verify the `gh api` check-runs evidence if more than a few days pass before this phase executes, since new commits will shift which SHA is "the last CI-verified one." The source-code file:line citations are stable unless REL-02 itself produces a fix that moves lines around (in which case re-grep before REL-05's final version-bump commit).
