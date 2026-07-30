---
plan: 12-05
phase: 12
requirements: [REL-05]
status: complete
completed: 2026-07-29
---

# 12-05 SUMMARY — Tag `v1.0.0`

## Objective

Draft the `v1.0.0` annotation, obtain Ben's explicit confirmation on it and on the
limitation-statement wording, then create and push the annotated tag on exactly the
SHA that `12-CI-ATTESTATION.md` proved green.

## What shipped

| Task | Outcome |
|------|---------|
| 1. Draft annotation | `12-TAG-ANNOTATION.md` (commit `67025fd`) |
| 2. Checkpoint — Ben's decision | **Answered: `tag-with-boundary-limitation` (option B)** |
| 3. Create + push tag | `v1.0.0` → `5edff41835b9c8e6daa59a51efce549460d88e5b`, pushed |

**Tag object:** `1ae0bc837b76c3705a5ce9671bb831181b2432e3`
**Peeled target:** `5edff41835b9c8e6daa59a51efce549460d88e5b` — matches the attested SHA exactly.
**Type:** plain annotated (not GPG-signed), matching the `v0.9` / `v1.0.0-rc.1` convention.
**Body as shipped:** `12-TAG-BODY-AS-SHIPPED.txt` (124 lines, byte-for-byte what `-F` received).

## Limitation-statement decision

§16 proposed shipping: *"`famp send` … does not initiate or complete the task FSM;
federated task initiation is not exposed through the v1.0 client interface."*

**That wording is false as of Phase 11's ADDR-02** — `crates/famp/src/cli/send/mod.rs`
mode-branches remote sends into typed, FSM-driving `RequestBody` / `CommitBody` /
`DeliverBody` envelopes, and `11-HUMAN-UAT.md` §4 records a real task reaching
`COMPLETED` on both hosts via the shipping CLI. It was **excluded** from the tag.

Ben selected **option (B)**: ship the accurate fire-and-forget exit-code boundary
instead — the same statement plan 12-01 shipped in `docs/GATEWAY-SETUP.md` §6,
`famp send --help`, and README, pinned by `gateway_setup_doc_accuracy.rs`. It appears
as its own paragraph in the annotation. ROADMAP success criterion 6 is satisfied: the
accurate statement shipped, the stale one did not.

## Verification performed

- **CI re-verified live immediately before tagging** — `gh api .../commits/5edff41…/check-runs`
  → `total_count: 11`, zero incomplete, zero non-success. Not inherited from any prior
  doc claim, per this phase's core discipline.
- SHA confirmed an ancestor of `origin/main` before tagging.
- `git rev-list -n 1 v1.0.0` == attested SHA (asserted, matched).
- `git cat-file -t v1.0.0` == `tag` (annotated, not lightweight).
- `git ls-remote --tags origin` confirms `refs/tags/v1.0.0^{}` = `5edff41…` on the remote.
- Annotation body validated pre-tag: placeholder removed, all 9 §16 checklist items
  present, stale §16 sentence absent (grep count 0), limitation paragraph present.

## Deviation

**Task 3 was executed by the orchestrator, not this plan's executor agent.** The executor
declined to act on the approval because it arrived as an agent-relayed message rather than
directly from Ben, and it could not distinguish a faithful relay from an inference. That
refusal was correct from its position — a `v1.0.0` tag push is the milestone's only
irreversible outward-facing action, and T-12-05-04 names exactly that failure mode.

The gap was verification, not consent: Ben's selection was made through the interactive
question tool and received directly by the orchestrator. The orchestrator therefore executed
Task 3 itself under the same constraints the executor had been given (final CI re-check,
tag the SHA by value never `HEAD`, delimited body only, plain annotated, tag-only push,
no GitHub release). All acceptance criteria were met and independently verified above.

**Worth carrying forward:** a checkpoint whose approval must survive an orchestrator→executor
hop needs an approval channel the executor can verify, or the orchestrator should own the
gated action outright. The current design makes the safe outcome a stall.

## Artifacts this plan produced

- git tag `v1.0.0` (annotated) → `5edff41835b9c8e6daa59a51efce549460d88e5b`
- `.planning/phases/12-v1-0-0-release-gate/12-TAG-ANNOTATION.md`
- `.planning/phases/12-v1-0-0-release-gate/12-TAG-BODY-AS-SHIPPED.txt`

## Self-Check: PASSED
