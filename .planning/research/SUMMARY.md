# Research Summary: FAMP v1.1 Open-Internet Federation

**Synthesized:** 2026-07-30
**Sources:** STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md

---

## Executive Summary

v1.1 replaces three crutches from v1.0 (Ben-controlled machines, Ben-controlled network, hand-copied keys) with genuine open-internet federation between two independent people. The good news, confirmed independently by both STACK and ARCHITECTURE: this is mostly an *integration* milestone, not a rewrite. Three of five feature areas (trust bootstrap, signed directory, push-notify) need **zero or near-zero new dependencies** and slot cleanly into the existing Layer 1/2 crates (`famp-bus`, `famp-gateway`, `famp` binary) without touching the frozen Layer 0 primitives. The two areas that do carry real engineering weight — public reachability and protocol-grade ingress — are correctly gated behind a zero-code Phase 13 spike and a well-understood, cheap-before-expensive ingress-ordering discipline, respectively.

The hard problem is not the transport — every research file converges on this — it's the human. The v1.0 mechanism (paste a pubkey blob, "warning: still imports" on mismatch) is architecturally identical to SSH's known-broken TOFU pattern and will fail with a real non-technical follower. FEATURES's step-count comparison across Magic Wormhole, Matrix SAS, Signal, and SSH gives a clear, evidence-backed answer: fail-loud beats silent-accept, and a low-entropy, one-guess PAKE-style exchange (Wormhole's model) beats both eyeball-fingerprint schemes and live-session SAS ceremonies for an unassisted, possibly-remote pairing.

The single most load-bearing architectural fact, from ARCHITECTURE: `strip_relay_fields` already erases every field that could signal "this came from a remote host" before an envelope hits the local bus — so the blocking inbound-content-is-DATA gate cannot be built by reading stored envelope bytes; provenance has to be carried as a new, additive field on `famp-bus`'s `Register` frame (Layer 1, not frozen) and threaded through all five known rendering surfaces (MCP `famp_inbox`/`famp_await`, CLI stdout, channel logs, and any future push payload). PITFALLS reinforces this is the milestone's single point of catastrophic failure if any one of those five surfaces is missed.

---

## Key Findings

### From STACK.md
- **`moka 0.12.15`** — bounded, TTL'd, concurrent-safe cache for replay/nonce dedup, dual-purposed for the revocation-key cache. Standard ecosystem choice for this shape.
- **`tower_governor 0.8.0`** (wraps `governor 0.10.4`) — drops onto the existing axum router via `.layer(...)` for rate limiting; flagged as ~1yr-stale release, low risk (thin wrapper over an actively-maintained core), but re-verify before locking.
- **No new crypto crate for trust bootstrap** — SAS (short authentication string) built from `sha2`/`base64` already in the stack, not a PAKE library.
- **`iroh 1.0.3`** is the single best-fit crate in the ecosystem for pubkey-addressed P2P with automatic hole-punch + relay fallback — but adopting it means *replacing* the shipped axum/rustls transport, not augmenting it. Flagged as an architectural cost, not a maintenance risk.
- **Self-hosted relay (Fly.io ~$2-5/mo or AWS Lightsail $5/mo)** is the recommended reachability starting point: needs zero new Rust dependencies (reuses `famp-inbox`/`famp-canonical`/`famp-crypto` + axum), and the relay itself needs no trust since every envelope is already signed.
- **Hosting cost figures are LOW confidence** — third-party aggregator data disagreed by up to 2x (notably Hetzner's 2026 price hikes). Do not treat these numbers as settled; verify against vendor pricing pages before Phase 13 locks a decision.
- No new crate needed for the signed peer directory (reuse `famp-canonical`/`famp-crypto` + existing router) or SEED-002 push-notify (reuse `tokio::process` + env-var metadata passing, avoiding shell-injection risk entirely).

### From FEATURES.md
- **Fail-loud pairing beats every silent-accept scheme on step count and safety.** Magic Wormhole: 2 human steps, hard abort on wrong code. Matrix SAS: 5-6 steps, explicit no-match button. Signal/SSH: silent accept, only "warning" — this is what v1.0 does today and is the known-broken pattern to replace.
- **Table stakes for v1.1:** fail-loud pairing, signed TTL-bounded peer directory (Matrix-key-list style, not a central registry), `expiry`-bounded freshness + bounded nonce cache, signed revocation statement, structural inbound-DATA quarantine + provenance tagging + adversarial CI corpus, MCP push-notification adapter.
- **Anti-features explicitly rejected:** central-authority auth keys (Tailscale-style — contradicts no-central-authority constraint), silent TOFU-then-optional-verify (what v1.0 does), prompt-level "ignore instructions" as sole injection defense, harness-side-only enforcement.
- **Nostr's laissez-faire replay model is the cautionary counter-example** — no mandated freshness window is unacceptable for a byte-exact, signature-verifiable protocol.
- The "lethal trifecta" (private data + untrusted content + external comms) is why v1.1 stays conversation-only: removing the tool-access leg is what keeps the trifecta from closing.

### From ARCHITECTURE.md
- **Reachability code is a sibling of `run_ingress`/`run_egress` inside `famp-gateway`**, composed under the existing `tokio::select!` — not a new `Transport` impl, not a separate process. Preserves `verify_inbound_any` as the sole trust decision site.
- **The keyring is hard-coded to one key per principal** (`Keyring: HashMap<Principal, TrustedVerifyingKey>`) — `pin_tofu` hard-rejects any second key. This must be extended (to `Vec<TrustedVerifyingKey>` or an active/retired struct) before rotation or revocation can exist; flagged as "genuinely new work, its own roadmap phase," not a reinterpretation.
- **`federation_format_ok` is format-only** (frozen `famp-envelope` crate) — it does NOT enforce freshness or consult a replay cache. All new v1.1 enforcement (replay cache, active expiry check, audience binding) must live in `famp-gateway` (`verify.rs` or a new `ingress_guard.rs`), never in the frozen envelope crate.
- **Signed peer directory should be a new crate** (`famp-directory`), not a module bolted onto `famp-keyring` (pure, dependency-light today) or `famp-gateway` (runtime, not offline job). Dependency direction: `famp-directory -> famp-keyring`, never reversed.
- **Push-notify (`famp watch --notify`) is additive** — ship as a thin CLI-side wrapper looping `famp await` (zero `famp-bus` change) before reaching for a true broker-side persistent-subscription design.
- **Provenance tagging cannot be added to the envelope `Value` itself** without reopening the frozen envelope crate or breaking `BUS-11` decodability — it belongs one layer up, as a new optional field on `famp-bus`'s `Register` frame (Layer 1, not frozen), following the existing additive-field pattern already used for `cwd`/`listen`.

### From PITFALLS.md (top 5)
1. **Relay becomes an unacknowledged trust anchor / metadata-leak point.** Signing protects payload integrity, not who-talked-to-whom-when. The Phase 13 decision record must explicitly name what the relay can/cannot see.
2. **Relay outage is a silent single point of failure.** `famp send`'s existing fire-and-forget boundary compounds with a flaky third-party relay — the human acceptance gate's pass criterion must be the *receiver's* own `famp inspect tasks` output, never sender-side exit 0.
3. **Symmetric NAT / hole-punch failure has no fallback path.** 15-30% of real hosts sit behind symmetric NAT; build relay-fallback first, hole-punch as an optimization, and test against a real symmetric-NAT network (a carrier hotspot) in the spike — not just networks Ben controls.
4. **TOFU re-pin UX sharing code with new-peer pin defeats cross-person trust.** "Key changed" must be a structurally distinct, harder-to-skip path from "new peer, first pin" — same class of mistake HPKP made in the opposite direction.
5. **Prompt injection / inbound-content-as-instructions — the milestone's blocking gate.** Delimiter/quarantine schemes are defeated by an attacker simply emitting the delimiter; sanitization can't fully enumerate natural language. The only structurally sound approach: immutable, unforgeable provenance tagging at every one of the five known rendering surfaces, verified by a FAMP-native adversarial corpus (not a borrowed benchmark) and an independent diff-only review — never a prompt-level "treat as data" instruction alone.

---

## Implications for Roadmap

### Resolved forks (per synthesis priorities)

1. **Trust bootstrap: SAS vs. PAKE.** STACK recommends a SAS (sha2/base64, no new crate); FEATURES points at Magic Wormhole's PAKE short-code as the strongest analog. Given ARCHITECTURE's finding that the keyring is one-key-per-principal and `pin_tofu` is the single integration point for any bootstrap mechanism, both a SAS and a PAKE terminate at the same `pin_tofu`/`save_to_file` call — the choice doesn't touch the keyring extension work either way. **Recommendation: PAKE-backed short-code (Wormhole-style, fail-loud, one-guess) over a passive SAS-eyeball-comparison**, because FEATURES's step-count/fail-loud table is decisive: a SAS that a human must actively compare (Matrix's model) is 5-6 steps with real risk of "looks close enough" acceptance; Wormhole's PAKE hard-aborts on a wrong code with zero judgment call required from the human. This does mean new engineering (a PAKE implementation + a rendezvous/mailbox service), correctly flagged HIGH complexity in FEATURES — but it directly serves the "unassisted follower" acceptance bar in a way a passive SAS does not.

2. **Reachability: `iroh` vs. self-hosted relay.** STACK flags iroh as the most elegant single-crate answer; ARCHITECTURE says reachability should be a sibling of `run_ingress`/`run_egress` inside the existing gateway. These are incompatible: iroh requires replacing the wire transport wholesale (its own QUIC `Endpoint`, not axum/rustls/reqwest). The real cost is throwing away a shipped, tested, Gate-A-proven transport mid-milestone for a milestone whose stated hard problem is the human, not the wire. **Recommendation: self-hosted relay (Fly.io/Lightsail, ~$2-5/mo, Ben-operated) as the default**, decided in Phase 13's spike alongside iroh as an explicitly-weighed-and-likely-rejected alternative — record the rejection rationale (transport-migration cost) in the decision doc rather than silently dropping it.

3. **Revocation: signed statement vs. `expiry`-field short-lived keys.** FEATURES recommends a signed revocation statement distributed out-of-band; PITFALLS independently converges on the same idea but explicitly names the cheaper alternative — leaning on the already-reserved `expiry` field (short-lived keys) so a compromised key's blast radius is capped by its own validity window "regardless of whether a revocation record ever gets seen." The `expiry`-based approach is cheaper (zero new message type, reuses machinery the ingress phase already builds) and arguably more correct for a no-CA bilateral model (a revocation record still requires reliable out-of-band distribution to a possibly-offline peer, which PITFALLS Pitfall 13 flags as unsolvable without a CA). **Recommendation: build revocation as a consumer of the freshness/expiry machinery first (short validity windows), and treat an explicit signed revocation statement as a defense-in-depth addition, not the primary mechanism.**

4. **Hosting costs are unsettled.** Every dollar figure in STACK's Section 1 table is LOW confidence (aggregator data disagreeing by up to 2x, notably Hetzner's 2026 price hikes). Do not let the roadmap or Phase 13 spike present a specific $/mo number as a locked decision input — the spike's job is to re-verify live vendor pricing, not trust this research pass's numbers.

### Suggested phase structure (extends Phase 13 onward)

1. **Phase 13 — Reachability spike (zero code).** Weigh self-hosted relay vs. iroh vs. hosted tunnel (Tailscale Funnel as best fallback; rule out ngrok's 2hr cap and Cloudflare Quick Tunnels' unstable URL). Must test against a real symmetric-NAT network, not just Ben's own. Decision record must name cost/month (re-verified live), named operator, AND what the relay/tunnel can/cannot see (Pitfall 1).
2. **Phase — Trust bootstrap v2: keyring format extension (multi-key + rotation/revocation tagging).** Must land before any new bootstrap UX or the directory crate, since both write through `pin_tofu` — a second migration later is the risk of skipping this.
3. **Phase — New bootstrap UX (PAKE-backed pairing) + Signed peer directory (`famp-directory`, new crate).** Genuinely parallel — both depend only on the extended keyring from step 2, not on each other.
4. **Phase — Inbound-content-is-DATA quarantine (blocking gate).** Technically independent of 1-3 (touches `famp-bus`'s `Register` frame + 5 CLI/MCP read sites), so it can and should be built in parallel with the above — but must be verified complete (five-surface adversarial corpus in CI + independent diff-only review) before the human acceptance gate.
5. **Phase — Protocol-grade ingress (replay cache, freshness enforcement, audience binding, DoS ordering, revocation-as-expiry).** Depends on the reachability model being decided (no point hardening ingress before the gateway is reachable) and benefits from the keyring/revocation work already existing.
6. **Phase — Reachability implementation** (per Phase 13's decision), gated by ingress hardening being complete first — never go live on the open internet before the replay/freshness/DoS checks exist.
7. **Human acceptance gate — scheduled early in relative sequencing (as soon as the quarantine gate + a minimal bootstrap path exist), not last.** Pass criterion must be the receiving human's own unprompted `famp inspect tasks` output, never a Ben-relayed report or sender-side exit 0.
8. **SEED-002 push-notify adapter** — genuinely orthogonal, ship whenever convenient (thin `famp await`-wrapping CLI loop, zero `famp-bus` risk).

**Must-land-together pairs:** reachability implementation + ingress hardening (never ship one live without the other); keyring extension + revocation data model (avoid a second format migration).

**Independently verifiable, no cross-phase coupling:** push-notify adapter; revocation reject-path unit tests (once keyring format lands); the quarantine adversarial corpus; the directory crate (fixture-based, no live gateway needed).

---

## Research Flags

**Needs deeper research during planning (`--research-phase`):**
- The PAKE-backed pairing phase — no existing FAMP code to extend; needs a rendezvous/mailbox design decision (how do two strangers with no existing channel find each other to run the PAKE exchange?).
- The reachability implementation phase, once Phase 13's spike concludes — the specific relay/tunnel choice will have its own integration questions not fully explorable until the spike closes.

**Standard/well-documented patterns, low research need:**
- Keyring multi-key extension (ARCHITECTURE already names the exact file-format and function-signature changes).
- Protocol-grade ingress ordering (PITFALLS + ARCHITECTURE both give a fully worked cheap-before-expensive check sequence with exact insertion points in `ingress.rs`).
- Signed peer directory crate (ARCHITECTURE gives the full dependency graph and naming).
- Push-notify adapter (ARCHITECTURE recommends the thin-wrapper design explicitly, zero ambiguity).

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH on versions (live crates.io fetch); MEDIUM on library-fit judgment; **LOW on hosting cost figures** (aggregator data, disagreed up to 2x — do not treat as settled) |
| Features | HIGH on documented protocol behavior (Signal/Matrix/Nostr/RFC 9421/OWASP); MEDIUM on which exact scheme FAMP should adopt (a judgment call, not settled by research) |
| Architecture | HIGH — every finding is grounded in the actual v1.0.0 tree with file:line citations; no speculative library research needed (brownfield code-archaeology) |
| Pitfalls | MEDIUM overall; HIGH on FAMP's own incident history; MEDIUM on cross-checked external claims (lethal trifecta, OWASP, NAT mechanics); LOW on several single-source web claims (individually flagged in the source doc) |

**Gaps to address during planning:**
- No FAMP-specific adversarial injection corpus exists yet — published benchmarks (AgentDojo, InjecAgent, WASP) are tool-calling-agent-shaped, not message-relay-shaped; the corpus must be built FAMP-native per PITFALLS's concrete list.
- The rendezvous mechanism for a PAKE-based pairing (how do two strangers, no existing channel, find each other to run the exchange?) has no existing design in this codebase — needs its own discussion/spec pass before planning.
- Symmetric-NAT test-network access for Phase 13's spike is an operational gap, not a research gap — needs a real carrier hotspot or equivalent, not simulated.
- Real, live vendor pricing (Fly.io/Lightsail/Hetzner) must be re-verified at spike time; this research's numbers are directional only.

---

## Sources

Aggregated from all four research files: crates.io live API, Context7 (`iroh`, `moka`, `tower-governor`), Magic Wormhole/Signal/Matrix/SSH/Syncthing/Tailscale/WireGuard/Nostr/Mastodon/Matrix federation docs, RFC 9421/9449, AWS SigV4 docs, OWASP GenAI LLM01:2025, Simon Willison's lethal-trifecta/CaMeL/MCP-injection posts, AgentDojo/InjecAgent/WASP benchmarks, and the FAMP repository itself (`ARCHITECTURE.md`, `.planning/PROJECT.md`, `docs/GATEWAY-SETUP.md`, `.planning/research/archive/v0.6/{STACK,PITFALLS}.md`).

---
*Research synthesis for: FAMP v1.1 Open-Internet Federation*
*Synthesized: 2026-07-30*
