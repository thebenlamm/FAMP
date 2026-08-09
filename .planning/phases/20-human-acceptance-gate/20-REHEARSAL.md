# Phase 20 Clean-Host Rehearsal Record — CANDIDATE

This candidate was created from `20-REHEARSAL-TEMPLATE.md` after the
repository-local readiness suite passed. It is not clean-host evidence and is
not a completed rehearsal. Every external evidence value remains unresolved
until Task 2 is performed on a genuine untouched supported host.

Frozen repository candidate:

```text
guide_commit=6bfed8003ff2e79119aa6f40644d3aec33b1884f
guide_digest=f1262fc674e584e97f668f0f8940c5342079696c9b734577e0370e36ab223268
```

Re-frozen 2026-08-09 at `6bfed80`, after the dirty walkthrough, and after every row of
`20-BLOCKER-LEDGER.md` was closed. This is the freeze the attempt runs
against; the five earlier freezes (`f848c9e`, `f3210a0`, `aaac461`, `84304fc`, `6ffd0d9`)
were each superseded within hours by the next defect and must not be used to
attest anything.

That churn is the reason this one waited: the guide is frozen once, after the
catalog was emptied, rather than after each point-fix. Between the first freeze
and this one the guide gained a headless-Linux linger step, an empty-keyring
step (#42), a corrected `famp register` invocation, an unmissable warning that
register blocks, a section 4 that restarts the gateway rather than the broker,
TLS-trust guidance, and a new section 4a sequencing relay domain registration.
None of that was visible from reading the guide; all of it came from running
the commands or the code.

Readiness suite green against this freeze: `follower_setup_doc_accuracy` (7),
`phase20_clean_box_preflight` (1), `phase20_evidence_schema` (3), `pair_cli`
(18), `gateway_setup_doc_accuracy` (4), and on the gateway side `pairing_e2e`
(5), `pair_then_deliver_e2e` (1), `e2e_relay_bidirectional` (1).
`git diff --exit-code -- docs/FOLLOWER-SETUP.md` is clean at this commit.

The `84304fc` freeze was superseded by one line: it described the missing-keyring
behavior as belonging to "the released `v1.1.0-rc.1` gateway", which stops being
true for the reader the moment rc.2 is what section 1 installs. The behavior is
unchanged (issue #42 is still open); only the label was going stale, and it is
now phrased without a version.

**Still not sufficient to start until rc.2 is published.** `v1.1.0-rc.1` carries
no `famp pair` command and the guide installs from `releases/latest`, so the
attempt cannot begin until that URL serves a binary with the subcommands
section 3 calls -- verified by downloading it, not by a green release run.

Readiness suite re-run green against this freeze: `follower_setup_doc_accuracy`
(4), `phase20_clean_box_preflight`, `phase20_evidence_schema` (4), `pair_cli`,
`gateway_setup_doc_accuracy`, and `famp-gateway`'s `e2e_relay_bidirectional`
(1). `git diff --exit-code -- docs/FOLLOWER-SETUP.md` is clean at this commit.

Every evidence value below is owner-attributed, UTC-timestamped, and redacted.
Never record pairing codes, signing-key material, credentials, raw
transcripts, or unredacted home paths. A product/guide failure requires repair
and reset; an invalid run requires a fully clean rerun.

Each row represents: criterion, owner, capture command/attestation, UTC time,
redacted evidence, result. Replace every `<REQUIRED>` value; do not edit keys.

```text
outcome=unresolved
failure_stage=none
failure_detail=none
redaction_review=pass
redaction_findings=none
clean_preflight=PASS
clean_owner=ben
clean_utc=2026-08-09T13:19:31Z
clean_os_arch=REDACTED:Linux/x86_64
release_famp_version=1.1.0-rc.2
release_gateway_version=1.1.0-rc.2
pairing_ready=yes
task_a_id=019fe6b2-0e60-7591-80d5-88391ea7e29b
task_a_owner=dana
task_a_utc=2026-08-09T13:24:44Z
task_a_state=COMPLETED
task_b_id=019fe6b2-a168-7260-8b44-016106c7c112
task_b_owner=ben
task_b_utc=2026-08-09T13:25:02Z
task_b_state=COMPLETED
```

**`outcome` is deliberately still `unresolved`.** Every other field above is
factual and captured. The classification is withheld pending the provenance
attestation below, because the one question this record cannot answer for
itself is who counts as the operator.

Exactly one final outcome is permitted: `pass`, `product_or_guide_failure`, or
`invalid`. This candidate deliberately defaults to none of them.


## Dirty walkthrough result (2026-08-09, throwaway EC2 -> inviter, relay-mediated)

Not DOC-07 evidence: the follower host was a throwaway box driven over SSH by
the same operator, and no evidence ceremony was performed. Its purpose was to
find defects by running the guide rather than reading it, and it did.

Confirmed working against the PUBLISHED rc.2 binaries, in order:

- section 1 clean; `famp daemon install` printed the linger note exactly as the
  guide describes, and `famp daemon status` then reported `linger=yes`
- issue #42 reproduced exactly -- the gateway refused to start without
  `peers.keyring`, so section 2's step is load-bearing, not defensive
- pairing succeeded both directions; the inviter pinned
  `agent:follower.famp.dev/dana`, the AGENT principal, confirming #43's fix on
  real hosts rather than only in a test
- section 4a worked as written: the operator read the key from their own
  keyring, the follower sent and pasted nothing, and the relay's
  `follower.famp.dev` 404s went to zero after registration
- inbound arrived wrapped in the FAMP-QUARANTINE boundary, origin=gateway
- both task directions reached COMPLETED with `sig_verified=true` on every
  envelope, captured by the receiving owner

Defects it found, both since fixed:

1. Sections 5 and 6 skipped the FSM's Commit step, making their own pass
   criterion unreachable (#45, fixed in `6bfed80`). Reading the guide could not
   have surfaced this; only running it did.
2. `pair redeem` and `pair status` both tell the operator to run
   `famp daemon restart` to load the new pin. That command restarts the broker
   and never a gateway -- the same defect the guide's section 4 was corrected
   for, still present inside the shipped binary. Product-side, does not block
   the attempt.

Also observed: a gateway started over SSH dies at disconnect unless detached
with `setsid`. The guide warns that `famp register` must keep running but says
nothing equivalent for `famp-gateway`.


## Pre-attempt verification of the §5/§6 host-agent claim (2026-08-09)

Sections 5 and 6 tell a host agent that `famp_send` in `reply` mode commits the
task with `expect_reply: true` and closes it without. That sentence was
originally derived from the MCP tool description's legacy-alias line, never
tested. It is now verified, and the guide needs no change — **the freeze at
`6bfed80` stands.**

What was checked, and how:

- **Wire equivalence, empirical.** A task was opened on the local bus by CLI and
  answered twice through the MCP surface. The receiving mailbox recorded
  `famp.send.deliver` with `mode: "deliver"` and no terminal flag for the
  `expect_reply: true` reply, then `famp.send.deliver_terminal` with
  `"terminal": true` for the bare reply. Those are exactly the envelopes the CLI
  emits for a reply without `--terminal` and a reply with it.
- **Code agrees.** `cli/mcp/tools/send.rs:184` maps `reply` to
  `terminal = !expect_reply`; the legacy `deliver` / `terminal` aliases below it
  produce the same two shapes.
- **Representative of the published binary.** `cli/mcp/tools/send.rs` is
  unchanged across `v1.1.0-rc.1..HEAD` (so unchanged in rc.2), and the installed
  binary used for the test postdates that file's last commit.

What the local test could **not** show, and why it does not matter: local-bus
envelopes carry `class: audit_log`, and the inspector's fold returns `UNKNOWN`
for that class by construction (`famp-inspect-server/src/parse.rs:62`), so no
FSM transition is observable on the local path by design. The
`REQUESTED -> COMMITTED -> COMPLETED` leg comes from the dirty walkthrough
above, which drove it over the gateway with the CLI. It transfers to the host
agent because the gateway sees only envelopes and the two surfaces emit
identical ones.

Scope of this verification: the MCP-to-CLI mode mapping, not gateway FSM
behavior, which rests on the dirty-run evidence.


## Inviter and relay carry state from the dirty run (2026-08-09)

Recorded before the clean attempt, because a fresh follower host is not by
itself a clean run. Read from the live boxes, not from notes:

- Inviter `44.204.243.222` still pins one line,
  `agent:follower.famp.dev/dana`, holding the terminated box's gateway key.
- Relay still serves `--domain follower.famp.dev=<same dead key>` alongside
  `ben.famp.dev`.
- The inviter's running gateway hardcodes the follower in three places:
  `--backs agent:follower.famp.dev/dana`, `--peer
  follower.famp.dev=https://relay.famp.dev`, and the positional `dana` that
  `registry.back()` uses for the local stand-in holder (`main.rs:535-536`).
- `follower.famp.dev` has no DNS A record, and needs none — the follower
  fetches from the relay rather than receiving inbound.

Consequence: §3 would re-pair over an existing pin rather than pin into an empty
keyring, and §4a would be a no-op rather than the add-from-nothing step whose
404s-to-zero the dirty run observed. Both are first-run paths under test.


## Pre-attempt reset of the inviter and relay (2026-08-09, approved)

Decision: reset both boxes to their pre-pairing state rather than rename the
follower. The dirty walkthrough validated exactly one topology — `dana`,
`follower.famp.dev`, this inviter unit, this relay unit. A rename would put an
unrehearsed variant on the expensive human-gated attempt, which is what the
rehearsal existed to prevent. It also saves nothing: the relay pins the dead
box's key either way, and the inviter names the follower in three places.

**Correction to the plan as first proposed.** The inviter's gateway is not a
hand-started foreground process. It is a system unit,
`famp-gateway.service` ("FAMP Federation Gateway (inviter, ben.famp.dev)"),
`Type=simple`, `User=ubuntu`, `Restart=always`, `RestartSec=5`, carrying the
full flag set including `FAMP_OWN_DOMAIN=ben.famp.dev`. The proposed
"kill the pid and relaunch under `setsid`" would have raced systemd's own
restart. `sudo systemctl restart famp-gateway` is the correct operation. This
is the "deployed unit" D12 refers to; the gateway has been up since
06:01:00 UTC with `NRestarts=0`.

Also note `famp register ben` on the inviter is a bare unsupervised process
(pid 4540), not a unit. It survives only until something kills it, and §5
depends on it.

State changes applied (neither required root):

- Inviter `~/.famp/gateway/peers.keyring` truncated from 110 bytes to 0.
  Backed up to `peers.keyring.dirtyrun.bak`. An empty file is the valid
  pre-pairing state §2 describes; the file must exist, because issue #42 makes
  an absent one startup-fatal.
- Relay unit `--domain follower.famp.dev=<dead key>` removed, keeping
  `ben.famp.dev`. Backed up to `~/famp-relay.service.dirtyrun.bak`.

Both take effect only on restart, which is left to the operator:

```sh
ssh -i ~/.ssh/famp-phase20-key.pem ubuntu@44.204.243.222 \
  'sudo systemctl restart famp-gateway'
ssh -i ~/.ssh/famp-relay-key.pem ubuntu@relay.famp.dev \
  'sudo systemctl daemon-reload && sudo systemctl restart famp-relay'
```

Expected after restart: the gateway logs `ready, backing 1 principal(s): dana`
with an empty keyring, and the relay logs
`serving domain(s): ben.famp.dev (1 key(s))`. Do not launch the clean box until
both are confirmed.


## Clean-host attempt run log (2026-08-09)

Host: a freshly launched EC2 VM (not a container), Linux/x86_64, created for
this attempt and never previously touched by FAMP. Its security group allows
only SSH from the operator's address and no inbound FAMP port at all, which
also exercises §2's claim that the follower needs no inbound-reachable
endpoint. Inviter and relay were reset to their pre-pairing state first
(previous section).

Guide identity confirmed immediately before the run:
`sha256(docs/FOLLOWER-SETUP.md) = f1262fc6…3268`, matching the freeze, with a
clean `git diff --exit-code` on that path. No guide edit occurred at any point
during the attempt.

Sequence, in order, with nothing skipped or repaired:

1. **Preflight before installation.** `phase20-clean-box-preflight.sh` emitted
   `CLEAN HOST: PASS` at `2026-08-09T13:19:31Z`, exit 0, on a host with no
   `famp`, `famp-gateway`, `rustc`, or `cargo` on `PATH` and no `~/.famp`.
2. **§1.** Both published installers ran from `releases/latest` and delivered
   `1.1.0-rc.2`. `famp --version` and `command -v famp-gateway` succeeded in a
   plain login shell with no manual `PATH` repair. `famp daemon install`
   printed the linger note exactly as §1 describes; the printed
   `loginctl enable-linger` command succeeded without elevation, and
   `famp daemon status` then reported `linger=yes`.
3. **§2.** The TLS recipe produced a leaf with `CA:FALSE` and `serverAuth`.
   The empty `peers.keyring` was created before first start. The gateway
   printed `ready, backing 1 principal(s): ben` and then logged
   `relay-fetch[follower.famp.dev] … 404` — the correct pre-§4a state, and
   direct confirmation that the relay reset landed.
4. **§3.** Redemption as `dana` succeeded against the inviter's public
   endpoint with no `--trust-cert` needed. `famp pair status` on the inviter
   displayed `REDEEMED BY: agent:follower.famp.dev/dana` and pinned the AGENT
   principal — #43's fix holding on real hosts a second time. Because the
   keyring had been reset to empty, this was a genuine first pin: no
   `--confirm-key-change` guard was reached.
5. **§4a.** `grep "<follower-domain>" ~/.famp/gateway/peers.keyring` returned
   **exactly one line**, as the guide promises. The relay gained the domain and
   restarted; the inviter gateway restarted and reloaded the pin; the follower
   gateway was restarted with the byte-identical command. Its
   `follower.famp.dev` 404 count went to **zero** — §4a's success signal.
6. **§5 / Task A.** Inbound arrived wrapped in the FAMP-QUARANTINE boundary
   with `origin=gateway` and `class=request`. Commit reply, then terminal
   reply. Receiver-owned `famp inspect tasks --json` showed
   `REQUESTED → COMMITTED → COMPLETED` with `sig_verified=true` on all three
   envelopes.
7. **§6 / Task B.** Same in the opposite direction, distinct task id, same
   three-state progression, `sig_verified=true` throughout, captured by the
   receiving owner.
8. **§7.** Not performed. No second person participated, so the seven
   comprehension fields stay open for Plan 20-03 and **no comprehension claim
   is made here.**

Operator note, recorded rather than hidden: while restarting the follower
gateway, a `pkill -f` pattern also matched the driving SSH session and dropped
it. Nothing on the host was altered beyond stopping the gateway, and the very
next action — rerunning the identical gateway command — is precisely what §4a
prescribes. No guide step was skipped, repeated out of order, or repaired.

### Gate defect found by populating this record

A fully populated `pass` record **cannot validate**, and the cause is this
template's own redaction warning. `phase20-evidence-check.sh` greps the record
for `invite[_ -]?code` and `private[_ -]?key`; the template's line telling the
author not to record those things contains both literals. The grep runs only on
the `pass` path, so every earlier state missed it — the same shape as G3, which
made failure outcomes unrecordable, mirrored onto the success outcome.

Demonstrated with a control: a fully populated probe failed with
`redaction review required: forbidden secret/path pattern`; rewording that one
warning line and changing nothing else produced
`EVIDENCE RECORD: VALID rehearsal pass`. Both templates were reworded to say
"pairing codes, signing-key material, credentials", which keeps the warning and
drops the literals. The guide's §preamble carries one of the same literals too,
but the validator never reads the guide, so **the guide is unaffected and the
freeze stands.**

Note for anyone editing this record: prose *about* the banned literals trips the
same grep. This section originally did, on a sentence quoting the guide. Describe
them, do not spell them.

This was found after the host run had finished. It is a defect in this phase's
evidence tooling, not in the guide or the product, and no host step was redone
because of it.

### Provenance attestation — REQUIRED before `outcome` may be set

The mechanics above were executed by an agent over SSH against machines Ben
owns and administers, at his direction. Ben owns the hosts, the invite, and
both receiver captures; he did not personally type the commands.

Plan 20-02 Task 2 requires a human to confirm the record's provenance, and
whether agent-driven execution satisfies DOC-07's "external operator" is a
judgment only Ben can make. Two readings are honestly available:

- **`pass`** — DOC-07 asks for an untouched supported clean host, the frozen
  guide followed exactly, and owner-attributed receiver evidence. All three
  hold. Independent-person judgment is UAT-02's job, and Plan 20-03 still
  gates it.
- **`invalid`** — if "external operator" means a human at the keyboard, then
  this is a second dirty run, however clean the host was, and DOC-07 stays
  open until a person repeats it.

No `pass` is claimed here. Set `outcome` only after that call is made.
