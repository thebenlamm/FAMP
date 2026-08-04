# Cross-Person Pairing (`famp pair`)

This is a MECHANISM REFERENCE for `famp pair invite|redeem|status|revoke`: what each
command does, what it enforces, and its operational caveats. It is explicitly **NOT**
the follower-facing setup walkthrough — that guide is Phase 20's DOC-06/DOC-07 and does
not exist yet. If you are the person being invited to pair and have never used FAMP
before, this document assumes background you may not have; wait for DOC-06.

Genuinely-different-networks reachability (REACH-04) is proven on loopback only as of
Phase 17. A NATed inviter — behind home/office NAT with no port forwarding — is **not**
a supported configuration today; `famp pair invite`'s `--url` must point at an address
the redeemer's network can actually reach.

## The three-step flow

1. **Inviter** confirms the follower's `famp` install already works (`famp --version`
   succeeds on their machine), then runs `famp pair invite --as <principal> --url <url>
   --confirm-installed` on their own gateway. This prints ONE artifact — install
   instructions, the consent warning, the redeem command, and the five-word code, in
   that order — and persists a `Pending` invite record locally.
2. **Inviter** sends that whole artifact to the follower over any channel they already
   use (text message, chat, email). The code is the LAST line so it survives even if
   the follower's install takes a while and they only read the bottom of a long message
   after the rest has scrolled off.
3. **Redeemer** (the follower) runs `famp pair redeem --from <url>` on their own
   machine, is prompted for the code, and types it. On success, the redeemer's side
   pins the inviter's key immediately. The **inviter's** side does not pin yet —
   pairing completes asymmetrically. The inviter runs `famp pair status` afterward,
   which prints who redeemed (principal + key_id) BEFORE writing anything, then pins.

## Why the code is typed at a prompt, never a command-line argument

`famp pair redeem` takes no positional argument and no code-bearing flag at all — the
CLI struct simply has no field that could carry it. A code-shaped invocation like
`famp pair redeem abc def ghi jkl mno` is rejected by clap before any code is ever
read. This is structural, not a runtime check: a command-line argument lands in
process argv (visible to anyone else on the machine via `ps`) and in shell history —
both durable, both readable long after the pairing window has closed. Reading the code
from stdin at an explicit prompt avoids both.

## The 24-hour window and the five-attempt budget

An invite is valid for 24 hours from the moment it is created and allows 5 wrong
guesses before it locks permanently. Both limits are enforced by the **inviter's
gateway** — the endpoint that receives the redemption POST — never by the client. A
redeemer's own machine has no way to bypass either limit; the classification
(`InviteStore::decide`) runs against the inviter's own on-disk store and its own wall
clock.

`famp pair invite` refuses to run without `--confirm-installed`, exiting 2 and creating
no record. This is not a formality: the 24-hour window starts the instant the invite is
created, so generating it before the follower's install is confirmed working risks the
window burning down entirely during their install — they receive a code for an invite
that has already expired by the time they can use it.

## `famp pair revoke`

`famp pair revoke --id <id>` cancels one outstanding invite; `--all-pending` cancels
every currently-`Pending` invite. Both are durable — the revocation is written into the
same `pairing.json` record store every other pairing write goes through, so it survives
a gateway restart.

Anyone who can reach the redemption endpoint can burn a follower's 5-guess budget by
sending 5 deliberately-wrong codes, locking a legitimate invite before the intended
follower ever gets to redeem it. This is a disclosed, accepted denial-of-service (see
`18-03-PLAN.md`'s threat register, T-18-18) — its remedy is one `famp pair revoke` plus
one fresh `famp pair invite`, not a structural fix.

## Recovering from a crash-orphaned lock file

Every read-modify-write of the invite store is serialized by a
`~/.famp/gateway/pairing.json.lock` file, removed automatically when the holding
process exits normally. If a `famp` or `famp-gateway` process is killed mid-write, this
lock file can be left behind, and every subsequent `famp pair` command will wait (then
fail with a busy message) until it clears. To recover: confirm no `famp` or
`famp-gateway` process is currently mid-write (`ps` for either binary), then delete
`~/.famp/gateway/pairing.json.lock` by hand.

## The pin is durable immediately, but not active until a restart

Once `famp pair status` confirms a pin, the redeemer's key is written and reloaded from
`peers.keyring` on disk — durable immediately. `famp-gateway` itself, however, loads
its `Arc<Keyring>` once at process startup and does not hot-reload. The newly pinned
key is not honored by a running gateway process until it restarts. Run
`famp daemon restart` (or manually restart the gateway if it is not daemon-managed) to
pick it up. This is the exact same limitation `famp peer rotate` and `famp peer revoke`
already ship under — pairing does not introduce a new gap, it inherits an existing one.

## What this document does not claim about the wordlist

The five-word code is drawn from a vendored, SHA-256-pinned 2048-word list (BIP-39
English). This document does not claim the list is transcription-robust or
minimum-edit-distance designed for spoken/typed accuracy — that property was logged as
an unverified assumption (`18-RESEARCH.md`, assumption A1) and only the
unique-4-character-prefix half of it was ever mechanically checked. Treat the code as
something to copy/paste or read carefully, not as forgiving of typos.
