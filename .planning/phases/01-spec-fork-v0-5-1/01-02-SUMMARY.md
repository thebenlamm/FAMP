---
phase: 01-spec-fork-v0-5-1
plan: 02
subsystem: spec
tags: [spec, rfc-8785, ed25519, canonical-json, idempotency, artifact-ids]
requires: [01-01]
provides:
  - "§4a Canonical JSON normative body (RFC 8785 JCS)"
  - "§4a.1 Example A (ASCII mixed-case) worked canonical-JSON bytes"
  - "§4a.2 Example B (U+1F600 supplementary-plane) worked canonical-JSON bytes"
  - "§7.1a Domain separation prefix (FAMP-sig-v1\\0)"
  - "§7.1b Ed25519 encoding (raw 32/64, unpadded base64url, verify_strict)"
  - "§7.1 recipient-binding amendment (signature binds `to`)"
  - "§13.1 Freshness window (±60s skew, 300s validity, federation caps)"
  - "§13.2 Idempotency (128-bit, 22-char, (sender,recipient) scope)"
  - "§3.6a Artifact identifiers (sha256:<hex> over canonical JSON)"
affects: [FAMP-v0.5.1-spec.md]
tech-stack:
  added: ["Python jcs 0.2.1 (one-off, external JCS reference; not committed)"]
  patterns: ["External JCS reference for byte-exact worked examples (PITFALLS P10)"]
key-files:
  modified:
    - FAMP-v0.5.1-spec.md
decisions:
  - "D-05/D-08 applied verbatim in §4a as RFC 8785 citations; no paraphrase"
  - "D-07 LOCKED: Examples A and B both included with jcs 0.2.1-generated hex"
  - "D-09 prefix `FAMP-sig-v1\\0` specified with literal hex byte row"
  - "D-11 recipient anti-replay made normative in §7.1 amendment"
  - "D-12 unpadded base64url + verify_strict locked in §7.1b"
  - "D-15 federation caps ±300s/1800s documented"
  - "D-16 idempotency tuple (id, idempotency_key, content_hash) normative"
  - "D-28 `sha<N>:` reserved but only sha256 accepted"
metrics:
  duration: "~20 minutes"
  tasks: 4
  files-changed: 1
  completed-date: 2026-04-12
---

# Phase 1 Plan 02: Canonical JSON + Ed25519 + Defaults Summary

**One-liner:** Wave-1 foundations: seven normative spec sub-sections plus two D-07-mandated worked canonical-JSON examples pasted byte-exact from Python `jcs 0.2.1`, locking RFC 8785, Ed25519 encoding, recipient binding, numeric defaults, idempotency format, and artifact-ID scheme in `FAMP-v0.5.1-spec.md`.

## Sub-sections Rewritten

| Section | Content | Commit |
| --- | --- | --- |
| §4a Canonical JSON | RFC 8785 §3.2.3 and §3.2.2.3 pull-quotes, duplicate-key rejection, no-Unicode-normalization, forbidden serde features | `67772f5` |
| §7.1 amendment | Recipient anti-replay (signature binds `to`) | `d211c78` |
| §7.1a Domain separation | Fixed prefix `FAMP-sig-v1\0` (12 bytes, hex `46 41 4d 50 2d 73 69 67 2d 76 31 00`); signing formula; rationale | `d211c78` |
| §7.1b Ed25519 encoding | RFC 8032 §5.1.2/§5.1.6/§5.1.7, RFC 4648 §5 unpadded base64url, decoder rejection list, verify_strict, weak-key rejection | `d211c78` |
| §13.1 Freshness | ±60s skew, 300s validity (RECOMMENDED); federation caps ±300s/1800s MUST NOT exceed; δ=60s guard band | `104de81` |
| §13.2 Idempotency | 128-bit random, 22-char unpadded base64url; scope `(sender, recipient)`; replay-cache tuple `(id, idempotency_key, content_hash)`; bounded + non-attacker-controllable eviction | `104de81` |
| §3.6a Artifact IDs | `sha256:<hex>` (64 lowercase hex) over canonical JSON of artifact body; `sha<N>:` reserved | `104de81` |
| §4a.1 Example A | `{"Zeta":1,"alpha":2,"Beta":3}` → `{"Beta":3,"Zeta":1,"alpha":2}`; hex `7b2242657461223a332c225a657461223a312c22616c706861223a327d` (29 bytes) | `2ce8df4` |
| §4a.2 Example B | `{"a":1,"😀":2,"z":3}` → `{"a":1,"z":3,"😀":2}`; hex `7b2261223a312c227a223a332c22f09f9880223a327d` (22 bytes) | `2ce8df4` |

## RFC Citations Added

- **RFC 8785 §3.1** — duplicate-key rejection (MUST upgrade over RFC 8259 §4 SHOULD)
- **RFC 8785 §3.2.3** — UTF-16 code-unit key sort
- **RFC 8785 §3.2.2.3** — ECMAScript `Number.prototype.toString` number formatting
- **RFC 8785 §6** — large-integer-as-string guidance
- **RFC 8032 §5.1.2** — 32-byte public-key encoding
- **RFC 8032 §5.1.6** — 64-byte signature (`R || S`)
- **RFC 8032 §5.1.7** — strict verification
- **RFC 4648 §5** — unpadded base64url alphabet (Table 2)
- **RFC 8259 §4** — reference for duplicate-key precedent

## Worked-Example Provenance

- **Tool:** Python `jcs 0.2.1` (installed in `/tmp/jcs-venv`, not committed)
- **Script:** `/tmp/compute-jcs-examples.py` (throwaway, removed after use; not committed per PITFALLS P10 rule that spec text holds the authoritative bytes)
- **Cross-check:** Phase 8 will independently regenerate these bytes using a second external JCS implementation; divergence will be treated as a spec bug.

## Changelog Entries Appended

| ID | Section | Summary |
| --- | --- | --- |
| Δ04 | §4a | RFC 8785 normative; §3.2.3/§3.2.2.3 pull-quotes; duplicate-key / no-Unicode-normalization |
| Δ05 | §4a | Forbid `arbitrary_precision` / `preserve_order`; reject NaN/±Infinity; int > 2^53 as strings |
| Δ06 | §4a.1 | Example A worked bytes (D-07 LOCKED, jcs 0.2.1) |
| Δ07 | §4a.2 | Example B worked bytes (D-07 LOCKED, jcs 0.2.1) |
| Δ08 | §7.1a | Domain-separation prefix `FAMP-sig-v1\x00` |
| Δ09 | §7.1 | Recipient anti-replay normative |
| Δ10 | §7.1b | Ed25519 encoding, unpadded base64url, verify_strict |
| Δ14 | §13.1 | ±60s / 300s RECOMMENDED with federation caps |
| Δ15 | §13.2 | 128-bit idempotency key, (sender,recipient) scope |
| Δ25 | §3.6a | Artifact IDs `sha256:<hex>` over canonical JSON |

## Commits

- `67772f5` feat(01-02): §4a canonical JSON normative body (RFC 8785)
- `d211c78` feat(01-02): §7.1a domain separation, §7.1b Ed25519, §7.1 recipient binding
- `104de81` feat(01-02): §13.1 freshness, §13.2 idempotency, §3.6a artifact IDs
- `2ce8df4` feat(01-02): §4a.1/§4a.2 worked canonical-JSON examples (D-07 LOCKED)

## Requirements Satisfied

SPEC-02, SPEC-03, SPEC-04, SPEC-07, SPEC-08, SPEC-18, SPEC-19.

## Deviations from Plan

None — plan executed exactly as written. All four tasks used the external `jcs 0.2.1` reference implementation as mandated by PITFALLS P10. Commits used `--no-verify` per the parallel-executor contract (Plan 01-03 running concurrently; orchestrator validates hooks once after wave).

## Scope Boundary

Did not touch §6.1 or §6.3 (those belong to Plan 01-03 running in parallel). Did not touch §7.1c worked Ed25519 signature example (Plan 06). Did not touch state-machine sections §9/§10/§11/§12 (Plan 04) or body schemas §8 (Plan 05).

## Self-Check: PASSED

- FOUND file: `FAMP-v0.5.1-spec.md`
- FOUND commit: `67772f5` (§4a)
- FOUND commit: `d211c78` (§7.1a/b, recipient binding)
- FOUND commit: `104de81` (§13.1/§13.2/§3.6a)
- FOUND commit: `2ce8df4` (§4a.1/§4a.2)
- GREP anchors verified: `RFC 8785`, `FAMP-sig-v1`, `recipient`, `unpadded base64url`, `sha256:<hex>`, `±60`, `300 seconds`, `idempotency.{0,30}128-bit`, `U+1F600`, `Example A`, `Example B`, `v0.5.1-Δ04..Δ10, Δ14/Δ15/Δ25`
- Throwaway script absent from tracked files: NOT-COMMITTED
