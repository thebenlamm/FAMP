# Personal network: what it does, what it does not, and what changed

Applies to the `famp network setup` / `famp connect` / `famp doctor` / `famp network` workflow.
Everything below was checked on macOS unless it says it was observed on Linux; other Linux
behavior is marked UNVERIFIED where it is a claim about systemd, ufw or pf that nobody has run.

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
6. **Some responses reveal state.** `sender_not_backed` (before verification) reveals which
   sender names this gateway serves. An unauthenticated caller can also tell whether a
   candidate principal has an ACTIVE pin (`invalid_signature`) or not (`unpinned_key`); that
   split is a documented Phase 9 decision. An authenticated peer can tell which recipient
   names are on the allowlist (403 versus 202).
7. **systemd user units do not enforce the unit's IP or filesystem directives.** Observed on two
   Linux boxes (devhome, devgpu) running the systemd `--user` manager: a test unit with
   `IPAddressDeny=any` still reached 1.1.1.1, and the manager logs that it is not running as
   root. `IPAddressAllow/Deny`, `PrivateTmp` and `ProtectSystem` in the generated unit are
   therefore **not a control** there; only `NoNewPrivileges` and `MemoryMax` were seen to take
   effect. The controls are the host firewall (`famp network firewall`, ufw), the Tailscale ACL,
   the pinned peers and the recipient allowlist. Check `systemctl --user status famp-gateway`
   and `systemd-analyze --user verify` after staging.
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
13. **The gateway's identity key stays under the user account in release 1.**
    `~/.famp/gateway/identity.ed25519` (mode 0600, in a 0700 directory) is readable by every
    process that runs as that user, including agent sessions. A different account for the
    gateway, or for woken sessions, is a release-2 design question.
14. **Card fields are self-asserted.** A peer's name, domain and address come from its own
    card. They are checked (exact SAN, one endpoint per computer, no changed card for a pinned
    peer, `--prune` never replaces a pin) and `connect` reaches each host over ssh with
    `StrictHostKeyChecking=yes`, but the key itself is only as trustworthy as that channel:
    compare each `key_id` out of band or pass `--expect`. Revocation is per principal, so a
    revoked key presented under a NEW principal is not caught.
15. **`network serve` does not re-check its bind address.** `famp network setup` refuses an address
    other than this computer's Tailscale IP, but the serve path trusts `network.json`. With the
    firewall default-deny, check the listener after `network enable` (`ss -ltn` on Linux,
    `lsof -iTCP -sTCP:LISTEN` on macOS).
16. **`import` is not transactional.** It writes the keyring before `network.json`; a crash
    between them leaves the keyring ahead of the config. Rerunning the same command converges.
17. **Allowlist file checks are not complete.** The gateway refuses a symlink, a non-regular
    file, an oversize or malformed file and one that others can write, and reads it from one
    open handle, but it does not check who owns the file or its parent directories, and
    `allow`/`deny` are read-modify-write without a lock (two at once can lose one edit).
18. **Resource limits are per source and not exhaustive.** The pre-verification request budget
    is charged after the request body has been read (bounded by 16 connections per source and
    a 15 s body deadline, in the gateway router only); a declared-oversize and a streamed-
    oversize body get differently shaped 413 responses.
19. **A CLI panic or crash is a local denial of service only.** The known one (a multibyte
    character at the cut of a remote error message) is fixed and tested; remote output is
    capped at 1 MiB.

20. **A delivery that times out is not cancelled and is not bounded.** The request answers
    `504` after 15 s, but the spawned delivery task keeps waiting for the sender registry and then
    for the local bus, with no timeout and no cap on how many such tasks exist (no semaphore).
    While the local bus is stalled, tasks pile up behind the registry mutex, each holding its
    request body (up to 1 MiB); the only bound is the per-sender rate budget (120 per minute) and
    the number of pinned peers. Three consequences:
    (a) the allowlist is checked BEFORE the task is spawned, not inside it, so a message that is
    already queued still lands after `famp network deny`; (b) a sender that re-sends after a 504
    under a NEW nonce can be delivered twice, because the first still lands; (c) memory grows
    with the backlog until the bus answers.
21. **A nonce is recorded before delivery, so delivery is at-most-once.** The replay record is
    written after the signature verifies and before the allowlist check and the delivery. If the
    allowlist refuses or the delivery fails or times out, a retry of the SAME signed envelope is
    refused as a replay; the sender must sign a new one (see 20b for the duplicate risk).
22. **The exact-SAN check binds a certificate to an IP address, not to a peer.** A certificate is
    accepted for the one address it lists and no other, but which computer owns that address is
    still the self-asserted card (limit 14); the firewall and Tailscale ACLs decide who can
    actually use it.
23. **Startup checks only that `--recipient-allowlist` was given, not the file.** A missing,
    malformed or unsafe file is not an error at start; it makes every delivery fail with `403`.
    `famp doctor` reports the recipients file, but only the CLI's own checks.
24. **`doctor` does not validate the keyring the way the gateway loads it.** It reads principal
    names from the file text and treats an unreadable file as "no pins" (`unwrap_or_default`), so
    a corrupt but readable keyring can pass `doctor` and still stop the gateway at its next start.
25. **`connect` has no overall timeout and can deadlock on a hostile remote.** It writes the whole
    bundle to the remote's stdin before it reads the remote's stdout; a remote that prints more
    than the pipe buffer before it reads stdin stalls both ends. Output is capped at 1 MiB and
    nothing is written anywhere until every check has passed, so interrupting it (Ctrl-C) is
    safe; rerun the same command.
26. **The pairing route is always served, without authentication.** `famp-gateway` mounts the
    Phase 18 `POST /famp/v1/pair/redeem` route on every start, including under
    `famp network serve`, which reads the default store `~/.famp/gateway/pairing.json`. With no
    store, or only revoked invites, it answers 404 and writes nothing. With any other invite in
    the store, pending or expired, anyone who can reach the port can try codes: a wrong code is
    recorded against the invite's attempt limit (a write to `pairing.json`), and the right code
    for a pending invite completes the redemption. The route never touches the peer keyring;
    pinning the redeemer is a separate step on this computer. **Before `famp network enable`,**
    look at `~/.famp/gateway/pairing.json`; if no pairing is in progress, move it out of
    `~/.famp/gateway/`.

## Behavior changes in this release
- **Restart the broker after installing these binaries.** A new `famp` talking to a broker that
  was started from an older binary gets a Hello error (`possible mixed FAMP binary/protocol`).
  After installing `famp` and `famp-gateway`, run `famp daemon restart`; open agent windows then
  re-register, and mailboxes are kept per name.
- `famp network serve` starts the gateway with `--pinned-only`, `--recipient-allowlist` and a
  hardened client. A recipient not on `~/.famp/recipients.json` is dropped with one fixed 403,
  and a missing, unreadable, malformed or group/world-writable file denies everything.
  `famp network setup` seeds an absent file with the computer's own name; nothing else creates it
  (`serve`, `stage` and `enable` never do), so deleting the file keeps everything denied until
  `famp network setup` or `famp network allow` recreates it. To deny everything on purpose, leave the
  list empty with `famp network deny`. **Default is deny.**
  `famp-gateway` itself refuses to start without `--recipient-allowlist`, and a router built
  without one denies every recipient: there is no flag that turns the check off. A gateway
  that used to run without the flag (a v1.0 deployment) must now be given one.
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
- Error responses carry fixed text (no principal, key window or server clock). Unpinned,
  expired and revoked senders are indistinguishable from each other (all `unpinned_key`); an
  active pin with a bad signature is still `invalid_signature` (limit 6).
- A verified delivery runs in its own task and the request waits at most 15 s for it (`504
  delivery_timeout`). **A 504 does not mean the message failed:** it can still land, and a
  client that disconnects mid-delivery does not cancel it. Waiting for the sender registry
  is bounded the same way, so one stuck local bus cannot park every request.
- Replay and per-sender rate are keyed on the full principal (`agent:<domain>/<name>`), a
  replay is refused before it is charged, and the signed recipient is checked before any
  nonce is recorded.
- `--prune` removes pins that are not in this run's set and keeps old revocation records, but it
  never replaces a peer that is already pinned: a listed host presenting a card with a pinned
  peer's name and a new key is refused, with or without `--prune`. To rotate a peer, prune it
  out in one run and add it back in the next.
- `--pinned-only` (opt-in, needs `--trust-cert`, refused with `--relay-fetch`): outbound TLS
  trusts only the pinned certificates, never the OS root store. The presented certificate must
  be byte-identical to a pinned one, list exactly the IP being dialed (and nothing else), and
  pass webpki name and validity checks; peers must be addressed by IP, and `http://` peers are
  refused. Envelopes are additionally signed end to end.
- A peer card's certificate must list exactly the card's address as its only subjectAltName
  and be an end-entity certificate, must be exactly one clean PEM block that the gateway's own
  loader accepts as a pinned root (so one bad card cannot stop the gateway for everyone), and no
  two computers may claim the same address and port. This is read from the certificate's DER, not from
  `openssl -text` output, which a hostile certificate can make look like anything.
- `famp network setup` refuses to bind anything but this computer's own Tailscale IP unless
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

## Release 2 hardening list (not built)
In priority order, from the reviews of the release-1 candidate:
1. Bound the detached delivery: a semaphore that answers `503` when full, a timeout INSIDE the
   task, and an allowlist recheck inside the task (limits 20, 21).
2. Re-check the bind address at `network serve` start (limit 15).
3. Key all post-verify state (rate, replay) on the VERIFIED envelope sender
   (`envelope_sender(&envelope)`) instead of the pre-verification peek, so it cannot disagree
   with what the signature proved.
4. Validate the keyring in `doctor` the way the gateway loads it, and give `connect` an overall
   timeout with the stdin write and stdout read decoupled (limits 24, 25).
5. mTLS, a certificate renewal command, transactional `import`, owner checks on the allowlist,
   and wake (below).
6. Do not mount the pairing route under `famp network serve` unless a pairing is in progress,
   and have `doctor` report a non-revoked invite in `gateway/pairing.json` (limit 26).

## Tracked follow-ups
mTLS or client-certificate pinning at the TLS layer; a certificate renewal command;
transactional `import`; a serve-time bind recheck (plus `doctor`); charging the per-source
budget before the body is read; owner checks on the allowlist; dynamic or domain-level peer
names; wake (above).
