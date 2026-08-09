# Phase 20 Clean-Host Rehearsal Record — CANDIDATE

This candidate was created from `20-REHEARSAL-TEMPLATE.md` after the
repository-local readiness suite passed. It is not clean-host evidence and is
not a completed rehearsal. Every external evidence value remains unresolved
until Task 2 is performed on a genuine untouched supported host.

Frozen repository candidate:

```text
guide_commit=84304fc231f43c27874bce5063630a53df3c3b1c
guide_digest=b818d2f8ccd6129db987e4ff798aa69c2dda4c87d4ede98e20939ed6df477c33
```

Re-frozen 2026-08-08 (final) at `84304fc`, after every row of
`20-BLOCKER-LEDGER.md` was closed. This is the freeze the attempt runs
against; the three earlier freezes tonight (`f848c9e`, `f3210a0`, `aaac461`)
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

**This freeze is still not sufficient to start.** `v1.1.0-rc.1` carries no
`famp pair` command, and the guide installs from `releases/latest`. The attempt
cannot begin until rc.2 is published and section 1's installers deliver a
binary that has the subcommands section 3 calls.

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
