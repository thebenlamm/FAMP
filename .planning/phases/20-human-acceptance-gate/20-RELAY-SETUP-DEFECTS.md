# `docs/RELAY-SETUP.md` — defects from its first real execution

**2026-08-08.** `docs/RELAY-SETUP.md` was written from the crate sources, not from
deploying anything. This records what broke when it was followed for real, standing
up `relay.famp.dev` on the Lightsail box. Fix these **before** the guide is
re-frozen for the clean-host attempt.

Every item below cost an improvisation. A doc that only works when an expert
improvises is exactly the failure Phase 20 exists to catch — a second person hitting
any of these would be stuck.

## Confirmed working (independently verified, not taken on report)

From a machine outside AWS, against the public trust store with no `-k`:

```text
relay.famp.dev:443 -> HTTP 404, TLS verify result 0 (valid)
issuer=C=US, O=Let's Encrypt, CN=YE2
subject=CN=relay.famp.dev
notAfter=Nov  6 20:35:02 2026 GMT
```

The 404 on `/` is correct — the relay serves only its own paths. Readiness line
observed on the box, matching `crates/famp-relay/src/main.rs:249` exactly:

```text
famp-relay: ready, serving domain(s): ben.famp.dev (1 key(s)) — audience https://relay.famp.dev
```

## Defects

### R1 — no installation instructions for the relay binary itself
The doc explains how to *run* `famp-relay` but never how to get it onto the host.
The deployer had to discover `famp-relay-installer.sh` in the release assets. Add an
install section mirroring `FOLLOWER-SETUP.md` §1.

### R2 — every example uses port 8443, but a real deployment wants 443
`RELAY-SETUP.md:170-183` shows `--listen 0.0.0.0:8443` throughout. Binding 443
requires privilege, which the doc never mentions — no note about running as root,
`CAP_NET_BIND_SERVICE`, or a reverse proxy. **Related security observation:** the
deployed relay currently runs **as root**, both to bind 443 and to read
`/etc/letsencrypt/live/*/privkey.pem`. That works but is not what should be
documented as the recommended posture. Decide the intended shape (dedicated user +
`CAP_NET_BIND_SERVICE` + a readable key copy) and document it.

### R3 — no systemd unit, so the relay does not survive a reboot
`RELAY-SETUP.md:155-183` stops at "run this command." A relay whose whole purpose is
being reachable must come back after a restart. A unit was hand-written with
`Restart=always`; that should be in the doc, not improvised per-deployment.

### R4 — TLS guidance predates the existence of a real DNS name
`RELAY-SETUP.md:69-81` describes cert requirements but says nothing about obtaining
one. Now that `relay.famp.dev` exists, Let's Encrypt is the obvious path and removes
the self-signed `--trust-cert` distribution problem entirely. Document the certbot
HTTP-01 flow. Also state explicitly that `fullchain.pem` is the right input to
`--tls-cert` (it was accepted, but the doc leaves the reader guessing between
fullchain and leaf).

### R5 — audience normalization is unclear at the default port
`RELAY-SETUP.md:86-90` illustrates `--public-url` only with a non-default port. With
443 the readiness line prints no `:443`, which is correct but surprising if you are
checking your config against the doc's example. Show the default-port case, since
that is what a real deployment uses.

### R6 — key-type guidance is narrower than reality
`RELAY-SETUP.md:73-74` lists "PKCS#8, RSA, or SEC1" without noting that certbot's
default ECDSA key (SEC1) works. A reader with an ECDSA key cannot tell from the
prose whether they need to convert it.

## Improvisations the doc should have anticipated

- **Port 80 must be reachable for HTTP-01.** On EC2 this meant editing the security
  group. The Lightsail box already had 80 open, which is *why* certbot worked there
  without comment — a reader on a locked-down host will not be so lucky. This also
  matters for **renewal**: certbot standalone needs 80 both free and reachable at
  renewal time, or the cert silently expires and the relay stops serving.
- **`loginctl enable-linger`** was needed on the inviter so the user broker survives
  logout. `famp daemon status` does surface this, but the follower guide never
  mentions it, and on a headless box reached only over SSH it is not optional.
- **Non-interactive SSH has no `~/.cargo/bin` on PATH.** Remote commands need
  absolute paths. Anyone scripting a deployment hits this immediately.

## Deployed state (for teardown and for the rehearsal)

| Thing | Value |
|---|---|
| Relay | Lightsail `famp-relay`, `relay.famp.dev` → `44.219.73.36`, port 443, systemd, running as root |
| Relay cert | Let's Encrypt, expires 2026-11-06 |
| Inviter | EC2 `i-0c63694b9fa161da3`, `ben.famp.dev` → `44.204.243.222`, t3.small, tagged `Purpose=FAMP-DOC-07` |
| Inviter domain | `ben.famp.dev`, gateway pubkey configured at the relay |
| Cost | EC2 ≈ $0.50–0.60/day on top of the pre-existing Lightsail $5/mo and Route53 $0.50/mo |

**Not yet done:** the follower host does not exist. Per issue #39 the relay must be
restarted once the follower's gateway identity exists so its `--domain` can be
added — harmless here only because no traffic has been queued yet.

## Inviter status correction (verified 2026-08-08, later the same day)

The row above says the inviter is deployed. That is true of the *host*, not of a
*serving gateway*. Verified directly, not taken on report:

```text
from outside AWS:  ben.famp.dev -> 44.204.243.222; tcp/443, tcp/8443, tcp/9443 all
                   refused; tcp/22 open
on the box:        pgrep -laf famp-gateway -> none
                   ss -tlnp                -> :22 only
                   systemctl --user        -> famp-broker.service active running
                   loginctl Linger         -> yes
                   ~/.famp/own-domain      -> ben.famp.dev
                   ~/.famp/gateway/identity.ed25519 -> present, 0600
                   /etc/letsencrypt/live/ben.famp.dev/ -> present (issued 21:34Z)
```

So execution-plan step 3 is **incomplete**: identity, domain, broker, linger, and
TLS material are all in place, but no `famp-gateway` process is running and no unit
exists to start one. Because there is no listener, the closed ports do not yet tell
us whether the EC2 security group also needs a rule — start the listener first, then
re-probe from outside; only a still-refused port after that implicates the SG.

This is R3 (no systemd unit) recurring on a second host: the relay needed a
hand-written unit and so will the inviter gateway.

**Access note:** the only SSH key that authenticates to the inviter is
`famp-phase20-key.pem`, and it currently lives *only* in an ephemeral session
scratchpad under `/private/tmp`. Copy it somewhere durable before relying on it.
