# Requirements: FAMP v1.1 Open-Internet Federation

**Defined:** 2026-07-30
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Milestone acceptance (an event, not a person):** an agent on Ben's machine and an agent on a second person's machine, in different networks with **no shared VPN** and **no hand-copied keys**, exchange signed envelopes in **both** directions and both task FSMs reach a terminal state. That person follows a doc **unassisted**.

> **Scoping note.** These requirements were scoped by the orchestrator on 2026-07-30 while Ben was away, under his standing authorization to "plan + execute everything that doesn't need me or a 2nd person," with reachability spend pre-authorized to ~$15/mo. Every judgment call the orchestrator made rather than deferred is marked **[ORCH]** with its rationale, so any of them can be vetoed cheaply on review.

---

## v1 Requirements

### Reachability (REACH)

Public reachability over the open internet. The model is decided **first**, in a zero-code spike, because it carries recurring cost and operator burden.

- [ ] **REACH-01**: A decision record names the chosen reachability model, its **re-verified live** cost/month (vendor pricing pages, not aggregators), the named operator, and explicitly what the relay/tunnel **can and cannot observe** about FAMP traffic.
- [ ] **REACH-02**: The spike's viability finding is validated against a **real symmetric-NAT network** (e.g. a carrier hotspot), not only networks Ben controls.
- [ ] **REACH-03**: `iroh` is explicitly weighed as the single-crate alternative and its rejection rationale (transport-migration cost against a shipped, Gate-A-proven axum/rustls transport) is recorded in the decision record rather than silently dropped.
- [ ] **REACH-04**: Two gateways on different networks, with no shared VPN, establish a working bidirectional path under the chosen model.
- [ ] **REACH-05**: A reachability failure (relay down, hole-punch failed, peer offline) surfaces at the sender as a distinct, actionable error — never as a silent fire-and-forget success.

### Keyring Format Extension (KEYR)

Load-bearing prerequisite: the keyring is hard-coded to exactly one key per principal today, which blocks rotation, revocation, and any new bootstrap path.

- [ ] **KEYR-01**: The keyring stores multiple keys per principal with explicit active/retired state, and **existing single-key keyring files load unchanged** (backward compatibility proven by a fixture test).
- [ ] **KEYR-02**: A peer's key can be rotated — a new key is pinned for a known peer without dropping the previous key until it is explicitly retired.
- [ ] **KEYR-03**: "Key **CHANGED** for a known peer" is a structurally distinct path from "new peer, first pin" — a different exit code and a different operator confirmation, not a warning line in a stream the operator has learned to ignore.

### Cross-Person Trust Bootstrap (PAIR)

The milestone's hard problem. Replaces v1.0's `peer export` → paste-a-blob-over-Signal → `peer import`, which is architecturally the same silent-accept pattern as SSH TOFU.

- [ ] **PAIR-01**: Two people with no prior shared secret complete **mutual** key pinning by exchanging a short code over any human channel.
- [ ] **PAIR-02**: A wrong code **hard-aborts** the pairing. No partial pin, no degraded-but-continuing state, and a bounded number of guess attempts.
- [ ] **PAIR-03**: A pairing code is single-use and has a bounded validity window; an expired or reused code is rejected.
- [ ] **PAIR-04**: Pairing completes without either party pasting a raw key blob or reading a fingerprint aloud for visual comparison.
- [ ] **PAIR-05**: A pairing failure tells the human **which** step failed and what to do next, in language that does not assume they know what a public key is.

### Signed Peer Directory (DIR)

- [ ] **DIR-01**: A `famp-directory` crate publishes a signed, TTL-bounded peer key list, canonicalized with the existing RFC 8785 JCS path and signed with the existing Ed25519 substrate.
- [ ] **DIR-02**: A consumer verifies the directory signature and rejects stale, expired, or unsigned entries **fail-closed**.
- [ ] **DIR-03**: The directory never becomes an implicit trust anchor — an entry present in a signed directory is **not** sufficient to pin a peer; explicit pinning is still required. Proven by a test that a directory-only peer is rejected.

### Protocol-Grade Ingress (INGR)

All four concerns explicitly deferred out of v1.0 as open-internet problems. All enforcement lands in `famp-gateway` — **never** in the frozen `famp-envelope`.

- [ ] **INGR-01**: An envelope whose timestamp falls outside the configured clock-skew window is rejected.
- [ ] **INGR-02**: A bounded, memory-capped replay/nonce cache rejects a replayed envelope. The relationship between cache TTL, the clock-skew window, and the cache size bound is stated as an inequality and **enforced by a test**, not left as a comment.
- [ ] **INGR-03**: Replay-cache behavior across a gateway restart is either durable, or the restart-reopens-the-window interval is explicitly bounded, documented, and tested.
- [ ] **INGR-04**: An envelope not addressed to this gateway's own domain **and** a principal it actually backs is rejected (audience binding).
- [ ] **INGR-05**: Check ordering is cheap-before-expensive: size/format/rate checks precede signature verification, and signature verification precedes **any** state mutation. The order is pinned by a test that fails if a later refactor reorders it.
- [ ] **INGR-06**: Rate limiting is keyed on something an attacker cannot trivially rotate, and the choice of key is justified in a comment tied to this requirement.
- [ ] **INGR-07**: Request bodies are bounded — an oversized body is rejected without being fully buffered into memory.
- [ ] **INGR-08**: Nonce scoping is **per-sender**, not global, so one peer cannot evict or collide with another peer's nonce entries.

### Key Revocation (REVK)

- [ ] **REVK-01**: Pinned keys carry a validity window; a key past its window is rejected at verify time regardless of whether any revocation record was ever received.
- [ ] **REVK-02**: A signed revocation statement, distributed over the same channel as the original pin, is verifiable and fail-closed — defense in depth on top of REVK-01, not the primary mechanism.
- [ ] **REVK-03**: An envelope signed before a revocation takes effect is rejected **after** it takes effect (no pre-revocation replay window).

### Inbound-Content-Is-DATA Boundary (QUAR) — **BLOCKING GATE**

Settled before any outside person connects. A remote agent must not be able to steer a local agent by sending it text. Structural and harness-agnostic — **not** a prompt convention, and **not** enforced in `~/.claude` wiring.

- [ ] **QUAR-01**: Remote origin survives to the mailbox. `strip_relay_fields` currently erases every field that could mark an envelope as relayed before the local bus write; provenance is carried instead as a new **additive** field on `famp-bus`'s `Register` frame (Layer 1 — not frozen), leaving `famp-envelope` untouched.
- [ ] **QUAR-02**: **Every** surface that renders received content marks remote-origin content structurally — `famp_inbox`, `famp_await`, `famp_channel_log`, CLI `inbox list`, and CLI `await`. A boundary covering four of five is not a boundary.
- [ ] **QUAR-03**: A **FAMP-native** adversarial corpus runs in CI. Published benchmarks (AgentDojo, InjecAgent, WASP) are tool-calling-agent-shaped, not message-relay-shaped — the corpus must be built for this threat model, including payloads that emit the tagging delimiter itself.
- [ ] **QUAR-04**: The corpus is proven **non-vacuous** by a falsification control: a named test that must FAIL when the quarantine is reverted, alongside a named test that must still PASS. Green under both states carries zero information.
- [ ] **QUAR-05**: A regression gate fails when a **new** rendering surface is added without tagging — the five-surface list cannot silently go stale.
- [ ] **QUAR-06**: The wake-up notification payload carries **no** attacker-controlled body text. (`famp-await.sh` is already correct on this and is the model to preserve.)
- [ ] **QUAR-07**: An independent, **diff-only** adversarial review of the quarantine passes. The reviewer receives the diff and the threat model, not the author's own findings.
- [ ] **QUAR-08**: Documentation states plainly what this boundary does **not** protect against, naming delimiter-emission and prompt-level mitigation as known-insufficient, so no future reader over-trusts it.

### Push Notification Adapter (WATCH) — SEED-002

Promoted from dormant. A stranger's agent waking reliably on inbound messages is part of the unassisted-follower experience; the blocking Stop-hook + `.famp-listen` sentinel convention is the brittlest part of onboarding someone new.

- [ ] **WATCH-01**: `famp watch --notify <command>` runs a command per arriving envelope for a bound identity.
- [ ] **WATCH-02**: No shell injection — envelope metadata reaches the command via environment variables, never interpolated into a shell string.
- [ ] **WATCH-03**: The notification payload obeys QUAR-06 (no attacker-controlled body text).
- [ ] **WATCH-04**: Ships with **zero** `famp-bus` change, preserving the permanent `just check-no-tokio-in-bus` gate.
- [ ] **WATCH-05**: Behavior on restart is defined and tested — either missed notifications are replayed from the mailbox cursor, or the loss window is explicitly bounded and documented.

### Documentation & Acceptance (DOC / UAT)

- [ ] **DOC-06**: A follower-facing setup guide takes a second person from zero to a working paired gateway. Gated by **semantic** assertions, not flag-greps — v1.0 shipped `GATEWAY-SETUP.md` with its wiring instructions inverted and a flag-grep gate passed it.
- [ ] **DOC-07**: The guide is validated end-to-end on a **fresh machine with no prior FAMP state** before the real human gate, so the one attempt with a real person is not spent discovering a missing prerequisite. **[ORCH]** — added as cheap insurance for the acceptance event; it is doc validation on a clean box, not a second human gate.
- [ ] **UAT-02**: **The acceptance event.** An agent on Ben's machine and an agent on a second person's machine, in different networks with no shared VPN and no hand-copied keys, exchange signed envelopes in **both** directions and both task FSMs reach a terminal state, with that person following DOC-06 unassisted. **Pass criterion is the receiving person's own `famp inspect tasks` output** — never a sender-side exit 0, and never a Ben-relayed report.

---

## v2 Requirements

Deferred to a future release. Tracked, not in this roadmap.

### FAMP-Sec Capability Plane

- **SEC-CAP-01**: Capability/approval/tool-admission plane — remote-triggered tools. Demand-gated to v2.0+. v1.1 stays conversation-only; removing the tool-access leg is precisely what keeps the "lethal trifecta" (untrusted content + private data + external communication) from closing.

### Conformance

- **GATE-B-01**: Conformance vector pack. Independent and **event-driven** — fires when a second implementer commits to interop, not on this milestone's schedule. See `WRAP-V0-5-1-PLAN.md`; SEED-001 is its `serde_jcs` gate.

### Deferred from v1.1 scope

- **DIR-04**: Directory-based automatic peer discovery (as opposed to the signed key list of DIR-01). Discovery without explicit pinning re-opens exactly the trust question DIR-03 closes.
- **REACH-06**: NAT hole-punching as an optimization over relay fallback. Build the fallback first; 15–30% of hosts sit behind symmetric NAT that hole-punching cannot solve at all.

---

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| FAMP-Sec capability / approval / tool-admission plane | v2.0+, demand-gated. v1.1 is conversation-only — no remote-triggered tools. Keeps the lethal trifecta open. |
| Gate B conformance vector pack | Event-driven on a second implementer committing to interop; independent of this milestone's schedule. |
| Any change to Layer 0 (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`) | Frozen this milestone. The nonce/expiry/capability/approval fields v1.0 reserved are already there — use them. No second signature, no parallel envelope type. |
| `iroh` as the transport | Elegant single-crate answer to reachability + hole-punch + pubkey addressing, but adopting it means **replacing** the shipped, Gate-A-proven axum/rustls transport mid-milestone, for a milestone whose stated hard problem is the human, not the wire. Rejection rationale recorded under REACH-03. |
| PAKE library integration via `opaque-ke` / `srp` | Wrong shape — both are asymmetric/verifier-based, built for client-server password auth, not flat peer-to-peer pairing between equals. |
| `ngrok` as the reachability answer | Free tier caps sessions at 2 hours — disqualifying for a persistent listener. |
| Central-authority auth keys (Tailscale-style) | Contradicts the no-central-authority constraint the protocol is built on. |
| Silent TOFU-then-optionally-verify | This is what v1.0 does and what PAIR-01..05 exist to replace. |
| Prompt-level "treat the following as data" as the injection defense | Named known-insufficient by OWASP LLM Top 10 2025 and the CaMeL/AgentDojo literature. Mitigation inside the model is not a boundary. |
| Harness-side-only enforcement of the DATA boundary | Untestable in FAMP's CI and silently fails to protect Codex, Grok, and every other client. |
| Broker-side persistent subscription for push-notify | `famp watch` ships as a thin CLI wrapper over `famp await` first (WATCH-04). A true pub/sub design collides with the tokio-free `famp-bus` gate and is not yet justified by use. |

---

## Traceability

Which phases cover which requirements. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| _(populated by roadmapper)_ | | |

**Coverage:**
- v1 requirements: 41 total
- Mapped to phases: 0 (roadmap pending)
- Unmapped: 41 ⚠️

---

## Orchestrator scoping calls — for cheap veto

| Call | Rationale | Reverse cost |
|------|-----------|--------------|
| PAKE-style short code (PAIR-01..05) over passive fingerprint comparison | Fail-loud beats compare-and-shrug. Wormhole: 2 human steps, hard abort. Matrix SAS: 5–6 steps with a real "looks close enough" risk. Serves the unassisted-follower bar directly. | Medium — PAIR-01/02 would relax to a compare-based flow; keyring work (KEYR-*) is unaffected either way since both terminate at the same pin site. |
| Self-hosted relay as the presumptive default, decided in the spike | Zero new Rust deps (relay is untrusted; envelopes already signed end-to-end). Within the ~$15/mo pre-authorization. | Low — the spike is a decision phase; REACH-01 records the choice before REACH-04 builds on it. |
| `expiry`-based short-lived keys as the primary revocation mechanism (REVK-01), signed statement as defense in depth (REVK-02) | Cheaper (reuses the freshness machinery INGR-01 already builds) and more correct with no CA — a revocation record still needs reliable delivery to a possibly-offline peer. | Low — REVK-02 already exists as the alternative; the ordering is what would flip. |
| DOC-07 added (fresh-machine dry run) | Not in Ben's brief. v1.0's single most important finding arrived at a human gate; a clean-box rehearsal protects the one attempt with a real person. | Trivial — drop the requirement. |
| Requirements scoped without per-category confirmation | Ben authorized "plan + execute" before leaving; his brief was specific enough that the categories are transcription, not invention. | Trivial — this table exists so any line can be struck on review. |

---
*Requirements defined: 2026-07-30*
*Last updated: 2026-07-30 after v1.1 milestone open*
