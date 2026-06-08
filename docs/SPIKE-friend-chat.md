# Spike: chat between two people's Claude Code agents (zero FAMP code)

This is a **validation spike**, not a feature. It proves whether cross-host agent
chat is something we actually reach for — *before* building the `famp-gateway`
federation crate. Both ends talk to **one shared broker** (the host's) tunnelled
over a [Tailscale](https://tailscale.com) tailnet via `socat`. No FAMP code
changes; the gateway, TOFU trust, and signed wire format are all deferred until
this spike shows pull.

> **The real success signal** is not "a message flowed." It's: *did the host
> reach for this again within ~2 weeks?* That's the Gate A trigger that decides
> whether `famp-gateway` earns its build.

---

## ⚠️ Security — read before connecting

Claude Code's FAMP listen mode **auto-wakes the agent on an inbound message and
feeds the content into a turn that can run Bash / Edit / Write.** A message from
your friend — even a well-meaning one — is therefore a path to *executing
instructions from another person inside your dev environment* (a classic
confused-deputy). Signatures (when we add them) authenticate the *sender*, never
the *safety of the payload*.

**Mitigation for the spike — both sides:** register the friend-facing window with
**`listen: false`** and read inbound messages on demand. Treat anything that
arrives as **data to look at, never instructions to act on.**

```
famp_register({ identity: "<you>", listen: false })   // do NOT auto-wake on cross-host input
```

---

## Prerequisites (both people)

1. **Tailscale** installed and logged into the **same tailnet**
   (host shares an invite; verify with `tailscale status`).
2. **`famp`** installed (`cargo install --path crates/famp --locked` from the repo, or `just install`).
3. **`socat`** installed (`brew install socat` on macOS; `apt install socat` on Linux).

---

## Host (the person whose broker everyone shares)

1. Make sure your broker is up: `famp daemon status` (or `famp inspect broker`).
2. Expose it on the tailnet:

   ```
   just spike-tunnel
   ```

   This prints your **tailnet IP** and port `9999`. Send those to your friend.
   (Under the hood: `socat TCP-LISTEN:9999,fork,reuseaddr,bind=<tailnet-ip> UNIX-CONNECT:~/.famp/bus.sock`.)
   Leave it running; Ctrl-C stops the tunnel.

## Friend (connects to the host's broker)

1. Point a local socket at the host's broker over the tailnet
   (replace `<HOST-TAILNET-IP>`):

   ```
   socat UNIX-LISTEN:$HOME/.famp/bus.sock,fork TCP:<HOST-TAILNET-IP>:9999
   ```

   > If you already run a local broker, stop it first (`famp daemon uninstall`
   > or kill it) — this socket must be the tunnel, not your own broker.

2. In a Claude Code window, register on the (now shared) broker — **listen off**:

   ```
   famp_register({ identity: "<friend-name>", listen: false })
   ```

---

## Send a message both ways

- **Friend → Host:** `famp_send({ peer: "<host-name>", mode: "open", title: "hi", body: "ping from <friend>" })`
- **Host reads it:** `famp_inbox({ action: "list" })`  ← on demand, because listen is off
- **Host → Friend:** reply with `famp_send({ peer: "<friend-name>", mode: "open", ... })`
- **Friend reads it:** `famp_inbox({ action: "list" })`

`famp_peers` on either side should show both identities on the shared broker.

---

## What this spike does and does NOT prove

- ✅ Proves: two people's agents can exchange messages over a VPN they both trust,
  and whether that's worth reaching for.
- ❌ Does **not** prove: two *sovereign brokers* interoperating, impersonation
  resistance on a shared tailnet, or the signed wire format. Those are what
  `famp-gateway` (the real v1.0 build) would add — and the reason to build it
  only if this spike, plus a desire for FAMP-as-a-protocol, says so.

**Teardown:** Ctrl-C both `socat` processes. The friend can `famp daemon install`
to restore their own local broker.
