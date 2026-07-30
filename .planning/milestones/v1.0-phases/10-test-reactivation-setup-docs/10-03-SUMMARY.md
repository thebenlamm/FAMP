---
phase: 10-test-reactivation-setup-docs
plan: 03
subsystem: docs
tags: [gateway, cli-accuracy-gate, ed25519, trust-bootstrap, tls, markdown]

# Dependency graph
requires:
  - phase: 08-trust-bootstrap-peer-verification
    provides: famp peer export/import Ed25519 TOFU trust bootstrap (TRUST-01)
  - phase: 09-end-to-end-cross-host-delivery
    provides: famp-gateway --listen/--tls-cert/--tls-key/--peer/--trust-cert cross-host relay + famp inspect tasks terminal-state verification
provides:
  - docs/GATEWAY-SETUP.md — own-machines-first two-machine gateway runbook
  - Two compiled binary-accuracy gates preventing the guide from drifting from the shipping CLI
  - README link from Quick Start's federation sentence
  - 10-HUMAN-UAT.md — the deferred Gate A human-acceptance record
affects: [milestone-close, v1.0.0-tag]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-crate accuracy-gate test spawning only its own crate's binary (avoids cross-package cargo_bin resolution)"
    - "Dynamic flag extraction from Markdown fenced code blocks (non-vacuous doc/binary drift detection, not a fixed whitelist)"

key-files:
  created:
    - docs/GATEWAY-SETUP.md
    - crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs
    - crates/famp/tests/gateway_setup_doc_accuracy.rs
    - .planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md
  modified:
    - README.md

key-decisions:
  - "Gateway accuracy gate extracts --flag tokens dynamically from the guide's fenced famp-gateway command examples (not a fixed whitelist) so an injected/typo'd flag is actually caught — verified via manual falsification (added --bogus-flag, gate FAILED, reverted, gate green again)"
  - "10-HUMAN-UAT.md carries the DOC-04 human clause as PENDING (Gate A dogfood), consistent with the D-07 decision not to claim DOC-04 fully done on the grep-gate alone"
  - "README links docs/GATEWAY-SETUP.md from Quick Start as the primary cross-host entry point; the historical 'Advanced: v0.8 federation CLI' section is left unmodified per RESEARCH Open Question 2"

patterns-established:
  - "Doc/binary accuracy gates live one per crate under that crate's own tests/ dir, spawning only Command::cargo_bin for that crate's own binary"

requirements-completed: [DOC-04]

coverage:
  - id: D1
    description: "docs/GATEWAY-SETUP.md documents the two-machine setup (prerequisites, gateway identity, out-of-band export/import + fingerprint check, start each gateway, connect/verify) using exact shipping flag spellings"
    requirement: DOC-04
    verification:
      - kind: other
        ref: "grep checks in 10-03-PLAN.md Task 1 acceptance_criteria (all flags/commands present, no-public-relay framing, secret key never instructed to be copied)"
        status: pass
    human_judgment: false
  - id: D2
    description: "A compiled accuracy gate fails if the guide's documented flags drift from famp-gateway's usage string or famp peer export/import --help"
    requirement: DOC-04
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs#gateway_usage_doc_accuracy"
        status: pass
      - kind: integration
        ref: "crates/famp/tests/gateway_setup_doc_accuracy.rs#gateway_setup_doc_accuracy"
        status: pass
      - kind: manual_procedural
        ref: "Falsification: injected --bogus-flag into docs/GATEWAY-SETUP.md, confirmed gateway_usage_doc_accuracy FAILED, reverted, confirmed green again"
        status: pass
    human_judgment: false
  - id: D3
    description: "10-HUMAN-UAT.md captures the unassisted two-machine walkthrough as a human-verified Gate A acceptance, not claimed by the grep gate"
    requirement: DOC-04
    verification: []
    human_judgment: true
    rationale: "The unassisted-success clause requires a real human running the guide on two physical machines he controls — this cannot be proven by any automated test in this repo. 10-HUMAN-UAT.md status is PENDING awaiting Ben's real run."

# Metrics
duration: 45min
completed: 2026-07-27
status: complete
---

# Phase 10 Plan 03: Two-Machine Gateway Setup Guide Summary

**docs/GATEWAY-SETUP.md two-machine runbook gated by two compiled binary-accuracy tests that dynamically extract and verify the guide's famp-gateway/famp peer command flags against the live CLI, plus a PENDING 10-HUMAN-UAT.md for Ben's real cross-host walkthrough**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-07-27T23:00:00Z (approx)
- **Completed:** 2026-07-27T23:42:50Z
- **Tasks:** 3/3 completed
- **Files modified:** 5 (2 created gate tests, 1 created guide, 1 modified README, 1 created gitignored UAT doc)

## Accomplishments
- Wrote `docs/GATEWAY-SETUP.md`: a copy-pasteable, own-machines-first (no public relay) runbook covering prerequisites, gateway identity, out-of-band `peer export`/`import` key exchange with fingerprint eyeball-check, starting each `famp-gateway`, and connect/verify via `famp inspect tasks --id <task_id> --json`. Every flag/command was cross-checked directly against `crates/famp-gateway/src/main.rs`'s `parse_args` and `crates/famp/src/cli/peer/{export,import}.rs`.
- Added two compiled accuracy gates: `famp-gateway::gateway_usage_doc_accuracy` (probes the no-args `usage:` stderr since `famp-gateway` has no `--help`, and dynamically extracts every `--flag` token from the guide's `famp-gateway` command examples rather than checking a fixed whitelist) and `famp::gateway_setup_doc_accuracy` (asserts `famp peer export/import --help` advertise `--as` and the subcommand names the guide uses).
- Falsified the gateway gate: injected `--bogus-flag` into a guide command example, confirmed the test FAILED with the expected "drifted from the shipping CLI" message, then reverted and confirmed green again — proving the gate is non-vacuous.
- Linked the guide from README's Quick Start federation sentence, leaving the historical `## Advanced: v0.8 federation CLI` section untouched.
- Authored `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` capturing the DOC-04 unassisted-success clause as Ben's PENDING Gate A dogfood acceptance, mirroring the `02-HUMAN-UAT.md` precedent's structure/evidence format.

## Task Commits

1. **Task 1: Write docs/GATEWAY-SETUP.md and link it from README** - `064155a` (docs)
2. **Task 2: Add the doc/binary accuracy gates (D-07)** - `9e448d9` (test)
3. **Task 3: Author 10-HUMAN-UAT.md for Ben's two-machine walkthrough** - not committed (`.planning/` is gitignored per project convention; file written to disk)

**Plan metadata:** pending final `docs(10-03): complete...` commit (skipped for `.planning/` paths — gitignored; STATE/ROADMAP/REQUIREMENTS under `.planning/` are also gitignored in this repo)

_Note: Task 2 was written and verified in a single commit rather than a separate RED/GREEN pair — the guide the gate validates already existed (Task 1) and already matched the binary, so there was no natural failing-test state to commit; non-vacuity was instead proven via the manual falsification step required by the plan's own acceptance criteria (inject a bogus flag, confirm FAIL, revert, confirm PASS)._

## Files Created/Modified
- `docs/GATEWAY-SETUP.md` - Two-machine gateway setup runbook (DOC-04)
- `crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs` - Accuracy gate for `famp-gateway`'s no-args usage stderr vs. the guide's command examples
- `crates/famp/tests/gateway_setup_doc_accuracy.rs` - Accuracy gate for `famp peer export/import --help` vs. the guide
- `README.md` - Quick Start federation sentence now links `docs/GATEWAY-SETUP.md`
- `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` - PENDING Gate A human-acceptance record (gitignored, not committed)

## Decisions Made
- The gateway accuracy gate extracts flags dynamically from the guide's fenced `famp-gateway` command examples instead of relying solely on a fixed `GATEWAY_FLAGS` whitelist, so a stray/typo'd flag in a command example is actually caught by the gate (verified via falsification), not just missing coverage of a known-good flag.
- DOC-04's human-verified "unassisted success" clause is explicitly NOT claimed done here — captured as PENDING in `10-HUMAN-UAT.md` per D-07, to be closed out by Ben's real two-machine run before milestone completion.

## Deviations from Plan

None - plan executed exactly as written. The one procedural nuance (single test-commit rather than separate RED/GREEN commits for Task 2, described above) reflects that the artifact under test already existed and already matched; it is not a deviation from the plan's acceptance criteria, which explicitly specified manual falsification (not a RED/GREEN authoring order) as the non-vacuity proof.

## Issues Encountered
- Pre-commit `rustfmt` hook caught formatting drift in both new test files (`just fmt` fixed it automatically before the Task 2 commit) — resolved inline, no scope change.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- DOC-04's automated half (guide content + binary-accuracy gates) is complete and green in `cargo nextest run --workspace` (973/973 passed) and `just lint`.
- Outstanding before `/gsd-complete-milestone` tags `v1.0.0`: Ben's real two-machine walkthrough per `10-HUMAN-UAT.md` (PENDING) — this is the milestone's Gate A dogfood and should be run before or as part of milestone close.
- No blockers for closing out Phase 10 or the remaining phase-level verification steps.

---
*Phase: 10-test-reactivation-setup-docs*
*Completed: 2026-07-27*

## Self-Check: PASSED

All created files verified present on disk; both task commit hashes (`064155a`, `9e448d9`) verified present in git log; README link to `docs/GATEWAY-SETUP.md` verified present.
