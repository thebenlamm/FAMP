---
phase: 20-human-acceptance-gate
plan: 02
subsystem: testing
tags: [acceptance, evidence, pairing, relay, gateway, documentation]
requires:
  - phase: 20-human-acceptance-gate
    provides: frozen follower guide, clean-host preflight, evidence validator, blank templates
provides:
  - a validated clean-host rehearsal record classified pass
  - inviter and relay reset to a genuine pre-pairing state
  - end-to-end gating of the evidence templates themselves
affects: [20-03, DOC-07, UAT-02, PAIR-05]
tech-stack:
  added: []
  patterns: [reset-rather-than-rename before a gated attempt, validate the real template not a synthetic stand-in]
key-files:
  created:
    - .planning/phases/20-human-acceptance-gate/20-02-SUMMARY.md
  modified:
    - .planning/phases/20-human-acceptance-gate/20-REHEARSAL.md
    - .planning/phases/20-human-acceptance-gate/20-REHEARSAL-TEMPLATE.md
    - .planning/phases/20-human-acceptance-gate/20-ACCEPTANCE-TEMPLATE.md
    - crates/famp/tests/phase20_evidence_schema.rs
key-decisions:
  - "Reset the inviter and relay to pre-pairing state rather than renaming the follower, so the attempt replays the topology the dirty walkthrough validated."
  - "outcome=pass was withheld until Ben made the provenance call on agent-driven-over-SSH execution; automation populated facts only."
patterns-established:
  - "A gated attempt resets every participating host, not just the one being called clean."
  - "Evidence-template contracts are validated against the real template text, prose included, never a synthetic field list."
requirements-completed: [DOC-07]
---

# Plan 20-02 — Clean-host rehearsal executed and attested

## Outcome

`20-REHEARSAL.md` is populated and classified **`pass`**.
`scripts/phase20-evidence-check.sh rehearsal` reports
`EVIDENCE RECORD: VALID rehearsal pass`. DOC-07 is closed. **UAT-02 remains
open** — no second person participated, §7 was not performed, and no
comprehension claim is made.

## What happened

Task 1 (freeze + repository-local readiness) was already complete at `5ca0538`
and re-frozen at `6bfed80` after the dirty walkthrough. Task 2 ran the genuine
clean-host attempt.

Two prerequisites were settled before the box was launched.

**The §5/§6 host-agent claim was unverified.** The guide told a host agent that
`famp_send` in `reply` mode commits with `expect_reply: true` and closes
without it — a sentence inferred from the MCP tool description's legacy-alias
line and never run. Driving both replies through the real MCP surface showed
the wire is identical to the CLI's without/with `--terminal`. The guide was
correct; no re-freeze. Full evidence in `20-REHEARSAL.md`.

**The inviter and relay still carried the dirty run's state** — a pin and a
relay domain both holding the terminated box's key. Renaming the follower was
rejected in favour of resetting both hosts, because a rename would have put an
unrehearsed topology on the gated attempt and would have saved no work: the
relay pinned the dead key either way, and the inviter names the follower in
three places. The reset also restored §3 and §4a to genuine first-run paths;
had it not been done, both would have been silently skipped. The payoff showed
up directly — §4a's `grep` returned exactly one line, as the guide promises,
which a rename would have made ambiguous.

The attempt then ran the frozen guide verbatim on a fresh Linux/x86_64 VM whose
security group allowed no inbound FAMP port at all, incidentally confirming
§2's claim that the follower needs none. Preflight passed before installation;
both tasks reached `COMPLETED` with `sig_verified=true` on receiver-owned
captures.

## Defect found

Populating the record for the first time exposed a gate defect: **a fully
populated `pass` record could not validate.** The validator greps the record
for two secret-shaped literals, and the templates' own redaction warning
spelled both out, so every `pass` record inherited them. The grep runs only on
the `pass` path, so every earlier `unresolved` state missed it — G3's shape
mirrored onto the success outcome.

The existing contract tests could not have caught it: they assemble synthetic
`key=value` rows, so template prose was never part of a validated body.
`real_templates_populated_as_pass_are_accepted` now fills the actual template
files and validates the whole text; it was confirmed red against the original
wording and green after the fix. `docs/FOLLOWER-SETUP.md` carries one of the
same literals but the validator never reads it, so the freeze stands and the
digest is unchanged at `f1262fc6…3268`.

## Verification

- `scripts/phase20-evidence-check.sh rehearsal .../20-REHEARSAL.md` → `VALID rehearsal pass`
- `cargo test -p famp --test phase20_evidence_schema` → 6 passed
- `follower_setup_doc_accuracy` (7), `phase20_clean_box_preflight` (1),
  `pair_cli` (18), `gateway_setup_doc_accuracy` (6) → all pass
- `cargo fmt --all --check` clean; `cargo clippy -p famp --tests` clean
- `git diff --exit-code -- docs/FOLLOWER-SETUP.md` clean

## Carried forward to 20-03

- **UAT-02 and PAIR-05 are untouched.** The seven comprehension fields are open
  and require an uncoached second person.
- **D12 is still a real DOC-06 gap.** The guide implies a free choice of
  `<follower-name>` while the inviter hardcodes `dana` in three places. This
  attempt worked because it used `dana`; a real second person would not know to.
- **Product defect, unfixed:** `pair redeem` and `pair status` both tell the
  operator to run `famp daemon restart`, which restarts the broker and never a
  gateway. Reproduced again on rc.2. §4 corrects for it in prose; the binary
  still misdirects.
- **Doc gap, unfixed:** the guide warns that `famp register` must keep running
  but says nothing equivalent for `famp-gateway`, which dies at SSH disconnect
  unless detached.
- `famp register ben` on the inviter is an unsupervised bare process with no
  unit; §5 depends on it surviving.
