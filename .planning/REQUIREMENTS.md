# Requirements: FAMP v1.1 Open-Internet Federation

**Defined:** 2026-07-30
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Milestone acceptance (an event, not a person):** an agent on Ben's machine and an agent on a second person's machine, in different networks with **no shared VPN** and **no hand-copied keys**, exchange signed envelopes in **both** directions and both task FSMs reach a terminal state. That person follows a doc **unassisted**.

> **Scoping note.** These requirements were scoped by the orchestrator on 2026-07-30 while Ben was away, under his standing authorization to "plan + execute everything that doesn't need me or a 2nd person," with reachability spend pre-authorized to ~$15/mo. Every judgment call the orchestrator made rather than deferred is marked **[ORCH]** with its rationale, so any of them can be vetoed cheaply on review.

> ## ⚠ OPEN SCOPE DECISION — BEN MUST DECIDE
>
> **The milestone brief's security-gate sentence overclaims what QUAR delivers, and the error is in our threat model, not in the implementation.**
>
> The brief says: *"A remote agent must not be able to steer my agent by sending it text. Design and test this as a hard boundary, not a prompt convention."* We justified conversation-only scope by arguing it removes the tool leg of the "lethal trifecta."
>
> **That justification is false.** An independent design review established it: conversation-only removes remote-triggered *FAMP* tools, but the receiving end is a listen-mode Claude Code session that auto-wakes on inbound messages, is instructed to call `famp_inbox`, and holds its **own full local toolset** — Bash, file access, and `famp_send` for exfiltration. Untrusted content + private data + tools + automatic ingestion: the trifecta is fully assembled **on the recipient's side**. FAMP removed the attacker's direct tool invocation; it did not remove the steered-local-agent path, which is precisely the threat the brief names.
>
> Structural tagging is spotlighting/datamarking. Published consensus (OWASP LLM01:2025, the spotlighting literature QUAR-08 itself cites) is that this **raises attack cost and does not bound a capable attacker** — and the attacker gets unlimited retries against a 23-hour listener. What QUAR-01..11 genuinely buys is the *prerequisite* for real enforcement: machine-checkable provenance. None of the actual boundaries (a `PreToolUse` hook that blocks tool calls once remote-tagged content has entered the turn; rendering remote bodies only through a quarantined summarizer; a listener profile with tools disabled) are possible while provenance is erased at `strip_relay_fields`.
>
> **The decision:** does v1.1 also ship harness-level tool-gating so the stated bar is actually met, or does v1.1 deliver provenance + honest documentation and defer enforcement?
>
> - **Option A — add tool-gating (meets the brief as written).** FAMP already ships hooks (`famp-await.sh`, the `famp hook` subcommand), so a `PreToolUse` gate is consistent with what it already does. Tension: it is Claude-Code-specific, and the brief also says enforce *harness-agnostic*. Those two instructions cannot both be fully satisfied — the honest resolution is a harness-agnostic provenance core plus per-harness enforcement adapters.
> - **Option B — provenance + honest docs, defer enforcement.** Cheaper and still valuable, but then **the milestone's gate sentence must be rewritten** to what is delivered, and UAT-02 must not be described as proving a steering boundary.
>
> **Orchestrator recommendation: Option A**, because the brief called this a *blocking* gate before any outside person connects, and Option B means an outside person connects to an agent that can still be steered. **Not actioned** — this is scope, and scope is Ben's call.
>
> **Work proceeding meanwhile is common to both options**: QUAR-01..11 are required either way, so nothing is wasted whichever is chosen.

---

## v1 Requirements

### Reachability (REACH)

Public reachability over the open internet. The model is decided **first**, in a zero-code spike, because it carries recurring cost and operator burden.

- [ ] **REACH-01**: A decision record names the chosen reachability model, its **re-verified live** cost/month (vendor pricing pages, not aggregators), the named operator, and explicitly what the relay/tunnel **can and cannot observe** about FAMP traffic.
- [ ] **REACH-02**: The spike's viability finding is validated against a **real symmetric-NAT network** (e.g. a carrier hotspot), not only networks Ben controls.
- [ ] **REACH-03**: `iroh` is explicitly weighed as the single-crate alternative and its rejection rationale (transport-migration cost against a shipped, Gate-A-proven axum/rustls transport) is recorded in the decision record rather than silently dropped.
- [ ] **REACH-04**: Two gateways on different networks, with no shared VPN, establish a working bidirectional path under the chosen model.
- [x] **REACH-05**: A reachability failure (relay down, hole-punch failed, peer offline) surfaces at the sender as a distinct, actionable error — never as a silent fire-and-forget success.

### Keyring Format Extension (KEYR)

Load-bearing prerequisite: the keyring is hard-coded to exactly one key per principal today, which blocks rotation, revocation, and any new bootstrap path.

- [x] **KEYR-01**: The keyring stores multiple keys per principal with explicit active/retired state, and **existing single-key keyring files load unchanged** (backward compatibility proven by a fixture test).
- [x] **KEYR-02**: A peer's key can be rotated — a new key is pinned for a known peer without dropping the previous key until it is explicitly retired.
- [x] **KEYR-03**: "Key **CHANGED** for a known peer" is a structurally distinct path from "new peer, first pin" — a different exit code and a different operator confirmation, not a warning line in a stream the operator has learned to ignore.

### Cross-Person Trust Bootstrap (PAIR)

The milestone's hard problem. Replaces v1.0's `peer export` → paste-a-blob-over-Signal → `peer import`, which is architecturally the same silent-accept pattern as SSH TOFU.

**MECHANISM DECIDED 2026-07-31 (Ben approved): a five-word TEXTED code (~55 bits from a 2048-word list). No PAKE. No capability link. No QR.**

The deciding insight, from the `matt-essentialist` review: **a PAKE is only required if the code must be *low*-entropy.** Magic Wormhole uses 16 bits *because* SPAKE2 grants exactly one guess. The milestone bar forbids a live call, so the code was always going to be **texted** rather than spoken — and once it is texted, entropy is free. Five words from a 2048-word list is ~55 bits, with security carried by **entropy + single-use + server-side attempt limits**. This dissolves the blocker that drove the earlier capability-link proposal: the verified absence of a production-grade Rust balanced-PAKE rules out *low-entropy* codes, not codes as such.

A texted word-code beat the 128-bit capability link on every axis that matters here: it survives phone→laptop transcription (five words vs. ~22 case-sensitive base64 chars — the link's advantage *inverts* exactly where it was supposed to help, since most non-technical people lack Signal Desktop); a sender-side link preview cannot burn it (messaging clients fetch URLs on the *sender's* device, consuming a GET-redeemed invite before the recipient ever sees it); it is not phishing-shaped ("click this link to connect your AI agent"); it needs no hosted web surface; and it keeps PAIR-01..05 honest **as written**, with no requirement redefinition. QR was dropped — it delivers to a device with a camera (the phone) while the software runs on the laptop, and `research/STACK.md:97` had already rejected it once on those grounds.

- [ ] **PAIR-01**: Two people with no prior shared secret complete **mutual** key pinning by exchanging a short code over any human channel. *(Satisfied by a five-word code, texted. ~55 bits.)*
- [ ] **PAIR-02**: A wrong code **hard-aborts** the pairing. No partial pin, no degraded-but-continuing state, and a bounded number of guess attempts. *(Attempt limits are server-side; entropy — not a one-guess PAKE — is what makes guessing infeasible.)*
- [ ] **PAIR-03**: A pairing code is single-use and has a bounded validity window; an expired or reused code is rejected. **Window is 24 hours, not 15 minutes** — a short window is a *low-entropy* mechanism, and a 15-minute clock expires while the follower is still installing. Single-use consumption MUST be **endpoint-enforced and persisted before expensive processing**; a service restart must not restore a consumed invite. Relay-enforced single-use is void against the malicious-rendezvous threat this design claims to tolerate. A `famp pair revoke` path must exist.
- [ ] **PAIR-04**: Pairing completes without either party pasting a raw key blob or reading a fingerprint aloud for visual comparison.
- [ ] **PAIR-05**: A pairing failure tells the human **which** step failed and what to do next, in language that does not assume they know what a public key is.
- [ ] **PAIR-06**: The code is entered via **stdin prompt, never as a command-line argument**. `famp pair <code>` would place the secret in `argv` (visible to `ps`) and in shell history in plaintext, durably — a leak path the design otherwise has no reason to create.
- [ ] **PAIR-07**: The **inviter** sees who redeemed the invite (peer principal + key_id) before the pin becomes durable, and both sides reach a plain-language done-signal — one sentence, not FSM JSON. Pairing completes asymmetrically (the redeemer sees success immediately; the inviter pins on its next poll), so without this the redeemer gets a success signal for a half-finished state, and a forwarded invite pairs silently.
- [ ] **PAIR-08**: Install instructions and the pairing code ship as **one artifact**, code at the bottom — so the invite outlives the follower's slowest step. The invite must be generated *after* the follower confirms their install works, not before; otherwise the validity window runs during the install.

### Signed Peer Directory (DIR)

- [ ] **DIR-01**: A `famp-directory` crate publishes a signed, TTL-bounded peer key list, canonicalized with the existing RFC 8785 JCS path and signed with the existing Ed25519 substrate.
- [ ] **DIR-02**: A consumer verifies the directory signature and rejects stale, expired, or unsigned entries **fail-closed**.
- [ ] **DIR-03**: The directory never becomes an implicit trust anchor — an entry present in a signed directory is **not** sufficient to pin a peer; explicit pinning is still required. Proven by a test that a directory-only peer is rejected.

### Protocol-Grade Ingress (INGR)

All four concerns explicitly deferred out of v1.0 as open-internet problems. All enforcement lands in `famp-gateway` — **never** in the frozen `famp-envelope`.

- [x] **INGR-01**: An envelope whose timestamp falls outside the configured clock-skew window is rejected.
- [x] **INGR-02**: A bounded, memory-capped replay/nonce cache rejects a replayed envelope. The relationship between cache TTL, the clock-skew window, and the cache size bound is stated as an inequality and **enforced by a test**, not left as a comment.
- [x] **INGR-03**: Replay-cache behavior across a gateway restart is either durable, or the restart-reopens-the-window interval is explicitly bounded, documented, and tested.
- [x] **INGR-04**: An envelope not addressed to this gateway's own domain **and** a principal it actually backs is rejected (audience binding).
- [x] **INGR-05**: Check ordering is cheap-before-expensive: size/format/rate checks precede signature verification, and signature verification precedes **any** state mutation. The order is pinned by a test that fails if a later refactor reorders it.
- [x] **INGR-06**: Rate limiting is keyed on something an attacker cannot trivially rotate, and the choice of key is justified in a comment tied to this requirement.
- [x] **INGR-07**: Request bodies are bounded — an oversized body is rejected without being fully buffered into memory.
- [x] **INGR-08**: Nonce scoping is **per-sender**, not global, so one peer cannot evict or collide with another peer's nonce entries.

### Key Revocation (REVK)

- [x] **REVK-01**: Pinned keys carry a validity window; a key past its window is rejected at verify time regardless of whether any revocation record was ever received.
- [x] **REVK-02**: A signed revocation statement, distributed over the same channel as the original pin, is verifiable and fail-closed — defense in depth on top of REVK-01, not the primary mechanism.
- [x] **REVK-03**: An envelope signed before a revocation takes effect is rejected **after** it takes effect (no pre-revocation replay window).

### Inbound-Content-Is-DATA Boundary (QUAR) — **BLOCKING GATE**

Settled before any outside person connects. A remote agent must not be able to steer a local agent by sending it text. Structural and harness-agnostic — **not** a prompt convention, and **not** enforced in `~/.claude` wiring.

- [x] **QUAR-01**: Remote origin survives to the mailbox. `strip_relay_fields` currently erases every field that could mark an envelope as relayed before the local bus write; provenance is carried instead as a new **additive** field on `famp-bus`'s `Register` frame (Layer 1 — not frozen), leaving `famp-envelope` untouched.
- [x] **QUAR-02**: **Every** surface that renders received content marks remote-origin content structurally. The surface list MUST be generated **mechanically** — every call site reaching `EnvelopeView::body()`, `write_outcome`, `emit_tail_line`, or serializing an `envelopes: Vec<Value>` — never hand-curated. Known surfaces: `famp_inbox`, `famp_await`, `famp_channel_log`, CLI `inbox list`, CLI `await`, **CLI `register --tail`** (`cli/register.rs:199-201`, `:270-272`, body rendered at `emit_tail_line` `:347`), and **CLI `wait-reply`** (`cli/wait_reply.rs:33-34`, plus its own inbox-first path `:53-67`). All content must flow through **one shared render helper**, not N ad-hoc implementations. *(Corrected 2026-07-30: the original five-surface list omitted `register --tail` and `wait-reply`. By this requirement's own standard — a boundary covering five of seven is not a boundary — the hand-curated list failed its own test. Hence the mechanical-generation mandate.)*
- [x] **QUAR-03**: A **FAMP-native** adversarial corpus runs in CI. Published benchmarks (AgentDojo, InjecAgent, WASP) are tool-calling-agent-shaped, not message-relay-shaped — the corpus must be built for this threat model, including payloads that emit the tagging delimiter itself.
- [x] **QUAR-04**: The corpus is proven **non-vacuous** by a falsification control: a named test that must FAIL when the quarantine is reverted, alongside a named test that must still PASS. Green under both states carries zero information.
- [x] **QUAR-05**: A regression gate fails when a **new** rendering surface is added without tagging — the five-surface list cannot silently go stale.
- [x] **QUAR-06**: The wake-up notification payload carries **no** attacker-controlled body text. (`famp-await.sh` is already correct on this and is the model to preserve.)
- [ ] **QUAR-07**: An independent, **diff-only** adversarial review of the quarantine passes. The reviewer receives the diff and the threat model, not the author's own findings.
- [x] **QUAR-08**: Documentation states plainly what this does **not** protect against, naming delimiter-emission and prompt-level mitigation as known-insufficient, so no future reader over-trusts it. It must **not** claim that a remote agent is prevented from steering a local agent — see QUAR-11. Honest wording: this delivers machine-checkable provenance and untrusted-marking at every rendering surface; steering-resistance remains the harness's job.
- [x] **QUAR-09**: Provenance is **fail-closed**. The broker stamps **every** mailbox append explicitly (`origin: local` | `origin: gateway`) at `Out::AppendMailbox` (`famp-bus/src/broker/handle.rs:455-460`); a **missing** stamp renders as `unknown — untrusted`, never as local. Absence must be anomalous. Proven by a **version-skew test**: an old gateway binary against a new broker must not produce unmarked remote content. *(An opt-in flag whose only setter is `ProxiedPrincipal::register` would silently evaporate under exactly the binary-skew this repo hits routinely — the daemon on the dev machine was running a stale build during this very milestone's planning.)*
- [x] **QUAR-10**: The provenance wire shape is carried by a **`BUS_PROTO_VERSION` bump 1 → 2 with a hard reject of proto-1 clients** *(decided 2026-07-30, Ben approved)*. The stamp cannot ride inside the envelope `Value` because `WireEnvelope` is `deny_unknown_fields` on decode — **preserve that**, since it is what makes the tag unforgeable by a remote sender. The rejection error must name the remedy (`just install` / `famp daemon restart`), matching the existing VER-01 precedent, and be pinned by a test. A README/CHANGELOG note must state that proto 2 requires reinstalling the client and restarting the daemon, and that a v1.0 client against a v1.1 broker fails loudly by design.

  **Why hard-reject rather than graceful degradation — this is a security decision, not a cost one.** An old client *cannot render provenance*. Serving it anyway means deliberately handing unmarked remote content to a client blind to it, which is exactly the fail-open hole QUAR-09 exists to close: **graceful degradation here IS the vulnerability.** Version-gated serving would have been real new broker-side state spent preserving a capability we specifically do not want.

  **Two options were considered and rejected**, both on evidence found in the tree rather than assumed: (a) an *additive sibling array* parallel to `envelopes` — **not actually additive**, because `BusReply` also carries `deny_unknown_fields` (`famp-bus/src/proto.rs:206`), so every shipped pre-v1.1 client hard-fails deserializing `InboxOk`/`AwaitOk`/`RegisterOk` the moment the field appears rather than ignoring it; (b) *Hello-version-gated serving* — not an existing mechanism to extend, since the Hello check today is a hard reject (`handle.rs:186`), so this meant new per-connection broker state.

  Known breakage, accepted: every unupgraded `famp` binary talking to a v1.1 broker, including `famp_channel_log` (which reads the JSONL **directly from disk, bypassing the broker**) and Grok's foreign implementation, which has already wedged a mailbox once. Judged acceptable because Gate B is still open with no named second implementer, and the failure is loud and self-describing rather than silent.

- [x] **QUAR-11**: A **laundering test** exists and PASSES, documenting a real limitation rather than hiding it: a remote-tagged message read by local agent A, whose body A then quotes into a message to local agent B, arrives at B **untagged**. The tag is one-hop. This must be stated in QUAR-08's documentation.

### Push Notification Adapter (WATCH) — SEED-002

Promoted from dormant. A stranger's agent waking reliably on inbound messages is part of the unassisted-follower experience; the blocking Stop-hook + `.famp-listen` sentinel convention is the brittlest part of onboarding someone new.

- [ ] **WATCH-01**: `famp watch --notify <command>` runs a command per arriving envelope for a bound identity.
- [ ] **WATCH-02**: No shell injection — envelope metadata reaches the command via environment variables, never interpolated into a shell string.
- [ ] **WATCH-03**: The notification payload obeys QUAR-06 (no attacker-controlled body text).
- [ ] **WATCH-04**: Ships with **zero** `famp-bus` change, preserving the permanent `just check-no-tokio-in-bus` gate.
- [ ] **WATCH-05**: Behavior on restart is defined and tested — either missed notifications are replayed from the mailbox cursor, or the loss window is explicitly bounded and documented.

### Distribution (DIST)

Today's only install path is `cargo install famp` — install rustup, then compile 15 crates. Phase 19 (Human Acceptance Gate)'s DOC-07 requires validating the setup guide on a fresh machine with no prior FAMP state; a fresh machine has no Rust toolchain either, so distribution must ship before that gate is reachable.

- [ ] **DIST-01**: A tagged release publishes prebuilt `famp` binaries for macOS arm64, macOS x86_64, and Linux x86_64 as downloadable release artifacts.
- [ ] **DIST-02**: A single documented command installs a working `famp` on a machine with **no Rust toolchain**, proven on a clean environment with no prior FAMP state.
- [ ] **DIST-03**: Published artifacts carry checksums, and the installer **verifies** them before installing — a corrupted or substituted artifact fails closed.
- [ ] **DIST-04**: The onboarding docs **lead with** the binary install path; `cargo install famp` remains documented only as the from-source fallback.
- [ ] **DIST-05**: Release artifacts are produced **only** by the tag-triggered workflow — no hand-built or manually uploaded binaries.

### Documentation & Acceptance (DOC / UAT)

- [ ] **DOC-06**: A follower-facing setup guide takes a second person from zero to a working paired gateway. Gated by **semantic** assertions, not flag-greps — v1.0 shipped `GATEWAY-SETUP.md` with its wiring instructions inverted and a flag-grep gate passed it.
- [ ] **DOC-07**: The guide is validated end-to-end on a **fresh machine with no prior FAMP state and no Rust toolchain** before the real human gate, exercising the **prebuilt-binary install path** (DIST-02) rather than `cargo install` — a fresh machine has no Rust, which is the entire point. **[ORCH]** — added as cheap insurance for the acceptance event; it is doc validation on a clean box, not a second human gate.
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
- **INGR-09**: Broker-side recipient-existence check before mailbox auto-vivification. `famp-bus`'s `send_agent`/`AppendMailbox` creates a mailbox for any `to` name with no existence check — found during Phase 17 planning, code-grounded, correctly scoped out of that phase because the fix lands in `famp-bus`, not `famp-gateway`. Severity is bounded, not open: reaching `AppendMailbox` requires passing `verify_inbound_any`, so the sender is an already-pinned peer — this is trust-abuse by a peer deliberately trusted, not anonymous-attacker DoS. Worth closing before the peer set is ever wider than people Ben has personally paired with.

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
| REACH-01 | Phase 13 | Pending |
| REACH-02 | Phase 13 | Pending |
| REACH-03 | Phase 13 | Pending |
| REACH-04 | Phase 17 | Loopback proven — 17-05's relay-fetch loop + three-process bidirectional e2e (`e2e_relay_bidirectional.rs`) passes with no direct peer address in either gateway's config. Genuinely-different-networks leg still PENDING (blocked on Ben, a second physical network); checkbox left unticked above pending that leg, per the Phase 10 DOC-04 precedent — do not read this row as closing the requirement. |
| REACH-05 | Phase 17 | Complete |
| KEYR-01 | Phase 15 | Complete |
| KEYR-02 | Phase 15 | Complete |
| KEYR-03 | Phase 15 | Complete |
| PAIR-01 | Phase 16 | Pending |
| PAIR-02 | Phase 16 | Pending |
| PAIR-03 | Phase 16 | Pending |
| PAIR-04 | Phase 16 | Pending |
| PAIR-05 | Phase 16 | Pending |
| PAIR-06 | Phase 16 | Pending |
| PAIR-07 | Phase 16 | Pending |
| PAIR-08 | Phase 16 | Pending |
| DIR-01 | Phase 20 | Pending |
| DIR-02 | Phase 20 | Pending |
| DIR-03 | Phase 20 | Pending |
| INGR-01 | Phase 17 | Complete |
| INGR-02 | Phase 17 | Complete |
| INGR-03 | Phase 17 | Complete |
| INGR-04 | Phase 17 | Complete |
| INGR-05 | Phase 17 | Complete |
| INGR-06 | Phase 17 | Complete |
| INGR-07 | Phase 17 | Complete |
| INGR-08 | Phase 17 | Complete |
| REVK-01 | Phase 15 | Complete |
| REVK-02 | Phase 15 | Complete |
| REVK-03 | Phase 15 | Complete |
| QUAR-01 | Phase 14 | Complete |
| QUAR-02 | Phase 14 | Complete |
| QUAR-03 | Phase 14 | Complete |
| QUAR-04 | Phase 14 | Complete |
| QUAR-05 | Phase 14 | Complete |
| QUAR-06 | Phase 14 | Complete |
| QUAR-07 | Phase 14 | Pending |
| QUAR-08 | Phase 14 | Complete |
| QUAR-09 | Phase 14 | Complete |
| QUAR-10 | Phase 14 | Complete |
| QUAR-11 | Phase 14 | Complete |
| WATCH-01 | Phase 21 | Pending |
| WATCH-02 | Phase 21 | Pending |
| WATCH-03 | Phase 21 | Pending |
| WATCH-04 | Phase 21 | Pending |
| WATCH-05 | Phase 21 | Pending |
| DIST-01 | Phase 18 | Pending |
| DIST-02 | Phase 18 | Pending |
| DIST-03 | Phase 18 | Pending |
| DIST-04 | Phase 18 | Pending |
| DIST-05 | Phase 18 | Pending |
| DOC-06 | Phase 19 | Pending |
| DOC-07 | Phase 19 | Pending |
| UAT-02 | Phase 19 | Pending |

**Coverage:**

- v1 requirements: 54 total. *(Count history: 41 was recorded at definition time and was simply wrong — exhaustive ID extraction during roadmap creation found 43. It became 46 when the independent design review added QUAR-09, QUAR-10, and QUAR-11 — but PAIR-06/07/08 were added to this doc's body in that same period without ever being added to this table, so the true count was already 49, not 46; that gap sat undetected until this update. It became 54 on 2026-08-02: +5 for DIST-01..05 (Ben-approved distribution phase, inserted before the Human Acceptance Gate) and the PAIR-06/07/08 traceability gap closed in the same pass. ROADMAP.md's summary line previously carried a stale 43/43 — corrected to 54/54 in both places.)*
- Mapped to phases: 54/54 ✓
- Unmapped: 0 ✓

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
*Last updated: 2026-08-02 — added DIST-01..05 (Distribution) as Phase 18, inserted before the Human Acceptance Gate (now Phase 19, was 18); Signed Peer Directory and Push Notification Adapter shifted to Phases 20 and 21 respectively. DOC-07 reworded to require the fresh-machine validation exercise the prebuilt-binary install path, not `cargo install`. Also closed a pre-existing traceability gap found while verifying this update's own arithmetic: PAIR-06/07/08 existed in this doc's body but were never added to the table below — added, mapped to Phase 16 alongside PAIR-01..05. All 54 v1 requirements mapped to Phases 13-21, 100% coverage, no orphans. Tool-gating (the OPEN SCOPE DECISION above) is NOT resolved by this update — it failed independent adversarial review and is on hold pending Ben's ruling; do not treat its absence here as a decision. See ROADMAP.md.*
