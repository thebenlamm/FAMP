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

We'll call the two machines **A** and **B** throughout.

## 1. Prerequisites

On **each** host:

- `famp` installed (`cargo install famp`) and the persistent broker running:
  ```bash
  famp daemon install
  ```
  `famp-gateway` talks to this local broker over a UDS socket
  (`--socket <path>`, defaulting to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock`)
  to back the principal(s) it relays.
- A TLS certificate + key for the host's `--listen` address (self-signed is
  fine for two machines you control — a corporate/internal CA also works).
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

**On A**, export A's public key under the principal name B will use to
address A:

```bash
famp peer export --as agent:hostA.example/gateway
```

This prints exactly one line:

```
agent:hostA.example/gateway <pubkey-b64url> <key_id>
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
agent:hostB.example/gateway`, copy the line to A, and `famp peer import` on
A.

After this step, both `~/.famp/gateway/peers.keyring` files have one pinned
entry: A trusts B's key, B trusts A's key.

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
| `<principal-name>...` | **Yes, ≥1** | Bare local principal name(s) this gateway backs, e.g. `alice` — at least one is required or the binary refuses to start |

**On A:**

```bash
famp-gateway --listen 0.0.0.0:8443 \
             --tls-cert /path/to/hostA.cert.pem --tls-key /path/to/hostA.key.pem \
             --peer hostB.example=https://hostB.example:8443 \
             alice
```

**On B:**

```bash
famp-gateway --listen 0.0.0.0:8443 \
             --tls-cert /path/to/hostB.cert.pem --tls-key /path/to/hostB.key.pem \
             --peer hostA.example=https://hostA.example:8443 \
             bob
```

Each gateway prints `famp-gateway: ready, backing N principal(s): ...` once
it has connected to its local broker and picked up its principals.

## 5. Connect / verify

From A, address B's principal directly (federation-qualified name:
`agent:hostB.example/bob`), and send a message via whatever client you're
using on A (e.g. `/famp-send` in Claude Code, or the FAMP MCP tools). Then
confirm the task's FSM reached a terminal state on **both** sides:

```bash
famp inspect tasks --id <task_id> --json
```

Look for the task to advance past `REQUESTED` to a terminal state
(`COMPLETED`, `FAILED`, or `CANCELLED`) on both A and B — this proves the
signed envelope was delivered, verified, and processed end-to-end across
the two hosts. Repeat the send in the other direction (B → A) to confirm
bidirectional delivery.

If delivery doesn't happen: re-check that each `--peer` domain matches what
you used in the other host's `--listen`/TLS setup, that both processes are
still running, and that the TOFU-pinned key in each `peers.keyring` matches
what the other side actually exported (Section 3).

---

For historical v0.8-era federation CLI (`famp init / setup / listen / peer
add / peer import`, removed in v0.9), see
[`## Advanced: v0.8 federation CLI`](../README.md#advanced-v08-federation-cli)
in the README — it is unrelated to the gateway described here.
