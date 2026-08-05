---
phase: 20
slug: human-acceptance-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-05
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests (`cargo test`) plus POSIX shell checks and blocking human verification |
| **Config file** | root `Cargo.toml`, `crates/famp/Cargo.toml`, `justfile` |
| **Quick run command** | `cargo test -p famp --test follower_setup_doc_accuracy` |
| **Full suite command** | `cargo test --workspace --no-fail-fast` plus clean-box rehearsal; UAT is a separate checkpoint |
| **Estimated runtime** | ~10 seconds quick; workspace and external runs are environment-dependent |

## Sampling Rate

- **After every task commit:** Run the narrow test or shell check named by that task.
- **After every plan wave:** Run `cargo test -p famp --test follower_setup_doc_accuracy` plus all focused preflight/evidence validators created in the wave.
- **Before `$gsd-verify-work`:** Full workspace suite, clean-box rehearsal, and real-person evidence review must be green.
- **Max feedback latency:** 30 seconds for repository-local edits; external rehearsal and UAT are explicit blocking checkpoints.

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 20-01-01 | 01 | 1 | DOC-06 | T-20-01 | Guide uses pairing, consent, explicit remote processing, and receiver proof without legacy key copy | integration | `cargo test -p famp --test follower_setup_doc_accuracy` | ❌ W0 | ⬜ pending |
| 20-01-02 | 01 | 1 | DOC-07 | Read-only clean-box predicate discriminates every contamination class without mutation or path leakage | shell/integration | `cargo test -p famp --test phase20_clean_box_preflight` | ❌ W0 | ⬜ pending |
| 20-01-03 | 01 | 1 | UAT-02, PAIR-05 | Blank templates and validator enforce owners, UTC timestamps, redacted OS/architecture and installed `famp`/`famp-gateway` versions for both machines, three outcomes, distinct tasks, and seven-message review | integration | `cargo test -p famp --test phase20_evidence_schema && ! scripts/phase20-evidence-check.sh rehearsal .planning/phases/20-human-acceptance-gate/20-REHEARSAL-TEMPLATE.md` | ❌ W0 | ⬜ pending |
| 20-02-01 | 02 | 2 | DOC-06, DOC-07 | Repository-only guide candidate is green and frozen while external fields remain unresolved | integration | `cargo test -p famp --test follower_setup_doc_accuracy --test phase20_clean_box_preflight --test phase20_evidence_schema --test pair_cli --test gateway_setup_doc_accuracy && cargo test -p famp-gateway --test e2e_relay_bidirectional && git diff --exit-code -- docs/FOLLOWER-SETUP.md && sha256sum docs/FOLLOWER-SETUP.md` | ❌ W0 | ⬜ pending |
| 20-02-02 | 02 | 2 | DOC-07, UAT-02 | Blocking owner checkpoint supplies clean-host/reachability/topology provenance and bidirectional receiver proof | checkpoint + integration | `cargo test -p famp --test phase20_evidence_schema && scripts/phase20-evidence-check.sh rehearsal .planning/phases/20-human-acceptance-gate/20-REHEARSAL.md` plus human provenance/redaction review | ❌ external | ⬜ pending |
| 20-03-01 | 03 | 3 | DOC-06, DOC-07, UAT-02 | Rehearsed guide and repository suite are green while participant/topology/reachability fields remain unresolved | integration | `cargo test -p famp --test follower_setup_doc_accuracy --test phase20_clean_box_preflight --test phase20_evidence_schema && cargo test --workspace --no-fail-fast && sha256sum docs/FOLLOWER-SETUP.md && scripts/phase20-evidence-check.sh rehearsal .planning/phases/20-human-acceptance-gate/20-REHEARSAL.md` | ❌ W0/external prerequisite | ⬜ pending |
| 20-03-02 | 03 | 3 | UAT-02, PAIR-05 | Blocking participant checkpoint supplies independence/no-coaching facts, separately owned UTC-timestamped redacted OS/architecture and installed `famp`/`famp-gateway` versions for both machines, both receiver proofs, and seven first paraphrases | checkpoint + integration | `cargo test -p famp --test follower_setup_doc_accuracy --test phase20_evidence_schema && scripts/phase20-evidence-check.sh acceptance .planning/phases/20-human-acceptance-gate/20-ACCEPTANCE.md` plus human provenance/redaction review | ❌ human | ⬜ pending |
| 20-03-03 | 03 | 3 | DOC-06, DOC-07, UAT-02, PAIR-05 | Closeout accepts only validator-green, human-reviewed passing evidence including owner/time/redacted OS/architecture and installed `famp`/`famp-gateway` versions for both machines | integration + human review | `cargo test -p famp --test follower_setup_doc_accuracy --test phase20_clean_box_preflight --test phase20_evidence_schema && cargo test --workspace --no-fail-fast && scripts/phase20-evidence-check.sh rehearsal .planning/phases/20-human-acceptance-gate/20-REHEARSAL.md && scripts/phase20-evidence-check.sh acceptance .planning/phases/20-human-acceptance-gate/20-ACCEPTANCE.md` | ❌ external | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

## Wave 0 Requirements

- [ ] `docs/FOLLOWER-SETUP.md` — DOC-06 linear guide.
- [ ] `crates/famp/tests/follower_setup_doc_accuracy.rs` — semantic role/order/CLI gate.
- [ ] `scripts/phase20-clean-box-preflight.sh` plus focused tests — fail-closed clean-state assertion.
- [ ] `20-REHEARSAL-TEMPLATE.md` and `20-ACCEPTANCE-TEMPLATE.md` — evidence schema and outcome classification.
- [ ] Automated synchronization/shape checks for the seven pairing comprehension prompts.

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Fresh supported machine has no prior FAMP state or Rust and completes the exact release-binary guide | DOC-07 | Host history, OS integration, network, and installer execution cannot be proven by repository-local tests | Capture pre-install preflight, follow the frozen guide exactly, record pairing/readiness/two receiver-terminal results, redact, and classify the rehearsal. |
| Second person completes the event unassisted on an independent network | UAT-02 | Personhood, independent administration, machine details, installed binary versions, no coaching, and network independence are external facts | Use the frozen guide and artifact; for both Ben and follower machines capture owner-attributed UTC-timestamped redacted OS/architecture and installed `famp`/`famp-gateway` versions; log questions; capture two receiver-owned terminal JSON results and both attestations; classify pass/failure/invalid. |
| Second person explains the next action for all seven pairing errors | PAIR-05 | Human comprehension has no mechanical proxy | Present each synchronized message without explanation or live fault injection, ask the neutral prompt, and record the person's first paraphrase and pass/fail. |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verification or explicit external/human checkpoints.
- [ ] Sampling continuity: no three consecutive repository tasks without an automated check.
- [ ] Wave 0 covers all missing test/evidence infrastructure.
- [ ] No watch-mode flags.
- [ ] Repository-local feedback latency is under 30 seconds.
- [ ] Clean-box rehearsal completed before the live invite is created.
- [ ] `nyquist_compliant: true` and `wave_0_complete: true` set only after the listed infrastructure exists and passes.

**Approval:** pending
