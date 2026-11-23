# Personal network: what it does, what it does not, and what changed

Applies to the `famp setup` / `famp connect` / `famp doctor` / `famp network` workflow.
Everything below was checked on macOS only; Linux behavior is marked UNVERIFIED where it
is a claim about systemd, ufw or pf that nobody has run.

## Scope of this release
- One identity (one Ed25519 key, one principal) per computer. Messages between computers
  are Ed25519-signed (INV-10) and carried by `famp-gateway`.
- Agents on the same computer talk through the local broker, unchanged.
- Remote messages are **delivered, never auto-woken**: they wait in the recipient's inbox
  until it reads it (`famp_inbox`). `crates/famp/tests/auto_wake_gate.rs` pins this.
  Treat every FAMP message as a request, not an order.

## Stated limits
1. **No TLS client authentication.** The gateway authenticates senders at the application
   layer only (signature against a pinned key). Any device that can reach the port can
   reach the pre-verification parser; apply the firewall rules (`famp network firewall`)
   before relying on the mesh. Neither ufw nor pf filters loopback, so other local users
   on the same computer are not stopped by them. mTLS is a tracked follow-up.
2. **Recipient binding is domain-level (D-22)** in the gateway itself. The recipient
   allowlist (below) closes it per computer when started by `famp network serve`.
3. **The replay cache is in memory.** After a gateway restart a captured envelope can be
   accepted again until its timestamp leaves the 300-second freshness window.
4. **Certificates last 365 days and there is no renewal command.** `famp doctor` warns
   within 30 days; a peer's lapsed certificate only breaks that peer, but renewal is manual.
5. **A peer's name is registered on the local broker.** If a local agent already uses a
   peer's name the gateway cannot start (the broker answers `NameTaken`).
6. **`sender_not_backed` (pre-verification) reveals which sender names this gateway serves.**
7. **systemd user units:** `IPAddressAllow/Deny`, `PrivateTmp` and `ProtectSystem` may be
   ignored or may stop the unit starting, depending on the distribution (UNVERIFIED). Check
   `systemctl --user status famp-gateway` and `systemd-analyze --user verify` after staging.
   `After=network-online.target` does not apply to user units; the unit waits for the broker
   and retries (`RestartSec=15`, up to 20 starts in 10 minutes).
8. **The pf anchor and ufw order are templates** to be applied and verified by hand; the pf
   syntax is UNVERIFIED on your macOS version. `ufw insert 1` can be refused on an empty
   ruleset; the template says how to append instead.
9. **"Staged, not enabled" holds back the service, not the trust files.** `network serve`
   rebuilds its arguments and trust bundle from `network.json` every time it starts. On a
   computer whose gateway is already running, trust written by `connect --stage` takes
   effect at the next crash, restart or reboot, not when you run `network enable`.
10. **Each computer needs a running local broker.** The gateway attaches to it and never
    starts one; `connect --stage` installs no services, so run `famp daemon install` yourself
    where `famp doctor` reports the broker missing. `network enable` refuses without one.
11. **Connection slots group IPv6 by /64**, so every host in one IPv6 /64 shares 16 slots
    (UNVERIFIED for Tailscale's addressing). The default setup binds Tailscale IPv4.
12. **Agents run with the permissions of the session that reads the inbox.** Nothing here
   limits what a session does with a message; that includes sessions started with bypass
   permissions.

## Behavior changes in this release
- `famp network serve` starts the gateway with `--pinned-only`, `--recipient-allowlist` and a
  hardened client. A recipient not on `~/.famp/recipients.json` is dropped with one fixed 403,
  and a missing, unreadable, malformed or group/world-writable file denies everything.
  `famp setup` seeds an absent file with the computer's own name; nothing else creates it
  (`serve`, `stage` and `enable` never do), so deleting the file keeps everything denied until
  `famp setup` or `famp network allow` recreates it. To deny everything on purpose, leave the
  list empty with `famp network deny`. **Default is deny.**
  A `famp-gateway` started without `--recipient-allowlist` behaves as before.
- Connection limits in `famp_transport_http::tls_server::serve_std_listener`: HTTP/1 only,
  one request per connection (no keep-alive, so a connection slot cannot be held by
  trickling requests), 10 s header and handshake timeouts, 128 connections and 16 per
  source (IPv6 grouped by /64). **`famp-relay` uses the same function, so a rebuilt relay
  inherits them; redeploying it is a separate decision.** The 15 s overall request-body
  bound is in the gateway's router only, not in `serve_std_listener`.
- Per-sender rate and replay state are charged only after the signature verifies. The
  pre-verification budget is per source address (300 per minute, bounded table); the
  relay-fetch path is not charged a shared bucket (it is bounded by its own poll loop, and
  every envelope still pays the per-verified-sender budget). This
  **supersedes D-10/INGR-06** ("an IP is the wrong key") for a firewalled personal network:
  source addresses are stable and not rotatable there. See `RateLimiter`'s doc comment.
- Error responses carry fixed text (no principal, key window or server clock), and an
  unpinned, expired or revoked sender all return the same `unpinned_key`.
- `--pinned-only` (opt-in, needs `--trust-cert`, refused with `--relay-fetch`): outbound TLS
  trusts only the pinned certificates, byte-for-byte, never the OS root store.
- A peer card's certificate must list exactly the card's address as its only subjectAltName
  and be an end-entity certificate, must be exactly one clean PEM block that the gateway's own
  loader accepts as a pinned root (so one bad card cannot stop the gateway for everyone), and no
  two computers may claim the same address and port. This is read from the certificate's DER, not from
  `openssl -text` output, which a hostile certificate can make look like anything.
- `famp setup` refuses to bind anything but this computer's own Tailscale IP unless
  `--allow-non-tailnet` is given.

## Release 2 (not built): waking agents
Design notes from review, so they are not rediscovered:
- Wake means notify or resume an existing session in its own permission mode. It must never
  spawn a process. A woken session reads the message itself; the notice carries no body.
- The bus origin a client declares is self-asserted (`famp-bus/src/broker/handle.rs`), so a
  wake permission cannot rest on an origin value. The safe carrier is a per-check read of an
  owner-only policy file keyed on the gateway-verified sender, read on every check so a kill
  switch works without a restart.
- A hop count cannot bound a loop (a woken agent's reply is a fresh send); a hard per-peer
  and per-target wake rate cap is what bounds it.
- Sessions enroll explicitly; the default deny-list includes OpenClaw/Jeeves, jeeves/sofer/
  offload, and `guardian-*`. Running wakeable sessions as a different account from the
  gateway conflicts with `service.rs`'s one-user model and must be settled first.

## Tracked follow-ups
mTLS or client-certificate pinning at the TLS layer; certificate renewal; dynamic or
domain-level peer names; wake (above).
