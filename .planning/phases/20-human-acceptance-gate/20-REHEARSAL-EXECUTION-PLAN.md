# Phase 20 Rehearsal — Execution Plan (no human input required)

**Written 2026-08-08.** Supersedes the assumption that the rehearsal was blocked on
Ben's home router. It is not. Nothing in this plan requires Ben.

## The unblock

20-02's rehearsal is a **guide-validation dry run**, not the acceptance event.
20-02-PLAN's `user_setup` asks only for a clean supported host with "network path
capable of reaching Ben's public inviter endpoint" — it does NOT require the two
machines to be on independently-administered networks. That requirement is D-07,
and D-07 governs **20-03**, the real second-person UAT.

So the inviter does not have to be Ben's laptop. It can be any host he controls
with a public endpoint.

## Topology

| Role | Host | Why |
|---|---|---|
| Relay | Lightsail `famp-relay`, `relay.famp.dev` → 44.219.73.36 | Already provisioned and billing; port 443 and 80 already open |
| Inviter ("Ben") | fresh EC2, `inviter.famp.dev` | Public IP, so `famp pair redeem --from` can dial it |
| Follower | fresh EC2, untouched | The DOC-07 clean host; preflight must PASS before anything is installed |

Domains follow the namespace model: `ben.famp.dev` and `follower.famp.dev` as
**separate** federation authorities. They must not share one — the relay queue is
`by_domain` with no per-recipient filter, so a shared domain is a shared mailbox.

**Honest limitation to record in the evidence:** both boxes are in AWS. For the
rehearsal that is acceptable and must be stated plainly in `20-REHEARSAL.md`; for
20-03 it is NOT, and the follower must be a real second person on their own network.

## Order of work

1. **Unbreak CI.** `main` is red and was red before today's push — see below.
2. **Deploy the relay.** Let's Encrypt cert for `relay.famp.dev` (port 80 is open, so
   HTTP-01 works), bind 443, `--public-url https://relay.famp.dev`, one `--domain`
   per participant. Use `docs/RELAY-SETUP.md` verbatim — this is its first real
   execution, so anywhere it is silent or wrong is a pre-freeze doc fix.
3. **Stand up the inviter gateway** and confirm it is reachable from outside.
4. **Re-freeze** the guide: re-run 20-01's suite, recompute `guide_commit` /
   `guide_digest`, refresh `20-REHEARSAL.md`'s candidate block. The current values
   are stale as of `de5aa1f`.
5. **Launch the clean follower box.** `scripts/phase20-clean-box-preflight.sh` FIRST,
   before installing anything. If it does not emit the clean signal, stop.
6. **Run `docs/FOLLOWER-SETUP.md` verbatim.** No edits mid-run. Capture
   receiver-owned terminal `famp inspect tasks --id <id> --json` for both directions.
7. **Classify one outcome** — `pass`, `product_or_guide_failure`, or `invalid` — and
   populate `20-REHEARSAL.md` with redacted, owner-attributed evidence.
8. **Tear down** and verify from an independent witness: fresh `describe` plus Cost
   Explorer ~48h later. A self-reported teardown is a claim, not a fact.

## Blocking CI failure (fix before anything else)

`crates/famp/tests/phase20_evidence_schema.rs:175` asserts
`!.planning/phases/20-human-acceptance-gate/20-REHEARSAL.md.exists()`. Plan 20-02
Task 1 creates exactly that file. 20-01 and 20-02 contradict each other.

It stayed invisible because both landed as docs-only commits: `f848c9e` and
`5ca0538` each got **0 check-runs** — `paths-ignore` yields zero runs, which is not
a pass. `94de8a6` was already failing both test jobs before any of today's work.

**Decision: option B′** (B, corrected after adversarial review refuted the first
formulation — see below). The assertion becomes **stage-aware**: if the file exists,
`unresolved` requires `<REQUIRED>` placeholders present AND validator-fail;
`pass` requires validator-pass; `product_or_guide_failure`/`invalid` are permitted;
anything else fails. The `20-ACCEPTANCE.md` line gets the same treatment — "absent
OR passes the acceptance validator" — because must-not-exist is the identical time
bomb waiting to detonate during 20-03.

**Why plain option B was wrong, and this matters beyond this test.** B asserted
`outcome=unresolved`. But `scripts/phase20-evidence-check.sh` rejects `unresolved`
twice — at `:16` (`''|*'<REQUIRED>'*|unresolved` → "incomplete field") and `:23`
(only `pass|product_or_guide_failure|invalid` accepted). `20-02-PLAN.md:131` defines
Task 2's verification as `cargo test …phase20_evidence_schema && …evidence-check.sh`,
so A demanded exactly what B rejected: **the checkpoint gate was unsatisfiable at
every outcome.** Worse, the assertion message ("rehearsal candidate must have
unresolved outcome") would have instructed a future agent to overwrite genuine
acceptance evidence to make CI pass — inverting the gate's meaning. Stage-aware
never needs a future edit, which is the property that actually prevents that.

Superseded rationale, recorded so it is not reused: the original argument was that
presence-plus-unresolved "distinguishes not-started from completed-then-deleted".
That is decorative and does not survive scrutiny — it distinguishes those cases no
better than must-not-exist did. B′ is right because 20-02-PLAN mandates the
candidate's existence, not because of that argument.

**Known separate defect (issue being filed, do not fix inline):**
`phase20-evidence-check.sh`'s `require()` rejects any field still holding
`<REQUIRED>`, so it cannot accept an honest `product_or_guide_failure` or `invalid`
record — exactly the records a failed run produces, where later fields legitimately
never got filled. Needs a Phase 20 scope decision.

Original (rejected) formulation, for the record: "exists and still carries
`outcome=unresolved`". Rationale: the candidate file's
entire purpose is to be *visibly incomplete* until the external run happens, so
presence is correct and unresolved-ness is the real invariant. Must-not-exist
cannot distinguish "not started" from "completed and deleted", which is the weaker
guarantee. `20-ACCEPTANCE.md` keeps its must-not-exist assertion — nothing creates
it before the acceptance event.

Verify the new assertion's red path: set `outcome=pass` in a copy and confirm the
test trips.

## Standing constraints

- Never fabricate rehearsal evidence. Automation validates shape; it never supplies
  or approves DOC-07/UAT-02 facts.
- Never edit the guide mid-attempt. Classify, reset, start a new clean attempt.
- No invite codes, keys, tokens, raw transcripts, or unredacted home paths in
  evidence.
- Do not create `20-02-SUMMARY.md` until a genuine rehearsal passes.
- `cargo nextest` hangs locally; use plain `cargo test` with exact `--test` targets.
- Run `just lint` for Rust changes.
