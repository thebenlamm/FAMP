# Phase 8: Signed Cross-Host Envelope + Trust Bootstrap - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.
>
> **Mode: `--auto`.** No interactive prompts. Each area was auto-resolved to the
> recommended option; the rejected alternatives are preserved below for review.

**Date:** 2026-07-23
**Phase:** 8-Signed Cross-Host Envelope + Trust Bootstrap
**Areas discussed:** Envelope shape, nonce/expiry enforcement, Trust bootstrap surface, Ingress verify + rejection

---

## Envelope shape (WIRE-02)

| Option | Description | Selected |
|--------|-------------|----------|
| One envelope, one signature (extend `famp-envelope` w/ omit-when-empty federation fields) | Single canonical signing input; local path stays byte-identical | ✓ |
| Separate `CrossHostEnvelope` wrapper with its own second signature | Nested double-sign; two canonical forms to keep byte-exact | |

**Selected:** Extend the existing envelope; single INV-10 signature (D-01, D-02, D-03).
**Notes:** Least-surprise reading of PROJECT.md "extend `famp-envelope`… without a wire break." Domain = `Principal.authority`; key_id = truncated b64url sha256 of pubkey (researcher confirms length).

---

## nonce / expiry enforcement scope

| Option | Description | Selected |
|--------|-------------|----------|
| Carry + sign both, enforce neither (format-validate only) | Proves signature+trust spine; anti-replay deferred to v1.1 | ✓ |
| Carry + sign + actively enforce expiry (reject expired) | Adds clock-skew failure mode to Phase 9 E2E | |
| Full replay cache + expiry rejection this phase | v1.1 public-internet-layer scope | |

**Selected:** Carry + sign, enforce neither (D-04).
**Notes:** Own-machines-first — keep any Phase 9 failure unambiguously in the spine, not new anti-replay state. Matches FAMP-Sec "serial-scoped negative cache is v2.0."

---

## Trust bootstrap surface (TRUST-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse peer-card + keyring line format (`famp peer export/import`) | One-line Signal-paste blob; `pin_tofu` already exists | ✓ |
| New multi-line PEM-style export + bespoke parser | Reinvents `file_format.rs`; harder to paste | |

**Selected:** Reuse `famp info` peer-card + `famp-keyring` `parse_line`/`pin_tofu` (D-05, D-06).
**Notes:** Roadmap says this phase "wires the preserved `famp-keyring`." Dedicated gateway peer keyring at `~/.famp/gateway/peers.keyring` (planner confirms path).

---

## Ingress verify + rejection semantics (WIRE-01, TRUST-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Pure `verify_inbound(bytes, &keyring)` fn; two loud reasons; no state on reject | Protocol-grade boundary; unit-testable; Phase 9 wires transport | ✓ |
| Chat-grade: strip envelope, re-inject into crypto-dropped local bus | Contradicts locked protocol-grade decision | |
| Single flat "rejected" error | Reproduces v0.9 flat-error incident class | |

**Selected:** Pure verify fn, `invalid_signature` vs `unpinned_key` split, zero state on reject (D-07, D-08).
**Notes:** Locked in Recent Decisions (protocol-grade ingress). Data-as-input; ChildGuard for any child-spawning tests.

---

## Claude's Discretion

- Exact `key_id` derivation + truncation length (researcher/planner).
- Exact on-disk peer-keyring path (planner, vs `paths.rs`).
- Final CLI noun/verb spelling — recommend top-level `famp peer` mirroring `famp info`.

## Deferred Ideas

- Active nonce/replay cache + expiry rejection → v1.1.
- Public-internet dumb relay → v1.1.
- Cross-person trust + signed peer directory → v1.1.
- FAMP-Sec capability/approval/tool-admission plane → v2.0+.
- Live two-process HTTP cross-host cycle → Phase 9 (GW-01/02/03).
- Deferred federation test triage (`_deferred_v1/`) → Phase 10.
