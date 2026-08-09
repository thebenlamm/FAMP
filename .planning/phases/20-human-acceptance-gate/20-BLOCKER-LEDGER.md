# Phase 20 — consolidated blocker ledger

**Written 2026-08-08 (night), after a two-reviewer adversarial pass on
`ec3c888..af59ccf` plus my own verification of every finding.**

Purpose: one catalog, so the fixes land in a single pass and the guide is frozen
**once** rather than after each point-fix. Tonight's pattern — two freezes, two
more defects found immediately after each — is what this document exists to stop.

Every row below was verified by opening the cited file myself. Reviewer findings
are hypotheses until then; one was refuted outright (R1) and one was corrected
(B2's severity reasoning).

## The headline: the rehearsal was never runnable

**`v1.1.0-rc.1` contains no `famp pair` command at all.**

```text
tag v1.1.0-rc.1        7f0af24  2026-08-03 08:51:05 -0400
cli/pair/redeem.rs +A  4312f90  2026-08-03 20:43:20 -0400   (~12h later)

git ls-tree -r --name-only v1.1.0-rc.1 | grep -c cli/pair   -> 0
git show v1.1.0-rc.1:crates/famp/src/cli/mod.rs | grep -c Pair -> 0
gh api repos/:owner/:repo/releases/latest --jq .tag_name     -> v1.1.0-rc.1
```

`docs/FOLLOWER-SETUP.md` §1 installs from `releases/latest` and forbids source
builds; §3 then calls `famp pair invite` / `famp pair redeem`. The attempt dies
at §3 on an unrecognized subcommand, before any other defect here can matter.

**Phase 20 is gated on publishing rc.2, not on a doc freeze.** It survived
because prior smoke-testing stopped after §1's `famp daemon install` and never
executed §3.

## P — product defects (need code + a release)

| id | defect | evidence | state |
|---|---|---|---|
| P1 | Pairing pins `agent:<domain>/gateway`; `send` signs `from = agent:<domain>/<name>`; ingress does an exact-match lookup → every paired peer rejects the other | `pair/redeem.rs:102`, `send/mod.rs:679`, `verify.rs:105`, `famp-keyring` `map.get` | **FIXED** by option A (`2ff774d`), issue #43 |
| P2 | `redeem.rs` writes the keyring with a raw `save_to_file` — a pin that fails reload-validation has already overwritten the last-good file, bricking the gateway with no CLI recovery | `redeem.rs:197` vs the hardened `status.rs::pin()` (`f092bd5`) | FIXED `1846c36` |
| P3 | Both pair call sites pass `rotate_to(..., confirmed = true)`, silently retiring an existing Active key. With `--as` now caller-controlled, a holder of a valid invite code can take over another agent's pin | `redeem.rs:189`, `status.rs:277`; semantics at `famp-keyring/src/lib.rs:406-412` | FIXED `9abf997` |
| P4 | Gateway hard-exits on a missing `peers.keyring`, so a first-run gateway cannot start before its first pairing | `famp-gateway/src/main.rs:575` | issue #42; doc workaround shipped in `aaac461` |

**P3 has a consequence neither reviewer raised.** `confirmed = false` alone is
not a safe fix: `Keyring::retire` refuses `Active` entries
(`famp-keyring/src/lib.rs:460-462`) and `revoke` leaves a tombstone that still
trips the duplicate check at load, so a *legitimate* key change would become
unrecoverable from the CLI. P3's fix must pair `confirmed = false` with an
explicit opt-in (`--confirm-key-change` or equivalent) that passes `true`.

## D — documentation defects in the frozen guide

| id | defect | evidence | state |
|---|---|---|---|
| D7 | §5/§6 say `famp register --name <name>`; the flag does not exist — it is positional `<NAME>` | `cli/register.rs:54`; shipped binary exits 2 | FIXED `a1887b8` |
| D8 | `famp register` is a long-lived blocking foreground process, printed as line 1 of a sequential block. Ctrl-C it and every later command fails `NotRegistered` | `register.rs` doc comment; `broker/identity.rs` `resolve_op_identity` | FIXED `a1887b8` |
| D9 | §4's `famp daemon restart` restarts **famp-broker only** — never a gateway. §4 exists to reload pinned keyrings, which load at *gateway* start, so it exits 0 having reloaded nothing | `cli/daemon/restart.rs:11`; no `gateway` anywhere in `cli/daemon/` | FIXED `a1887b8` then REPAIRED `88d4111` |
| D10 | Onboarding `follower.famp.dev` at the relay is a mid-attempt operator step the guide never mentions, and `RELAY-SETUP.md`'s mechanism (paste a `famp peer export` blob) is forbidden by FOLLOWER-SETUP's own preamble and by the accuracy test's `FORBIDDEN_LITERALS` | `famp-relay/src/http.rs:137-140,213-215`; issue #39 | FIXED `84304fc` |
| D11 | The invite artifact prints `famp pair redeem --from <url>` with no `--trust-cert`, so a self-signed inviter cert fails as `Could not reach {url}` — misreporting a TLS-trust failure as gateway-down | `pair/invite.rs`, `pair/redeem.rs` client build | FIXED `a1887b8` |
| D12 | The follower must be named exactly `dana` for the inviter's current routing, but the guide's `<follower-name>` implies free choice | deployed unit; `build_route_map` path 2 | RECORDED — the rehearsal uses `dana`; still a real DOC-06 gap for 20-03 |

## G — gate defects (why none of the above was caught)

| id | defect | state |
|---|---|---|
| G1 | `follower_setup_doc_accuracy` executes only §1's fenced block (`classify_section1_commands` takes the first block containing `famp-installer.sh`). §2–§7 get string-anchor assertions only, and `famp register` is in no anchor. This is the D1 hole recurring one section down | FIXED `0b080c5`, plus two further holes closed in `88d4111` |
| G2 | No test pairs and then delivers. `e2e_relay_bidirectional` bootstraps via `peer_export --as ALICE` ("for the AGENT identities") and never invokes `famp pair`; `pairing_e2e` asserts the pins land and never sends an envelope. Both halves green, seam untested — this is exactly how P1 shipped | FIXED `4e377af` — red path reproduced independently, control held |
| G3 | `phase20-evidence-check.sh`'s `require()` rejects any field still holding `<REQUIRED>`, so an honest `product_or_guide_failure` or `invalid` record cannot validate — and 20-02 Task 2's verify runs that script. The most likely outcome of any attempt is unrecordable by the plan's own gate | FIXED `d49df10`, issue #40 closed |

## R — refuted / corrected

- **R1 (refuted).** A reviewer claimed the inviter unit's positional `dana` means
  `agent:ben.famp.dev/ben` does not exist, so Task B would die. Wrong: the
  positional arg is the **local stand-in for a remote principal**
  (`registry.back()`, `build_route_map` path 2 at `main.rs:386-394`); `ben` is
  registered by `famp register ben`. The deployed config is correct. STATE.md's
  phrase "wired into the inviter unit" is loose and should be tightened — the
  unit wires the *follower* side.
- **R2 (corrected).** "`--as` is a breaking CLI change against published rc.1"
  is false — `famp pair` is not in rc.1 at all, so no published invocation
  breaks. The design call stands; that justification for it does not.
- **R3 (checked, clean).** No orphaned static IP: one Lightsail static IP
  (`44.219.73.36`, attached), zero EC2 Elastic IPs. The earlier
  `54.158.102.139` was released at reprovision.

## Decision taken

**Keep option A now, option B later** (Ben, 2026-08-08).

- **A (shipped, `2ff774d`)**: `pair redeem --as <name>` pins the principal
  `send` will actually use. Unblocks rc.2.
- **B (deferred)**: ingress resolves the signing key by the sender's *domain*
  gateway principal, falling back from the exact match. The egress signature is
  the gateway's key, so trust is machine-to-machine and agent-level granularity
  is illusory today. B removes A's ceiling — because one pubkey may be pinned
  under only one principal (`DuplicatePubkey`, fatal at load,
  `famp-keyring/src/lib.rs:153-160`), **A allows exactly one addressable agent
  per peer machine.** File B before this ledger is closed.

## Order of work

1. P2, P3 — finish the pairing fixes (shared validated-write helper; explicit
   key-change opt-in).
2. G2 — the pair-then-deliver regression test that P1's fix should have been
   gated on.
3. G1 — generalize the doc gate to every fenced block, then D7–D11 against it.
4. G3 — unjam the evidence validator for failure outcomes.
5. Re-freeze the guide **once**.
6. Cut and publish **rc.2** (human-gated: run by whoever received the approval,
   never delegated to a subagent).
7. Dirty rehearsal on a throwaway box against the **published rc.2** binaries —
   the only test that would have caught tonight's headline.
8. Clean-host attempt.
