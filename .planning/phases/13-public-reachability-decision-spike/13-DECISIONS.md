# Phase 13 Reachability Decision

**Status:** REACH-01 and REACH-03 satisfied by this record — see the traceability entries in REQUIREMENTS.md. **REACH-02 remains OPEN** — validating against a real symmetric-NAT network needs a carrier hotspot, which requires Ben. Phase 13 as a whole is not closed until REACH-02 runs; this record is the phase's decision-record deliverable (success criterion 5), not a claim that the phase is complete.

**Authority:** Ben pre-authorized reachability spend up to ~$15/mo before leaving on 2026-07-30, with the instruction to pick from the spike's evidence and build on it. This record exercises that authorization. All pricing inputs are vendor-verified in [`13-PRICING-VERIFIED.md`](13-PRICING-VERIFIED.md).

**Promoted from DRAFT, 2026-08-02.** Drafted 2026-07-30 (committed `26c95d7`, amended `61909cb`) as `.planning/research/REACH-DECISION-DRAFT.md`; never previously filed as Phase 13's own output, and REACH-01/REACH-03 were never checked in REQUIREMENTS.md despite the draft satisfying both. That gap — bookkeeping, not missing analysis — is closed by this promotion. No content below was altered to make the record agree with what Phase 17 built; verification against the shipped code (next section) confirms the two already agreed.

---

## Verified against what Phase 17 shipped (2026-08-02)

Checked before promoting, not after adjusting the text to match — a decision record edited to agree with the code after the fact would be worthless:

- **Store-and-forward shape.** `crates/famp-relay/src/queue.rs`'s `RelayQueues`: bounded, TTL-swept, in-memory, per-domain, drop-oldest under sustained overflow (logged, never silent). Matches this record's "self-hosted relay" model and the 17-04 plan's "bounded opaque store-and-forward queue" description — no divergence.
- **Fetch-authorization mechanism.** `crates/famp-relay/src/fetch_auth.rs`'s `sign_fetch_auth`: Ed25519 signature over a canonical form, reusing each gateway's own identity key. Grepped for `bearer`/token-shaped credentials in the fetch-auth module: none found. This record didn't specify the exact mechanism (that was Phase 17/17-04's own decision, D-26), but a signed-fetch design is consistent with — and stronger than — anything this record assumed; no divergence to report.
- **Confidentiality claim.** This record already states the relay sees **plaintext** envelope bodies (corrected 2026-07-31, `61909cb` — see the `⚠ CORRECTION` block below, left intact from the original draft). Matches Phase 17's shipped reality: `famp-crypto`/`famp-envelope`/`famp-gateway` sign but do not encrypt. No divergence.

**Conclusion: shipped and decided agree on all three checked points.** Nothing here required reconciliation.

---

## Decision: self-hosted relay, Ben-operated, on AWS Lightsail

**Cost:** $5.00/mo flat. 512MB RAM, 2 vCPU, 20GB SSD, 1TB transfer, **public IPv4 included at that price with no separate charge**. Verified live against `https://aws.amazon.com/lightsail/pricing/` on 2026-07-30.

**Operator:** Ben. Single box, single service, no follower-side infrastructure.

### Why a relay rather than a tunnel

The milestone's acceptance bar is not "packets flow." It is **a second person follows a doc unassisted**. Judged against that bar, every tunnel option moves setup burden onto the person least equipped to debug it:

- **Tailscale Funnel** looked like the lowest-burden option until the bidirectional requirement (REACH-04) was applied. A *visitor* to a Funnel URL needs nothing — but FAMP needs both gateways to **receive** inbound envelopes, so both operators must run their own Funnel endpoint. The follower therefore installs Tailscale and creates an account as the operator of their own tailnet. Each side runs an independent tailnet, so this does not literally violate "no shared VPN" — but it is a real, non-optional follower-side install, not the zero-touch story the single-direction framing suggests.
- **Cloudflare Quick Tunnel** is **disqualified**: the `*.trycloudflare.com` URL is regenerated on every restart, which breaks the stable-address story DIR-01 needs, and Cloudflare's own docs scope it to "testing and development only" with a 200-concurrent-request cap and no SLA.
- **Cloudflare named tunnel** requires the follower to own a domain and delegate it to Cloudflare nameservers. Domain purchase plus DNS delegation plus dashboard config is far past the unassisted bar.
- **ngrok** is **volatile, not disqualified**. Its current docs say free endpoints have no session timeout, contradicting the widely-repeated 2-hour cap — but that cap appears to have been imposed in Feb 2026 and reversed since, with no change-date on the docs page. A policy that flipped twice in one year is not something to build an acceptance gate on. Free tier is also 1GB/mo.

A Ben-operated relay inverts the burden: the follower installs `famp`, pairs, and is done. Nothing else to sign up for.

### Why not Oracle Always Free ($0/mo)

Genuinely free and still offered to new signups, verified. Rejected on **reliability**, not cost — two vendor-documented risks:
1. Oracle's own docs name "out of host capacity" as an expected condition for Always Free shapes, with "try another AD or wait and retry" as the remediation. Free and obtainable-on-demand are different claims.
2. Oracle reclaims an A1 instance when 95th-percentile utilization stays under 20% for 7 days. A low-traffic relay between two people is close to the definition of an idle instance.

Saving $5/mo is not worth a relay that can vanish. Revisit if traffic ever justifies it.

### Cheaper options not taken

- **Fly.io ~$2.02/mo** compute is the cheapest, but its default is a shared/anycast IPv4 and a *dedicated* IPv4 is a separate +$2/mo line item — an ambiguity that has to be resolved at provision time. Marginal saving, added uncertainty.
- **DigitalOcean $4.00/mo** with IPv4 included is a genuinely good option and only $1 cheaper.

**Lightsail wins on operator friction, not price.** Ben already has an AWS account, AWS tooling, and an existing `/deploy` skill built specifically around provisioning Lightsail VMs. The $1–3/mo difference is noise next to using infrastructure he already operates. **This is the softest reasoning in the decision** — if the AWS-familiarity premise is wrong, DigitalOcean at $4/mo is the swap, and nothing downstream changes.

### What the relay can and cannot observe (REACH-01, required)

> ### ⚠ CORRECTION — 2026-07-31
> **An earlier version of this section claimed the relay "cannot read message bodies… forwards opaque bytes." That was FALSE and is corrected below.** FAMP **signs but does not encrypt** — verified against the tree: there is no payload encryption anywhere in `famp-crypto`, `famp-envelope`, or `famp-gateway`. The relay terminates TLS and sees **plaintext envelopes**. Signing gives integrity and authenticity, not confidentiality. The error mattered because it understated what the relay operator can see and would have propagated into the follower-facing doc.

**Cannot:** forge or alter an envelope without detection. Every envelope is Ed25519-signed and verified end-to-end under the `FAMP-sig-v1\0` domain prefix, so a malicious relay cannot inject, modify, or replay-with-changes anything the receiver will accept. It also cannot impersonate a peer, since it holds no private key.

**Can — read everything.** Because there is no payload encryption, the relay sees **full message content in plaintext**, not just metadata. **The relay operator can read every message that transits their box.** Since Ben operates the relay, Ben can read the traffic of anyone who federates through it. This must be stated plainly and prominently in the follower-facing doc (DOC-06) — a second person cannot meaningfully consent to using this relay without knowing it.

**Consequence for relay design (fed into Phase 17, confirmed shipped as such above):** queue-drain authorization is therefore a **confidentiality** boundary, not merely an availability one. Whoever can drain a queue can *read* it. That rules out first-come/TOFU registration of queue owners at the relay — an attacker who registered a victim's queue first would read their plaintext, not merely deny them service.

**Options if this is unacceptable** (all out of scope for v1.1, recorded so the tradeoff is explicit rather than forgotten): end-to-end payload encryption using the peer keys already exchanged; or a relay that forwards at the TCP layer without terminating TLS; or per-pair relays. Each is real work and none is required for the milestone's acceptance bar — but "the operator can read your messages" is a property, not a bug to be discovered later.

**Can:** everything *about* the traffic — which principals talk to which, when, how often, message sizes, and timing. **Signing protects payload integrity, not metadata privacy.** Anyone with access to the relay host sees the full social graph and activity pattern of every pair using it. Since Ben operates it, Ben has that visibility over anyone who federates through it — that must be stated plainly in the follower-facing doc (DOC-06), not buried.

**Also:** the relay is a **single point of failure**. If it is down, delivery silently stops. That is precisely why REACH-05 exists — a reachability failure must surface at the sender as a distinct, actionable error rather than a fire-and-forget success. (REACH-05 shipped in Phase 17, `AckDisposition::Failed`.)

---

## REACH-03: why not `iroh`

`iroh 1.0.3`, published 2026-07-20 (1.0 line launched 2026-06-15) — verified live via the crates.io API. It is the best-fit crate in the ecosystem for pubkey-addressed P2P with hole-punching plus relay fallback, and n0 documents roughly 9 of 10 connections ending up fully direct.

**Rejected, for two independent reasons:**

1. **Transport migration cost.** iroh brings its own QUIC `Endpoint`; adopting it means *replacing* the shipped axum/rustls/reqwest transport that Gate A proved live on two machines — not augmenting it. Throwing away a working, tested transport mid-milestone, for a milestone whose declared hard problem is the human rather than the wire, is the wrong trade.

2. **"Free relays" does not survive contact with production — and this is vendor-documented, not inferred.** n0's own FAQ scopes the public relays to **development and testing only**, rate-limited to prevent abuse, and directs production workloads to either a self-hosted relay or a paid managed tier. The managed tier's price could not be found on any page fetched. So iroh in production means self-hosting a relay anyway — comparable ops burden to the decision above, just a different binary. **iroh buys you out of writing hole-punch code, not out of running infrastructure.**

Recorded rather than silently dropped, per REACH-03. If hole-punching later becomes worth the migration, this reasoning is what should be revisited — the rejection is about timing and transport churn, not about iroh being bad.

---

## REACH-02: OPEN — and a correction to our own stated premise

**Blocked on Ben.** Validating against a real symmetric-NAT network needs a carrier hotspot. See `.planning/REACH-02-HOTSPOT-WALKTHROUGH.md` for the procedure Ben needs to run.

**Correction worth carrying:** REQUIREMENTS.md's REACH-06 and earlier drafts cite "15–30% of hosts behind symmetric NAT." **That figure could not be re-verified against any current primary source.** The best citable measurement located is Richter et al., IMC 2016 (`arXiv:1605.05606`) — roughly a decade stale — reporting ~11% symmetric-dominant among non-cellular CGN ASes and a bimodal cellular split with **~40% symmetric-dominant**. A 2023 paper (arXiv:2311.04658) tests 5G cross-connectivity but did not yield an extractable percentage in this pass.

**Treat the prevalence as UNKNOWN at current recency.** Do not quote 15–30% as 2026 data in any shipped doc.

This does not weaken the decision — it strengthens it. A decade-old measurement showing ~40% symmetric on cellular is a reason to make **relay fallback mandatory rather than optional**, which is exactly what a relay-first architecture delivers. Hole-punching stays a later optimization (REACH-06, deferred to v2).

---

## Phase 13 status

| Item | Status |
|---|---|
| REACH-01 — model, live-verified cost, named operator, observability boundary | ✓ satisfied by this record — checked in REQUIREMENTS.md 2026-08-02 |
| REACH-03 — iroh weighed, rejection rationale recorded | ✓ satisfied by this record — checked in REQUIREMENTS.md 2026-08-02 |
| REACH-02 — validated against a real symmetric-NAT network | ✗ **BLOCKED — needs Ben's carrier hotspot** |

Phase 13 stays open until REACH-02 closes. Phase 17 already built against this decision (confirmed above, 2026-08-02) — REACH-02 validates the fallback assumption rather than the choice of model, which is why Phase 17 was correctly allowed to proceed without waiting on it.

---
*Drafted 2026-07-30 under standing pre-authorization. Promoted from DRAFT and filed as Phase 13's output 2026-08-02, after confirming Phase 17's shipped implementation matches this record with no divergence. Every price traced to a vendor URL fetched 2026-07-30; see [`13-PRICING-VERIFIED.md`](13-PRICING-VERIFIED.md) for the 8-item COULD NOT VERIFY list.*
