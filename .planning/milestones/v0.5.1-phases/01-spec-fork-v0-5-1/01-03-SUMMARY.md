---
phase: 01-spec-fork-v0-5-1
plan: 03
subsystem: spec-fork/identity
tags: [spec, identity, agent-card, key-rotation, breaking-change]
requires:
  - 01-01 (Wave 0 scaffold — §6.1/§6.3 stubs)
provides:
  - "§6.1 normative Agent Card shape (federation_credential + federation_signature)"
  - "§6.3 card versioning rules (card_version, min_compatible_version, rotation survival)"
affects:
  - "Phase 2 famp-crypto trust-list interface (consumes federation_credential lookup)"
  - "Phase 4 famp-identity (consumes full §6.1 card shape — BREAKING vs v0.5)"
  - "Phase 5 famp-fsm (consumes §6.3 rotation survival clause, links to §11.2a)"
tech-stack:
  added: []
  patterns:
    - "federation-scoped credential signing (replaces circular self-signature)"
    - "commit-time card_version binding with min_compatible_version survival gate"
key-files:
  created: []
  modified:
    - FAMP-v0.5.1-spec.md  # §6.1 Agent Card (revised), §6.3 Card versioning, Δ11/Δ12 changelog
decisions:
  - "Use federation_signature (not generic signature) as the signature field name on cards — resolves RESEARCH §8 gap 2 naming ambiguity"
  - "federation_credential is opaque to FAMP; trust-list lookup is federation-specific"
  - "min_compatible_version > card_version is malformed and rejected at parse"
  - "Repudiation on rotation via min_compatible_version = N+1, error cause card_rotation_repudiated"
metrics:
  duration: "~15 min"
  completed: 2026-04-12
---

# Phase 01 Plan 03: Agent Card & Card Versioning Summary

**One-liner:** Resolved v0.5's circular Agent Card self-signature by introducing `federation_credential` + `federation_signature`, and pinned `card_version` / `min_compatible_version` rotation semantics so in-flight commits survive key rotation deterministically.

## What Was Built

### §6.1 Agent Card (revised) — SPEC-05

Replaced the v0.5 `signature` (self-signed) field with two required fields:

- **`federation_credential`** — opaque string identifier that resolves via the federation trust list to the Ed25519 public key used to sign the card. Format is federation-specific; FAMP treats it as an opaque byte-string lookup key.
- **`federation_signature`** — 86-char unpadded base64url Ed25519 signature over the canonical JSON of the card body *with `federation_signature` omitted*.

Added a normative 7-step verification procedure (parse → extract → omit → canonicalize per §4a → trust-list lookup → `verify_strict` per §7.1b with §7.1a prefix → reject with `unauthorized` on any failure).

Made explicit that `public_key` (agent's own envelope-signing key) and `federation_credential` (card issuer) are distinct and MUST NOT be assumed equal.

**BREAKING CHANGE from v0.5.** v0.5 cards MUST be rejected by v0.5.1 verifiers and vice versa. Flagged in changelog Δ11 and in §6.1 body so downstream Phase 4 (`famp-identity`) planner cannot miss it.

### §6.3 Card versioning — SPEC-06

Pinned the two integer version fields:

- `card_version`: monotonic non-negative integer, strictly increasing on every publication.
- `min_compatible_version`: non-negative integer, MUST be ≤ `card_version`. A card with `min_compat > card_version` is malformed and rejected at parse.

**Rotation survival clause:** in-flight commits bound to card `N` survive rotation to `N+1` iff the new card's `min_compatible_version ≤ N`.

**Repudiation path:** issuer sets `min_compatible_version = N+1` on the new card; receivers then reject any bound operation referencing card `≤ N` with `unauthorized` / `card_rotation_repudiated`.

**Fresh vs bound:** fresh requests use latest card; bound operations use the card version captured at commit time per §11.2a (D-24 cross-reference forward to Plan 04).

Included a worked timeline example (t0..t3) showing both survival and repudiation outcomes.

## Deviations from Plan

None — both tasks executed exactly as the plan specified. The parallel-executor race with Plan 02 (both editing `FAMP-v0.5.1-spec.md`) was handled by using `git stash` to isolate Task 1 and Task 2 commits atomically.

### Parallel-execution note

Plan 02 committed Δ04–Δ10, Δ14, Δ15, Δ25 to the changelog concurrently. This plan inserted Δ11 and Δ12 without collision (Δ11 slot was reserved for §6.1 by the plan, Δ12 for §6.3). The RESEARCH §6 table used a different Δ numbering (Δ11=§7.1c, Δ12=§6.1, Δ13=§6.3); the PLAN's numbering supersedes RESEARCH per CONTEXT D-03 process.

## Changelog Entries Added

- `v0.5.1-Δ11` — §6.1 Agent Card — BREAKING: federation_credential + federation_signature replace self-sig
- `v0.5.1-Δ12` — §6.3 Card versioning — card_version / min_compatible_version rotation survival pinned

## Verification

All plan-specified anchors pass:

```
rg -q 'federation_credential' FAMP-v0.5.1-spec.md      # OK
rg -q 'federation_signature'  FAMP-v0.5.1-spec.md      # OK
rg -q 'NOT self-signed'       FAMP-v0.5.1-spec.md      # OK
rg -q 'v0\.5\.1-Δ11'          FAMP-v0.5.1-spec.md      # OK
rg -q 'card_version'          FAMP-v0.5.1-spec.md      # OK
rg -q 'min_compatible_version' FAMP-v0.5.1-spec.md     # OK
rg -q '§11\.2a'               FAMP-v0.5.1-spec.md      # OK
rg -q 'v0\.5\.1-Δ12'          FAMP-v0.5.1-spec.md      # OK
```

Scope boundary respected: edits confined to §6.1 and §6.3 sub-sections plus two changelog entries. No changes to §3.6a, §4a, §7.1.x, §13.x, or other Plan 02 territory.

## Commits

- `73f7bc9` — feat(01-03): §6.1 Agent Card federation credential (SPEC-05)
- `6852728` — feat(01-03): §6.3 card versioning + rotation semantics (SPEC-06)

## Known Stubs

None. Both §6.1 and §6.3 are fully populated normative text. Forward references to §11.2a (Plan 04) and §8a (Plan 05) are intentional and documented as cross-references, not stubs.

## Downstream Consumers

- **Phase 2 `famp-crypto`** — trust-list interface must expose `fn lookup(federation_credential: &str) -> Option<VerifyingKey>`.
- **Phase 4 `famp-identity`** — `AgentCard` struct must include all 12 required fields from §6.1.1; `signature` field from v0.5 must NOT appear. Breaking-change banner is load-bearing for migration guidance.
- **Phase 5 `famp-fsm`** — the rotation survival clause in §6.3.2 and the commit-time binding in §11.2a (Plan 04) together define commit resolution across rotation; both must be implemented together.

## Self-Check: PASSED

- Files modified exist: `FAMP-v0.5.1-spec.md` ✓
- Commits exist in git log: `73f7bc9`, `6852728` ✓
- All plan success criteria anchors pass `rg -q` ✓
- No edits outside §6.x sections ✓
