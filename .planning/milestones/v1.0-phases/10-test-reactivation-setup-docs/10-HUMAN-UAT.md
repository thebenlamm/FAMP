---
status: failed
phase: 10-test-reactivation-setup-docs
source: [10-03-PLAN.md, 10-CONTEXT.md D-07]
started: 2026-07-27T00:00:00Z
updated: 2026-07-28T00:00:00Z
outcome: wire-proven-bidirectionally; unassisted-guide clause FAILED (guide wrong in 8 ways + no shipping client can address a remote principal); v1.0.0 blocked pending C2/C5 fix
---

## Why this exists

DOC-04 requires two things: (1) a setup guide whose commands/flags match the
shipping binary, and (2) proof that a developer can follow the guide
**unassisted** and reach a working cross-host connection. The automated
accuracy gates added in this phase
(`crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs`,
`crates/famp/tests/gateway_setup_doc_accuracy.rs`) fully cover (1) — they
fail CI if `docs/GATEWAY-SETUP.md` drifts from `famp-gateway`'s usage string
or `famp peer export/import --help`. They do **not** and cannot cover (2):
whether the guide's prose, ordering, and troubleshooting notes are actually
followable by a human who has never run the gateway before, on two real
machines.

Per D-07 (CONTEXT.md), this human clause is **the milestone's Gate A
dogfood** — Ben running `docs/GATEWAY-SETUP.md` unassisted between his
laptop and a dev server (direct connection or his existing VPN, no public
relay) — and is tracked here as a deferred acceptance, the same pattern used
for the DAEMON-06 Linux behavioral UAT at v0.11 close (STATE.md §"Deferred
Items"). **DOC-04 is NOT claimed fully done on the grep-gate alone; this file
is what carries the outstanding human-verified clause until Ben completes
the run.**

## Scenario

Ben follows `docs/GATEWAY-SETUP.md` from a cold start, unassisted, on two
machines he controls:

- **Machine A:** Ben's laptop.
- **Machine B:** a dev server Ben controls, reachable from A either directly
  or over a VPN Ben already runs. **No public relay, no public DNS
  requirement** — this is explicitly own-machines-first (PROJECT.md,
  10-CONTEXT.md §domain).

He reaches a working **bidirectional** cross-host connection: a signed
envelope delivers A→B, a signed envelope delivers B→A, and both task FSMs
independently reach a terminal state.

## Checklist (mirrors the guide's five sections)

- [ ] **1. Prerequisites** — `famp daemon install` run on each host; broker
      reachable on both A and B; TLS cert/key prepared for each host's
      `--listen` address.
- [ ] **2. Gateway identity** — confirmed `~/.famp/gateway/identity.ed25519`
      is generated automatically (no manual keygen step needed) on first
      `famp peer export` / `famp-gateway` invocation on each host.
- [ ] **3. Out-of-band key exchange** — `famp peer export --as
      agent:hostA.example/gateway` on A, pasted into `famp peer import` on
      B (and the reverse, B→A); the `key_id` fingerprint eyeball-check was
      actually performed before trusting each import (not skipped).
- [ ] **4. Start each gateway** — `famp-gateway --listen <addr> --tls-cert
      <path> --tls-key <path> --peer <domain>=<url> <principal>` started
      successfully on both A and B, each printing `famp-gateway: ready,
      backing N principal(s): ...`.
- [ ] **5. Connect / verify** — a message sent from A's principal to B's
      federated principal, and vice versa; `famp inspect tasks --id
      <task_id> --json` run on both sides shows the task FSM advancing to a
      terminal state (`COMPLETED`, `FAILED`, or `CANCELLED`) for both
      directions.

## Acceptance criteria

- [ ] A signed envelope delivers A→B and the task FSM reaches a terminal
      state on both A's and B's `famp inspect tasks` output.
- [ ] A signed envelope delivers B→A and the task FSM reaches a terminal
      state on both sides.
- [ ] Ben completed the walkthrough using **only** `docs/GATEWAY-SETUP.md`
      — no undocumented steps, no help from anyone who built the feature.
- [ ] Any point where the guide was unclear, wrong, or missing a step is
      recorded below and fixed in `docs/GATEWAY-SETUP.md` (a doc fix, not a
      re-run of this UAT, unless the fix changes a command/flag the
      accuracy gates check).

## Status

**FAILED (as a guide-followability test) — but the underlying wire is PROVEN
BIDIRECTIONALLY.** Run on 2026-07-28 by `opus` (Ben's laptop, macOS,
100.99.215.33) ↔ `zed` (Ben's home server `home-devbox`, Ubuntu,
100.112.29.111) over a Tailscale tailnet, coordinating over FAMP, build
`c91e794`.

**What passed (evidence):** a signed `request` envelope crossed the tailnet
in **both** directions and landed in the real remote agent's mailbox —
leg 1 alice→bob `task_id=019fa681-7a21-73f3-a49c-de62b58e8639` (in server's
`bob` mailbox), leg 2 bob→alice `task_id=019fa6a2-adff-7ad3-84cc-984d305e8b29`
(in laptop's `alice` mailbox), each confirmed by on-disk grep **and**
`famp inbox`. Egress-signed with the sender gateway's Ed25519 key,
TOFU-verified at the receiver's ingress against the out-of-band-pinned peer
key (Ben eyeball-verified both fingerprints), wrapper-stripped per BUS-11.
Free negative control: an unsigned probe body → `HTTP 400 invalid_signature`,
zero bus writes. **Precise claim:** delivery is *gated* on verification (the
stripped wrapper is by design), so mailbox presence *entails* a successful
signature check — NOT "the delivered envelope carries a signature" (it
doesn't).

**Why the acceptance criteria are NOT met (why status = failed):**
1. **No shipping client can perform this** — both legs required a
   hand-written injector (`crates/famp-gateway/tests/wire_proof_inject.rs`,
   `#[ignore]`d) using a raw bus API, because `famp send`/`famp_send`
   hardcode `to = agent:local.bus/<name>` and emit class `audit_log`. The §5
   recipe ("send via `/famp-send`") is non-functional. **This is the v1.0.0
   blocker.**
2. **The FSM did not reach a terminal state** — a single `request` was sent,
   not a full request→commit→deliver→ack cycle (and chat-style `famp send` is
   `audit_log`, unmodeled by the FSM anyway). The "terminal state" acceptance
   clause is unreachable via shipping tools.
3. **The guide could not be followed unassisted** — it is wrong/incomplete in
   8 ways (below); we succeeded only by reading source and correcting each.

**Path to flip this to `passed`:** land the C2/C5 shipping-client fix
(design verdict already settled — two external deep-research passes + zed's
source control converge on sender-side split-addressing) + fix the 8 findings
below, then **re-run this dogfood driving the fixed `famp send` (no
injector)**. Until then, DOC-04's human clause is FAILED and v1.0.0 must not
be tagged. `/gsd-verify-work` and `/gsd-complete-milestone` must surface this.

## Findings (8) — every one invisible to Linux-only CI

1. **Wiring inverted (doc §4):** a gateway backs the **remote** principal, not the local one.
2. **Pin label wrong (doc §3):** export/pin under the **sender agent principal**, not `/gateway` — ingress verifies on the envelope `from`.
3. **Keyring load-once (doc):** no hot-reload; the §3-before-§4 (pin-before-launch) ordering is load-bearing but unstated.
4. **Duplicate-pubkey bricks startup (code+doc):** re-exporting under a corrected label leaves two lines with one pubkey → keyring fails closed (`duplicate pubkey`); and `"ready"` prints **before** the keyring loads → false-success health signal. Fix: strip stale line; load keyring before printing ready.
5. **serverAuth EKU required on macOS (doc §1):** Apple's verifier rejects a no-EKU cert (`EkuError`); Linux webpki tolerates absent EKU.
6. **macOS host firewall stalls inbound (doc/setup §1):** unmentioned; leg 1 hid it (outbound only). Pre-authorize `famp-gateway` (`socketfilterfw --add`/`--unblockapp` or the Allow prompt).
7. **Transport swallows its error chain (code):** `famp-transport-http/src/error.rs:63` Display=`"reqwest failure"`; `egress.rs:211` `e.to_string()` discards the `#[source]`. Every transport fault logs identically — forced two hand-rolled probes. Fix: log the `.source()` chain.
8. **CA:TRUE-as-leaf rejected on Linux (doc §1) — mirror of #5:** webpki rejects `CA:TRUE` used as the server end-entity (`CaUsedAsEndEntity`); Apple tolerates it. The two verifiers are strict in **opposite** directions; a naive `openssl req -x509` cert fails on **both**. "Self-signed is fine" is **actively wrong**.

**Canonical cert recipe (satisfies both platforms):**
```
openssl req -x509 -newkey rsa:2048 -nodes -days 800 \
  -keyout <host>.key.pem -out <host>.cert.pem -subj "/CN=<host>" \
  -addext "subjectAltName=IP:<tailnet-ip>,DNS:<hostname>" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
  -addext "extendedKeyUsage=serverAuth"
```

**Recommended §5 pre-flight (the discipline that saved the debug loop):** after both gateways are up, **probe the live peer's TLS+ingress before sending** — a `400 invalid_signature` back means TLS validated + ingress reached + security gate working, in one shot. "Send and see" is what hid #5/#6/#8 behind `"reqwest failure"`.
