---
phase: 1
slug: spec-fork-v0-5-1
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-13
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for a **docs-only** phase: the deliverable is `FAMP-v0.5.1-spec.md`, so validation is grep-based anchor presence plus RFC citation integrity.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `just` + `ripgrep` (no test runner — docs phase) |
| **Config file** | `Justfile` recipe `spec-lint` (Wave 0 installs) |
| **Quick run command** | `just spec-lint` |
| **Full suite command** | `just spec-lint && just ci` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `just spec-lint`
- **After every plan wave:** Run `just spec-lint && just ci`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 2 seconds

---

## Per-Task Verification Map

Every SPEC-xx requirement maps to one grep-anchor check in `just spec-lint`. The check command runs `rg -q '<anchor>' FAMP-v0.5.1-spec.md` and exits non-zero if missing. Task IDs are placeholder — gsd-planner will finalize mapping when plans are created.

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-00-01 | 00 | 0 | scaffold | scaffold | `test -f FAMP-v0.5.1-spec.md && test -f Justfile` | ❌ W0 | ⬜ pending |
| 1-00-02 | 00 | 0 | lint recipe | scaffold | `just --list \| rg -q 'spec-lint'` | ❌ W0 | ⬜ pending |
| 1-xx-01 | TBD | 1 | SPEC-01 | anchor | `rg -q 'v0.5.1 Changelog' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-02 | TBD | 1 | SPEC-02 | anchor | `rg -q 'RFC 8785' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-03 | TBD | 1 | SPEC-03 | anchor | `rg -q 'FAMP-sig-v1' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-04 | TBD | 1 | SPEC-04 | anchor | `rg -q 'recipient.{0,20}anti-replay\|binds.{0,10}\`to\`' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-05 | TBD | 1 | SPEC-05 | anchor | `rg -q 'federation_credential' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-06 | TBD | 1 | SPEC-06 | anchor | `rg -q 'card_version' FAMP-v0.5.1-spec.md && rg -q 'min_compatible_version' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-07 | TBD | 1 | SPEC-07 | anchor | `rg -q '±60' FAMP-v0.5.1-spec.md && rg -q '300.{0,10}seconds' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-08 | TBD | 1 | SPEC-08 | anchor | `rg -q 'idempotency.{0,30}128-bit' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-09 | TBD | 1 | SPEC-09 | anchor | `rg -q 'ack.disposition.{0,50}terminal' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-10 | TBD | 1 | SPEC-10 | anchor | `rg -q 'envelope-level.{0,20}whitelist\|FSM.{0,20}inspects' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-11 | TBD | 1 | SPEC-11 | anchor | `rg -q 'transfer.{0,10}timeout.{0,20}tiebreak' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-12 | TBD | 1 | SPEC-12 | anchor | `rg -q 'EXPIRED.{0,20}deliver' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-13 | TBD | 1 | SPEC-13 | anchor | `rg -q 'conditional.{0,10}lapse' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-14 | TBD | 1 | SPEC-14 | anchor | `rg -q 'COMMITTED_PENDING_RESOLUTION' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-15 | TBD | 1 | SPEC-15 | anchor | `rg -q 'supersession.{0,30}round' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-16 | TBD | 1 | SPEC-16 | anchor | `rg -q 'capability.{0,20}snapshot.{0,20}commit-time' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-17 | TBD | 1 | SPEC-17 | anchor | `for b in commit propose deliver control delegate; do rg -q "\`$b\` body" FAMP-v0.5.1-spec.md \|\| exit 1; done` | ⬜ | ⬜ pending |
| 1-xx-18 | TBD | 1 | SPEC-18 | anchor | `rg -q 'sha256:<hex>' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-19 | TBD | 1 | SPEC-19 | anchor | `rg -q 'unpadded base64url' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-20 | TBD | 1 | SPEC-20 | anchor | `rg -q 'FAMP_SPEC_VERSION\s*=\s*"0\.5\.1"' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-VEC | TBD | 2 | worked example | byte-check | `rg -q 'ed25519.{0,30}vector.{0,30}#1' FAMP-v0.5.1-spec.md && rg -q '[0-9a-f]{128}' FAMP-v0.5.1-spec.md` | ⬜ | ⬜ pending |
| 1-xx-CHG | TBD | 2 | SPEC-01 delta list | structure | `rg -c 'v0\.5\.1-Δ[0-9]{2}' FAMP-v0.5.1-spec.md \| awk -F: '$2 >= 20 {exit 0} {exit 1}'` | ⬜ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `FAMP-v0.5.1-spec.md` — empty/stub file at repo root (placeholder header + spec-version constant block) so every subsequent `rg` check has a target
- [ ] `Justfile` recipe `spec-lint` — runs the ripgrep anchor list above; exits non-zero on first missing anchor
- [ ] `Justfile` recipe `ci` updated to depend on `spec-lint` so CI fails when the spec fork is missing anchors
- [ ] `.github/workflows/ci.yml` — confirms `just ci` picks up the new recipe (no workflow change expected, just verification)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Prose clarity / RFC 2119 normative tone in rewritten sections | SPEC-09..SPEC-16 | Subjective editorial quality cannot be grepped | Human reviewer reads each rewritten section, confirms MUST/SHOULD/MAY usage is correct and unambiguous |
| Cross-reference integrity (no broken `§N.M` pointers after sub-section additions) | SPEC-01 | Section anchors can be grepped individually but broken-link detection needs a reader | Human scan of changelog entries against the sections they cite |
| Changelog entry justification quality | SPEC-01 | Each Δ entry's "resolution summary" must match the PITFALLS finding it cites — content comparison | Human diff of changelog vs `docs/PITFALLS.md` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (spec file + Justfile recipe)
- [ ] No watch-mode flags
- [ ] Feedback latency < 2s
- [ ] `nyquist_compliant: true` set in frontmatter once gsd-planner finalizes task IDs

**Approval:** pending
