# REACH-02: The Carrier-Hotspot Test — Walkthrough

**For:** Ben. **Time:** ~15 minutes. **Needs:** your phone (cellular hotspot) + one laptop. No FAMP code required — this runs entirely on tools already installed.

---

## Why this test exists

v1.1's whole premise is that two people on **different networks they don't control** can exchange messages. v1.0 only ever proved two machines on a network you *do* control.

The reachability decision (self-hosted relay, Lightsail, ~$5/mo) rests on one assumption:

> **A home or cellular network will not let an outside machine dial in, so messages must be *pulled* outward to a relay rather than *pushed* inward to the peer.**

That sounds obviously true. It's the kind of assumption that's obviously true right up until it isn't — and if it's wrong in either direction, we're building the wrong thing:

- If direct dial-in **does** work from a normal network, the relay may be unnecessary — a recurring cost and a metadata-exposure point we didn't need.
- If outbound-to-relay **doesn't** work from cellular, the relay design is broken for exactly the user we care about.

There's a second reason, and it's the honest one. The research pass could **not** verify the commonly-cited "15–30% of hosts are behind symmetric NAT" figure for 2026 — the best citable source is a 2016 paper (~40% on cellular), a decade stale. So we're currently reasoning from a number nobody can stand behind. **This test replaces a stale statistic with one measurement of your actual network.** One data point beats a decade-old average.

---

## What we're actually measuring

Three questions, in order of importance:

| # | Question | Why it matters |
|---|---|---|
| 1 | Does **outbound** HTTPS work from cellular? | The relay design depends entirely on this. If it fails, the design fails. |
| 2 | Can an outside machine **dial in** to you? | If it can, a relay may be optional. If it can't, the relay is mandatory — not a preference. |
| 3 | What **NAT type** is the carrier using? | Determines whether hole-punching could *ever* work, which decides if REACH-06 is worth revisiting later. |

---

## Setup

1. Turn on your iPhone's **Personal Hotspot**.
2. Join your laptop to it — **Wi-Fi off from your home network first**, so you're genuinely on cellular and not silently still on home Wi-Fi. Verify:

```bash
# Should show a carrier IP, NOT a 192.168.x.x home-router address
curl -s https://api.ipify.org; echo
```

Write that public IP down — call it `PUBLIC_IP`. If it looks like your usual home IP, you're not actually on cellular; turn Wi-Fi fully off and rejoin.

---

## Test 1 — Outbound reachability (the one that matters most)

```bash
curl -s -o /dev/null -w "HTTPS out: %{http_code} in %{time_total}s\n" https://api.ipify.org
curl -s -o /dev/null -w "HTTPS out (port 8443): %{http_code}\n" https://cloudflare.com:8443 --max-time 8 || echo "port 8443 blocked"
```

**Expected:** the first returns `200`. **If it doesn't, stop and tell me** — that would mean the carrier blocks ordinary outbound HTTPS, which breaks the entire design and is a far bigger finding than this test was scoped for.

The second is a bonus: some carriers only allow 443/80. If 8443 is blocked, the relay must listen on **443**, which is a real constraint worth knowing before we provision anything.

---

## Test 2 — Can anything dial in? (proves the relay is mandatory)

Start a listener on the laptop:

```bash
# leave this running
nc -l 9999
```

Then from **any machine not on the hotspot** — your other machine on home Wi-Fi is fine:

```bash
# replace with the PUBLIC_IP you wrote down
nc -vz -w 5 PUBLIC_IP 9999
```

**Expected:** it fails — timeout or refused. That failure is a **success for our purposes**: it confirms nothing can reach you unsolicited, so pulling from a relay is the only workable shape.

**If it connects**, that's genuinely surprising and worth a conversation: your carrier is handing out a routable address, and direct-dial becomes possible for *some* users — though still not something we could rely on generally.

---

## Test 3 — NAT type (decides whether hole-punching is ever viable)

This is what actually answers the symmetric-NAT question. Two STUN servers, two different public-facing ports:

```bash
# install once if needed: brew install stuntman
stunclient stun.l.google.com 19302
stunclient stun1.l.google.com 19302
```

Read the **mapped address** line each prints.

- **Same public port from both servers** → cone NAT. Hole-punching *could* work.
- **Different public port from each** → **symmetric NAT.** Hole-punching cannot work, ever, for this network. Relay fallback is mandatory, not merely prudent.

If `stunclient` isn't available and you'd rather not install it, skip this test — Tests 1 and 2 carry the decision on their own. Test 3 only informs whether REACH-06 (hole-punching as a later optimization) is worth revisiting.

---

## What to send me

Paste these back:

1. `PUBLIC_IP` (or just "carrier IP, not home") — plus whether the 8443 check passed
2. Test 2's result: **did the inbound connection fail?** (expected: yes, it failed)
3. Test 3's two mapped addresses, if you ran it — **same port or different?**

---

## How I'll read the results

| Outcome | What it means |
|---|---|
| Outbound OK, inbound blocked, symmetric NAT | **The expected result.** Confirms relay-first, confirms hole-punching is a dead end, closes REACH-02. |
| Outbound OK, inbound blocked, cone NAT | Relay-first still correct. Records that hole-punching *could* work later — REACH-06 stays a live option rather than a dead one. |
| Outbound OK, **inbound succeeds** | Surprising. Direct-dial is possible on your carrier. Doesn't overturn relay-first (we can't assume every follower gets this), but it changes what we tell people is possible. |
| **Outbound blocked** | Design-breaking. Stop; we rethink the transport before Phase 17 goes further. |
| 8443 blocked | The relay must listen on 443. Small but load-bearing — cheaper to know now than after provisioning. |

---

## One caveat about what this proves

This measures **your** carrier, on **one** day. It does not establish a general prevalence rate, and I won't write it up as though it does — that's the exact overreach that produced the unverifiable "15–30%" figure in the first place.

What it *does* give us is a real, dated, reproducible observation of the network conditions the design has to survive, which is strictly better than a stale citation. If a second person later runs the same three commands on their network, that's a second data point, and the doc for that is this file.

---
*Written 2026-07-31 for REACH-02. Closes the last open item in Phase 13.*
