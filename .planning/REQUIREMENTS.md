# Requirements: FAMP v1.1 Open-Internet Federation

**Defined:** 2026-07-30
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Milestone acceptance (an event, not a person):** an agent on Ben's machine and an agent on a second person's machine, in different networks with **no shared VPN** and **no hand-copied keys**, exchange signed envelopes in **both** directions and both task FSMs reach a terminal state. That person follows a doc **unassisted**.

> **Scoping note.** These requirements were scoped by the orchestrator on 2026-07-30 while Ben was away, under his standing authorization to "plan + execute everything that doesn't need me or a 2nd person," with reachability spend pre-authorized to ~$15/mo. Every judgment call the orchestrator made rather than deferred is marked **[ORCH]** with its rationale, so any of them can be vetoed cheaply on review.

> ## ✅ SCOPE DECISION RESOLVED — Option B (Ben, 2026-08-02)
>
> **v1.1 delivers machine-checkable provenance plus honest documentation. It does NOT ship harness-level tool-gating.** The milestone's security-gate sentence (QUAR section below) is corrected to state only what ships, not the "hard boundary" the original brief asked for.
>
> **Why, recorded as fact rather than opinion** — an independent adversarial review of the proposed Option A (`PreToolUse` tool-gating) design found four defeats, three of them structural:
>
> 1. **MCP-only arming, non-MCP bypass.** The gate would arm when the `famp` MCP server serves remote-origin content, but the MCP server is not the only consumer of remote content: `crates/famp/src/cli/inbox/list.rs:75` renders quarantine-wrapped remote bodies with zero references to MCP session state, so `famp inbox list` run through Bash delivers remote content to the model with the gate never arming. This bypass was demonstrated accidentally, in normal use, before the gate was even designed.
> 2. **Provenance laundering defeats it mesh-wide.** `crates/famp/tests/quarantine/laundering.rs` documents and TESTS (QUAR-11) that remote content re-emitted by a local agent arrives stamped `origin: local` and renders unmarked. One non-hooked peer in the mesh — a Codex or Grok window, both shipped plugins — disarms the gate for every agent behind it.
> 3. **Denylist, not deny-by-default.** The denied-tool set is a blocklist that omits sub-agent dispatch and every third-party MCP server the operator has installed.
> 4. **No session key.** There is no `session_id` anywhere in `crates/famp/src/cli/mcp/`, so the marker cannot even be keyed to a Claude Code session.
>
> **Conclusion, recorded explicitly:** shipping the gate would have licensed the claim "tool execution is blocked once remote content enters the turn," which is false in the common case. An operator believing that might connect to a peer they would otherwise refuse — that moves real risk from disclosed to believed-handled, which is worse than the honest documentation already in `docs/QUARANTINE.md`.
>
> **What this is NOT:** not a requirement for a replacement mechanism. A tools-restricted listener profile is under independent adversarial review separately; nothing is committed to it here, and its absence from this document should not be read as a decision either way — promising a replacement before it survives review would repeat exactly the mistake being corrected here.
>
> **QUAR-01..11 are unaffected and stand as shipped**: provenance is real, it works, and it is the prerequisite for any future enforcement. Only the claim about what it alone buys was wrong — not the work itself.
>
> **Update, same day: the auto-wake gate ships (QUAR-12..15).** Both dead designs above attacked the TOOLS leg and the TRANSMIT leg of the lethal trifecta. Neither attacked **automatic ingestion** — the one leg FAMP owns end to end via the broker, not the harness. Enforcing there, not in a harness FAMP does not control, is why this one survives adversarial review where the other two didn't. See the QUAR section below for the actual requirements.
>
> **The road not taken, recorded so it isn't silently re-lost:** the strictly stronger design is **held-by-default-at-ingress** — a remote envelope is held at the broker's ingress append site and never enters the recipient's mailbox at all until a human releases it, or the peer has a standing per-peer auto-deliver grant. It answers all four defeats above by construction, and it is the natural upgrade path if the threat model ever demands more than "doesn't auto-wake." **Deliberately not v1.1** — the auto-wake gate delivers most of the value at a fraction of the build. Deferred, not rejected — do not re-derive this from scratch later.

---

## v1 Requirements

### Reachability (REACH)

Public reachability over the open internet. The model is decided **first**, in a zero-code spike, because it carries recurring cost and operator burden.

- [x] **REACH-01**: A decision record names the chosen reachability model, its **re-verified live** cost/month (vendor pricing pages, not aggregators), the named operator, and explicitly what the relay/tunnel **can and cannot observe** about FAMP traffic.
- [x] **REACH-02**: The spike's viability finding is validated against a **real network Ben does not control** (e.g. a carrier hotspot, public Wi-Fi) — not only networks he controls.

  **Validated 2026-08-02, Verizon cellular hotspot, two runs.** Public IP `174.228.224.145`, ownership verified via reverse DNS (`145.sub-174-228-224.myvzw.com`, AS6167 Verizon Business) — not inferred from address shape. Outbound HTTPS 443: 200/200. Port 8443: not blocked (301/301) — the relay is not forced onto port 443. Inbound (probed *from* the Lightsail relay `54.158.102.139` into the hotspot): TCP 9999 timed out both runs; run 2 added a positive control — the relay first proved it could reach `1.1.1.1:443` before failing to reach the hotspot, ruling out "the relay has no outbound networking" as an alternative explanation. NAT type: 4 STUN servers agreed on the same external mapping within each run => **cone NAT**, not symmetric. Full write-up: `.planning/phases/13-public-reachability-decision-spike/13-REACH-02-RESULTS.md`.

  **Why the wording changed from "symmetric-NAT" to "a network Ben does not control."** The original wording said "symmetric-NAT" because a single 2016 citation (~40% symmetric-dominant on cellular) predicted the test environment would be symmetric — a citation the research pass had *already* flagged as unverifiable and a decade stale (`13-PRICING-VERIFIED.md`). "Symmetric" was a **prediction about the test environment**, not a property the requirement's design actually needed; testing falsified the prediction (this carrier is cone NAT). The requirement's actual intent — validate against a network Ben doesn't control, not only ones he does — is fully satisfied regardless of NAT flavor. **Critically, relay-first does not depend on the NAT being symmetric**: it is confirmed by the inbound result alone (nothing can dial in), which holds on every NAT flavor, cone included. Stated explicitly so this conclusion is never read as resting on the falsified premise.

- [x] **REACH-03**: `iroh` is explicitly weighed as the single-crate alternative and its rejection rationale (transport-migration cost against a shipped, Gate-A-proven axum/rustls transport) is recorded in the decision record rather than silently dropped.
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

Settled before any outside person connects. Delivers machine-checkable provenance: every mailbox append is fail-closed stamped `origin: local` | `origin: gateway` (never defaulting to trusted), and remote-origin content is structurally quarantined at every rendering surface, mechanically gated in CI. Structural and harness-agnostic — **not** a prompt convention, and **not** enforced in `~/.claude` wiring. **This is the prerequisite for enforcement, not enforcement itself — it does NOT prevent a remote agent from steering a local agent by sending it text.** See the resolved scope decision above and `docs/QUARANTINE.md`.

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

**The auto-wake gate (QUAR-12..15, added 2026-08-02, resolving the tool-gating scope decision above).** Two dead designs (a `PreToolUse` hook, a tools-restricted listener profile) both tried to enforce in the harness, which FAMP does not control. Neither attacked the one leg of the lethal trifecta FAMP owns end to end: **automatic ingestion** — whether a parked `famp await` wakes at all. Enforcing that in the broker, which FAMP does control, is a real boundary rather than a claim about one. `AwaitFilter` already exists at `crates/famp-bus/src/proto.rs:54` and `BUS_PROTO_VERSION` is already 2 — the likely integration point, named here as context, not a design decision; the implementation is plan-phase work.

- [ ] **QUAR-12**: A remote-origin (non-`Local`) envelope does **not** satisfy a parked `famp await` — it never auto-wakes an idle agent. Local-origin traffic is unaffected; the same-host mesh's auto-wake behavior is unchanged.
- [ ] **QUAR-13**: The filter is enforced **broker-side**, never in the CLI drain. A client-side filter would advance the read cursor past envelopes it declined to deliver — the 999.1 failure class.
- [ ] **QUAR-14**: Proven by tests, not by inspection: (a) a gateway-origin envelope delivered to a parked awaiter does **not** wake it; (b) that same envelope **is** visible on the next human-initiated inbox read — held back from auto-wake, never dropped; (c) a local-origin envelope **does** still wake a parked awaiter. All three, or the gate is either vacuous or a data-loss bug.
- [ ] **QUAR-15**: The consent warning lives in the **pairing artifact** (DOC-06), at the moment of consent — not only in `docs/QUARANTINE.md`, which the person who most needs the warning will never open. Wording to adapt: *pairing with a peer means their agent's messages will be read by your agent, which can run commands on your machine — pair only with someone you'd let type into your terminal.*

### Push Notification Adapter (WATCH) — SEED-002

Promoted from dormant. A stranger's agent waking reliably on inbound messages is part of the unassisted-follower experience; the blocking Stop-hook + `.famp-listen` sentinel convention is the brittlest part of onboarding someone new.

- [ ] **WATCH-01**: `famp watch --notify <command>` runs a command per arriving envelope for a bound identity.
- [ ] **WATCH-02**: No shell injection — envelope metadata reaches the command via environment variables, never interpolated into a shell string.
- [ ] **WATCH-03**: The notification payload obeys QUAR-06 (no attacker-controlled body text).
- [ ] **WATCH-04**: Ships with **zero** `famp-bus` change, preserving the permanent `just check-no-tokio-in-bus` gate.
- [ ] **WATCH-05**: Behavior on restart is defined and tested — either missed notifications are replayed from the mailbox cursor, or the loss window is explicitly bounded and documented.

### Distribution (DIST)

Today's only install path is `cargo install famp` — install rustup, then compile 15 crates. Phase 19 (Human Acceptance Gate)'s DOC-07 requires validating the setup guide on a fresh machine with no prior FAMP state; a fresh machine has no Rust toolchain either, so distribution must ship before that gate is reachable.

- [x] **DIST-01**: A tagged release publishes prebuilt binaries for macOS arm64, macOS x86_64, and Linux x86_64 as downloadable release artifacts. **Binary set: `famp`, `famp-gateway`, and `famp-relay`** — widened 2026-08-02 (Ben-approved) from the original "`famp` binaries" wording, which was a requirement-text gap: `famp-gateway` is a separate `[[bin]]` target that `docs/GATEWAY-SETUP.md` requires on `PATH`, so shipping only `famp` would leave Phase 20's second person unable to federate — the exact gap this phase exists to close. `famp-relay` is a third bin target and is near-free once the matrix exists. See `.planning/phases/16-distribution/16-CONTEXT.md` D-02.
- [ ] **DIST-02**: A single documented command installs a working `famp` on a machine with **no Rust toolchain**, proven on a clean environment with no prior FAMP state.
- [x] **DIST-03**: Published artifacts carry checksums, and the installer **verifies** them before installing — a corrupted or substituted artifact fails closed.
- [ ] **DIST-04**: The onboarding docs **lead with** the binary install path; a **working** from-source command remains documented only as the fallback. Corrected 2026-08-02 (Ben-approved): this requirement originally named `cargo install famp`, which presumed that command works. It does not — `famp` was never published to crates.io (VERIFIED twice against the crates.io API), yet six doc sites instruct users to run it (`README.md:192`, `docs/GETTING-STARTED.md:43`, `docs/GATEWAY-SETUP.md:24`, `docs/ONBOARDING.md:12,26,32,37`). The fallback becomes the `--path`/`--git` from-source form and crates.io publication is explicitly NOT undertaken. See `.planning/phases/16-distribution/16-CONTEXT.md` D-01.
- [x] **DIST-05**: Release artifacts are produced **only** by the tag-triggered workflow — no hand-built or manually uploaded binaries.

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

- **Signed Peer Directory (DIR-01..04), cut from v1.1 scope (2026-08-02, Ben approved).** DIR-01..03 (the signed key list itself) were a v1.1 requirement until this date; cut because the cost/benefit only turns positive at a peer count this milestone will not reach — it publishes a signed key list that is explicitly never a trust anchor (DIR-03) for a peer set of two, over a ceremony that happens once per peer, and it adds a **second write path** into the keyring integration point Phase 15 just spent four plans hardening. **Trigger: revisit when the peer set exceeds ~5** — the point manual pairing actually becomes the bottleneck it exists to relieve — same event-driven pattern as Gate B below.
  - **DIR-01**: A `famp-directory` crate publishes a signed, TTL-bounded peer key list, canonicalized with the existing RFC 8785 JCS path and signed with the existing Ed25519 substrate.
  - **DIR-02**: A consumer verifies the directory signature and rejects stale, expired, or unsigned entries **fail-closed**.
  - **DIR-03**: The directory never becomes an implicit trust anchor — an entry present in a signed directory is **not** sufficient to pin a peer; explicit pinning is still required.
  - **DIR-04**: Directory-based automatic peer discovery (as opposed to the signed key list of DIR-01..03). Discovery without explicit pinning re-opens exactly the trust question DIR-03 closes. Deferred independently of the DIR-01..03 cut above — this one was never in v1.1 scope.
- **REACH-06**: NAT hole-punching as an optimization over relay fallback. Build the fallback first. *(Corrected 2026-08-02: earlier wording here called hole-punching categorically impossible, citing a stale, unverifiable "15–30% symmetric NAT" figure. Measured 2026-08-02 on one real carrier (Verizon cellular): cone NAT, not symmetric — hole-punching could work there given a coordination server. That is a single-carrier, single-day measurement, not a prevalence claim: viable on at least one real carrier, not viable universally. Relay fallback remains mandatory regardless — REACH-02's inbound-dial-in result holds on every NAT flavor, so this does not change the decision, only the framing of why hole-punching is deferred rather than ruled out.)*
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
| REACH-01 | Phase 13 | Complete — see `.planning/phases/13-public-reachability-decision-spike/13-DECISIONS.md`; verified against Phase 17's shipped implementation 2026-08-02, no divergence |
| REACH-02 | Phase 13 | Complete — validated 2026-08-02 on a real Verizon cellular hotspot (two runs, cone NAT); see `.planning/phases/13-public-reachability-decision-spike/13-REACH-02-RESULTS.md` and `.planning/REACH-02-HOTSPOT-WALKTHROUGH.md` (executed) |
| REACH-03 | Phase 13 | Complete — see `.planning/phases/13-public-reachability-decision-spike/13-DECISIONS.md` |
| REACH-04 | Phase 17 | Loopback proven — 17-05's relay-fetch loop + three-process bidirectional e2e (`e2e_relay_bidirectional.rs`) passes with no direct peer address in either gateway's config. Genuinely-different-networks leg still PENDING (blocked on Ben, a second physical network); checkbox left unticked above pending that leg, per the Phase 10 DOC-04 precedent — do not read this row as closing the requirement. |
| REACH-05 | Phase 17 | Complete |
| KEYR-01 | Phase 15 | Complete |
| KEYR-02 | Phase 15 | Complete |
| KEYR-03 | Phase 15 | Complete |
| PAIR-01 | Phase 18 | Pending |
| PAIR-02 | Phase 18 | Pending |
| PAIR-03 | Phase 18 | Pending |
| PAIR-04 | Phase 18 | Pending |
| PAIR-05 | Phase 18 | Pending |
| PAIR-06 | Phase 18 | Pending |
| PAIR-07 | Phase 18 | Pending |
| PAIR-08 | Phase 18 | Pending |
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
| QUAR-12 | Phase 19 | Pending |
| QUAR-13 | Phase 19 | Pending |
| QUAR-14 | Phase 19 | Pending |
| QUAR-15 | Phase 19 | Pending |
| WATCH-01 | Phase 21 | Pending |
| WATCH-02 | Phase 21 | Pending |
| WATCH-03 | Phase 21 | Pending |
| WATCH-04 | Phase 21 | Pending |
| WATCH-05 | Phase 21 | Pending |
| DIST-01 | Phase 16 | Complete |
| DIST-02 | Phase 16 | Pending |
| DIST-03 | Phase 16 | Complete |
| DIST-04 | Phase 16 | Pending |
| DIST-05 | Phase 16 | Complete |
| DOC-06 | Phase 20 | Pending |
| DOC-07 | Phase 20 | Pending |
| UAT-02 | Phase 20 | Pending |

**Coverage:**

- v1 requirements: 55 total. *(Count history: 41 → 43 (tally correction) → 46 (QUAR-09/10/11 added) → 49 (PAIR-06/07/08 traceability gap found and closed) → 54 on 2026-08-02, first pass (+5 DIST-01..05) → 55 on 2026-08-02, second pass this same day: -3 for DIR-01/02/03 cut from v1 scope and moved to the deferred/backlog section (event-driven, peer count > ~5), +4 for QUAR-12..15 (the broker-side auto-wake gate, resolving the tool-gating scope decision). Net this pass: -3+4 = +1, 54→55.)*
- Mapped to phases: 55/55 ✓ — re-verified mechanically (every **XXX-NN** ID in the v1 body diffed against every traceability row, zero gaps either direction)
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
*Last updated: 2026-08-02 (third pass) — two independent reviews (adversarial security + right-sizing) landed and Ben approved the outcome. Added QUAR-12..15: a broker-side auto-wake gate — a remote-origin envelope never satisfies a parked `famp await` — enforcing the one trifecta leg (automatic ingestion) FAMP owns end to end, unlike the two dead harness-side designs. Recorded held-by-default-at-ingress as the known-stronger road not taken, deferred not rejected. Cut DIR-01/02/03 (Signed Peer Directory) from v1.1 scope entirely, merged into the deferred/backlog section with DIR-04 under an event-driven trigger (peer count > ~5), same pattern as Gate B — the phase is removed from ROADMAP.md, not just its requirements. Net count this pass: -3 (DIR) +4 (QUAR) = +1, 54→55. Phase renumbering (ROADMAP.md): Distribution 18→16, Pairing 16→18, new Auto-Wake Gate phase created at 19, Human Acceptance Gate 19→20, Push Notification Adapter stays 21 — reflecting Ben's approved critical-path order (13 → Distribution → Pairing → auto-wake gate → REACH-02/04 validation → Human Acceptance Gate); Phases 14/15/17 (already executed) keep their numbers unchanged. All 55 v1 requirements mapped, re-verified mechanically (every ID in the body diffed against every traceability row, zero gaps). (Second pass, same day: OPEN SCOPE DECISION resolved to Option B, tool-gating claims corrected across REQUIREMENTS/PROJECT/ROADMAP.md and docs/QUARANTINE.md. First pass, same day: added DIST-01..05 as Phase 18, closed a PAIR-06/07/08 traceability gap.) See ROADMAP.md.*

*Last updated: 2026-08-02 (fourth pass) — checked REACH-01 and REACH-03. The Phase 13 decision record existed as an unfiled DRAFT (`.planning/research/REACH-DECISION-DRAFT.md`, committed 26c95d7, amended 61909cb) satisfying both requirements' literal text but never promoted or checked off — bookkeeping gap, not missing analysis. Verified against Phase 17's shipped code before promoting (store-and-forward shape, the signed-fetch authorization mechanism, the plaintext-not-opaque confidentiality claim) — no divergence between decided and shipped, so no reconciliation was needed. Promoted and filed as `.planning/phases/13-public-reachability-decision-spike/13-DECISIONS.md` + `13-PRICING-VERIFIED.md`; the loose `research/` copies were removed now that the phase directory is the canonical location. REACH-01 and REACH-03 now checked; REACH-02 and REACH-04 stay UNCHECKED — both need Ben on a second real network, and a good decision record is not evidence of that. Also fixed a stale "opaque bytes" claim in ROADMAP.md's Phase 13 success criteria (the same error the decision record itself corrected on 2026-07-31 but which had survived in the roadmap's own text). No requirement ID added, removed, or renamed; count stays 55/55.*

*Last updated: 2026-08-02 (fifth pass) — REACH-02 run and checked. Ben ran the hotspot walkthrough twice on a real Verizon cellular hotspot; results at `.planning/phases/13-public-reachability-decision-spike/13-REACH-02-RESULTS.md`. Reworded REACH-02 from "real symmetric-NAT network" to "real network Ben does not control" — the original wording named a NAT flavor the design never actually needed; it was a stale, unverified 2016-citation-driven prediction about what the test environment would turn out to be, and testing falsified it (this carrier is cone NAT, not symmetric). The requirement's real intent — a network outside Ben's control — is satisfied regardless, and relay-first is confirmed independently of NAT flavor by the inbound-dial-in result alone. Corrected REACH-06 in the v2 backlog, which had called hole-punching categorically impossible off the same stale figure; reworded to "viable on at least one real carrier (measured), not viable universally, relay fallback remains mandatory." REACH-04 stays UNCHECKED — still needs two gateways on genuinely different networks exchanging bidirectionally; a provisioned relay box is not that. No requirement ID added, removed, or renamed; count stays 55/55, re-verified mechanically (every ID in the v1 body diffed against every traceability row, zero gaps either direction).*
