# REACH-02 Results: Real Carrier-Hotspot Validation

**Status:** REACH-02 CLOSED, checked in `.planning/REQUIREMENTS.md` 2026-08-02.
**Procedure:** `.planning/REACH-02-HOTSPOT-WALKTHROUGH.md` (executed as written, with the two methodology fixes below applied).
**Run date:** 2026-08-02, two independent runs, real Verizon cellular hotspot.

---

## Caveat, stated up front and not to be lost later

**This measures ONE carrier, on ONE day.** It establishes no prevalence rate. Do not let any future write-up drift into a claim like "cellular is cone NAT" — that would repeat the exact overreach that produced the original unverifiable "15–30% symmetric NAT" figure this test was built to replace. This is a single dated, reproducible observation of the network conditions the design has to survive — nothing more, nothing less.

---

## Network verification

**Public IP:** `174.228.224.145`.
**Ownership verified via reverse DNS**, not inferred from address shape: `145.sub-174-228-224.myvzw.com`, `AS6167 Verizon Business`. (Contrast: the home network's IP that day was `70.111.79.171` — confirming the hotspot was genuinely a distinct network, not a home connection mislabeled.)

---

## Test 1 — Outbound reachability

| Run | HTTPS 443 | Port 8443 |
|---|---|---|
| 1 | 200 in 0.20s — PASS | 301 in 0.33s — **not blocked** |
| 2 | 200 in 0.18s — PASS | 301 in 0.25s — **not blocked** |

The relay is not forced onto port 443; 8443 works fine on this carrier.

---

## Test 2 — Inbound dial-in

Probed **from** the Lightsail relay (`54.158.102.139`) **into** the hotspot, both runs: TCP port 9999 timed out; the local listener received nothing.

**Positive control (run 2):** before the inbound probe, the relay first proved it could reach `1.1.1.1:443` and succeeded, ruling out "the relay has no outbound networking" as an alternative explanation for the inbound timeout. This control was the fix applied per the walkthrough's Task 3(b) correction — without it, a timeout is ambiguous between "inbound is blocked" (the finding) and "the prober is broken" (an artifact).

**Result: relay-first confirmed.** Nothing can dial in unsolicited on this carrier. This result holds independent of NAT flavor — it does not depend on the network being symmetric or cone.

---

## Test 3 — NAT type

4 STUN servers queried, one fixed local UDP port per run:

- **Run 1:** all four servers returned `174.228.224.145:4190`.
- **Run 2:** all four servers returned `174.228.224.145:4180`.

Identical external mapping across all four servers within each run => **cone NAT**, both runs. (The external port differs *between* the two runs because the local source port differed between runs — that is expected, and is not a contradiction of the within-run consistency that defines cone NAT.)

**Method note:** `stunclient` was not installed and nothing was installed on Ben's machine for this test; a throwaway Python STUN client was used instead.

---

## What this means for REACH-02 and REACH-06

**REACH-02** asked for validation against a real network Ben does not control — originally worded "symmetric-NAT" because a stale, unverifiable 2016 citation predicted the test environment would be symmetric (see `13-DECISIONS.md`'s "Correction worth carrying" section and `13-PRICING-VERIFIED.md`). That prediction is now falsified: this carrier is cone NAT. The requirement's real intent — a network outside Ben's control, not a specific NAT flavor — is satisfied regardless, and REACH-02 is closed on that basis (see the reworded requirement in `REQUIREMENTS.md`).

**Critically, this does not weaken the relay-first conclusion.** Relay-first is confirmed by the **inbound** result alone (nothing can dial in), which holds on every NAT flavor — cone included. It was never contingent on the NAT being symmetric.

**REACH-06** (hole-punching as a v2 optimization) previously described hole-punching as categorically impossible, citing the same stale figure. That framing is now corrected in `REQUIREMENTS.md`: on a cone NAT, hole-punching *could* work given a coordination server — measured true for at least this one real carrier, on this one day. Not adopted this milestone; not proven universal; relay fallback stays mandatory regardless.

---

## Cross-reference

- Requirement text and checkbox: `.planning/REQUIREMENTS.md` (REACH-02, REACH-06).
- Procedure run: `.planning/REACH-02-HOTSPOT-WALKTHROUGH.md` (marked EXECUTED, both methodology fixes applied — real `ipinfo.io` ownership lookup in place of an IP-shape eyeball check, and a positive control ahead of the inbound probe).
- Phase 13 decision record: `13-DECISIONS.md` (relay-first model, cost, iroh rejection).
- Stale-citation background: `13-PRICING-VERIFIED.md`.

---
*Filed 2026-08-02. Closes the last open item in Phase 13 alongside REACH-01/03 (already checked) and pending REACH-04 (needs two gateways on genuinely different networks — still open, tracked separately, not this file's subject).*
