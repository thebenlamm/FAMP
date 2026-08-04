# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- ✅ **v0.9 Local-First Bus** — Phases 1–4 + close-fix Phase 5 (shipped 2026-05-04). UDS-backed broker replacing the per-identity TLS listener mesh; zero crypto on the local path; IRC-style channels; durable per-name mailboxes; 8-tool stable MCP surface. 5/5 phases, 35 plans, **85/85 requirements**, audit `passed`. Federation internals (`famp-transport-http`, `famp-keyring`) preserved in CI via library-API `e2e_two_daemons`; escape-hatch tag `v0.8.1-federation-preserved`. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).
- ✅ **v0.10 Inspector & Observability** — Phases 1–3 (shipped 2026-05-11). Read-only inspector RPC on the v0.9 broker UDS + `famp inspect` CLI subcommand. Closes the conversation-state opacity gap that produced three recurring v0.9 incidents (orphan socket-holder vs stale PID file, task FSM invisibility, stale-mailbox relays). 26/26 requirements, audit `passed`. See [milestones/v0.10-ROADMAP.md](milestones/v0.10-ROADMAP.md) · [milestones/v0.10-REQUIREMENTS.md](milestones/v0.10-REQUIREMENTS.md) · [milestones/v0.10-MILESTONE-AUDIT.md](milestones/v0.10-MILESTONE-AUDIT.md).
- ✅ **v0.11 Broker Daemon & Cross-Tool Bootstrap** — Phases 4–6 (shipped 2026-06-06). Service-managed daemon (`famp daemon install`) restores the broker-presence guarantee that `56b2293` (correctly) removed; EPERM sandbox diagnostics + daemon install/status/uninstall/restart lifecycle + version-skew detection + daemon-first cross-platform README. 15/15 requirements, audit waived (Phase 6 human-verify E2E). See [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md).
- ✅ **v1.0 Federation Profile — Gateway Core** — Phases 7–12 (shipped 2026-07-29, tagged `v1.0.0` at `5edff41`). Gate A fired (Ben's sustained cross-machine use); this milestone closes it: an agent on one of Ben's machines exchanges a signed FAMP envelope with an agent on a second machine he controls, bidirectionally and reliably, over a network he fully controls (direct or a VPN he already runs — no public relay, no cross-person trust). Resolves the broker-liveness fork (same-host `kill(pid,0)` reaping a naively-proxied remote principal), ships `famp-gateway` (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`, signed cross-host envelopes (INV-10 + forward-compat fields), two-machine TOFU key bootstrap, and retires the ~27 parked federation tests (triaged 27/27 RETIRE, with the real Phase 9 E2E pinned into CI in their place). Gate B (conformance vector pack, 2nd implementer) stays event-driven and out of this milestone's scope. **Delivered:** 6 phases (7–12), 29 plans, 29/29 requirements, 106 commits over 7 days; scope grew past the planned Phases 7–10 by two phases — Phase 11 (the Gate A dogfood found no shipping client could address a remote principal, plus 8 setup-guide defects and a `from`-forgery hole) and Phase 12 (design review C's §16 nine-item release checklist). UAT-01 proven live macOS ↔ Linux, terminal COMPLETED on both hosts. See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) · [milestones/v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md).
- 🚧 **v1.1 Open-Internet Federation** — Phases 13–21 (opened 2026-07-30, in progress). Two **different people** exchange signed FAMP envelopes over the open internet, in different networks with no shared VPN and no hand-copied keys, both task FSMs reaching a terminal state, the second person following a doc unassisted. Replaces v1.0's three crutches (Ben-controlled machines, Ben-controlled network, hand-copied keys). 55/55 requirements mapped, Phases 14/15/17 executed, Phase 13 complete (decision record filed, REACH-02 closed 2026-08-02 against a real Verizon cellular hotspot), 16/18/19/20/21 not yet started. See `## Phase Details` below.

## Phases

Full phase details (goals, dependencies, success criteria, per-plan lists) live in the
per-milestone archives under [`milestones/`](milestones/) — one `v<X.Y>-ROADMAP.md` per
shipped milestone. This file stays constant-size: collapsed history here, active work
expanded, backlog at the bottom.

### 🚧 v1.1 Open-Internet Federation (Phases 13–21) — IN PROGRESS

- [ ] **Phase 13: Public Reachability Decision (Spike)** - Zero-code decision record naming the reachability model, live-verified cost/month, named operator, and what the relay/tunnel can and cannot observe.
- [ ] **Phase 14: Inbound-Content-Is-DATA Quarantine** - Structural, harness-agnostic, fail-closed provenance tagging at all seven rendering surfaces, proven by a FAMP-native adversarial corpus with a falsification control and closed out by an independent diff-only review. BLOCKING GATE — must be verified complete before Phase 20.
- [ ] **Phase 15: Keyring Multi-Key Extension + Revocation** - Multi-key-per-principal keyring with rotation and expiry/revocation, backward-compatible with existing single-key files. Must land before Phase 18 (Pairing).
- [x] **Phase 16: Distribution** - Prebuilt `famp`, `famp-gateway`, and `famp-relay` binaries for macOS arm64/x86_64 and Linux x86_64, published by a tag-triggered release workflow and installed by a checksum-verified curl command on a machine with no Rust toolchain. (completed 2026-08-03)
- [ ] **Phase 17: Protocol-Grade Ingress + Reachability Implementation** - Replay cache, freshness enforcement, audience binding, DoS-safe ordering, and the live reachability path from Phase 13 — shipped together, never one without the other.
- [ ] **Phase 18: Cross-Person Trust Bootstrap (Pairing)** - Fail-loud short-code pairing between two people with no prior shared secret, replacing v1.0's paste-a-blob TOFU. Mechanism: a five-word texted code (~55 bits, 2048-word list); security rests on entropy + single-use + server-side attempt limits. **No PAKE** (decided 2026-07-31, see REQUIREMENTS.md).
- [ ] **Phase 19: Auto-Wake Gate** - A remote-origin envelope never auto-wakes a parked `famp await`, enforced broker-side — the real enforcement mechanism the tool-gating scope decision resolved to, after two harness-side designs failed adversarial review.
- [ ] **Phase 20: Human Acceptance Gate** - A second person, unassisted, exchanges signed envelopes bidirectionally with Ben's agent over the open internet; both task FSMs reach a terminal state.
- [ ] **Phase 21: Push Notification Adapter** - `famp watch --notify` replaces the await-poll + Stop-hook + sentinel convention with zero `famp-bus` change.

Full phase details (goals, dependencies, success criteria): see `## Phase Details` below.

<details>
<summary>✅ v1.0 Federation Profile — Gateway Core (Phases 7–12) — SHIPPED 2026-07-29, tagged <code>v1.0.0</code> at <code>5edff41</code></summary>

- [x] Phase 7: Broker-Liveness Fork + Gateway Skeleton (3/3 plans) — completed 2026-07-23
- [x] Phase 8: Signed Cross-Host Envelope + Trust Bootstrap (4/4 plans) — completed 2026-07-23
- [x] Phase 9: End-to-End Cross-Host Delivery (5/5 plans) — completed 2026-07-27
- [x] Phase 10: Test Reactivation + Setup Docs (3/3 plans) — completed 2026-07-27 (`10-VERIFICATION.md` `human_needed` on DOC-04's unassisted-follower clause only; superseded by Phase 11's UAT-01 PASS)
- [x] Phase 11: Shipping-Client Remote Addressing + Setup Hardening (8/8 plans) — completed 2026-07-29
- [x] Phase 12: v1.0.0 Release Gate (5/5 plans) — completed 2026-07-29

Details: [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) · [milestones/v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md) · phase artifacts under [milestones/v1.0-phases/](milestones/v1.0-phases/)

</details>

<details>
<summary>✅ v0.11 Broker Daemon & Cross-Tool Bootstrap (Phases 4–6) — SHIPPED 2026-06-06</summary>

- [x] Phase 4: Broker Lifecycle & Bootstrap Diagnostics (3/3 plans) — completed 2026-06-04
- [x] Phase 5: Daemon Service Management & Version Safety (5/5 plans) — completed 2026-06-04
- [x] Phase 6: Onboarding & Cross-Platform Docs (3/3 plans) — completed 2026-06-06

Details: [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.10 Inspector &amp; Observability (Phases 1–3) — SHIPPED 2026-05-11</summary>

- [x] Phase 1: Broker Diagnosis & Identity Inspection (4/4 plans) — completed 2026-05-10
- [x] Phase 2: Task FSM & Message Visibility (3/3 plans) — completed 2026-05-10
- [x] Phase 3: Load Verification & Integration Hardening (3/3 plans) — completed 2026-05-11

Details: [milestones/v0.10-ROADMAP.md](milestones/v0.10-ROADMAP.md) · [milestones/v0.10-REQUIREMENTS.md](milestones/v0.10-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.5.1 → v0.9 (spec fork, foundation crates, personal runtime, Claude Code integration, local-first bus) — SHIPPED 2026-04-13 → 2026-05-04</summary>

Phase numbering reset per milestone through v0.9; each milestone's phases, plans, and
success criteria are preserved in its own archive:

- ✅ v0.9 Local-First Bus — Phases 1–4 + close-fix Phase 5 (35 plans) — [archive](milestones/v0.9-ROADMAP.md) · [reqs](milestones/v0.9-REQUIREMENTS.md) · [audit](milestones/v0.9-MILESTONE-AUDIT.md)
- ✅ v0.8 Usable from Claude Code — Phases 1–4 + v0.8.x bridge (18 plans) — [archive](milestones/v0.8-ROADMAP.md) · [reqs](milestones/v0.8-REQUIREMENTS.md) · [audit](milestones/v0.8-MILESTONE-AUDIT.md)
- ✅ v0.7 Personal Runtime — Phases 1–4 (15 plans) — [archive](milestones/v0.7-ROADMAP.md) · [reqs](milestones/v0.7-REQUIREMENTS.md) · [audit](milestones/v0.7-MILESTONE-AUDIT.md)
- ✅ v0.6 Foundation Crates — Phases 1–3 (9 plans) — [archive](milestones/v0.6-ROADMAP.md) · [reqs](milestones/v0.6-REQUIREMENTS.md) · [audit](milestones/v0.6-MILESTONE-AUDIT.md)
- ✅ v0.5.1 Spec Fork — Phases 0–1 (9 plans) — [archive](milestones/v0.5.1-ROADMAP.md) · [reqs](milestones/v0.5.1-REQUIREMENTS.md) · [audit](milestones/v0.5.1-MILESTONE-AUDIT.md)

</details>

### 📋 Gate B — conformance vector pack (still open, event-driven)

Independent of v1.1 and of any version number; fires when a second implementer commits
to interop, ships at whatever tag is current. Draft plan: `.planning/WRAP-V0-5-1-PLAN.md`;
SEED-001 is its RFC 8785 gate (already green in CI). Not scheduled by this roadmap.

The former "v1.1 sketch" entry that lived here (public reachability, cross-person trust
bootstrap, signed peer directory, protocol-grade ingress) is now the real, roadmapped
v1.1 milestone above (Phases 13–21) — see `## Phase Details`.

The 2026-06-08 mesh-VPN "Future Milestone Sketch" that used to live in this file is
superseded and preserved in [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md): the
tailnet is no longer the trust boundary, because the shipped gateway verifies
Ed25519/INV-10 at the boundary itself.

## Phase Details

### Phase 13: Public Reachability Decision (Spike)

**Goal:** The public-reachability model for v1.1 — the piece that carries real recurring infra cost and ongoing operator burden — is decided and recorded, not built, with live-verified cost figures and an honest weighing of the single-crate alternative, validated against a real network Ben does not control rather than only networks he controls.
**Depends on:** Nothing (first v1.1 phase)
**Requirements:** REACH-01, REACH-02, REACH-03
**Success Criteria** (what must be TRUE):

  1. A decision record names the chosen reachability model (self-hosted relay vs. hosted tunnel vs. direct/no-relay) with cost/month re-verified live against vendor pricing pages (not aggregators) and a named operator. ✓ satisfied — self-hosted relay, AWS Lightsail, $5.00/mo, Ben-operated.
  2. The decision record states plainly what the relay/tunnel can and cannot observe about FAMP traffic. ✓ satisfied, and corrected: the relay sees **plaintext** envelope bodies, not opaque bytes — FAMP signs but does not encrypt (metadata/timing/who-talked-to-whom are also visible to it).
  3. The spike's viability finding is validated against a real network Ben does not control (e.g. a carrier hotspot, public Wi-Fi), not only networks he controls. ✓ satisfied — 2026-08-02, Verizon cellular hotspot, two runs, cone NAT; see `.planning/phases/13-public-reachability-decision-spike/13-REACH-02-RESULTS.md`.
  4. `iroh` is explicitly weighed as the single-crate pubkey-addressed alternative, with its rejection rationale (transport-migration cost against the shipped, Gate-A-proven axum/rustls transport) recorded in the decision doc rather than silently dropped. ✓ satisfied.
  5. Zero production code ships in this phase — the deliverable is the decision record that Phase 17 builds against. ✓ satisfied; Phase 17's shipped implementation was checked against this record 2026-08-02 and matches with no divergence.

**Plans:** 1 (decision record, no code) — `.planning/phases/13-public-reachability-decision-spike/13-DECISIONS.md`, promoted from draft and filed 2026-08-02. **Phase 13 complete**: REACH-01, REACH-02, and REACH-03 all satisfied (REACH-02 closed 2026-08-02).

### Phase 14: Inbound-Content-Is-DATA Quarantine

**Goal:** Remote-origin content is structurally, unforgeably tagged at every surface that renders it — machine-checkable provenance, proven non-vacuous by a FAMP-native adversarial corpus and closed out by an independent diff-only review. This is the prerequisite for enforcement, not enforcement itself: it does **not** prevent a remote agent from steering a local agent by sending it text (see the RESOLVED scope decision in REQUIREMENTS.md and `docs/QUARANTINE.md`). It is still the milestone's blocking security gate in the sequencing sense: it must be verified complete before Phase 20 lets a second person's traffic reach this host.
**Depends on:** Nothing — technically independent of reachability, keyring, and pairing (it touches `famp-bus`'s `Register` frame and reply shapes, Layer 1, plus seven CLI/MCP read sites), so it is deliberately sequenced early rather than left until the gate that needs it.
**Requirements:** QUAR-01, QUAR-02, QUAR-03, QUAR-04, QUAR-05, QUAR-06, QUAR-07, QUAR-08, QUAR-09, QUAR-10, QUAR-11
**Success Criteria** (what must be TRUE):

  1. Remote origin survives to the mailbox via a new additive field on `famp-bus`'s `Register` frame — `famp-envelope` and `famp-canonical` stay untouched.
  2. Provenance is fail-closed: the broker stamps EVERY mailbox append at `Out::AppendMailbox`, and a missing stamp renders as `unknown — untrusted`, never as local (QUAR-09).
  3. Every one of the seven rendering surfaces (`famp_inbox`, `famp_await`, `famp_channel_log`, CLI `inbox list`, CLI `await`, CLI `register --tail`, CLI `wait-reply`) visibly marks remote-origin content, all through one shared render helper; the surface list is generated mechanically, and a new surface added later without tagging fails a regression gate automatically.
  4. `BUS_PROTO_VERSION` moves 1 → 2 with a hard reject of proto-1 clients whose error names the remedy, plus a README/migration note stating that proto 2 requires reinstalling the client and restarting the daemon (QUAR-10).
  5. A FAMP-native adversarial corpus (not a borrowed tool-calling-agent benchmark), including payloads that emit the tagging delimiter itself, runs in CI, proven non-vacuous by a named test that FAILS when the quarantine is reverted and a named test that still PASSES under the same revert.
  6. The wake-up notification payload continues to carry no attacker-controlled body text (the existing Stop-hook shim is the model, not the gap).
  7. A laundering test PASSES documenting that the tag is one-hop, and shipped documentation states plainly what this boundary does not protect against — explicitly NOT claiming it prevents a remote agent from steering a local agent (QUAR-08, QUAR-11).
  8. An independent, diff-only adversarial review (reviewer sees the diff and threat model only, never the author's own findings) has passed.

**Plans:** 5/5 plans executed
Plans:

- [x] 14-01-PLAN.md — Tracer: fail-closed provenance spine, gateway to `famp_inbox`, plus the proto 1 → 2 bump
- [x] 14-02-PLAN.md — Expand to the remaining six rendering surfaces through one shared render helper
- [x] 14-03-PLAN.md — Mechanical surface enumeration + QUAR-05 regression gate in `just ci` and GitHub Actions
- [x] 14-04-PLAN.md — FAMP-native adversarial corpus, falsification control patch, one-hop laundering test
- [x] 14-05-PLAN.md — Falsification run captured, version-skew tests, QUAR-08 docs + proto-2 migration note, QUAR-07 handoff

**Constraint:** All work lands in `famp-bus` (additive `Register` field, stamped reply/record shape) and the CLI/MCP surface. `famp-envelope`, `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm` are frozen this milestone and must not be touched. QUAR-07 is run externally by famp-lead-730, not by the executing session.

### Phase 15: Keyring Multi-Key Extension + Revocation

**Goal:** The keyring supports more than one key per principal with an explicit active/retired/revoked lifecycle, so rotation and revocation exist as a real remediation path before any new bootstrap or directory mechanism writes through the same integration point.
**Depends on:** Nothing structurally required to start, but must complete before Phase 18 (Pairing) — it writes through the same `pin_tofu`/`rotate_to` integration point, and a second on-disk format migration later is the risk of building on top of the old one first. (The Signed Peer Directory, which shared this constraint, was cut from v1.1 scope 2026-08-02 — see REQUIREMENTS.md's deferred/backlog section.)
**Requirements:** KEYR-01, KEYR-02, KEYR-03, REVK-01, REVK-02, REVK-03
**Success Criteria** (what must be TRUE):

  1. Existing single-key keyring files load unchanged — proven by a fixture test, not just code review.
  2. A peer's key can be rotated: a new key is pinned for a known peer without dropping the previous key until it is explicitly retired.
  3. "Key changed for a known peer" is a structurally distinct path (different exit code, different operator confirmation) from "new peer, first pin" — never a warning line in a stream the operator has learned to ignore.
  4. A pinned key past its validity window is rejected at verify time regardless of whether any revocation record was ever received.
  5. A signed revocation statement is verifiable and fail-closed as defense-in-depth on top of the expiry mechanism, and an envelope signed before a revocation takes effect is rejected once delivered after the revocation takes effect — no pre-revocation replay window.

**Plans:** 4/4 plans executed

Plans:

- [x] 15-01-PLAN.md — Lock the on-disk record shape (one-way door) and the REVK-02 authorized-signer rule; write 15-DECISIONS.md
- [x] 15-02-PLAN.md — Tracer: multi-key + revocation record shape end-to-end, expired key rejected at gateway ingress, KEYR-01 fixture proof
- [x] 15-03-PLAN.md — `rotate_to`/`retire` plus `famp peer rotate`/`retire`; unconfirmed key change exits 2 with zero mutation
- [x] 15-04-PLAN.md — Signed revocation statement, `famp peer revoke`/`import-revocation`, REVK-03 no-replay-window proof

**Constraint:** Changes land in `famp-keyring` and `famp-gateway`'s `verify.rs` (Layer 2). `famp-envelope` stays frozen — revocation is a keyring-side and gateway-side concern, not a wire-format change.

### Phase 16: Distribution

**Goal:** A second person with no Rust toolchain installs a working `famp` from a published release artifact on a clean machine — closing the gap where today's only install path (`cargo install famp`) requires a full Rust toolchain and compiling 15 crates, which makes Phase 20's fresh-machine validation unreachable as things stand.
**Depends on:** Nothing structurally required to start; independent of every other v1.1 phase. Must complete before Phase 20 (Human Acceptance Gate) — its DOC-07 fresh-machine validation needs a binary install path to exercise. Sequenced ahead of Pairing (Phase 18) in the approved critical-path order: the real bottleneck to the human gate is physical/logistical readiness, not cryptography, so get the install path solved first.
**Requirements:** DIST-01, DIST-02, DIST-03, DIST-04, DIST-05
**Success Criteria** (what must be TRUE):

  1. A tagged release publishes prebuilt binaries — **`famp`, `famp-gateway`, and `famp-relay`** — for macOS arm64, macOS x86_64, and Linux x86_64 as downloadable release artifacts, produced only by the tag-triggered workflow — no hand-built or manually uploaded binaries. (Binary set widened from `famp` alone 2026-08-02, Ben-approved: the gateway is a separate bin target that `docs/GATEWAY-SETUP.md` requires on `PATH`, so `famp` alone would not unblock Phase 20.)
  2. A single documented command installs a working `famp` on a machine with no Rust toolchain, proven on a clean environment with no prior FAMP state.
  3. Published artifacts carry checksums, verified by the installer before installing — a corrupted or substituted artifact fails closed. Docs state the honest boundary: checksums prove the download matches what the release workflow produced; they do not prove the workflow itself was uncompromised. Artifact signing is a named follow-up, not this phase.
  4. Onboarding docs lead with the binary install path; a **working** from-source command remains documented only as the fallback. (Corrected 2026-08-02: the original wording named `cargo install famp`, which has never worked — `famp` was never published to crates.io, and six doc sites tell users to run it. Publishing to crates.io was considered and explicitly rejected; the docs move to the `--path`/`--git` form.)

**Plans:** 5/5 plans executed

Plans:

- [x] 16-01-PLAN.md — TRACER: settle D-08 from a real arm64-macOS build log, then adopt `dist` 0.32 and generate the tag-triggered pipeline for 3 binaries × 3 pinned targets (DIST-01, DIST-03, DIST-05)
- [x] 16-02-PLAN.md — release-pipeline gates: `dist` drift check, the DIST-05 sole-producer structural gate, installer shellcheck, all wired into an additive `release-gate` workflow (DIST-01, DIST-05)
- [x] 16-03-PLAN.md — DIST-03 falsification pair: the installer fails closed on a corrupted artifact, proven discriminating by a checksum-stripped-installer inversion (DIST-03)
- [x] 16-04-PLAN.md — docs lead with the binary path, every from-source command actually works, D-06's claim boundary locked, all gated by a compiled doc-accuracy test on the `paths-ignore`d docs commit shape (DIST-02, DIST-04)
- [x] 16-05-PLAN.md — no-Rust container install gate, version bump, and the human-gated pre-release tag that proves DIST-01/02 by published artifacts rather than dry runs (DIST-01, DIST-02, DIST-05)

**Waves:** W1 = 16-01 · W2 = 16-02, 16-03, 16-04 (no file overlap) · W3 = 16-05
**Decisions:** `.planning/phases/16-distribution/16-CONTEXT.md` (D-01..D-08, user-approved 2026-08-02) · research: `16-RESEARCH.md` · patterns: `16-PATTERNS.md` · validation: `16-VALIDATION.md`
**Plan-time finding:** `dist` derives the release tag from `[workspace.package] version` (`1.0.0`), and `v1.0.0` already exists — so a version bump is a hard prerequisite to any tagged release, and it trips `version_strings_unified` in `crates/famp/src/cli/mod.rs`, which pins the version literal. Handled in 16-05; the test is updated, never weakened.
**Constraint:** Docs must lead with a `curl`-based installer rather than "download from the releases page." Browsers set `com.apple.quarantine` on downloads and `curl` does not, so the browser path forces macOS Gatekeeper/notarization work the curl path avoids entirely — a real cost avoided by a doc-ordering decision, not one worth rediscovering later.

### Phase 17: Protocol-Grade Ingress + Reachability Implementation

**Goal:** The reachability model Phase 13 decided on is implemented and never goes live without the ingress hardening (freshness, replay-cache, audience binding, DoS-safe check ordering) that lets an open-internet-facing gateway survive abuse — the two ship together, never one without the other.
**Depends on:** Phase 13 (reachability model decision — this phase builds what Phase 13 chose)
**Requirements:** REACH-04, REACH-05, INGR-01, INGR-02, INGR-03, INGR-04, INGR-05, INGR-06, INGR-07, INGR-08
**Success Criteria** (what must be TRUE):

  1. Two gateways on different networks, with no shared VPN, establish a working bidirectional path under the chosen model.
  2. A reachability failure (relay down, hole-punch failed, peer offline) surfaces at the sender as a distinct, actionable error — never a silent fire-and-forget success.
  3. A replayed envelope and an envelope whose timestamp falls outside the configured clock-skew window are both rejected; the relationship between cache TTL, clock-skew window, and cache size bound is stated as an inequality and enforced by a test.
  4. An envelope not addressed to this gateway's own domain and a principal it actually backs is rejected (audience binding); check ordering runs cheap-before-expensive (size/format/rate before signature, signature before any state mutation), pinned by a test that fails if a later refactor reorders it.
  5. An oversized request body is rejected without being fully buffered into memory, and nonce scoping is per-sender so one peer cannot evict or collide with another peer's entries.

**Plans:** 6/6 plans executed

Plans:

- [x] 17-01-PLAN.md — TRACER: ingress guard skeleton + pre-verify freshness gate + the check-order reorder pin (INGR-01, INGR-05)
- [x] 17-02-PLAN.md — bounded per-sender replay cache, the TTL/skew/size inequality as executable code, restart-window decision (INGR-02, INGR-03, INGR-08)
- [x] 17-03-PLAN.md — audience binding pre-verify, non-rotatable rate-limit key, body-cap tests (INGR-04, INGR-06, INGR-07)
- [x] 17-04-PLAN.md — new `famp-relay` crate: bounded opaque store-and-forward queue, signed-fetch drain authorization against operator-configured pubkeys (REACH-04)
- [x] 17-05-PLAN.md — gateway relay-fetch loop through the single ingest core + three-process bidirectional e2e (REACH-04)
- [x] 17-06-PLAN.md — relay-failure surface: `AckDisposition::Failed` onto the original sender's mailbox (REACH-05)

**Waves:** W1 = 17-01, 17-04, 17-06 (no file overlap) · W2 = 17-02 · W3 = 17-03 · W4 = 17-05
**Constraint:** All new enforcement lands in `famp-gateway` (`verify.rs` or a new `ingress_guard.rs`) — never in the frozen `famp-envelope`, whose `federation_format_ok` stays format-only.
**Open on completion:** REACH-02 closed 2026-08-02 — see `.planning/phases/13-public-reachability-decision-spike/13-REACH-02-RESULTS.md`. REACH-04's genuinely-different-networks leg is proven on loopback only and stays OPEN; the cross-network leg is a clearly-marked pending item (Phase 10 DOC-04 precedent) — a provisioned relay box is not that, and REACH-02's closure does not carry REACH-04 along with it. No Lightsail provisioning happens in this phase (D-04).

### Phase 18: Cross-Person Trust Bootstrap (Pairing)

**Goal:** Two people with no prior shared secret and no assumed cryptography background complete mutual key pinning by exchanging a short code over any human channel (Signal, voice, text), with a wrong or expired code hard-aborting rather than silently degrading — replacing v1.0's paste-a-blob TOFU pattern, which is architecturally the same failure mode as SSH's known-broken TOFU.
**Depends on:** Phase 15 (the extended keyring — pairing writes through the same `pin_tofu`/`rotate_to` integration point)
**Requirements:** PAIR-01, PAIR-02, PAIR-03, PAIR-04, PAIR-05, PAIR-06, PAIR-07, PAIR-08
**Success Criteria** (what must be TRUE):

  1. Two people complete mutual key pinning by exchanging a short code over any human channel, with no raw key blob pasted and no fingerprint read aloud for visual comparison.
  2. Entering a wrong code hard-aborts the pairing — no partial pin, no degraded-but-continuing state — within a bounded number of guess attempts.
  3. A pairing code is single-use with a bounded validity window; an expired or reused code is rejected.
  4. A pairing failure names which step failed and what to do next, in language that does not assume the human knows what a public key is.
  5. The pairing artifact carries the QUAR-15 consent warning at the moment of consent — pairing with a peer means their agent's messages will be read by your agent, which can run commands on your machine.

**Plans:** 1/3 plans executed

Plans:
**Wave 1**

- [x] 18-01-PLAN.md — TRACER: one texted five-word code pins two machines end to end, gated by a blocking rendezvous-transport decision (PAIR-01, PAIR-06)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 18-02-PLAN.md — hard-abort, five-guess server-side budget, single-use + 24h window surviving restart, `famp pair revoke` (PAIR-02, PAIR-03)

**Wave 3** *(blocked on Wave 2 completion)*

- [ ] 18-03-PLAN.md — one artifact with consent before code, plain-language failure taxonomy, observe-before-pin done-signals (PAIR-04, PAIR-05, PAIR-07, PAIR-08)

**Waves:** W1 = 18-01 · W2 = 18-02 · W3 = 18-03 (strictly serial — all three plans write the same `famp::pairing` and `famp pair` files; no two can share a wave)
**One-way door — RESOLVED 2026-08-03, Ben selected `option-a`:** the pairing rendezvous transport was a blocking `checkpoint:decision` at the head of 18-01; it is now closed and the executor must NOT re-ask it. Selected: a dedicated unauthenticated `POST /famp/v1/pair/redeem` on the inviter's own gateway, with its own `Router` and state type, merged before the shared 1 MiB body cap, 404ing whenever no `Pending` invite exists. Routing through `famp-relay` (option-b) was rejected as blocked today — enqueue 404s until a domain is manually pre-registered with a relay restart, and `verify_inbound_any` rejects the unpinned senders every pairing peer is by definition. Accepted limitation: option-a requires the INVITER to be publicly reachable (true for the Lightsail-fronted gateway, not true for a NATed inviter); the symmetric case is deliberately out of scope this milestone.
**Still open for execution:** the BIP-39 wordlist **licensing determination** (18-01 Task 3) is a human call — the executor records the upstream LICENSE text verbatim and marks it UNRESOLVED rather than characterizing whether it permits vendoring.
**Constraint:** `famp-envelope` and every Layer 0 crate stay frozen — pairing is a parallel, non-envelope wire. `cargo nextest` hangs on this repo, so every verify uses plain `cargo test`; every Rust-touching task also runs `just lint` (nursery lints beyond plain clippy).
**Open on completion:** PAIR-05's comprehension half ("language that does not assume they know what a public key is") is not mechanically assertable and closes only at Phase 20's UAT-02. A new pin is durable but not active until `famp daemon restart` — the same gap `peer rotate`/`peer revoke` already ship under. REACH-04's cross-network leg stays open; the design deliberately avoids needing it by making the redeemer's call outbound-only.

### Phase 19: Auto-Wake Gate

**Goal:** A remote-origin envelope never auto-wakes a parked `famp await` — a real, broker-enforced boundary on the one trifecta leg FAMP owns end to end (automatic ingestion), resolving the tool-gating scope decision after two harness-side designs (a `PreToolUse` hook, a tools-restricted listener profile) both failed independent adversarial review for trying to enforce in the harness instead. Local-origin traffic keeps its auto-wake behavior unchanged.
**Depends on:** Phase 14 (the provenance stamp this gate reads already exists and is fail-closed), Phase 18 (Pairing — a peer to test the gate against; also the vehicle for the QUAR-15 consent warning)
**Requirements:** QUAR-12, QUAR-13, QUAR-14, QUAR-15
**Success Criteria** (what must be TRUE):

  1. A gateway-origin envelope delivered to a parked `famp await` does NOT wake it.
  2. That same envelope IS visible on the next human-initiated inbox read — held back from auto-wake, never dropped.
  3. A local-origin envelope DOES still wake a parked awaiter — the same-host mesh's auto-wake behavior is unaffected.
  4. The filter is enforced broker-side, not in the CLI drain — proven by the same test suite, not by inspection; a client-side filter would advance the read cursor past envelopes it declined to deliver (the 999.1 failure class).
  5. The consent warning appears in the pairing artifact (DOC-06) at the moment of consent, not only in `docs/QUARANTINE.md`.

**Plans:** TBD
**Constraint:** `AwaitFilter` already exists at `crates/famp-bus/src/proto.rs:54` and `BUS_PROTO_VERSION` is already 2 — the likely integration point, named here as context. The implementation approach is plan-phase work, not decided here.
**Road not taken (recorded, not rejected):** held-by-default-at-ingress — a remote envelope is held at the broker's ingress append site and never enters the recipient's mailbox at all until a human releases it or the peer has a standing per-peer auto-deliver grant. Strictly stronger than this phase; deliberately not v1.1 because this phase delivers most of the value at a fraction of the build. Natural upgrade path if the threat model ever demands more.

### Phase 20: Human Acceptance Gate

**Goal:** A second person — on their own machine, their own network, no shared VPN, no hand-copied keys — follows a setup guide unassisted and exchanges signed envelopes bidirectionally with an agent on Ben's machine, both task FSMs reaching a terminal state. **Minimum capability set for this to be meaningful:** the quarantine gate already verified complete (Phase 14), a no-Rust-toolchain install path (Phase 16), a live, ingress-hardened reachability path (Phase 17), a working pairing bootstrap (Phase 18), and the auto-wake gate (Phase 19) — this phase does not re-open any of that work, it only exercises it. The push-notify adapter (Phase 21) is not required for this event and is deliberately sequenced after it — delaying the human gate for it would violate the instruction to schedule it as early as the dependency chain honestly allows. (The signed peer directory, previously sequenced here too, was cut from v1.1 scope entirely on 2026-08-02 — see REQUIREMENTS.md.)
**Depends on:** Phase 14 (quarantine verified complete — blocking prerequisite), Phase 16 (binary distribution — the fresh-machine validation has no Rust toolchain to build from), Phase 17 (reachability implementation + ingress hardening), Phase 18 (pairing bootstrap), Phase 19 (auto-wake gate)
**Requirements:** DOC-06, DOC-07, UAT-02
**Success Criteria** (what must be TRUE):

  1. A follower-facing setup guide takes a second person from zero to a working paired gateway, gated by semantic assertions rather than flag-greps (v1.0 shipped a guide with inverted wiring instructions that a flag-grep gate passed).
  2. The guide is validated end-to-end on a fresh machine with no prior FAMP state and no Rust toolchain, exercising the prebuilt-binary install path (Phase 16), before the one real-person attempt.
  3. An agent on Ben's machine and an agent on a second person's machine — different networks, no shared VPN, no hand-copied keys — exchange signed envelopes in both directions, and both task FSMs reach a terminal state.
  4. The pass criterion is the receiving person's own `famp inspect tasks` output — never a sender-side exit 0, and never a Ben-relayed report.

**Plans:** TBD

### Phase 21: Push Notification Adapter

**Goal:** A stranger's agent wakes reliably on inbound messages via a first-class `famp watch --notify` command, replacing the `famp await` long-poll + `.famp-listen` sentinel + global Stop-hook convention as the primary wake path — with zero risk to the tokio-free `famp-bus` core. Genuinely orthogonal to every other phase in this milestone; scheduled last only because nothing blocks it and it blocks nothing, not because it is an afterthought.
**Depends on:** Nothing
**Requirements:** WATCH-01, WATCH-02, WATCH-03, WATCH-04, WATCH-05
**Success Criteria** (what must be TRUE):

  1. `famp watch --notify <command>` runs a command per arriving envelope for a bound identity.
  2. Envelope metadata reaches the notified command via environment variables only — never interpolated into a shell string, so no shell-injection surface exists.
  3. The notification payload obeys the same no-attacker-controlled-body-text discipline as the existing Stop-hook shim (QUAR-06).
  4. Ships with zero `famp-bus` change, preserving the permanent `just check-no-tokio-in-bus` gate.
  5. Behavior on restart is defined and tested: either missed notifications are replayed from the mailbox cursor, or the loss window is explicitly bounded and documented.

**Plans:** TBD

## Progress Table

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 2. Crypto Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 3. Core Types & Invariants | v0.6 | 2/2 | Complete | 2026-04-13 |
| 1. Minimal Signed Envelope | v0.7 | 3/3 | Complete | 2026-04-13 |
| 2. Minimal Task Lifecycle | v0.7 | 3/3 | Complete | 2026-04-13 |
| 3. MemoryTransport + TOFU Keyring | v0.7 | 4/4 | Complete | 2026-04-13 |
| 4. Minimal HTTP Transport | v0.7 | 5/5 | Complete | 2026-04-14 |
| 1. Identity & CLI Foundation | v0.8 | 3/3 | Complete | 2026-04-14 |
| 2. Daemon & Inbox | v0.8 | 3/3 | Complete | 2026-04-14 |
| 3. Conversation CLI | v0.8 | 4/4 | Complete | 2026-04-14 |
| 4. MCP Server & Same-Laptop E2E | v0.8 | 3/3 | Complete | 2026-04-15 |
| 1. `famp-bus` library + audit-log MessageClass | v0.9 | 3/3 | Complete | 2026-04-28 |
| 2. UDS wire + CLI + MV-MCP rewire + hook subcommand | v0.9 | 14/14 | Complete | 2026-04-30 |
| 3. Claude Code integration polish | v0.9 | 6/6 | Complete | 2026-05-03 |
| 4. Federation CLI unwire + federation-CI preservation | v0.9 | 8/8 | Complete | 2026-05-04 |
| 5. v0.9 Milestone Close — CC-07 + HOOK-04b + verification backfill | v0.9 | 5/5 | Complete   | 2026-06-04 |
| 1. Broker Diagnosis & Identity Inspection | v0.10 | 4/4 | Complete | 2026-05-10 |
| 2. Task FSM & Message Visibility | v0.10 | 3/3 | Complete | 2026-05-10 |
| 3. Load Verification & Integration Hardening | v0.10 | 3/3 | Complete | 2026-05-11 |
| 4. Broker Lifecycle & Bootstrap Diagnostics | v0.11 | 3/3 | Complete | 2026-06-04 |
| 5. Daemon Service Management & Version Safety | v0.11 | 5/5 | Complete | 2026-06-04 |
| 6. Onboarding & Cross-Platform Docs | v0.11 | 3/3 | Complete | 2026-06-06 |
| 7. Broker-Liveness Fork + Gateway Skeleton | v1.0 | 3/3 | Complete    | 2026-07-23 |
| 8. Signed Cross-Host Envelope + Trust Bootstrap | v1.0 | 4/4 | Complete    | 2026-07-23 |
| 9. End-to-End Cross-Host Delivery | v1.0 | 5/5 | Complete   | 2026-07-27 |
| 10. Test Reactivation + Setup Docs | v1.0 | 3/3 | Complete   | 2026-07-27 |
| 11. Shipping-Client Remote Addressing + Setup Hardening | v1.0 | 8/8 | Complete | 2026-07-29 |
| 12. v1.0.0 Release Gate | v1.0 | 5/5 | Complete | 2026-07-29 |
| 13. Public Reachability Decision (Spike) | v1.1 | 1/1 decision record | Complete | 2026-08-02 |
| 14. Inbound-Content-Is-DATA Quarantine | v1.1 | 5/5 | In Progress|  |
| 15. Keyring Multi-Key Extension + Revocation | v1.1 | 4/4 | In Progress|  |
| 16. Distribution | v1.1 | 5/5 | In Progress|  |
| 17. Protocol-Grade Ingress + Reachability Implementation | v1.1 | 6/6 | In Progress|  |
| 18. Cross-Person Trust Bootstrap (Pairing) | v1.1 | 1/3 | In Progress|  |
| 19. Auto-Wake Gate | v1.1 | 0/0 | Not started | - |
| 20. Human Acceptance Gate | v1.1 | 0/0 | Not started | - |
| 21. Push Notification Adapter | v1.1 | 0/0 | Not started | - |

## Backlog

### Phase 999.1: `famp await` crash safety — cursor advance vs flush ordering (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 while wiring a Claude Code Stop hook that blocks on `famp await --timeout 23h`. Open question: if the `famp await` process is SIGKILL'd (or its parent dies) after the inbox cursor has advanced but before stdout is flushed/consumed by the caller, is the entry lost? Verification test: run `famp await` in a subshell, SIGKILL immediately after a peer sends, then check whether `famp inbox list` still shows the entry. If lost, cursor should only advance after successful flush/ack. Low urgency (single-consumer listeners rarely crash mid-flush) but a real correctness concern for the protocol layer.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.2: Multi-listener lock semantics — concurrent `famp await` consumers (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 during adversarial review of the Stop hook listener. If two processes (e.g., two Claude Code windows sharing the same cwd + `.famp-listen` sentinel, or just two shells) both call `famp await` against the same `FAMP_HOME`, what happens? Expected: serialize cleanly via `inbox.lock` so exactly one consumer gets each new entry; the other blocks and awaits the next. Feared: cursor race where both processes read the same entry (duplicate delivery) or one deadlocks. Test plan: spawn two concurrent `famp await` processes against the same FAMP_HOME, have a peer send one envelope, verify exactly one consumer receives it and the other continues blocking. Low near-term priority (single-listener is the current usage pattern) but important before encouraging multi-listener workflows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.3: `heartbeat` envelope class — work-in-progress visibility (BACKLOG)

**Goal:** Define and ship a low-bandwidth `heartbeat` envelope class so a long-running worker can periodically signal "still alive, working on `<one-liner>`" without the originator having to poll. Eliminates the failure mode where 8–15 minute silent gaps in a multi-agent task look indistinguishable from a crashed daemon.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-a starved 21 minutes watching agent-b silently work on a pressure-tested artifact, then the operator intervened thinking it was stuck. Today there is no protocol-level signal between "actively working" and "crashed mid-task." Proposal: new envelope class `heartbeat` carrying `{ task_id, working_on: <≤120 char string>, ts }`; sender emits at most every N minutes (default 5) or on demand from a hypothetical `famp_status` MCP tool; receiver-side, the originator's `famp_await` surfaces "agent-b heartbeat at HH:MM, working on: ..." rather than rendering silence as suspicious. Sized as substrate work because it touches `famp-envelope` (new MessageClass) and `famp-fsm` (heartbeat is non-state-advancing — does not consume a slot in the 5-state FSM, but the inbox surface treats it like a deliver).

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.4: `user_attention` envelope class — human-in-loop primitive (BACKLOG)

**Goal:** Define and ship a `user_attention` envelope class so a worker can explicitly mark a task as "blocked pending human input" — distinct from `REQUESTED`, `COMMITTED`, or any of the three terminal states. The inbox surface and orchestrator must render this as a first-class human-action signal, not just another deliver.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the same 3-agent pressure test. Symptom: agent-c (a reviewer-role agent on call) said "this needs the operator" during round-2 escalation; agent-b had no FAMP-native primitive to forward the blocked-on-human state to agent-a (the orchestrator) in a way that would surface differently from a normal reply. Workaround used: a prose-tagged deliver, indistinguishable from any other reply. Proposal: new envelope class `user_attention` carrying `{ task_id, reason: <markdown blob explaining what input is needed>, suggested_actions?: Vec<string> }`; receiver-side, `famp_inbox list` and `famp_await` MUST flag these distinctly (e.g., a separate column or icon). Open design question: does this advance the FSM (new state `BLOCKED_HUMAN`?) or is it a non-state-advancing signal layered on COMMITTED? Likely the latter — keeps the 5-state FSM intact and matches the heartbeat (999.3) pattern.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.5: Spec-by-path tracking — `~/Workspace/...` paths in messages (BACKLOG, deferred to v1.0)

**Goal:** Track the spec-by-path gap explicitly so it isn't forgotten before v1.0. The gap is already covered structurally by the v1.0 federation gateway design — this entry exists so there is a discoverable link from the pressure-test findings to the federation work, and so v1.0 planning explicitly verifies the gap is closed.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-b sent absolute filesystem paths (e.g. `~/Workspace/FAMP/...`, `~/Workspace/<other-project>/...`) inside envelope bodies because the protocol has no native way to address a spec/artifact by content-id or by federation-resolvable URL. Today this works only because all three agents are co-resident on the same Mac with the same `$HOME`. The moment any agent runs cross-host, every such reference is dead. v0.9 (local-first bus, in design at `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`) does NOT address this — it's a same-host design. v1.0's federation gateway is the right home for content-addressable refs (or signed-URL refs) because that's the layer where cross-host trust + transport already exists. **Action for v1.0 planning:** when scoping the federation gateway, include an explicit requirement that an envelope can carry a portable artifact reference (sha256-id or signed URL) and the receiver can dereference it without trusting the sender's filesystem. **Status (2026-07-23):** not picked up by the v1.0 Gateway Core roadmap (Phases 7–10) — those phases carry direct filesystem-independent principal addressing (name/principal, not path) but no portable content-addressable artifact reference. Remains open for v1.1+.

Plans:

- [ ] TBD — to be folded into v1.0 federation gateway scope, NOT promoted independently. (Surface during /gsd:new-milestone for v1.0.)

### Phase 999.7: Broker inspect ingress prioritization (BACKLOG)

**Goal:** Prevent saturated inspect RPC traffic from monopolizing the broker's shared ingress queue before the inspect semaphore is reached.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 after adversarial review of the `inspect_load_does_not_starve_bus_messages` flake fix. The current mitigation bounds inspect filesystem dispatch and removes unbounded shed-path reply tasks, but all client frames still share `broker_rx`: inspect `Hello`, `Inspect`, and disconnect frames can fill or monopolize ingress before `Out::InspectRequest` reaches the semaphore. Future planning should evaluate splitting inspect ingress from ordinary bus ingress, classifying inspect frames before the shared broker actor queue, or giving ordinary bus traffic priority/budgeted draining so live `Send`/`Inbox` traffic cannot be delayed by saturated inspect connection churn.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.8: Audit-log FSM state handling for inspect tasks (BACKLOG)

**Goal:** Teach `famp inspect tasks` and message metadata to derive stable FSM states from v0.9/v0.10 `audit_log` send envelopes, not only canonical `request|commit|deliver|control` classes.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 during adversarial review of inspect load-test hardening. `derive_fsm_state` currently maps canonical envelope classes explicitly, but current local bus sends are encoded as `class: "audit_log"` with `body.event` and `body.details.mode`. Mailbox-only task rows and message metadata can therefore surface `UNKNOWN` for valid local bus task traffic. Future work should add explicit audit-log send-mode handling, e.g. `famp.send.new_task` / `mode: new_task` -> `REQUESTED`, `mode: deliver` -> `COMMITTED`, terminal deliver modes -> `COMPLETED|FAILED|CANCELLED`, with focused unit tests for task rows and message rows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

---
*Roadmap updated: 2026-06-03 — v0.11 Broker Daemon & Cross-Tool Bootstrap roadmap created. Three phases (4–6) covering 15/15 requirements: Phase 4 (BLC-01, BLC-02, BOOT-01 — broker lifecycle flag + sandbox diagnostics), Phase 5 (DAEMON-01..06, BOOT-02, VER-01, VER-02 — daemon service lifecycle + version safety), Phase 6 (DOC-01..03 — onboarding docs + cross-platform boundary). Phase 5 guardian plist-review gate is a blocking pre-load requirement. Phase dirs: `.planning/phases/04-*`, `05-*`, `06-*`. Prior milestone: v0.10 Inspector & Observability shipped 2026-05-11 (3/3 phases, 10/10 plans, 26/26 requirements). v0.10 Inspector & Observability recut after matt-essentialist + zed-velocity-engineer review. Three-phase structure: Phase 1 (Broker Diagnosis & Identity Inspection — closes orphan-listener incident class end-to-end, 16 reqs), Phase 2 (Task FSM & Message Visibility — I/O-bound handlers + the budget/cancel reqs that finally have something real to enforce against, 9 reqs), Phase 3 (Load Verification & Integration Hardening, 1 req). 26/26 v1 requirements mapped. Original cut (RPC-foundation-with-stub-handlers in Phase 1, all CLI in Phase 2) rejected as yak-shaving — Phase 1 success criteria around budget+cancel were testing synthetic test-only handlers, not the inspector's real work surface. INSP-RPC-02 reworded from runtime property test to compile-time `&BrokerState` signature + workspace dep-graph gate (`just check-inspect-readonly`). Phase numbering reset to Phase 1 per FAMP convention (v0.7/v0.8/v0.9 each reset; v0.10 follows). Independent of v1.0 federation gates (Gate A: Ben symmetric cross-machine; Gate B: 2nd implementer interop) which were unwelded 2026-05-09 per `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`. v0.9 Local-First Bus shipped 2026-05-04; v0.8 shipped 2026-04-26; v0.7 shipped 2026-04-14; v0.6 + v0.5.1 shipped 2026-04-13.*

*Roadmap updated: 2026-07-23 — v1.0 Federation Profile — Gateway Core roadmap created. Four phases (7–10), continuing sequential numbering from v0.11's Phase 6 (not reset), covering 13/13 v1 requirements from the 2026-07-23 REQUIREMENTS.md (supersedes the 2026-06-08 mesh-VPN Gate A draft). Foundation-first ordering, each phase gating the next: Phase 7 (LIVE-01, LIVE-02, GW-04 — resolves the broker-liveness fork with the Design-A local-proxy recommendation and stands up the `famp-gateway` skeleton; the spine every later phase depends on), Phase 8 (WIRE-01, WIRE-02, TRUST-01, TRUST-02 — signed cross-host envelope + two-machine TOFU key bootstrap, reusing `famp-crypto`/`famp-canonical`/`famp-keyring` without rebuilding them), Phase 9 (GW-01, GW-02, GW-03 — full bidirectional request→commit→deliver→ack cycle across two machines, proving Phases 7+8 compose), Phase 10 (TEST-01, TEST-02, DOC-04 — reactivates the ~27 parked `crates/famp/tests/_deferred_v1/` tests, lands a live two-process E2E in `just ci`, ships the two-machine setup guide). Scope is deliberately narrow: own-two-machines only (direct or Ben-controlled VPN), no public relay, no cross-person trust, no signed directory, no capability/approval plane — all deferred to v1.1/v2.0 per REQUIREMENTS.md v2 Requirements. Gate B (conformance vector pack) stays independent and out of this milestone. Phase dirs: `.planning/phases/07-*` through `10-*` (to be created at plan-phase time).*

*Roadmap updated: 2026-07-29 — **v1.0 Federation Profile — Gateway Core CLOSED and archived**; `v1.0.0` tagged and pushed at `5edff41`. Shipped 6 phases (7–12), 29 plans, 29/29 requirements — two more phases than the four planned at roadmap creation, both discovery-driven: Phase 11 (the Gate A dogfood found no shipping client could address a remote principal, plus 8 setup-guide defects and a sender-`from` forgery hole) and Phase 12 (design review C's §16 nine-item release checklist). Phase details and the superseded 2026-06-08 mesh-VPN Gate A sketch moved to `milestones/v1.0-ROADMAP.md`; this file collapsed to milestone groupings per the constant-size convention. Backlog (999.x) preserved verbatim below — its phase dirs were restored to `.planning/phases/` after the archiver swept them in with v1.0's. Closeout was `override_closeout`: 42 pre-existing open artifacts acknowledged and deferred, none a v1.0 requirement gap (see STATE.md § Deferred Items). Next milestone not yet defined; Gate B (conformance vector pack) and the v1.1 open-internet sketch both remain event-driven.*

*Roadmap updated: 2026-07-30 — **v1.1 Open-Internet Federation roadmap created.** Eight phases (13–20), continuing sequential numbering from v1.0's Phase 12 (not reset), covering 43/43 v1 requirements from the 2026-07-30 REQUIREMENTS.md (the requirements doc's own header undercounted this at 41; corrected during roadmap creation, see REQUIREMENTS.md § Traceability). Phase order honors five hard constraints from the milestone brief rather than natural-category grouping alone: (1) Phase 13 is a zero-code reachability spike (REACH-01..03) that gates only REACH-04/05, not KEYR/PAIR/QUAR/DIR/WATCH; (2) Phase 14 (QUAR, the inbound-content-is-DATA blocking gate) is sequenced immediately after the spike — architecturally independent of everything else, so built early rather than left to shadow the human gate; (3) Phase 15 (KEYR + REVK) lands before Phase 16 (PAIR) and Phase 19 (DIR), both of which write through the same `pin_tofu` integration point, and pairs the keyring extension with the revocation data model per the "must-land-together" guidance (avoids a second on-disk format migration); (4) Phase 17 pairs protocol-grade ingress with the reachability implementation per the second "must-land-together" pair — never ship one live without the other; (5) Phase 18 (the human acceptance gate, UAT-02 + DOC-06/07) sits as early as the real dependency chain allows — after Phase 14 (quarantine verified), Phase 16 (pairing works), and Phase 17 (reachability + ingress live) — not parked in the final phase; Phase 19 (directory) and Phase 20 (push-notify) are deliberately sequenced *after* it since neither gates it and delaying the human gate for either would have been exactly the mistake the brief warned against. Layer 0 (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`) stays frozen across all eight phases — every phase's constraint notes name the Layer 1/2 files it actually touches. Phase dirs: `.planning/phases/13-*` through `20-*` (to be created at plan-phase time).*

*Roadmap updated: 2026-08-02 — **Phase 18: Distribution inserted**, Ben-approved, before the Human Acceptance Gate. Today's only install path (`cargo install famp`) needs a full Rust toolchain, which makes Phase 19's (formerly Phase 18's) fresh-machine, no-prior-FAMP-state validation unreachable as written — DIST-01..05 (a tag-triggered release workflow publishing checksummed prebuilt binaries for macOS arm64/x86_64 and Linux x86_64, installed by one documented command, docs leading with that path over `cargo install`) close that gap. Phase 19 (Human Acceptance Gate, was 18), Phase 20 (Signed Peer Directory, was 19), and Phase 21 (Push Notification Adapter, was 20) shift accordingly; Phases 13–17 keep their numbers and none of their content changed. Phase 19's Depends-on gained Phase 18; its DOC-07 success criterion now names the binary install path explicitly rather than leaving "fresh machine" ambiguous about which install path is under test. Milestone requirement count corrected from a stale 43/43 to the accurate 54/54 (49 pre-existing + 5 new DIST). Neither file previously had this right: REQUIREMENTS.md's own coverage note claimed 46/46, but its traceability table was already missing PAIR-06/07/08 (present in the requirements body, never added to the table) — true pre-DIST total was 49, not 46. Both gaps — this file's stale phase count and REQUIREMENTS.md's missing PAIR rows — are closed in this same pass; see REQUIREMENTS.md's own coverage note for detail. **Separately, NOT part of this update:** a proposed PreToolUse tool-gating phase did not survive independent adversarial review (the MCP-server-only gate has at least four bypasses, including `famp inbox list` via Bash rendering remote content with the gate never arming) and is on hold pending Ben's ruling — no tool-gating phase or requirement exists in this roadmap, and none should be inferred. Phase dirs: `.planning/phases/18-*` through `21-*` (to be created at plan-phase time; existing `19-*`/`20-*` dirs, if any exist on disk from before this renumbering, need review before reuse).*

*Roadmap updated: 2026-08-02 (later same day) — **two more approvals landed and change the plan again, superseding the phase numbers in the note directly above.** (1) **Signed Peer Directory phase CUT entirely** — DIR-01..04 moved to REQUIREMENTS.md's deferred/backlog section under an event-driven trigger (peer count > ~5), same pattern as Gate B; the phase (previously numbered 20) no longer exists in this roadmap at all. Reason: it publishes a signed key list that is explicitly never a trust anchor (DIR-03) for a peer set of two, over a once-per-peer ceremony, and adds a second write path into the keyring integration point Phase 15 just spent four plans hardening — cost/benefit only turns positive at a peer count this milestone will not reach. (2) **New Phase 19: Auto-Wake Gate** — QUAR-12..15, a broker-side enforcement that a remote-origin envelope never auto-wakes a parked `famp await`. This is the mechanism the tool-gating scope decision (see the note two entries up, and REQUIREMENTS.md) actually resolves to: two harness-side designs (PreToolUse hook, tools-restricted profile) both attacked legs of the trifecta the harness doesn't control; this attacks automatic ingestion, the leg the broker does control. To make Phase 19 sit before Phase 20 (Human Acceptance Gate) in the numbering, Distribution and Pairing swapped slots — **Distribution is now Phase 16** (was 18), **Pairing is now Phase 18** (was 16) — reflecting the approved critical-path order: 13 (reachability decision, backfill separately) → Distribution → Pairing → Auto-Wake Gate → REACH-02/04 live validation → Human Acceptance Gate. This order was chosen because the real remaining bottleneck is physical/logistical (a second machine, a second person, a distributed binary) rather than cryptographic. Phases 14/15/17 (already executed) keep their numbers unchanged throughout; only not-yet-started phases were renumbered, which is why this was safe to do with zero blast radius on shipped work. Human Acceptance Gate's Depends-on gained Phase 19; Phase 15's Depends-on note dropped its Directory reference. Requirement count: 54 → 55 (-3 DIR, +4 QUAR-12..15), re-verified mechanically (every ID in REQUIREMENTS.md's v1 body diffed against every traceability row, zero gaps). Phase 13's decision-record backfill is explicitly NOT done here — separate job, briefed separately, only the roadmap order reflects its priority. Phase dirs: `.planning/phases/16-*` through `21-*` now name the FINAL post-swap assignments; no plans have been created for any of them yet, so no directory-rename cleanup is needed.*
