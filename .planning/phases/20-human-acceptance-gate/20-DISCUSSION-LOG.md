# Phase 20: Human Acceptance Gate - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-05
**Phase:** 20-human-acceptance-gate
**Areas discussed:** follower guide shape, clean-box rehearsal, observer boundary, acceptance evidence, failure-message comprehension

---

## Follower Guide Shape

| Option | Description | Selected |
|--------|-------------|----------|
| One linear pairing-first walkthrough | A standalone zero-to-paired guide using prebuilt binaries and `famp pair`, with deeper mechanism docs linked only as references. | ✓ |
| Patch the legacy gateway guide | Add pairing sections to the current two-machine/manual-key guide. | |
| Documentation index only | Point followers at several existing documents and let them assemble the sequence. | |

**User's choice:** [auto] One linear pairing-first walkthrough.
**Notes:** The current pairing reference explicitly says the follower walkthrough does not yet exist, and the gateway guide still teaches manual key transfer.

---

## Clean-Box Rehearsal

| Option | Description | Selected |
|--------|-------------|----------|
| Exact disposable clean environment | Assert no prior FAMP state or Rust, install release binaries, and exercise the full guide before the human event. | ✓ |
| Installation-only smoke test | Validate only that the release binary starts on a clean machine. | |
| Rely on CI fixtures | Treat installer and E2E tests as sufficient preparation. | |

**User's choice:** [auto] Exact disposable clean environment.
**Notes:** This directly implements DOC-07 and protects the scarce real-person attempt.

---

## Observer Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Observe, do not coach | Ben prepares his side, sends guide/artifact, logs questions, and does not operate or reveal the next step on the follower machine. | ✓ |
| Limited troubleshooting | Ben may provide recovery commands after a failure. | |
| Collaborative setup | Ben and follower work through setup together. | |

**User's choice:** [auto] Observe, do not coach.
**Notes:** A missing next action is a guide/UAT failure, not an invitation to coach through the gate.

---

## Acceptance Evidence

| Option | Description | Selected |
|--------|-------------|----------|
| Receiver-owned terminal evidence | Two tasks, opposite directions, each proven by the receiver's own terminal-state JSON plus redacted environment and topology evidence. | ✓ |
| Gateway-log evidence | Treat signed relay log entries as delivery proof. | |
| Sender success evidence | Treat both `famp send` exit-zero results as success. | |

**User's choice:** [auto] Receiver-owned terminal evidence.
**Notes:** This is the explicit UAT-02 pass criterion; sender exit status is only local acceptance.

---

## Failure-Message Comprehension

| Option | Description | Selected |
|--------|-------------|----------|
| Natural failures plus safe scenario review | Record naturally encountered messages and have the second person explain the remaining messages' next actions without coaching. | ✓ |
| Deliberately trigger every failure live | Mutate or attack the live pairing event until all seven paths occur. | |
| Mechanical tests only | Leave human comprehension unmeasured. | |

**User's choice:** [auto] Natural failures plus safe scenario review.
**Notes:** This closes PAIR-05's deferred human half without consuming invite attempts or corrupting acceptance state merely to manufacture failures.

## the agent's Discretion

- Guide filename and layout.
- Clean-environment technology within the supported release matrix.
- Evidence template formatting and redaction mechanics.
- Exact semantic test organization.

## Deferred Ideas

- Push notification adapter (Phase 21).
- Signed peer directory and automated NAT traversal (outside v1.1).
