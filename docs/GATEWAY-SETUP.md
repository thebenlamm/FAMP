# Two-Machine Gateway Setup (v1.0 Federation)

This guide walks through standing up `famp-gateway` between **two machines
you own** — e.g. your laptop and a dev server, connected directly or over a
VPN you already control. **There is no public relay.** Both hosts must be
directly reachable from each other (or reachable over your own VPN) at the
address you give `--listen`; FAMP does not provide discovery, NAT traversal,
or a hosted directory.

Everything below is copy-pasteable and uses the exact flag spellings the
shipping `famp-gateway` and `famp peer` binaries accept — see
`crates/famp-gateway/src/main.rs` and `crates/famp/src/cli/peer/` if you want
to verify this yourself. A compiled accuracy test
(`gateway_usage_doc_accuracy.rs` / `gateway_setup_doc_accuracy.rs`) fails CI
if this guide's flags ever drift from the binary.

We'll call the two machines **A** and **B** throughout. Machine A runs the
real identity `alice`; machine B runs the real identity `bob`.

## 1. Prerequisites

On **each** host:

- `famp` and `famp-gateway` installed, and the persistent broker running:
  ```bash
  curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
  curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-gateway-installer.sh | sh
  famp daemon install
  ```
  Both installers write to `~/.cargo/bin`; if that directory isn't already
  on your `PATH`, the installer prints a warning plus the shell-profile line
  to add — don't skip it. Building from source instead? `cargo install
  --path crates/famp` and `cargo install --path crates/famp-gateway` from a
  clone.

  `famp-gateway` talks to this local broker over a UDS socket
  (`--socket <path>`, defaulting to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock`)
  to back the principal(s) it relays.
- A TLS certificate + key for the host's `--listen` address. **"Self-signed
  is fine" is not enough** — Apple's TLS verifier (macOS clients) requires an
  `extendedKeyUsage = serverAuth` EKU on the leaf cert (a no-EKU cert fails
  with `EkuError`), while Linux's `webpki` verifier rejects a cert whose
  `basicConstraints` marks it `CA:TRUE` when used as a server end-entity
  (`CaUsedAsEndEntity`). The two verifiers are strict in **opposite**
  directions, so a naive `openssl req -x509` cert fails on **both**
  platforms. Use this recipe — it verifies on macOS Apple SecTrust **and**
  Linux webpki:
  ```bash
  openssl req -x509 -newkey rsa:2048 -nodes -days 800 \
    -keyout <host>.key.pem -out <host>.cert.pem -subj "/CN=<host>" \
    -addext "subjectAltName=IP:<tailnet-ip>,DNS:<hostname>" \
    -addext "basicConstraints=critical,CA:FALSE" \
    -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
    -addext "extendedKeyUsage=serverAuth"
  ```
  Run this on each host with that host's own `<tailnet-ip>`/`<hostname>` in
  the SAN — a corporate/internal CA also works as long as it issues leaf
  certs with the same `CA:FALSE` + `serverAuth` shape.
- **macOS only — pre-authorize inbound connections.** macOS's host firewall
  silently drops unsolicited inbound TCP to a newly-built binary until you
  approve it (or you'll only discover this because *outbound* egress from
  your laptop works fine while nothing ever reaches you). Before starting
  the gateway on a macOS host, pre-authorize it:
  ```bash
  sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add "$(which famp-gateway)"
  sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp "$(which famp-gateway)"
  ```
  (Or click "Allow" on the inbound-connection prompt the first time a peer
  connects — but doing it ahead of time avoids losing the first message
  while you're not watching for the prompt.)

  **The allow-rule is bound to the binary's PATH, not to the program.** An
  approval granted to `target/release/famp-gateway` during development does
  **not** cover `~/.cargo/bin/famp-gateway` after `just install-all` — they are
  different paths, so the deployed binary is silently blocked again even though
  `socketfilterfw --listapps` shows a `famp-gateway` entry. Check that the
  listed path is the one you are actually running (`which famp-gateway`), and
  re-add it after switching. Observed during the UAT-01 dogfood, where the
  pre-existing rule covered only the `target/release` path.
- The gateway's on-disk identity lives under `$FAMP_HOME` (or `$HOME/.famp`
  if unset):
  - `~/.famp/gateway/identity.ed25519` — this gateway's own signing key.
    **Never copy this file anywhere.** It is generated automatically on
    first use.
  - `~/.famp/gateway/peers.keyring` — the pinned public keys of peer
    gateways. `famp peer import` is the only writer; the ingress
    verification path is the only reader.

## 2. Gateway identity

You don't need to run a separate "create identity" command — the signing
key at `~/.famp/gateway/identity.ed25519` is generated the first time
anything reads it (`famp peer export`, or `famp-gateway` itself). It's
idempotent: once generated, the same path always returns the same key, so
you can safely re-run `peer export` later without regenerating (and
invalidating) trust your peer already pinned.

## 3. Out-of-band key exchange

This is a **manual, out-of-band** step — no key material ever crosses FAMP
itself. You move a single public line between machines over your own
clipboard/Signal/whatever channel you already trust.

**Pin under the sender AGENT principal, never a `gateway`-suffixed label.**
Ingress verification looks up the pinned key by the envelope's `from` field
(`verify.rs` -> `peek_sender` -> `keyring.get(from)`), and `from` is always
an agent principal (`agent:<domain>/<name>`) whose `<name>` is a real sender
identity like `alice` or `bob` — never a synthetic segment naming the
gateway role itself. The domain in that principal is the same value this
host's own-domain is configured to (see §5) — export under that exact
authority.

**On A**, export A's public key under the principal `bob` on B will use to
address A's traffic — that is, under A's own sender identity `alice`,
domain-qualified with A's own-domain:

```bash
famp peer export --as agent:hostA.example/alice
```

This prints exactly one line:

```
agent:hostA.example/alice <pubkey-b64url> <key_id>
```

Copy that whole line (principal, public key, fingerprint) — **not** the
`identity.ed25519` file — to B.

**On B**, import it:

```bash
famp peer import
# paste the line, then Ctrl-D (EOF)
```

or, if you saved it to a file:

```bash
famp peer import /path/to/hostA-export.txt
```

Before trusting it, eyeball-check the `key_id` fingerprint in the pasted
line against what A actually printed — this is your defense against a
corrupted paste or a swapped line. `famp peer import` re-derives the
fingerprint from the pubkey and prints a `warning:` to stderr if it doesn't
match what was pasted (it still imports — the fingerprint is advisory, not
a hard gate — so don't skip the eyeball check). Importing a **different**
key for a principal that's already pinned is rejected outright
(fails closed, no silent overwrite).

**Now repeat in the other direction:** on B, `famp peer export --as
agent:hostB.example/bob`, copy the line to A, and `famp peer import` on A.

After this step, both `~/.famp/gateway/peers.keyring` files have one pinned
entry: A trusts B's key (under `agent:hostB.example/bob`), B trusts A's key
(under `agent:hostA.example/alice`).

**Pin before you launch — the keyring loads once, at startup, with no
hot-reload.** `famp-gateway` reads `peers.keyring` a single time when it
starts; it never re-reads the file while running. Complete this whole
section (both directions) *before* starting either gateway in §4. If a
gateway is already running when you fix a pin, you must restart it — editing
the file underneath a live process has no effect.

**Watch out for the duplicate-pubkey brick.** If you re-export under a
corrected label (e.g. you first exported the wrong `/gateway`-suffixed label
and are now fixing it), `famp peer import`-ing the corrected line **without
first removing the stale line** leaves two entries in `peers.keyring` that
share one pubkey. `Keyring::load_from_file` fails closed on that shape
(`duplicate pubkey`) and the gateway refuses to start at all. Before
re-pinning a corrected label, strip the stale line from the peer's
`peers.keyring` (or delete the file and re-import cleanly) rather than
appending a second entry.

## 4. Start each gateway

`famp-gateway` has no `--help` (it's a hand-rolled parser, not clap) — the
full flag surface is:

```
famp-gateway [--socket <path>] --listen <addr> --tls-cert <path> \
             --tls-key <path> [--peer <domain>=<url>]... [--trust-cert <path>] \
             <principal-name>...
```

| Flag | Required | Notes |
|------|----------|-------|
| `--socket <path>` | No | Local broker UDS path; defaults to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock` |
| `--listen <addr>` | **Yes** | `SocketAddr` this host's gateway binds for inbound deliveries, e.g. `0.0.0.0:8443` |
| `--tls-cert <path>` | **Yes** | TLS certificate served on `--listen` |
| `--tls-key <path>` | **Yes** | TLS private key for that certificate |
| `--peer <domain>=<url>` | No, repeatable | Maps a remote federation domain to that peer's gateway base URL, e.g. `hostb.example=https://hostb.example:8443` |
| `--trust-cert <path>` | No | Extra CA cert to trust when calling out to peers (omit to use the system trust store) |
| `<principal-name>...` | **Yes, ≥1** | Bare local principal name(s) this gateway backs, e.g. `bob` — at least one is required or the binary refuses to start |

**The `<principal-name>` a gateway backs is the REMOTE principal it proxies
for, not a local one.** A gateway running on A relays traffic on behalf of
a principal that lives on the *other* side of the wire from A's own
identity — it fronts the far end of the connection A is dialing into, not
the local identity `alice` already talks to its own broker directly.
Concretely: on A, the gateway backs the remote principal `bob`, not the
local principal `alice`. On B, the gateway backs the remote principal
`alice`, not the local principal `bob`.

**On A:**

```bash
famp-gateway --listen 0.0.0.0:8443 \
             --tls-cert /path/to/hostA.cert.pem --tls-key /path/to/hostA.key.pem \
             --peer hostB.example=https://hostB.example:8443 \
             bob
```

**On B:**

```bash
famp-gateway --listen 0.0.0.0:8443 \
             --tls-cert /path/to/hostB.cert.pem --tls-key /path/to/hostB.key.pem \
             --peer hostA.example=https://hostA.example:8443 \
             alice
```

Each gateway loads its keyring **first**, then connects to its local
broker and picks up its principals, and only after the keyring has loaded
prints `famp-gateway: ready, backing N principal(s): ...`. If you see
`ready` before you've confirmed the keyring loaded (or the process exits
right after with a `duplicate pubkey` error), the ready line does not mean
what it says — treat it as a live signal only once §3's pinning is known
correct and the process is still running a few seconds later.

## 5. Configure your own-domain

Before you can send to a remote peer, this host needs exactly one
**own-domain** value — the federation authority this host stamps into the
`from` of every remote-addressed envelope, and the same authority the peer
pinned your key under in §3. `famp send`, `famp peer export`, and
`famp-gateway` must all agree on this single value, or the peer's ingress
will reject your envelopes as an unpinned key.

Resolution precedence (highest wins):

1. `--domain <value>` — a per-invocation CLI flag (e.g. `famp send --to
   agent:hostB.example/bob --domain hostA.example ...`).
2. `FAMP_OWN_DOMAIN` — an environment variable, set once for the shell/session.
3. `$FAMP_HOME/own-domain` — a file containing a single line, the persistent
   host-level default (`$FAMP_HOME` defaults to `~/.famp`).

If none of the three is set, `famp send`'s remote branch fails with a typed
error naming all three sources — it never silently falls back to a local
send. To configure A once and forget it:

```bash
echo "hostA.example" > ~/.famp/own-domain
```

## 6. Connect / verify

From A, address B's principal directly with the domain-qualified `--to`,
using the real shipping `famp send` client (own-domain resolved per §5):

```bash
famp send --to agent:hostB.example/bob --new-task --body "hello from A"
```

**What a successful `famp send` confirms — and what it does not.** A zero
exit code confirms only that the local broker accepted the envelope into
the gateway-backed outbound mailbox on this host. It does not confirm that
the gateway has drained, signed, and relayed the envelope, that the remote
gateway verified it, that it reached the remote mailbox, or that the task
FSM advanced on the far side — egress is a decoupled background drain loop
(polling roughly once a second) that the CLI process never waits on. That
is the fire-and-forget boundary. The `famp inspect tasks` check below is
what actually confirms end-to-end delivery.

Then confirm the task's FSM reached a terminal state on **both** sides:

```bash
famp inspect tasks --id <task_id> --json
```

Look for the task to advance past `REQUESTED` to a terminal state
(`COMPLETED`, `FAILED`, or `CANCELLED`) on both A and B — this proves the
signed envelope was delivered, verified, and processed end-to-end across
the two hosts. Repeat the send in the other direction (B → A,
`famp send --to agent:hostA.example/alice --new-task ...`) to confirm
bidirectional delivery.

**Before you send, probe the live peer first.** Once both gateways are up,
send one throwaway message and check for an HTTP `400 invalid_signature`
(or a real delivery) rather than assuming silence means failure — that
single response tells you TLS validated, ingress was reached, and the
security gate is working, all in one shot. A generic transport error
("reqwest failure" or similar) with no further detail usually means TLS or
firewall, not signing — re-check §1's cert recipe and firewall step before
re-checking application-layer wiring.

If delivery doesn't happen: re-check that each `--peer` domain matches what
you used in the other host's `--listen`/TLS setup, that both processes are
still running, and that the TOFU-pinned key in each `peers.keyring` matches
what the other side actually exported (§3).

**Known limitation (leaf-name ambiguity, not yet resolved).** A remote send
like `--to agent:hostB.example/bob` routes the *local* bus frame on A using
only the bare leaf name `bob` — the domain qualifies the envelope's `to`/
`from`, not the bus routing target. If a host also has a **local** holder
registered under that same bare name (e.g. A also has a locally-registered
`bob`), an inbound frame could be mis-routed to the wrong local holder.
Until fully-qualified local routing lands, keep leaf names unambiguous per
host — don't reuse a bare name both as a local holder and as the backed
principal of a gateway on the same machine.

---

For historical v0.8-era federation CLI (`famp init / setup / listen / peer
add / peer import`, removed in v0.9), see
[`## Advanced: v0.8 federation CLI`](../README.md#advanced-v08-federation-cli)
in the README — it is unrelated to the gateway described here.
