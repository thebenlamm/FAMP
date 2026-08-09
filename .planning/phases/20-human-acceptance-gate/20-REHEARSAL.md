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
Never record invite codes, private keys, authentication tokens, raw
transcripts, or unredacted home paths. A product/guide failure requires repair
and reset; an invalid run requires a fully clean rerun.

Each row represents: criterion, owner, capture command/attestation, UTC time,
redacted evidence, result. Replace every `<REQUIRED>` value; do not edit keys.

```text
outcome=unresolved
failure_stage=<REQUIRED>
failure_detail=<REQUIRED>
redaction_review=<REQUIRED>
redaction_findings=<REQUIRED>
clean_preflight=<REQUIRED>
clean_owner=<REQUIRED>
clean_utc=<REQUIRED>
clean_os_arch=<REQUIRED>
release_famp_version=<REQUIRED>
release_gateway_version=<REQUIRED>
pairing_ready=<REQUIRED>
task_a_id=<REQUIRED>
task_a_owner=<REQUIRED>
task_a_utc=<REQUIRED>
task_a_state=<REQUIRED>
task_b_id=<REQUIRED>
task_b_owner=<REQUIRED>
task_b_utc=<REQUIRED>
task_b_state=<REQUIRED>
```

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
