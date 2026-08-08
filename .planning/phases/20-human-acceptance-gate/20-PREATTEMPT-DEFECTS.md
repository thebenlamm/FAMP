# Phase 20 — Pre-Attempt Defect Log

> **RESOLUTION STATUS — updated 2026-08-08, after quick task `260808-ix4`.**
> Each row below was re-checked against its destination file, not against this
> document's own earlier text.
>
> | Defect | State | Evidence |
> |---|---|---|
> | D1 `famp-gateway --help` exits 1 | **FIXED** | `de5aa1f` — §1 now uses `command -v famp-gateway` |
> | D2 §2 points at the v1.0 topology doc | **FIXED** | `de5aa1f` + `624ef80` — §2 states the inbound requirement directly; `GATEWAY-SETUP.md`'s no-relay claim and flag surface corrected; `README.md` paraphrase fixed |
> | D3 accuracy gate never executed commands | **FIXED** | `185dd9a` — `crates/famp-gateway/tests/follower_setup_gateway_commands.rs`, 3 tests incl. `red_path_trips_on_prerepair_invocation`; wired to CI via `cargo nextest run --workspace` (`ci.yml:119`) |
> | D4 inviter needs inbound reachability | **OPEN** | Product property, not a doc bug. Blocked on an inbound-reachable inviter endpoint; see the host table below. |
> | D5 relay host is bare | **OPEN** | Re-confirmed exhaustively 2026-08-08 (full-filesystem `find`, all regions, no container services) |
>
> **The freeze recorded below is now STALE.** `docs/FOLLOWER-SETUP.md` changed in
> `de5aa1f`, so both the commit and the SHA-256 in this file and in
> `20-REHEARSAL.md`'s candidate block describe a guide that no longer exists.
> Re-freezing is deliberately NOT done here: per 20-02-PLAN's D-01/D-02/D-04 the
> guide is frozen as a rehearsal candidate at Task 1 and only proven by a
> successful run. Refresh the candidate block immediately before the attempt.
>
> A new doc landed alongside these repairs: `docs/RELAY-SETUP.md`, the
> store-and-forward relay procedure that did not previously exist anywhere
> (closing D3's documentation half). It was drafted by a Codex agent and its
> flag surface, four exact missing-flag error strings, readiness-line format,
> and four-key-per-domain limit were each verified against the crate sources
> before it was committed.

**Status: no rehearsal attempt has started.** Nothing below was observed during a
DOC-07 run. Every item was found by reading shipped code/docs and by smoke-testing
a *throwaway* Linux host that is explicitly not the clean-host candidate.

That distinction is load-bearing. Per 20-02-PLAN's D-06/D-10, an attempt begins at
the preflight on the untouched host, and the guide may not be edited mid-attempt.
Because no attempt has begun, these defects may be repaired now — repair, re-run
Plan 20-01's suite, re-freeze the guide commit/digest, and only then start the run.

Frozen candidate at time of writing (superseded once repairs land):

```text
guide_commit=f848c9e747ad769a162408249a8dd084f34e2350
guide_digest=43f793114a9e51cf2a94c86dea47077cc1b800c2b344d81fa0bcc04eb6e1a01c
```

---

## D1 — `famp-gateway --help` is instructed by the guide and exits 1

`docs/FOLLOWER-SETUP.md` §1 tells the follower to run `famp-gateway --help` as one
of two post-install verification commands. On a fresh host running the published
`v1.1.0-rc.1` Linux binary:

```text
$ famp-gateway --help
famp-gateway: famp-gateway: unrecognized argument '--help' — no such flag
usage: famp-gateway [--socket <path>] --listen <addr> --tls-cert <path> ...
exit=1
```

This is not drift — `docs/GATEWAY-SETUP.md` §4 already states plainly that
"`famp-gateway` has no `--help` (it's a hand-rolled parser, not clap)". Two shipped
docs disagree, and the follower-facing one is the wrong half. A follower's second
command failing with `unrecognized argument` reads as a broken install.

Note the accuracy gate did not catch this: `follower_setup_doc_accuracy` checks flag
*spellings* against the binary, not whether a documented command *succeeds*. A
command that is spelled correctly and exits non-zero passes it.

**Repair options:** drop the `--help` line, or replace it with an invocation that
exits 0. Whichever is chosen, the accuracy test should gain a case that executes the
guide's verification commands rather than only parsing them.

## D2 — §2 delegates reachability to a v1.0 doc that mandates the forbidden topology

`FOLLOWER-SETUP.md` §2 sends both owners to `docs/GATEWAY-SETUP.md` for "the
production procedure". That document is titled *Two-Machine Gateway Setup (v1.0
Federation)* and, in its opening paragraph, states:

> **There is no public relay.** Both hosts must be directly reachable from each
> other (or reachable over your own VPN) …

It then documents, as the only path: a shared/own VPN, manual `famp peer export` /
`famp peer import` of public-key lines pasted between machines, and a positional
`<principal-name>` argument form. `FOLLOWER-SETUP.md`'s own preamble forbids the
first two outright ("Do not use a shared VPN, copy private keys, paste peer key
blobs"), and Phase 18 replaced the third path with `famp pair`.

So §2 — the step that establishes reachability, i.e. the precondition for everything
after it — has no followable procedure for the topology this milestone shipped.

## D3 — no document covers relay deployment

`famp-relay` ships as a binary in `v1.1.0-rc.1` (`famp-relay-installer.sh` plus
per-target archives) and `crates/famp-gateway/tests/e2e_relay_bidirectional.rs`
documents the working topology in its module comment: each gateway points both
`--peer <domain>=<url>` and `--relay-fetch <url>` at the *relay*, never at the other
gateway's `--listen`; the relay itself takes
`--listen / --tls-cert / --tls-key / --public-url / --domain <domain>=<pubkey>`,
with operator-configured domain keys rather than TOFU.

None of that appears in any file under `docs/`. The only relay mentions are
`DISTRIBUTION.md` (build targets) and `GATEWAY-SETUP.md` (asserting relays do not
exist).

## D4 — the inviter needs inbound reachability, and the relay cannot supply it

`crates/famp/src/cli/pair/redeem.rs` builds a plain `reqwest` client and dials the
inviter's gateway URL directly; there is no relay branch on the pairing path. 20-CONTEXT
D-07 states the same requirement from the other side: "The inviter URL must be
publicly reachable by the follower before the invite's 24-hour clock begins."

Consequence: deploying the relay does **not** remove the inviter's need for an
inbound-reachable HTTPS endpoint. Relay-fetch covers message transport; pairing is
a direct dial.

Measured against the available hosts (2026-08-08):

| Host | Inbound status | Evidence |
|---|---|---|
| Ben's MacBook Air | blocked | cone NAT, inbound timed out — 13-REACH-02-RESULTS.md |
| `home-devbox` | blocked, likely openable | public IPv6 `2600:4040:a337:ce00:…`; listener confirmed bound on `*:8443` via `ss`, yet TCP from two independent external hosts failed while a same-command control to `2606:4700:4700::1111:80` succeeded from both. Router/ISP inbound-v6 firewall. |
| `air-server` | no direct endpoint | Tailscale reports no direct endpoint (relayed) |
| Lightsail `famp-relay` | reachable, but bare | see D5 |

## D5 — the Lightsail relay host has no relay software on it

`famp-relay` (Lightsail, account `559846026666`, static IP `44.219.73.36`) is
running and SSH-reachable, but carries nothing:

```text
sudo find / -xdev -iname '*famp*'   -> (no output)
systemctl / systemctl --user         -> no famp unit
sudo ss -tlnp                        -> sshd :22, systemd-resolved :53 only
```

Port 443 is open at the Lightsail firewall and refuses instantly at the socket. The
home directory contains only stock dotfiles dated 2026-08-07, and the static IP was
allocated the same day — consistent with the instance having been reprovisioned
after any earlier deployment.

---

## Smoke-test results (throwaway host `i-08f6373fbf656f05e`, terminated after use)

Stock Ubuntu 24.04 x86_64, `t3.small`, dual-stack. **Not** the clean-host candidate.

| Check | Result |
|---|---|
| `phase20-clean-box-preflight.sh` on a stock AMI | `CLEAN HOST: PASS`, exit 0 |
| `famp-installer.sh` | ok — `famp 1.1.0-rc.1 x86_64-unknown-linux-gnu` |
| `famp-gateway-installer.sh` | ok |
| `famp --version` in the same shell | exit 127 — expected; guide's "open a new shell" remedy is accurate |
| `famp --version` in a new login shell | `famp 1.1.0-rc.1`, exit 0 |
| `famp-gateway --help` | **exit 1** — see D1 |
| `famp daemon install` (headless systemd `--user`) | succeeds; broker `RUNNING` |
| linger | `Linger=no`; `famp daemon status` warns clearly and prints the exact `loginctl enable-linger` remedy without running it |
| IPv6 egress | control to `2606:4700:4700::1111:80` OK |

Two things worth carrying forward. First, EC2 is a valid DOC-07 host — the preflight
passes on a stock AMI, and a VM from a stock image is none of the prohibited
substitutions (container, source build, prior FAMP/Rust state, cleanup-after-preflight).
**x86_64 only**: the preflight's case arm accepts `Linux:x86_64` and no other Linux
arch, so Graviton instances are rejected by design.

Second, the headless-Linux daemon path is in better shape than the v0.11 support
boundary suggested — `famp daemon install` handles a no-linger host by warning rather
than failing. The guide never mentions linger, which is a gap but a mild one, since
`famp daemon status` surfaces it unprompted.

## Suggested order

1. Ben opens inbound TCP 8443 to `home-devbox`'s public IPv6 on his router; re-run the
   two-sided reachability test with its control before believing it.
2. Repair D1 and D2; decide whether D3's relay is needed at all — if `home-devbox`
   becomes inbound-reachable, both gateways can dial each other directly and the relay
   is not on the critical path for this run.
3. Re-run Plan 20-01's test suite, re-freeze `guide_commit` / `guide_digest`, and
   rewrite `20-REHEARSAL.md`'s candidate block against the new freeze.
4. Launch a fresh untouched host and start the attempt.
