---
phase: quick-260729-sw0
plan: 01
subsystem: docs
tags: [docs, versioning, v1.0]
dependency-graph:
  requires: []
  provides: [reconciled-post-v1.0-doc-framing]
  affects: [README.md, ARCHITECTURE.md, CLAUDE.md]
tech-stack:
  added: []
  patterns: [doc-version-reconciliation]
key-files:
  created: []
  modified:
    - README.md
    - ARCHITECTURE.md
    - CLAUDE.md
decisions:
  - "R7 (replay-defense bullet narrowing) applied — grep for seen_nonce/replay_cache/nonce_cache/nonce_seen in crates/famp-gateway/src/ returned no matches"
  - "Executed as three atomic per-task commits (task_commit_protocol) rather than one combined diff; the plan's mid-task verify greps that assumed cumulative uncommitted state (e.g. Task 2's 'ARCHITECTURE.md,README.md,' check) were re-run as a cumulative check at Task 3 instead, against pre-plan HEAD 6b31f93"
metrics:
  duration: ~25min
  completed: 2026-07-29
status: complete
---

# Quick Task 260729-sw0: Reconcile post-v1.0.0 doc version framing Summary

Reconciled README.md, ARCHITECTURE.md, and CLAUDE.md — all three still described
v0.11 as the current runtime and the v1.0 federation gateway as a future,
trigger-gated milestone, six months after `v1.0.0` shipped and tagged at `5edff41`
(2026-07-29). Applied all 18 verbatim edits from the plan exactly as specified.

## STEP 1 — Frozen-anchor audit (verbatim output, run before any edit)

```
=== audit 1 (README|ARCHITECTURE.md|CLAUDE.md refs in crates/, excluding _deferred_v1) ===
crates/famp-crypto/src/lib.rs:31://! unsigned rejected") for the protocol framing. See `README.md` for the
crates/famp-crypto/src/hash.rs:9://! See `README.md` `## Content addressing (CRYPTO-07)` and
crates/famp-gateway/tests/principal_send_drain.rs:60:/// class that never fires the task FSM — CLAUDE.md). `AnyBusEnvelope::decode`
crates/famp/tests/gateway_setup_doc_accuracy.rs:14, 17, 39, 321, 332 (comments + fixture path)
crates/famp/tests/readme_line_count_gate.rs:1, 19, 21, 26, 46, 74, 78, 91, 95
crates/famp/tests/hook_runner_path_parity.rs:124  <-- investigated separately, see below
crates/famp-taskdir/src/error.rs:50://! Variants are narrow per CLAUDE.md ("phase-appropriate error enums").

=== audit 2 (readme.contains / fence_body.contains assertions) ===
gateway_setup_doc_accuracy.rs:331  readme.contains("Federation gateway (v1.0, shipped)")
readme_line_count_gate.rs:21, 52 (cargo install famp), 56 (famp install-claude-code),
  60 (/famp-register), 77 (!brew install famp), 94 (!/famp-msg)

=== audit 3 (Justfile / scripts/) ===
(no output — zero hits)
```

`hook_runner_path_parity.rs:124` (`home.join("README.md")`) investigated and
confirmed NOT a repo-root doc reader — it writes a synthetic placeholder file
(`"x"` content) into a temp `$HOME` fixture to test the Stop-hook shim's path
handling; unrelated to the two real doc-gate tests. Audit confirms the plan's
frozen-string table (`gateway_setup_doc_accuracy.rs`, `readme_line_count_gate.rs`)
is complete — no undocumented test-asserted doc string was found.

## R7 guard grep result

`grep -rn "seen_nonce\|replay_cache\|nonce_cache\|nonce_seen" crates/famp-gateway/src/`
returned **no matches** (exit 1) — R7 was **applied**: the replay-defense bullet
in README.md's "Not Shipped Yet" section was narrowed to state the envelope
carries `nonce`/`expiry` and is format-validated at ingress, but no seen-nonce
cache rejects a re-sent envelope.

## Edits applied

- **Task 1 (README.md, 9 edits R1-R9):** status line, lead paragraph, local-bus/
  federation profile bullets, inbox-list bullet, "Not Shipped Yet" label + replay
  bullet, v0.8 escape-hatch paragraph, Current Milestones tail. Frozen anchors
  (`Federation gateway (v1.0, shipped)` bullet, Quick Start fence, `brew install
  famp` / `/famp-msg` negatives) verified byte-identical post-edit via `git diff`
  grep — zero matches on any of those literals on a +/- line.
- **Task 2 (ARCHITECTURE.md, 9 edits A1-A9):** `include_terminal` deferral wording,
  v0.9 section header tense, MCP contract sentence tense, layer table row 2, layer-
  status paragraph tense, new `## v1.0 — federation gateway (shipped at the
  v1.0.0 tag, 2026-07-29)` section (wire/trust/addressing/scope), transport-crate
  guidance, crate-count sentence (15→16, v0.11.0→v1.0.0), new `famp-gateway`
  table row.
- **Task 3 (CLAUDE.md, 4 edits C1-C4):** conformance-target constraint (vector
  pack did not ship in v1.0, Gate B named open), Architecture lead paragraph
  (local-first AND federated, v1.0 shipped), v0.11 paragraph label (dropped
  "current runtime"), superseded "v1.0 readiness trigger (named)" paragraph
  replaced with a shipped-v1.0 summary (Gate A closed, Gate B open, next
  milestone not yet defined).

## Verification

- `cargo build -p famp --bin famp`: succeeds.
- `cargo test -p famp --test readme_line_count_gate`: `test result: ok. 3 passed`
  (non-zero, matches expected N).
- `cargo test -p famp --test gateway_setup_doc_accuracy`: `test result: ok. 1
  passed` (non-zero, matches expected N).
- `git diff --name-only 6b31f93 HEAD | sort` (cumulative, against pre-plan HEAD):
  exactly `ARCHITECTURE.md`, `CLAUDE.md`, `README.md`.
- `git status --porcelain`: empty (clean tree post-commit).
- `grep -cE '^\| \`famp' ARCHITECTURE.md`: `16` — crate table has 16 data rows
  including the new `famp-gateway` row.
- Frozen-literal re-check across the full cumulative README.md diff
  (`git diff 6b31f93 HEAD -- README.md | grep -E '...'`): zero matches — no
  frozen anchor appears on any +/- line across all three README commits.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written for all 18 edits.

### Execution-mode adaptation (not a deviation from content, only from verify-command literalism)

The task_commit_protocol mandates one atomic commit per task. The plan's
per-task `<verify>` blocks were written assuming all edits landed as one
uncommitted diff (e.g. Task 2's automated check literally expects
`git diff --name-only` to equal `ARCHITECTURE.md,README.md,` — true only if
README.md is still unstaged). Since README.md was already committed after
Task 1, `git diff --name-only` after Task 2 correctly showed only
`ARCHITECTURE.md`. Re-ran the boundary proof cumulatively at the end (`git diff
--name-only 6b31f93 HEAD`, where `6b31f93` is the pre-plan HEAD) — confirms
exactly the three target files with no fourth path, satisfying the same intent
the plan's per-task check was protecting.

## Out-of-scope stale items (recorded, not fixed — follow-up quick task candidates)

1. **README.md "exposes eight tools"** (~line 300 pre-edit) — the MCP registry
   has twelve descriptors (an anti-drift unit test already exists for the
   count). A count error, not version framing — out of scope per plan.
2. **README.md "## Repo Layout"** (~lines 704-713 pre-edit) — omits `famp-bus`
   (v0.9), the three `famp-inspect-*` crates (v0.10), `famp-inbox`,
   `famp-taskdir`, and `famp-gateway` (v1.0). Stale across three milestones,
   not a v1.0-framing artifact — out of scope per plan.
3. **`crates/famp-gateway/Cargo.toml:9`** — package description still says
   "(Layer 2 skeleton)", stale since the gateway shipped. Cargo.toml was an
   explicit hard prohibition in this plan (three-file constraint).

## GSD-managed-block drift warning (CLAUDE.md C1)

C1 (conformance-target edit) lands inside the `<!-- GSD:project-start
source:PROJECT.md -->` managed block (CLAUDE.md:1-19), and Task 2's
ARCHITECTURE.md edits are the durable source feeding the `<!--
GSD:architecture-start source:ARCHITECTURE.md -->` block (CLAUDE.md:54-107) for
C2-C4. A future `/gsd-docs-update` regeneration from `PROJECT.md` could revert
C1 specifically, since PROJECT.md itself was not edited (out of scope — this
task's constraint was exactly three files: README.md, ARCHITECTURE.md,
CLAUDE.md). If PROJECT.md's conformance-target line is not separately updated,
the next doc regeneration may reintroduce the stale "vector pack ships in v1.0"
claim into CLAUDE.md.

## Known Stubs

None — this is a documentation-only reconciliation; no code stubs introduced.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema
changes. All edits are prose reconciling documentation with already-shipped
(and already-tested) v1.0.0 behavior.

## Self-Check: PASSED

- `README.md` FOUND (git diff confirms modifications, commit `27d584d`)
- `ARCHITECTURE.md` FOUND (git diff confirms modifications, commit `5252373`)
- `CLAUDE.md` FOUND (git diff confirms modifications, commit `ba884ed`)
- Commit `27d584d` FOUND in `git log --oneline`
- Commit `5252373` FOUND in `git log --oneline`
- Commit `ba884ed` FOUND in `git log --oneline`
