<!-- generated-by: gsd-doc-writer -->
# Store-and-Forward Relay Setup

Use `famp-relay` when two `famp-gateway` instances can make outbound HTTPS
connections but neither gateway can accept an inbound connection from the
other. Each gateway sends envelopes to the relay and polls the relay for its
own domain's queued envelopes.

The relay is a bounded, in-memory buffer. It is not a durable message broker,
a pairing proxy, or end-to-end encryption.

## Topology

For a gateway using a relay, `--peer <domain>=<url>` selects where outbound
envelopes are posted, while `--relay-fetch <url>` selects where that gateway
polls for envelopes addressed to its own domain.

```text
Gateway A                         famp-relay                         Gateway B
own domain: a.example       https://relay.example:8443        own domain: b.example

--peer b.example=RELAY  ──POST──> queue[b.example]
--relay-fetch RELAY     ──GET────> queue[a.example]

                              queue[a.example] <──POST──  --peer a.example=RELAY
                              queue[b.example] <──GET───  --relay-fetch RELAY
```

All four network operations above are outbound connections from a gateway to
the relay. Neither gateway needs the other gateway's address, and neither one
needs to accept an inbound connection from the other.

> **Do not point either relay-facing flag at the other gateway.** On A,
> `--peer b.example=https://relay.example:8443` and
> `--relay-fetch https://relay.example:8443` both point at the relay. B is
> symmetric. They never point at the other gateway's own `--listen` address.
> The bidirectional relay test asserts that neither gateway's argument vector
> contains the other gateway's listen address; see
> [`e2e_relay_bidirectional.rs`](../crates/famp-gateway/tests/e2e_relay_bidirectional.rs).

`famp-gateway` still requires its own `--listen`, `--tls-cert`, and `--tls-key`
arguments at startup. In this topology, that listener is not the message path
between the two gateways.

## Confidentiality warning

> **The relay operator can read every message that transits the relay.** FAMP
> signs envelopes but does not encrypt them. The relay terminates TLS and sees
> full plaintext message bodies, not only routing metadata. It also sees the
> complete social graph: who talks to whom, when, how often, message sizes, and
> timing. Signing provides integrity and authenticity, never confidentiality.
> Anyone you ask to federate through somebody else's relay must be told this
> before they agree.

A relay cannot alter or forge a signed envelope without the receiving gateway
detecting it, and it does not hold a participating gateway's private signing
key. Those facts do not make the message confidential from the relay operator.
The full boundary and its correction are recorded in
[`13-DECISIONS.md`](../.planning/phases/13-public-reachability-decision-spike/13-DECISIONS.md#what-the-relay-can-and-cannot-observe-reach-01-required).

Queue-drain authorization is therefore a confidentiality boundary, not merely
an availability control. Whoever can drain a domain's queue can read its
plaintext contents. For that reason, the relay operator configures queue-owner
keys explicitly with `--domain`; the relay never learns queue ownership through
TOFU or first-come registration.

## Deploy the relay

### TLS certificate and key

`famp-relay` always serves TLS. `--tls-cert` must name a PEM file containing at
least one certificate. `--tls-key` must name a PEM file containing a supported
PKCS#8, RSA, or SEC1 private key, and the key must work with the certificate
chain. The server does not request a TLS client certificate; queue drains are
authorized by signed fetch headers instead.

Use a certificate that validates for the hostname in the relay URL. Gateways
use the operating system root store by default. For a private CA or self-signed
development certificate, pass the PEM certificate to each gateway with
`--trust-cert`; that file is added to, rather than substituted for, the OS root
store.

### Public URL and fetch audience

Set `--public-url` to the HTTPS base URL the gateways use, such as
`https://relay.example:8443`. It is required because it becomes the audience
bound into every signed queue-drain request. The relay normalizes the audience
to scheme, lowercased host, and an explicit non-default port; it excludes path,
query, and trailing slash. The gateway applies the same normalization to its
`--relay-fetch` URL.

Use the same clean base URL for `--public-url`, every `--relay-fetch`, and the
URL portion of every `--peer`. A wrong audience causes the relay's exact fetch
authorization error:

```text
fetch audience mismatch: expected '<expected>', presented '<presented>'
```

### Obtain each gateway's public key

On each participating gateway host, set its own-domain and export the gateway
identity under a real sender agent principal. For example, on A:

```bash
FAMP_OWN_DOMAIN=a.example famp peer export --as agent:a.example/alice
```

The command prints exactly three whitespace-separated fields:

```text
agent:a.example/alice A_PUBLIC_KEY_B64URL A_KEY_ID
```

The relay operator needs the **second** field, `A_PUBLIC_KEY_B64URL`, and maps
it to A's domain:

```text
--domain a.example=A_PUBLIC_KEY_B64URL
```

Repeat on B with `agent:b.example/bob`. Share public export lines, never
`identity.ed25519` or any other private-key file. A repeated `--domain` may add
rotation keys for the same domain, but each domain is limited to four distinct
configured keys. Adding a fifth fails at startup with:

```text
famp-relay: --domain: domain '<domain>' already has 4 configured keys (RELAY_MAX_KEYS_PER_DOMAIN) — remove an old key before adding another
```

Adding or replacing an operator-configured key requires restarting the relay;
there is no hot registration or TOFU path.

The receiving gateways separately need the sender-agent export lines in their
peer keyrings so they can verify fetched envelopes. For the two-party example,
B imports A's full export line and A imports B's full export line with the
`famp peer import` command before either gateway starts. The relay's
domain-to-drain-key map and each gateway's inbound peer keyring enforce
different checks even when the same exported public key supplies both
configurations.

### Relay flag surface

These are all flags accepted by the hand-rolled `famp-relay` parser. Every
flag family is required and has no default.

| Flag | Required | Default | Meaning | Exact missing-flag error line |
|------|----------|---------|---------|-------------------------------|
| `--listen <addr>` | Yes | None | `SocketAddr` to bind, such as `0.0.0.0:8443` | `famp-relay: --listen <addr> is required, e.g. --listen 0.0.0.0:8443` |
| `--tls-cert <path>` | Yes | None | PEM certificate chain served by the relay | `famp-relay: --tls-cert <path> is required` |
| `--tls-key <path>` | Yes | None | PEM private key for the relay certificate | `famp-relay: --tls-key <path> is required` |
| `--public-url <url>` | Yes | None | Published relay base URL and signed-fetch audience | `famp-relay: --public-url <url> is required — it is the audience every fetch signature is bound to, so a wrong or absent value fails every fetch with an audience mismatch` |
| `--domain <domain>=<pubkey>` | Yes, repeatable | None | Explicitly authorizes one of up to four distinct public keys to drain that domain | `famp-relay: at least one --domain <domain>=<pubkey> is required — a relay serving no domains has nothing to do` |

### Start the relay

This worked example uses:

| Item | Value |
|------|-------|
| Relay URL | `https://relay.example:8443` |
| Relay bind address | `0.0.0.0:8443` |
| A's domain and sender | `a.example`, `agent:a.example/alice` |
| B's domain and sender | `b.example`, `agent:b.example/bob` |

Replace the two public-key placeholders with the second fields of the exports
described above, then start the relay:

```bash
famp-relay --listen 0.0.0.0:8443 \
  --tls-cert /etc/famp-relay/relay.cert.pem \
  --tls-key /etc/famp-relay/relay.key.pem \
  --public-url https://relay.example:8443 \
  --domain a.example=A_PUBLIC_KEY_B64URL \
  --domain b.example=B_PUBLIC_KEY_B64URL
```

After argument parsing, TLS loading, and socket binding succeed, the relay
prints a readiness line of this shape:

```text
famp-relay: ready, serving domain(s): a.example (1 key(s)), b.example (1 key(s)) — audience https://relay.example:8443
```

## Wire both gateways to the relay

The relevant gateway flags are:

| Flag | Required by `famp-gateway` | Default or absence behavior | Relay-topology use |
|------|----------------------------|-----------------------------|--------------------|
| `--listen <addr>` | Yes | None | Required local HTTPS listener; the other gateway does not use it in this topology |
| `--tls-cert <path>` | Yes | None | Certificate for this gateway's own listener |
| `--tls-key <path>` | Yes | None | Private key for this gateway's listener certificate |
| `--peer <domain>=<url>` | No, repeatable | No route is added for an absent peer | Map the remote domain to the relay URL; duplicate domains are rejected |
| `--backs agent:<domain>/<name>` | No, repeatable | With no `--backs`, bare names work only when exactly one `--peer` is configured | Explicitly bind the remote principal stand-in to its matching `--peer` domain |
| `--trust-cert <path>` | No | OS root store only | Add a private relay CA or self-signed relay certificate to outbound TLS trust |
| `--relay-fetch <url>` | No | No relay polling | Poll this relay for the gateway's own-domain queue |
| `<principal-name>...` | Yes, at least one | None | Bare remote principal name this gateway backs locally |

The gateway's own-domain is mandatory, but `--domain` is not a
`famp-gateway` flag. The gateway resolves its domain from `FAMP_OWN_DOMAIN`,
then `$FAMP_HOME/own-domain` if the environment variable is absent.

On A, back remote principal Bob, post B-bound traffic to the relay, and poll
the same relay for A-bound traffic:

```bash
FAMP_OWN_DOMAIN=a.example famp-gateway \
  --listen 0.0.0.0:9443 \
  --tls-cert /etc/famp/gateway-a.cert.pem \
  --tls-key /etc/famp/gateway-a.key.pem \
  --backs agent:b.example/bob \
  --peer b.example=https://relay.example:8443 \
  --relay-fetch https://relay.example:8443 \
  --trust-cert /etc/famp/relay-ca.pem \
  bob
```

On B, use the symmetric configuration:

```bash
FAMP_OWN_DOMAIN=b.example famp-gateway \
  --listen 0.0.0.0:9443 \
  --tls-cert /etc/famp/gateway-b.cert.pem \
  --tls-key /etc/famp/gateway-b.key.pem \
  --backs agent:a.example/alice \
  --peer a.example=https://relay.example:8443 \
  --relay-fetch https://relay.example:8443 \
  --trust-cert /etc/famp/relay-ca.pem \
  alice
```

If the relay certificate chains to an OS-trusted public root, omit
`--trust-cert` from both commands. Do not replace either `--peer` URL or
either `--relay-fetch` URL with `https://gateway-a.example:9443` or
`https://gateway-b.example:9443`; those direct listener URLs are not part of
the relay message path.

## What this does not solve: pairing

The relay carries message transport, not the pairing handshake.

`famp pair redeem --from <url>` dials the inviter's gateway directly over
HTTPS. The implementation in
[`redeem.rs`](../crates/famp/src/cli/pair/redeem.rs) removes any trailing
slash from `--from`, appends `/famp/v1/pair/redeem`, and calls
`client.post(&url)` on that address. There is no relay branch on this path.

Therefore, the relay does **not** remove the inviter's need for an
inbound-reachable HTTPS gateway endpoint at pairing time. If the inviter's
gateway cannot be reached directly, this command cannot redeem the invite
through `famp-relay`:

```bash
famp pair redeem --from https://gateway-inviter.example:9443
```

The `--from` URL above must reach the inviter's own gateway listener, not the
relay. Solve pairing reachability separately; do not advertise the relay as a
pairing solution.

## Operational notes

The relay is a single point of failure and its queues exist only in process
memory. Restarting the relay loses every queued envelope. While it is down,
gateways cannot enqueue new messages or drain already queued messages.

An immediate outbound relay failure is surfaced locally at the original
sender. The sending gateway logs:

```text
famp-gateway: egress[<name>]: failed to relay envelope: <debug error>
```

It also writes an unsigned local `class: "ack"` envelope to the original
sender's mailbox with `body.disposition: "failed"`. Its reason starts with the
stable prefix:

```text
famp-gateway relay failed: could not relay to <recipient>: <debug error>
```

That notification is asynchronous; local acceptance by `famp send` is not a
claim that the relay or recipient accepted the envelope.

There is a harder failure boundary after the relay returns HTTP 202. The
sending gateway then considers the POST successful. If that accepted entry is
later evicted, expires, or is lost in a relay restart, the original sender does
not receive the immediate relay-failure `ack`; the relay only logs eviction.
Do not treat HTTP 202 as end-to-end delivery confirmation.

Queue limits are fixed in `queue.rs`:

| Bound | Source value | Behavior |
|-------|--------------|----------|
| Entries per destination domain | 1,024 | At the cap, enqueue drops the oldest entry, accepts the newest, and logs `dropped oldest entry (queue at cap)` |
| Entry TTL | 900 seconds | An entry exactly 900 seconds old is retained; an older entry is removed during a drain or sweep |
| Background sweep interval | 30 seconds | Reclaims expired queue entries and fetch-auth state even when nobody polls a domain |
| Fetch batch | 64 entries | One authorized fetch drains at most 64 FIFO entries; the remainder stays queued |
| Request body | 1,048,576 bytes | Larger enqueue or fetch request bodies are rejected with HTTP 413 |

The enqueue route is intentionally unauthenticated at the relay layer. Anyone
who knows the relay URL and a configured destination domain can fill that
domain's queue and force legitimate old entries out after their senders have
already received HTTP 202. Operate the relay with that denial-of-service risk
understood; message signatures do not prevent queue flooding.
