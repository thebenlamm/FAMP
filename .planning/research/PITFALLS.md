# Pitfalls Research

**Domain:** Open-internet federation for a signed agent-messaging protocol — adding public reachability, cross-person trust bootstrap, protocol-grade ingress, and a harness-agnostic inbound-content-is-DATA boundary to an already-shipped two-machine/one-operator gateway (FAMP v1.0.0)
**Researched:** 2026-07-30
**Confidence:** MEDIUM overall. HIGH on FAMP-internal history (own shipped incidents, cited from PROJECT.md/ARCHITECTURE.md/GATEWAY-SETUP.md). MEDIUM on cross-checked external claims (Simon Willison's lethal trifecta, OWASP LLM Top 10 2025, CaMeL/AgentDojo, RFC-adjacent NAT/replay-cache mechanics — corroborated across ≥2 independent sources). LOW on single-source web claims, flagged individually below.

> This document extends `.planning/research/archive/v0.6/PITFALLS.md` (canonical JSON / Ed25519 / FSM pitfalls, still valid and untouched by v1.1 since Layer 0 primitives are frozen per PROJECT.md). It does not restate those. It also extends the project's own lived incident history: the v1.0 Phase-11 discovery that green CI proved nothing about shipping-client remote addressing, the inverted `GATEWAY-SETUP.md` wiring that a flag-grep gate passed, the BUS-11 unreadable-envelope bug only a real two-process E2E caught, and the sender-`from` forgery hole only adversarial review caught. Section 6 (prompt injection) is the milestone's declared BLOCKING security gate and is written to the highest rigor in this document.

---

## Critical Pitfalls

### Pitfall 1: Relay becomes an unacknowledged trust anchor and metadata-leak point

**What goes wrong:** Once phase-1 picks a relay/tunnel model (self-hosted VM, hosted tunnel service, or direct-dial), every envelope's *size, timing, cadence, source/destination pairing* passes through a third party even though the envelope body is Ed25519-signed. The signature protects integrity/authenticity of the payload; it does nothing about who-talked-to-whom-when. Teams ship "the relay never sees plaintext, so it's fine" without naming what it *does* see, and without accounting for the relay operator (a VM you or a vendor runs) as a party who can go down, get subpoenaed, get compromised, or silently start logging.

**Why it happens:** The team's mental model is "signing solves trust," carried over correctly from the wire layer (INV-10) but incorrectly extended to the relay layer, which is a *reachability* concern, not a *content-trust* concern. The v1.0 Phase-1 spike framing ("cost/month + named operator") already anticipates the operational half of this but not the metadata-leak half.

**How to avoid:** (a) Write the relay's threat model explicitly as a requirement artifact before implementation: "the relay can see: X, Y, Z; the relay cannot see: A, B, C" — and test that list, don't just assert it. (b) If self-hosting, treat the relay VM like any other production dependency: monitoring, an on-call name, patch cadence — not a "spin it up and forget it" box (this is the same class of mistake as `56b2293`'s pre-daemon orphan-broker problem, just at a different layer). (c) Log at the relay only connection metadata needed for operations (uptime, byte counts) — never envelope headers/timestamps beyond what TLS already exposes on the wire, and document that TLS itself already leaks size/timing to any network observer regardless of relay choice.

**Warning signs:** The phase-1 spike decision doc names a cost/month and an operator but has no explicit "what can this box see" line. No monitoring/alerting attached to the relay. "Relay is stateless so it doesn't matter" asserted without a test proving no envelope content is persisted or logged.

**Phase to address:** Phase 13 (the zero-code reachability spike) — the decision record itself must contain the metadata-exposure line, not just cost/operator.

---

### Pitfall 2: Relay outage is a silent single point of failure with no signal to either peer

**What goes wrong:** v1.0's `famp send` already has a known fire-and-forget boundary (documented in `GATEWAY-SETUP.md`: "a zero exit code confirms only that the local broker accepted the envelope... egress is a decoupled background drain loop"). Layer a flaky third-party relay under that boundary and the failure mode compounds: sender sees success (local accept), relay is actually down, message never crosses, and *nothing* tells either human. This is worse than the direct-VPN case in v1.0 because a VPN outage is usually visible to the operator (network drops entirely); a relay outage while the local network stays up is invisible.

**Why it happens:** The existing `famp inspect tasks` verification step (documented as the actual proof of delivery) was designed and dogfooded against a same-operator two-machine setup where checking both sides was trivial. Cross-person, checking "did it land on the other human's inspect output" requires that human to be actively looking — the exact "unassisted follower" failure class the milestone explicitly worries about.

**How to avoid:** (a) Build a relay health check into the gateway itself — the gateway should periodically probe its own egress path and surface `RELAY_UNREACHABLE` the same way `famp inspect broker` already surfaces `DOWN_CLEAN`/`STALE_PID`/`ORPHAN_HOLDER` for the local broker. (b) Do not let "the send command exited 0" be the acceptance criterion in the Phase 2-3 human gate — require the *receiving* human's own `famp inspect tasks` output as the pass criterion, matching the v1.0 UAT-01 pattern that already works. (c) Consider a heartbeat/keepalive envelope class so a stalled relay is detected within minutes, not "whenever someone happens to check."

**Warning signs:** The human acceptance gate's pass criterion is "sender saw exit 0" instead of "receiver confirmed via inspect." No test simulates relay-down-but-local-broker-up.

**Phase to address:** Whichever phase builds the chosen relay/tunnel integration (post Phase-13 spike); the human gate phase (2-3) must encode the receiver-side proof requirement in its UAT script.

---

### Pitfall 3: Symmetric NAT and hole-punching failures have no fallback path built

**What goes wrong:** NAT hole punching (STUN-style) only works when both NATs use endpoint-independent (non-symmetric) mapping. Symmetric NAT allocates a *different* external port per destination, so the address a STUN server observes cannot be reused by the peer attempting to connect back — hole punching fails deterministically, not intermittently, for this NAT class. An estimated 15-30% of real-world hosts sit behind symmetric NAT, CGNAT, or restrictive firewalls. Teams that build only the hole-punch path and treat the relay as a rare fallback discover in the field that the "rare" fallback is needed for a meaningful fraction of real users — including possibly the second person in this milestone's own acceptance test, whose network FAMP does not control.

**Why it happens:** Hole-punch demos work great on the developer's own network (often not behind symmetric NAT) and the failure only appears on someone else's network — exactly the class of bug the milestone already flags as its top risk ("no shipping client could address a remote principal" from v1.0 Phase 11 arrived at the final human gate for an analogous reason: it worked for the builder, not the real second party).

**How to avoid:** (a) Do not build hole-punching as the primary path with relay as an afterthought; build and test the TURN-style relay fallback *first*, since it is the only path that unconditionally works, then layer direct/hole-punch as an optimization. (b) In the Phase-13 spike, explicitly test against at least one symmetric-NAT network (a hotspot on many carrier networks is a cheap and reliable way to get one) — do not validate reachability only from networks you control. (c) Record which of "direct-dial + port forward," "hole punch," or "always-relay" was chosen and why cost/operator matters most for the rejected paths — this is exactly the recorded-decision deliverable the milestone already requires.

**Warning signs:** Reachability testing was only ever done between machines Ben controls (repeating the v1.0 own-machines-first blind spot one layer up). No test uses a real symmetric-NAT network. The design doc treats "P2P direct" as the common case and relay as an edge case without evidence.

**Phase to address:** Phase 13 (spike) must include a symmetric-NAT test network as a hard requirement, not an optional nice-to-have.

---

### Pitfall 4: TLS cert issuance/renewal on the relay/gateway box silently expires with no owner watching

**What goes wrong:** v1.0's own `GATEWAY-SETUP.md` already documents two divergent, easy-to-get-wrong TLS pitfalls (macOS SecTrust requires `serverAuth` EKU; Linux webpki rejects `CA:TRUE` on a leaf cert) discovered only in the Gate A dogfood. Add a relay/public listener and add a *renewal* dimension: an 800-day self-signed cert (the exact recipe GATEWAY-SETUP.md ships) will expire in the middle of this milestone's real-world life, on a box that — per the milestone's second-person acceptance requirement — is not being watched by Ben.

**Why it happens:** Self-signed certs with long validity feel "done" once they verify once. Nobody wires up an expiry alert because the initial setup already required so much manual TLS wrangling that "get it working once" felt like the finish line.

**How to avoid:** (a) Reuse the `famp inspect broker`-style proactive health-check pattern for cert expiry: a `famp inspect gateway` (or equivalent) command that reports days-until-expiry, so a person checking in occasionally sees it before it's a crisis. (b) If using a hosted tunnel service (one option the Phase-13 spike will weigh), prefer one with automatic cert rotation (e.g., Let's Encrypt-backed) over another hand-rolled 800-day self-signed cert — this is a strong argument in the cost/operator tradeoff the spike must record. (c) Regression-test the exact GATEWAY-SETUP.md TLS recipe stays valid for whatever new listener code ships — don't let a new listener quietly need a third, undocumented EKU shape.

**Warning signs:** No expiry-check command exists. The chosen relay approach reuses the hand-rolled OpenSSL recipe from GATEWAY-SETUP.md verbatim without an automated renewal story.

**Phase to address:** Phase 13 spike (recorded decision should weigh renewal automation as a cost factor); the ingress-hardening phase should ship the health-check command.

---

### Pitfall 5: The relay becomes an open amplifier for third parties

**What goes wrong:** Any endpoint that accepts unauthenticated connections and forwards/relays traffic on behalf of a caller is a documented DDoS-amplification target class — TURN servers specifically are abused for exactly this reason ("attackers abuse TURN servers for DDoS because they're widely deployed, publicly accessible, and many are misconfigured without rate limiting" — a single attacker sending ~100 requests can generate millions of downstream connection attempts through a misconfigured relay). If FAMP's relay accepts any inbound connection before verifying it belongs to a known, pinned peer pair, it is structurally the same shape as an open relay.

**Why it happens:** The relay's job (get bytes from A to B when direct doesn't work) is easy to build in a way that also, incidentally, gets bytes from *anyone* to *anyone* — because rejecting unknown source/destination pairs is an extra step that isn't needed to pass the happy-path two-person test.

**How to avoid:** (a) The relay must reject any connection that doesn't map to a currently-pinned peer pair from the signed peer directory — before doing any forwarding work, not as a downstream filter. (b) Rate-limit per source IP and per claimed identity independently (see Pitfall 9 on rate-limiting-by-attacker-controlled-key). (c) Never build a "generic TCP/UDP relay" — build a FAMP-envelope-aware relay that can apply the DoS-ordering checks from Section 4 at the relay boundary too, not just at the final gateway ingress.

**Warning signs:** The relay accepts and forwards bytes for any destination without checking the sender/recipient pair against the peer directory. No per-source rate limit exists at the relay layer (only at the gateway).

**Phase to address:** Whichever phase builds the relay component; must be reviewed alongside Section 4 (DoS ordering) since it's the same class of boundary.

---

### Pitfall 6: TOFU silently re-pins on key change, defeating cross-person trust entirely

**What goes wrong:** v1.0's `famp peer import` already fails closed on *re-importing a different key for an already-pinned principal* ("Importing a different key for a principal that's already pinned is rejected outright — fails closed, no silent overwrite" — this is good and already correct per GATEWAY-SETUP.md). The v1.1 risk is a *regression* of this property when the trust-bootstrap flow is redesigned to be friendlier for a real human replacing hand-copied blobs: a "just click accept" or "auto-refresh peer directory entry" flow that re-pins on change without a loud, distinct signal is exactly the failure TOFU is famous for. Community-documented evidence (SSH's `known_hosts`, HPKP's deprecation) shows this is not hypothetical: SSH gets this right (loud, blocking `WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!` on key change vs silent accept on first use); HPKP got this wrong in the other direction (hard-failing on legitimate rotation bricked sites) and was deprecated as a result.

**Why it happens:** Designers optimize the "new peer" path for friendliness (the entire point of replacing hand-copied blobs) and then reuse the same friendly code path for "known peer, key changed" because it's less code — but these are semantically opposite events (welcome a stranger vs. detect a possible attack) and must not share a UI or exit code.

**How to avoid:** (a) Structurally separate "pin new principal" from "principal's key changed" in the code — different function, different exit code, different CLI message, ideally different color/icon if there's any UI. (b) "Key changed" must be a hard, unskippable stop by default (matching v1.0's existing `peer import` behavior) with an explicit, effortful override (not a single keypress) — e.g. requiring the fingerprint to be re-verified out-of-band, not just an "accept" button. (c) Add a regression test that pins peer A, then attempts to import a *different* key for the same principal, and asserts both the rejection and a distinct message/exit code from the "new peer, first pin" path — the same discipline that already caught the duplicate-pubkey brick in GATEWAY-SETUP.md.

**Warning signs:** The new trust-bootstrap flow's code path for "peer directory entry changed" reuses the "new peer" acceptance function. No test distinguishes exit codes/messages between the two cases. UX copy for "key changed" reads similarly friendly to "welcome, new peer."

**Phase to address:** The cross-person trust bootstrap phase — must ship the distinguishing test before the human acceptance gate (Phase 2-3), since this is the exact mechanism protecting against a swapped-identity attack on the person who is not Ben.

---

### Pitfall 7: The out-of-band verification channel is not actually out-of-band, or has no meaningful entropy/rate limit

**What goes wrong:** v1.0's `peer export`/`import` explicitly relies on "your own clipboard/Signal/whatever channel you already trust" as out-of-band — this only holds if that channel is genuinely independent of the channel being bootstrapped. If the replacement trust-bootstrap flow introduces any form of short code (a 6-digit pairing code, a short URL) sent *through the same network path being established* (e.g., emailed by an automated system that itself could be MITM'd, or displayed over an unauthenticated web page), it stops being out-of-band. Separately, short verification codes are a known weak point: Signal's own safety-number verification ceremony is documented (via published usability studies) as "nearly unusable in practice... hard to find, hard to use, and mostly misunderstood, even by experts" — meaning even a technically-sound short-code scheme fails if a real, non-expert second person won't actually perform it. A short code with too little entropy, or with no rate limit on guesses, is separately brute-forceable.

**Why it happens:** Designers assume "send a code over channel X" is inherently safe because it's not the primary wire, without asking whether channel X's own integrity depends on the thing being bootstrapped, and without user-testing whether a real non-technical second person will actually complete a manual comparison ceremony.

**How to avoid:** (a) Explicitly name the out-of-band channel in the design doc and verify its independence — "Signal/iMessage/a phone call" genuinely is independent of a FAMP relay; "an emailed link generated by the relay service" is not. (b) If using a short code, size it for the threat model (Signal-scale safety numbers are dozens of digits specifically because a 6-digit code is brute-forceable against an online verifier; add explicit rate limiting on attempts regardless of length). (c) Given the milestone's own framing — this is "the hard problem, not the transport" and the acceptance criterion is an *unassisted* second person — user-test the actual verification ceremony with a real non-technical person before the Phase 2-3 gate, the same way Signal's ceremony was found unusable only via real usability studies, not code review.

**Warning signs:** The out-of-band channel is generated or delivered by FAMP infrastructure itself. No fixed entropy/rate-limit spec exists for any short code. The verification ceremony has never been watched being performed by someone who didn't design it.

**Phase to address:** Cross-person trust bootstrap phase (design); Phase 2-3 human gate (validation with the real second person, unassisted).

---

### Pitfall 8: No bound on the verification window lets a stale, forgotten pairing be completed later by an attacker

**What goes wrong:** If a trust-bootstrap flow generates a pending pairing/invitation that stays valid indefinitely (e.g., "paste this exported line whenever"), an attacker who intercepts or guesses that pending invitation can complete it long after the legitimate parties moved on and stopped watching for it — the classic unbounded-verification-window failure. v1.0's `peer export`/`import` sidesteps this today because it's a synchronous copy-paste with no server-held pending state, but any replacement mechanism that introduces a server-mediated exchange (e.g., a directory-assisted pairing code) reintroduces exactly this risk.

**Why it happens:** Bootstrap flows are designed for the happy path where both humans complete the exchange within minutes; nobody sets an expiry because "why would it take longer than a few minutes" — until it does (a person steps away, comes back the next day, and the pairing code from yesterday still works).

**How to avoid:** (a) Any pending-pairing state must carry a short, explicit expiry (minutes, not hours) enforced server-side (or peer-side) with a hard rejection past that window. (b) Log/surface an explicit "this invitation expired" distinct from "this invitation was invalid" so a legitimate but slow user gets an actionable message instead of a silent failure.

**Warning signs:** Any new server-mediated pairing/invitation object has no `expires_at` field, or the expiry is measured in days.

**Phase to address:** Cross-person trust bootstrap phase, if the replacement design introduces any server-mediated (non-synchronous-copy-paste) exchange step.

---

### Pitfall 9: Nonce/replay cache design gets the TTL vs. clock-skew vs. cache-bound relationship backwards

**What goes wrong:** Three failure modes, all real and all independently documented in replay-cache literature: (1) an **unbounded** nonce cache grows forever and is a straightforward memory-exhaustion DoS vector for anyone who can generate valid-looking envelopes; (2) a TTL **shorter** than the tolerated clock-skew window rejects genuinely valid messages from a peer whose clock is a few seconds ahead/behind (a real risk here specifically because, per the milestone, this is now two machines under *two different operators* who do not share NTP/administrative control — v1.0's own-machines-first framing masked this because Ben's own machines were plausibly well-synced); (3) a TTL **longer** than the cache's actual retention bound (e.g., cache evicts on an LRU/size basis before the nominal TTL elapses under load) silently reopens the replay window even though the stated policy looks correct on paper.

**Why it happens:** These are two independently-tuned numbers (cache eviction policy, and the freshness/`expiry` field's semantic window) that must satisfy `cache_retention_window >= replay_TTL >= max_tolerated_clock_skew`, and it's easy to tune one without checking the inequality against the other two. Cited practice: TTL should be set to match the maximum allowed clock-skew window (commonly ~300s in real deployments) so that "by the time a nonce expires from the cache, the timestamp attached to it is too old to be accepted by the temporal validation check anyway" — i.e., the cache TTL and the freshness-window check must be derived from the *same* constant, not independently chosen.

**How to avoid:** (a) Derive the replay-cache TTL and the envelope freshness/`expiry` acceptance window from a single named constant in code (mirroring the project's own existing discipline of a single `FAMP_SPEC_VERSION` constant rather than scattered literals — see archived Pitfall 14). (b) Bound the cache by both time *and* size (an LRU eviction as the hard backstop under adversarial nonce-flooding, size chosen so worst-case memory is a fixed, documented number). (c) Persist the cache across broker/gateway restarts (see Pitfall 10) or, if that's rejected as too costly, document explicitly that a restart reopens a bounded replay window of exactly the TTL duration and treat that as an accepted, sized risk — not an accidental one. (d) Scope nonces per-sender, not globally — a global nonce space means a burst of legitimate traffic from one honest sender can evict cache entries a different sender's replay-detection depends on, and it also means the cache must scale with total federation traffic rather than per-peer traffic.

**Warning signs:** TTL and clock-skew tolerance are two different, independently-set constants in the code. No size-based cap on the cache exists — only time-based. Nonce cache is a single global structure keyed only by nonce value with no sender dimension. No test asserts the `cache_retention >= TTL >= skew` inequality.

**Phase to address:** Protocol-grade ingress phase (freshness / replay-cache requirement). The v1.0 envelope already reserves `nonce` and `expiry` — this phase is where their semantics get pinned; a unit test enforcing the three-way inequality should be a hard gate before that phase closes.

---

### Pitfall 10: Replay cache not persisted across restart reopens the replay window on every deploy

**What goes wrong:** If the nonce/replay cache lives only in the gateway process's memory, every restart (a crash, a deploy of a new binary, a `famp daemon restart`) instantly and silently resets the cache to empty — during which every previously-seen envelope within the freshness window becomes replayable again. Given the project's own precedent (the daemon is explicitly designed to be long-lived and restarted across `cargo install`/`brew upgrade` per the v0.11 version-skew work), restarts are a *routine* event here, not a rare one — this makes an in-memory-only replay cache a routine, not edge-case, vulnerability window.

**Why it happens:** An in-memory `HashSet`/LRU is the natural first implementation and works fine in every functional test, because tests don't restart the process mid-replay-window. The restart-reopens-window failure mode only shows up under adversarial testing or in a long-running production deploy — precisely the gap the project's own PITFALLS.md (archived) already flagged for the *broker* (Pitfall 15/replay-cache row) but which needs re-flagging now that the *gateway's* federation-layer cache is the new instance of the same shape.

**How to avoid:** (a) Persist the replay cache (or at minimum, the high-water-mark timestamp of the last accepted nonce per sender) to disk with atomic write+fsync, matching the project's own established `famp-inbox`/`famp-taskdir` atomic-write discipline. (b) On restart, refuse to accept any envelope with a timestamp older than "last known accepted timestamp" even if the nonce itself isn't in a (now-empty) in-memory set — a coarser but restart-safe check. (c) Add an adversarial test: kill -9 the gateway mid-session, replay a message from just before the kill, restart, and assert it's still rejected.

**Warning signs:** The replay cache is a bare in-memory collection with no persistence path. No test restarts the process and attempts a replay.

**Phase to address:** Protocol-grade ingress phase, same phase as Pitfall 9 — restart-safety should be a named acceptance criterion, not an afterthought.

---

### Pitfall 11: DoS ordering does expensive Ed25519 verification before cheap size/format/rate checks

**What goes wrong:** If ingress code path is "read full body → parse JSON → canonicalize → verify signature → THEN check size/rate/format," an attacker can force expensive canonicalization and Ed25519 verification work on every packet just by opening connections and sending garbage, without ever needing a valid signature. Published DoS-resistant protocol design ("Efficient, DoS-Resistant, Secure Key Exchange" — the JFK line of work, and its many descendants) is unanimous on the ordering principle: cheap, stateless checks (size caps, format/parseability, freshness/timestamp plausibility, per-source rate limit) must gate expensive ones (signature verify, canonicalization, any per-message state lookup) — never the reverse. v1.0's HTTP transport already enforces a 1MB body limit (`TRANS-*` requirements) but that guard needs re-verification at the *gateway* ingress boundary specifically, since the gateway is the new open-internet-facing surface, not the original same-host HTTP transport it was built for.

**Why it happens:** Code is naturally written in "parse then validate then trust" order because that's the logical/semantic order a human thinks about the message; the *cost* order (cheap-to-expensive) is a distinct axis that has to be deliberately imposed on top of the logical order, and it's easy to skip when the code already "works" against a well-behaved peer.

**How to avoid:** (a) Explicit ordering discipline documented and tested: (1) connection-level rate limit by source IP *before* any bytes are read past the first frame length header; (2) reject if declared body size exceeds cap *before* reading the rest of the body; (3) reject on malformed JSON/canonicalization failure *before* attempting signature verification; (4) reject on stale/future timestamp (freshness check, cheap) *before* signature verification; (5) only then run Ed25519 verify (expensive) and replay-cache lookup. (b) Add a benchmark/adversarial test that floods the ingress with intentionally-oversized or malformed bodies and asserts CPU time per rejected request stays near-constant regardless of body size — proving the size check actually gates before the expensive work, not just that both checks exist somewhere in the code.

**Warning signs:** Code review shows `verify_strict(...)` called before a body-size or rate check in the same function. No adversarial benchmark measures cost-per-rejected-request for oversized/malformed input specifically.

**Phase to address:** Protocol-grade ingress phase — this is a code-ordering discipline, testable with a targeted micro-benchmark; should gate that phase's exit criteria alongside the freshness-window work (Pitfalls 9-10), since all three share the same ingress code path.

---

### Pitfall 12: Rate limiting keyed on something the attacker fully controls

**What goes wrong:** The classic version of this mistake: rate-limiting by the claimed sender principal (`from` field) rather than by connection-level identity (source IP, TLS client identity, or — best — the pinned peer's actual key) lets an attacker simply claim a different `from` on every request to reset their rate-limit bucket. v1.0 already had to fix an analogous forgery hole (the sender-`from` forgery hole fixed in Phase 11, where the broker now binds `from` to the *authenticated* identity rather than trusting a client-supplied value) — the same discipline must extend to any new rate-limiting logic added for the open-internet ingress: never key a limit on a field the sender writes into the envelope body.

**Why it happens:** `from` is the most semantically obvious key to rate-limit on ("limit messages per sender") and it's already present in the data structure, so it's the path of least resistance — but it's exactly the field the ingress code already had to learn (the hard way, in Phase 11) not to trust.

**How to avoid:** (a) Rate-limit primarily on connection-level facts the attacker cannot forge without real cost: source IP (imperfect but attacker-costly to rotate at scale), or better, the actual verified Ed25519 key that produced a *valid* signature on a prior message (post-verification bucket) combined with a separate, stricter pre-verification bucket keyed on raw connection/IP for the not-yet-verified case. (b) Never use claimed `from`, claimed `sender_key_id`, or any other unauthenticated envelope field as a rate-limit key. (c) Explicit test: send N envelopes with N different claimed `from` values from the same connection/IP and assert the rate limiter still throttles as if it were one source.

**Warning signs:** Rate-limit bucket key is `envelope.from` or `envelope.sender_key_id` read before signature verification. No test tries the "different claimed sender per request from the same connection" bypass.

**Phase to address:** Protocol-grade ingress phase, alongside Pitfall 11 — same code path, same review pass.

---

### Pitfall 13: Key revocation without a CA cannot be reliably distributed, and pre-revocation messages can be replayed as if still valid

**What goes wrong:** With no CA/CRL infrastructure (which this milestone explicitly does not build — the signed peer directory is the closest analog), revocation is fundamentally a "tell everyone who might trust this key" broadcast problem with no central authority to make it durable. Two specific failure shapes: (1) a revocation notice itself needs distribution — if it only reaches peers who happen to be online/reachable at the moment it's issued, any peer who was offline (a durable-mailbox design goal FAMP already has for messages) may never see it and keeps trusting a compromised key indefinitely; (2) a message signed *before* revocation but delivered/processed *after* it (because it sat in a durable mailbox, or because of network delay) is ambiguous — is it a legitimate old-but-valid message, or an attacker replaying a stolen pre-compromise message now that they also stole the key? A pure "is this signature valid" check cannot distinguish these without an explicit revocation-timestamp comparison against the message's own signed timestamp.

**Why it happens:** Revocation is usually designed as a point-in-time boolean check ("is this key currently revoked") without also recording *when* it was revoked and cross-checking that against the message's own claimed signing time — which requires the freshness/`nonce`/`expiry` machinery from Section 3 to already exist and be trustworthy, so revocation design is easy to get wrong if built independently of that work rather than on top of it.

**How to avoid:** (a) A revocation record must itself be a signed, durable, timestamped object in the signed peer directory (not a side-channel announcement) so it propagates through the same durable-mailbox/directory-sync mechanism as everything else, and a late-joining or previously-offline peer picks it up on next sync rather than never. (b) Reject any message whose *signed timestamp* is after the revocation record's timestamp for that key — this uses the same freshness fields the ingress work already needs, so build revocation as a consumer of that machinery, not a parallel one. (c) Explicitly do NOT try to retroactively invalidate messages signed *before* revocation (that's provenance/audit-log territory the project already scoped out) — only gate messages timestamped after. (d) Name the residual risk in the design doc: a peer who never re-syncs the peer directory keeps trusting a revoked key forever — this is an inherent property of no-CA revocation and should be an accepted, documented limitation, not a silent gap.

**How to solve "revoking the key you need to sign the revocation with":** This is real when the *signing* key itself is compromised (vs. some other credential). Two established alternative shapes, either acceptable here: (i) a designated separate "recovery"/meta key, established at directory-registration time, whose only power is signing revocation records for the primary key (asymmetric, narrow-scope, rarely used — the closest analog to the project's existing narrow-enum error design philosophy); or (ii) treat every key as inherently short-lived (a rotation-by-expiry model, matching published practice: "short-lived certificates make revocation unnecessary because validity is shorter than the time it typically takes to revoke" and "identity-based systems auto-invalidate at each epoch boundary") so that a compromised key's blast radius is capped by its own expiry regardless of whether a revocation record ever gets seen. Given this milestone explicitly already reserves `expiry` on the envelope, leaning toward (ii) — short validity windows over a bolt-on revocation mechanism — is the lower-novelty, more consistent-with-existing-design choice.

**Warning signs:** Revocation is implemented as a "push a notification" side-channel rather than a durable signed directory entry. No comparison exists between a message's signed timestamp and the relevant key's revocation timestamp. No documented answer to "what happens to a peer who's been offline since before the revocation."

**Phase to address:** Protocol-grade ingress / signed peer directory phase — revocation should be scoped as a consumer of the freshness machinery (Pitfalls 9-10), not a separate feature built in isolation.

---

### Pitfall 14: Prompt injection / inbound-content-as-instructions — the milestone's BLOCKING gate (highest-severity section)

This is the milestone's declared blocking security gate: settled *before any outside person connects*, because a remote agent must not be able to steer Ben's agent by sending it text. Everything below is written to name explicitly what does **not** work, per the quality gate's requirement.

**What goes wrong, specifically:**

1. **Delimiter/quarantine schemes are defeated by the attacker simply emitting the delimiter.** "Spotlighting" (delimiting, datamarking, encoding untrusted text so its provenance is salient to the model) is a real, published technique (Microsoft/academic work, arXiv 2403.14720) — but subsequent research explicitly finds delimiter-based approaches "either ineffective at preventing attacks or [effective] at significant costs to task utility," and independent commentary states plainly: "delimiters won't save you from prompt injection." An attacker who knows (or guesses, or brute-forces) the delimiter/marker scheme simply includes it in their payload, and the model — which has no hard architectural wall, only a soft in-context convention — can be induced to treat the injected delimiter as authoritative.

2. **Sanitization/escaping leaks because natural language has no fixed grammar to escape.** Unlike SQL injection (where a fixed, parseable grammar lets you mechanically escape/parameterize), there is no complete enumeration of "instruction-shaped text" in natural language to strip or escape — any sanitizer is a blocklist against an open-ended space, and OWASP's own 2025 LLM Top 10 (prompt injection ranked #1, second consecutive edition) states plainly that "it is unclear if there are fool-proof methods of prevention for prompt injection" given "the stochastic influence at the heart of the way models work."

3. **Quarantine applied at one surface while another surface renders raw content is the single most likely real failure mode for FAMP specifically.** This is the project's own architecture talking: the MCP tool surface, the CLI `stdout` path, `famp_await`'s wake-up notification string ("New FAMP message from alice."), `famp inbox`/`famp inspect messages` output, and any future push-notification payload (Section 7) are *five distinct rendering surfaces* for the same underlying inbound content. A quarantine/labeling scheme applied only to, say, the MCP `famp_inbox` JSON response but not to a CLI error message that happens to echo a rejected envelope's body, or not to the `famp_await` notification string itself, leaves an unguarded path. The project's own documented "double-print pattern" investigation (v0.10 non-goals, `CLAUDE-CODE-CONTEXT-GUIDE.md`) already establishes that this codebase has *multiple, independently-evolving surfaces* that each touch message content — exactly the shape where a fix at one surface and a regression at another is easy to ship unnoticed.

4. **The "lethal trifecta" is the correct architectural test, and FAMP's own design puts all three legs in play by construction.** Simon Willison's framing (private data + untrusted content + external communication, all co-present = exfiltratable) is the standard test cited across OWASP-adjacent and industry security writing. FAMP's federated agent-messaging design *is* an instance of the trifecta almost by definition once v1.1 ships: the agent has access to private data (the local filesystem/session context an MCP-connected coding agent already has), it is now exposed to untrusted content (inbound FAMP messages from a remote, unauthenticated-in-intent human/agent), and it has external communication capability (it can `famp_send` a reply, and — more importantly — whatever *other* tools the harness (Claude Code, Codex) exposes to the same agent session, which FAMP does not control). Removing one leg is the only defense that actually works, and FAMP can only control one of the three legs directly: it cannot remove "private data" (that's the harness's context) and it cannot remove "external communication" (that's the harness's other tools) — it can only harden the "untrusted content" leg, which is exactly why the milestone scopes this as FAMP-side structural quarantine at the boundary, not a harness-side fix.

5. **A harness-side / prompt-level fix is explicitly out because it is untestable in FAMP's own CI and does not protect other clients.** The milestone's own framing states this directly: enforcement must be "FAMP-side and harness-agnostic... not as a prompt convention and not in `~/.claude` wiring — a harness-layer boundary is untestable in FAMP's CI and silently fails to protect Codex/Grok/other clients." This is architecturally correct and matches the published finding that prompt-level/in-context mitigations (however phrased — "treat the following as data," a system-prompt instruction to ignore embedded commands, etc.) are not a security boundary because they compete with the attacker's text in the *same channel and same trust level* the model reasons over; there is no structural reason the model must obey the developer's framing over the attacker's, especially under an adversarially-optimized payload.

**What does structurally work (with named caveats, not oversold):** Dual-LLM / CaMeL-style structural separation — a privileged component that never sees untrusted content directly, and a quarantined component that can process untrusted content but has no ability to call tools or trigger external communication — is the only approach with a *provable* (not merely empirical) security property, because the separation is enforced by what each component is architecturally capable of doing, not by what either component is told. Caveats to name explicitly: (a) Willison's original Dual-LLM pattern was itself found to have a real gap — the quarantined LLM can still have its extracted "task" overridden by embedded instructions, causing exfiltration to an attacker-chosen address, if the interface between quarantined and privileged components isn't sufficiently narrow; (b) CaMeL closes that gap via a custom interpreter with data-flow capability tracking, but even CaMeL only reaches provable security on 77% of AgentDojo's tasks (vs. 84% success for an undefended baseline) — i.e., a real utility cost, not a free lunch, and it is stated as "incompatible with open-ended agents issuing dynamically determined tool calls," which describes exactly what an MCP-connected coding agent does. For FAMP's actual shape (a message-passing protocol, not a tool-execution agent — v1.1 is explicitly conversation-only, no remote-triggered tools, per the milestone's scope), the practically achievable version of this pattern is narrower and more tractable: FAMP-side, treat every field of an inbound envelope's body as an inert string with an attached, immutable, unforgeable "untrusted, from: agent:X" provenance tag at every surface that renders it, and never let FAMP code itself construct a prompt, a shell command, or any instruction-shaped string by concatenating inbound body content with anything privileged.

**What published evaluation of such a boundary looks like:** Benchmarks exist and should inform the adversarial corpus, not be treated as a substitute for one built for FAMP's specific surfaces: AgentDojo (97 user tasks, 629 security test cases spanning banking/Slack/travel/workspace-style tool-use scenarios), InjecAgent (1,054 cases, indirect injection via tool outputs specifically — the closest published analog to "injection arriving via a message payload rather than a direct prompt"), WASP (web-agent-specific, VisualWebArena-based), and the Agent Security Bench (16 attack types × 11 defenses × 10 scenarios). None of these are FAMP-shaped out of the box (they test tool-calling agents, not a message-relay protocol), so the adversarial corpus that actually matters here must be built FAMP-native.

**What the FAMP-native adversarial corpus should contain, concretely:** (a) payloads that attempt to make the receiving agent execute a tool call/shell command via text embedded in the message `body` field; (b) payloads targeting each of the five known rendering surfaces separately (MCP `famp_inbox` response, CLI `famp inspect messages` stdout, the `famp_await`/`famp watch --notify` wake-up notification string, any future push-payload text, and error messages that echo rejected/malformed envelope content back to a log or terminal) — a payload that's blocked at one surface but not tested at the other four is not evidence of a fixed boundary; (c) payloads using known-ineffective mitigations as bait — e.g. content that includes a guessed or brute-forced delimiter/marker string, to prove the boundary does not rely on delimiter secrecy; (d) payloads that attempt the "lethal trifecta" chain specifically — instructing the receiving agent to read local private data and `famp_send` it back out, or to `famp_send` it to a *third* principal (exfiltration via the protocol's own legitimate reply capability, not an external channel) — because FAMP's `famp_send` tool is itself part of the "external communication" leg and this is the FAMP-specific version of the generic exfiltration attack; (e) payloads crafted to survive round-trip through canonical JSON / JCS re-serialization (since the message body is signed-and-canonicalized data, a payload should be tested both raw and after a canonicalize-then-render cycle, in case any transformation step incidentally un-escapes something); (f) regression payloads for every finding from (b)-(e) pinned into CI as named test cases, the same discipline the project already applies to conformance vectors (archived Pitfall 10: vectors must have a named, non-self-generated source and a clear "what this proves" label) — an adversarial corpus that is itself generated by the same team building the defense risks the identical "tests the bug, not the fix" failure the project's own canonical-JSON vector work already learned to avoid.

**Warning signs:** Any design doc or PR description that says "we handle this with a system prompt" or "we tell the agent to treat this as data" as the *only* mitigation. A quarantine/labeling fix landing in one file (e.g. the MCP server) without a corresponding grep/test across `cli/`, `famp_await`, and any notify-adapter code for the same content field. An adversarial corpus with fewer than the five-surface coverage above, or one written entirely by the implementer with no external/independent reviewer pass (matching the project's own established pattern: adversarial review must be a separate, cold pass — see the `/gsd-validate` vs. adversarial-review distinction already codified in this project's memory).

**Phase to address:** This is explicitly the milestone's early, blocking gate — "settled before any outside person connects" — meaning it should be its own phase, sequenced *before* the human-acceptance-gate phase (2-3), not folded into general ingress hardening. Verification for this phase must include: (1) the five-surface adversarial corpus running in CI as a named, regression-pinned test suite; (2) an independent adversarial review pass (per this project's own standing practice) with only the diff, explicitly hunting for a surface the corpus missed; (3) an explicit written statement of the residual risk that remains even after this gate (there is no claim of "solved," only "the specific known-ineffective mitigations are not relied upon, and the five known surfaces are covered").

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|---|---|---|---|
| "TLS to the relay is enough, we don't need to think about relay metadata" | Faster relay ship | Undocumented trust-anchor/metadata-leak surface (Pitfall 1) that nobody signed off on | Never — write the "what can the relay see" line even if the answer is "not much" |
| In-memory-only replay cache | Simple, fast to build | Restart reopens the replay window every daemon upgrade (Pitfall 10) | Only if the design doc explicitly names and accepts this as a bounded, documented risk — not silently |
| Rate-limit keyed on `envelope.from` | Reuses an existing field, less code | Trivially bypassed by an attacker rotating claimed sender (Pitfall 12) — the same class of bug already fixed once in Phase 11 | Never |
| Delimiter/prompt-level "treat as data" instruction as the injection defense | Zero engineering cost, ships same day | Published-ineffective; does not satisfy the milestone's own stated bar (harness-agnostic, FAMP-side) | Never for the blocking gate; may be a *supplementary* layer once a real structural boundary exists |
| Symmetric-NAT fallback treated as low-priority "we'll add relay later" | Hole-punch demo ships faster | 15-30% of real users (possibly including the milestone's own second person) never connect (Pitfall 3) | Never — build relay-fallback first, hole-punch as the optimization |
| Self-generated adversarial-injection test corpus, no independent reviewer | Fast to write, feels complete | Same "tests the bug not the fix" failure this project already learned from canonical-JSON vectors (archived Pitfall 10) | Never for the blocking gate; acceptable as a first draft only if followed by an independent pass before the gate closes |
| Short-code out-of-band pairing with no expiry | Simpler state model | Unbounded verification window (Pitfall 8) lets a stale pairing be completed by an attacker later | Never — even a generous (hours, not days) expiry is better than none |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|---|---|---|
| Relay/tunnel service (hosted) | Assume the vendor's TLS termination means "no trust decision needed" | The vendor is a new trust anchor by construction; name what it sees in the Phase-13 decision record |
| STUN/TURN or equivalent NAT-traversal library | Test only against your own (non-symmetric) network | Test against at least one real symmetric-NAT / CGNAT network before declaring reachability "solved" |
| Signed peer directory sync | Treat directory sync as best-effort, no ordering/staleness guarantee | Revocation records specifically need a defined propagation guarantee (durable, retried) — a "maybe it syncs eventually" directory silently reopens Pitfall 13 |
| Push-notification service (Section 7 adapter) | Push payload carries the raw message body as notification text | Push payload should carry only opaque metadata ("new message from X, tap to view") — never render body text in a push payload (this is itself an inbound-content-to-untrusted-surface instance, see Pitfall 14 surface list) |
| `famp inspect`-style health commands | Ship one for the broker, forget the gateway/relay | Extend the same "dead-thing diagnosis" pattern (HEALTHY/DOWN/STALE/ORPHAN) to the new relay/gateway ingress boundary, per Pitfall 2 |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|---|---|---|---|
| Global (not per-sender) nonce cache | Cache pressure scales with total federation traffic, not per-peer traffic; one busy peer evicts another's replay protection | Scope nonce cache per-sender (Pitfall 9) | Once more than a couple of peers are federating concurrently |
| Unbounded replay cache | Slow, then OOM, under sustained traffic or adversarial nonce-flooding | Bound by both time and size (Pitfall 9) | Any sustained public-internet exposure — this is not a "someday" scale concern once the gate is public |
| Expensive-verify-before-cheap-check ingress ordering | CPU pegged under a flood of garbage/oversized requests despite a "1MB limit" existing somewhere in the code | Enforce and benchmark the cheap-before-expensive ordering (Pitfall 11) | The first real internet-facing exposure — this is a day-one risk once reachable publicly, not a scale threshold |
| Relay with no per-source rate limit | Becomes a DDoS amplifier for third parties (Pitfall 5) | Rate-limit per source IP and per pinned-identity independently, before forwarding | As soon as the relay's listen address is publicly reachable |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---|---|---|
| TOFU re-pin UX shares code/UI with new-peer pin | Silent MITM acceptance on key change, defeating cross-person trust entirely | Structurally distinct code path, exit code, and UX copy for "changed" vs. "new" (Pitfall 6) |
| Out-of-band channel not actually independent of the bootstrapped path | Trust bootstrap reduces to "no verification at all" | Name and verify the channel's independence explicitly; never auto-generate the OOB artifact from FAMP infra itself (Pitfall 7) |
| Rate limit keyed on attacker-controlled field | Trivial DoS/rate-limit bypass | Key on connection identity or post-verification key, never claimed `from` (Pitfall 12) — same class as the already-fixed Phase 11 forgery hole |
| Revocation with no distribution guarantee | Compromised key trusted forever by any peer who was offline at revocation time | Revocation as a signed, durable, directory-synced record, not a side-channel push (Pitfall 13) |
| Prompt-level "treat as data" as the sole inbound-content defense | Published-ineffective; attacker text and defender framing share one channel and one trust level | FAMP-side structural quarantine with immutable provenance tagging at every rendering surface (Pitfall 14) — never rely on delimiters/prompt instructions alone |
| Any single rendering surface skipped when hardening inbound content | The one skipped surface (notification string, CLI stdout, error echo) becomes the live attack path | Enumerate all five known surfaces explicitly and test each independently (Pitfall 14) |
| Push-notification payload includes raw message body text | Payload itself becomes a sixth untrusted-content-rendering surface, likely rendered by code that was never in scope for the injection-hardening review | Push payloads carry only opaque metadata; body is fetched (and quarantined) only through the already-hardened surfaces (Section 7 + Pitfall 14) |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---|---|---|
| "Just click accept" trust-bootstrap flow | Trains the second person to reflexively accept any prompt, including a genuine key-change warning later | Make "new peer" friendly but make "key changed" visually and interactively distinct — friction is a feature there, not a bug (Pitfall 6) |
| Manual safety-number-style comparison ceremony, undocumented for a non-expert | The exact ceremony that made Signal's own verification "nearly unusable... even by experts" in published usability studies | User-test the actual ceremony with the real second, non-technical person before the Phase 2-3 gate — don't assume a technically-correct design is usable |
| Relay/gateway health invisible to a non-operator second person | Second person has no way to tell "it's not working" from "it's just slow" | Extend the `famp inspect`-style legible-diagnosis pattern to the new relay/gateway boundary (Pitfall 2) so a non-Ben operator has a command to run, not a mystery |
| Push notification silently dropped, no in-app fallback | Second person believes the system is broken (or worse, believes a message was never sent) when it's a transient push-delivery gap | Treat push as a hint, not the source of truth — pair every push with a durable, poll/inspect-able record so a missed push is recoverable, not lost (Section 7) |

---

## "Looks Done But Isn't" Checklist

- [ ] **Reachability decision:** Often tested only on networks the builder controls — verify at least one real symmetric-NAT/CGNAT network was used in the Phase-13 spike (Pitfall 3)
- [ ] **Relay:** Often ships with no health/expiry check — verify a `famp inspect`-style command reports relay reachability and TLS cert days-remaining (Pitfalls 2, 4)
- [ ] **Relay:** Often accepts forward-for-anyone traffic — verify it rejects any connection not mapping to a pinned peer pair before doing forwarding work (Pitfall 5)
- [ ] **Trust bootstrap:** Often shares code between "new peer" and "key changed" — verify distinct exit codes/messages/tests exist for both (Pitfall 6)
- [ ] **Trust bootstrap:** Often has no expiry on pending pairing state — verify a short, enforced expiry exists if any server-mediated exchange step was added (Pitfall 8)
- [ ] **Replay cache:** Often unbounded and in-memory-only — verify a size+time bound AND a restart-survival test both exist (Pitfalls 9, 10)
- [ ] **Replay cache:** Often global instead of per-sender — verify nonce scoping is per-sender (Pitfall 9)
- [ ] **Ingress ordering:** Often verifies signatures before checking size/rate — verify a benchmark proves near-constant rejection cost regardless of payload size (Pitfall 11)
- [ ] **Rate limiting:** Often keyed on a client-supplied field — verify the key is connection-identity or post-verification, not claimed `from` (Pitfall 12)
- [ ] **Revocation:** Often a side-channel push with no durability guarantee — verify it's a signed, directory-synced record and that message-timestamp-vs-revocation-timestamp comparison exists (Pitfall 13)
- [ ] **Inbound-content boundary:** Often hardened at one surface only — verify all five known rendering surfaces (MCP response, CLI stdout, `famp_await`/`watch --notify` string, push payload, error-echo) are covered by the SAME adversarial corpus (Pitfall 14)
- [ ] **Inbound-content boundary:** Often relies on a delimiter/prompt convention as the real defense — verify the corpus specifically includes a guessed/emitted-delimiter bypass attempt and that it's rejected structurally, not just semantically (Pitfall 14)
- [ ] **Inbound-content boundary:** Often reviewed only by its own implementer — verify an independent, diff-only adversarial review pass happened before the blocking gate closed (Pitfall 14)
- [ ] **Push adapter:** Often assumed at-most-once — verify the design explicitly handles at-least-once delivery (duplicate notifications) and missed-on-restart recovery via a durable fallback (Section 7)
- [ ] **Human acceptance gate:** Often accepted on sender-side "exit 0" alone — verify the pass criterion requires the *receiving* human's own `famp inspect tasks` confirmation, matching the working v1.0 UAT-01 pattern (Pitfall 2, Section 8)

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---|---|---|
| Relay chosen without symmetric-NAT testing (Pitfall 3) | MEDIUM | Re-run the Phase-13 spike against a real symmetric-NAT network; if it fails, add the relay fallback path — this is a design-doc amendment, not necessarily a rewrite if relay was already the documented fallback |
| TOFU re-pin shares UI with new-peer accept (Pitfall 6) | LOW-MEDIUM if caught pre-gate; HIGH if a real second-person key change goes unnoticed post-ship | Split the code path, add the distinguishing test, re-run the Phase 2-3 gate scenario with a deliberate key-swap injected |
| In-memory-only replay cache (Pitfall 10) | LOW-MEDIUM | Add persistence (atomic write, matching existing `famp-inbox` discipline); add the restart-replay adversarial test; no protocol/wire change needed since this is implementation-internal |
| DoS ordering wrong (verify-before-cheap-check) (Pitfall 11) | LOW if caught in review; MEDIUM if caught via a real flood in production | Reorder the ingress function; add the cost-benchmark regression test; no wire-format change needed |
| Revocation with no distribution guarantee (Pitfall 13) | HIGH if discovered after a real compromise incident | Requires the signed peer directory's sync guarantee to be strengthened — potentially a design-level change if directory sync was built as best-effort |
| Prompt-level-only injection mitigation shipped (Pitfall 14) | HIGH — this is the blocking gate; discovering it's insufficient after ship means the "no outside person connects until this is settled" precondition was violated | Immediately restrict/pause remote-agent connections; retrofit FAMP-side structural quarantine at all five surfaces; run the full adversarial corpus before re-opening to any second person |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---|---|---|
| 1. Relay as unnamed trust anchor / metadata leak | Phase 13 (reachability spike) | Decision record explicitly names what the relay/tunnel can and cannot see |
| 2. Relay outage silent SPOF | Reachability implementation phase + Phase 2-3 human gate | Gateway self-health-check exists; human gate pass criterion requires receiver-side `famp inspect tasks` confirmation, not sender exit code |
| 3. Symmetric NAT / hole-punch failure with no fallback | Phase 13 (spike) | Spike explicitly tested against a real symmetric-NAT/CGNAT network; relay-first, hole-punch-as-optimization ordering recorded |
| 4. TLS cert issuance/renewal unwatched | Phase 13 (decision) + ingress-hardening phase | `famp inspect`-style cert-expiry command ships; hosted-tunnel option scored higher if it auto-rotates |
| 5. Relay as open amplifier | Relay implementation phase | Relay rejects non-pinned-pair connections before forwarding; per-source and per-identity rate limit both tested |
| 6. TOFU silent re-pin on key change | Cross-person trust bootstrap phase | Distinct code path/exit code/message for "new" vs "changed"; regression test for both |
| 7. OOB channel not actually independent / low-entropy short code | Cross-person trust bootstrap phase | Channel independence documented; any short code sized against brute-force with an explicit rate limit |
| 8. Unbounded verification window | Cross-person trust bootstrap phase (if server-mediated exchange added) | Pending-pairing state carries a short, enforced expiry |
| 9. Nonce/TTL/clock-skew/cache-bound mismatch | Protocol-grade ingress phase | Single derived constant enforces `cache_retention >= TTL >= skew`; per-sender scoping test |
| 10. Replay cache not restart-persistent | Protocol-grade ingress phase | Atomic-write persistence; kill-and-replay adversarial test |
| 11. Expensive-before-cheap DoS ordering | Protocol-grade ingress phase | Cost-per-rejected-request benchmark for oversized/malformed input |
| 12. Rate limit keyed on attacker-controlled field | Protocol-grade ingress phase | Rotating-claimed-`from` bypass test fails to evade the limiter |
| 13. Revocation undistributable / pre-revocation replay | Protocol-grade ingress / signed peer directory phase | Revocation as durable signed directory record; message-timestamp-vs-revocation comparison test |
| 14. Prompt injection / inbound-content-as-instructions | Its own early, BLOCKING phase before Phase 2-3 human gate | Five-surface adversarial corpus in CI; independent diff-only adversarial review; explicit residual-risk statement |
| Push-notification-adapter payload as a sixth untrusted surface | SEED-002 push-notification adapter phase | Payload carries opaque metadata only; covered by the same Pitfall-14 corpus, not a separate ungated surface |
| Human-gate false-positive ("unassisted follower" that wasn't) | Phase 2-3 human gate design | Pass criterion is the second person's own unprompted output, not a relayed or Ben-assisted confirmation — matching the exact gap the v1.0 Phase-10/11 history already exposed once |

---

## Process/Verification Pitfalls Specific to This Milestone

### How teams fool themselves that an "unassisted follower" gate passed when it did not

**What goes wrong:** The single most expensive lesson already banked in this project's own history (v1.0 Phase 10 → 11) is that a doc-following gate can be marked `human_needed`/pass based on someone who already knows the system's internals following their own doc — which proves the doc is *internally consistent*, not that a genuine newcomer can complete it *unassisted*. The failure repeats itself unless the gate is designed so the follower (a) has never seen the underlying code, (b) is not on a call with, in the same room as, or receiving live guidance from the doc's author, and (c) the "pass" signal comes from the follower's own independent output (their own `famp inspect tasks`, their own terminal), not a relayed report.

**Prevention:** Recruit the real second person *early* (already planned — "a second person is lined up, so the real-person gate can sit at Phase 2-3" per PROJECT.md) specifically so this gate can fail loudly at a phase boundary with room to fix, rather than at the final gate the way v1.0's Phase 10/11 discovery did. Script the UAT so the second person runs the doc from a machine Ben has never touched, and the pass evidence is a screenshot/output *they* generate and send, not a description Ben gives on their behalf.

**Phase:** Phase 2-3 (explicitly named in PROJECT.md as moved early for this exact reason).

### Why accuracy gates that grep for tokens miss semantic inversions

**What goes wrong:** `GATEWAY-SETUP.md`'s own history is the direct precedent: a compiled accuracy test extracted flags from the live CLI and passed, while the prose around those flags had the wiring backwards (§4's principal-name semantics were inverted — "the gateway backs the REMOTE principal, not local"). A flag-grep gate proves *the flags exist and are spelled correctly*; it says nothing about whether the surrounding sentence describing *how to use* those flags is true. This will recur for any new setup doc this milestone produces (relay setup, trust-bootstrap walkthrough, signed-directory config) unless the gate is explicitly upgraded.

**Prevention:** For any new setup/how-to doc this milestone ships, require at least one **semantic** assertion per doc section — not "does flag X appear" but "does running the documented command sequence, verbatim, on two real machines produce the documented result." The project already has the right instinct (compiled accuracy tests) — extend it, for v1.1 docs specifically, to include a live two-process (or two-person) dry run before merge, not just a static flag-presence check.

**Phase:** Whichever phase writes the new cross-person setup guide (trust bootstrap + relay setup) — should ship its own live-dry-run check alongside the flag-grep gate, modeled on the fix that closed Phase 11's finding.

### Testing a two-person flow when you are one person

**What goes wrong:** Every phase before the real second person is available risks validating the trust-bootstrap and reachability flows only against Ben-playing-both-roles (two terminals, one operator) — which cannot catch the failure modes that specifically require two *different* people: a genuinely independent out-of-band channel (Pitfall 7), a genuinely different network/NAT class (Pitfall 3), or a genuinely unbriefed follower (the process pitfall above). This is the same "own-machines-first" blind spot that shaped (correctly, for v1.0) the decision to prove the gateway spine before adding a second person — but here the second person *is* the thing being tested, so simulating them with Ben is not a lesser version of the real test, it's a different test that doesn't exercise the risk at all.

**Prevention:** Explicitly label any pre-Phase-2-3 dry run as "operator self-test, does not validate cross-person risk" in its own verification doc — don't let a green self-test read as evidence for the cross-person claims (Pitfalls 3, 6, 7, 8) it cannot actually test. Falsify before committing: name the cheapest test that would prove the self-test insufficient (e.g., "would this pass if the OOB channel were secretly the same channel as the relay?") and confirm the self-test can't distinguish that case.

**Phase:** Every phase before Phase 2-3 — a documentation/process discipline, not a code gate; carried explicitly into the verification section of each phase's plan.

---

## Sources

- [The lethal trifecta for AI agents: private data, untrusted content, and external communication — Simon Willison](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/) — primary framing for Section 6 (MEDIUM confidence, cross-checked)
- [CaMeL offers a promising new direction for mitigating prompt injection attacks — Simon Willison](https://simonwillison.net/2025/Apr/11/camel/) — Dual-LLM pattern gap and CaMeL's structural fix (MEDIUM confidence, cross-checked)
- [OWASP Top 10 for LLM Applications 2025 (official PDF)](https://owasp.org/www-project-top-10-for-large-language-model-applications/assets/PDF/OWASP-Top-10-for-LLMs-v2025.pdf) — prompt injection ranked #1, "no fool-proof method" language (MEDIUM confidence)
- [Defending Against Indirect Prompt Injection Attacks With Spotlighting (arXiv 2403.14720)](https://arxiv.org/pdf/2403.14720) — spotlighting technique and its own limitations (MEDIUM confidence)
- [Indirect Prompt Injections: Are Firewalls All You Need, or Stronger Benchmarks? (arXiv 2510.05244)](https://arxiv.org/html/2510.05244v1) — critique of delimiter/firewall-only defenses (LOW-MEDIUM confidence, single-cluster source)
- [AgentDojo Benchmark: LLM Security Evaluation](https://www.emergentmind.com/topics/agentdojo-benchmark) — 97 tasks / 629 security cases, CaMeL's 77% provable-security figure (MEDIUM confidence)
- [InjecAgent, WASP, Agent Security Bench — surveyed via web search, arXiv preprints] — adversarial-corpus design references for Section 6 (LOW confidence, preprint-stage sources not independently re-verified)
- [Short Take: Why Trust-On-First-Use Doesn't Work (Even for SSH) — agwa.name](https://www.agwa.name/blog/post/why_tofu_doesnt_work) — TOFU first-connection weakness and HPKP deprecation precedent (MEDIUM confidence, cross-checked against SSH known_hosts documentation)
- [How to Verify Signal Safety Numbers / usability studies on Signal's authentication ceremony](https://discuss.techlore.tech/t/a-guide-to-verifying-signal-safety-numbers/2289) — SAS/out-of-band verification UX failure precedent (LOW-MEDIUM confidence)
- [TCP/UDP Hole Punching for NAT Traversal — emergentmind / oneuptime / thelinuxcode](https://www.emergentmind.com/topics/nat-traversal-tcp-hole-punching) — symmetric-NAT failure mechanics and TURN fallback (MEDIUM confidence, cross-checked across multiple independent explainers)
- [TURN Security Threats: A Hacker's View — Enable Security](https://www.enablesecurity.com/blog/turn-server-security-threats/) — relay-as-DDoS-amplifier precedent (LOW-MEDIUM confidence)
- [Solving The Revocation Gap With Short-Lived Certificates — DigiCert](https://www.digicert.com/blog/solving-short-lived-certificates) — short-lived-credential-over-revocation design pattern (LOW-MEDIUM confidence)
- [Efficient, DoS-Resistant, Secure Key Exchange for Internet Protocols (JFK, Columbia CS)](https://www.cs.columbia.edu/~smb/papers/jfk-ccs.pdf) — cheap-before-expensive DoS-ordering principle (MEDIUM confidence, established protocol-design literature)
- [Enhancing SYN Cookie Security Against DDoS Attacks: Mitigating Replay Attacks with Nonce Implementation (MDPI)](https://www.mdpi.com/1999-5903/18/6/323) — nonce/TTL/clock-skew cache design relationship (LOW-MEDIUM confidence)
- `/Users/benlamm/Workspace/FAMP/.planning/PROJECT.md` — v1.1 milestone scope, v1.0 shipped history, Phase 10/11 discovery of the shipping-client gap, sender-`from` forgery fix
- `/Users/benlamm/Workspace/FAMP/ARCHITECTURE.md` — layer model, MCP tool surface, message-flow rendering surfaces used in Section 6's five-surface analysis
- `/Users/benlamm/Workspace/FAMP/docs/GATEWAY-SETUP.md` — v1.0 TLS/EKU pitfalls, TOFU pin/re-pin behavior, duplicate-pubkey brick, fire-and-forget send confirmation boundary
- `/Users/benlamm/Workspace/FAMP/.planning/research/archive/v0.6/PITFALLS.md` — prior pitfalls research this document extends without restating (canonical JSON, Ed25519, FSM, conformance-vector provenance discipline referenced throughout)

---
*Pitfalls research for: FAMP v1.1 Open-Internet Federation*
*Researched: 2026-07-30*
