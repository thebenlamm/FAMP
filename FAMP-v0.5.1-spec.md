# FAMP — Federated Agent Messaging Protocol, v0.5.1

*Reviewer-audited revision of v0.5*

---

## Conventions

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **NOT RECOMMENDED**, **MAY**, and
**OPTIONAL** in this document are to be interpreted as described in
BCP 14 [RFC 2119] [RFC 8174] when, and only when, they appear in all
capitals, as shown here.

---

## Spec-version constant

```
FAMP_SPEC_VERSION = "0.5.1"
```

Implementations MUST emit this exact string, case-sensitive, in any envelope
header or Agent Card field that references the spec version. A message with
a mismatched version is rejected with `unsupported_version`.

---

## §4a Canonical JSON

Canonical JSON for FAMP is **RFC 8785 JSON Canonicalization Scheme (JCS)**.

This section is a thin normative wrapper around RFC 8785. Where the text of
this specification and RFC 8785 disagree on any edge case, **RFC 8785 is
authoritative**.

### §4a.0 Key sort (RFC 8785 §3.2.3)

> "JSON object members MUST be sorted based on the UTF-16 code units of their
> names." — RFC 8785 §3.2.3

An implementation that compares keys as UTF-8 byte strings or as Rust
`str::cmp` output is non-conformant. Supplementary-plane characters
(U+10000 and above) sort by UTF-16 surrogate pair order (code units in the
D800–DFFF range), **not** by Unicode codepoint. See Example B (§4a.2) for
the demonstrating vector.

### §4a.0.1 Number formatting (RFC 8785 §3.2.2.3)

> "JSON numbers MUST be represented as specified by Section 7.1.12.1 of
> ECMAScript (ECMA-262), which is equivalent to the 'Number.prototype.toString'
> method." — RFC 8785 §3.2.2.3

Additional normative clauses:

a. `NaN`, `+Infinity`, and `-Infinity` MUST be rejected at the serializer
   boundary with a typed error. They have no canonical JSON representation.
b. Integers whose absolute value exceeds `2^53` MUST be represented as JSON
   **strings** per RFC 8785 §6 guidance. The IEEE 754 double-precision mantissa
   cannot represent `2^53 + 1` distinctly from `2^53`; any implementation that
   round-trips a large integer through a `f64` is non-conformant.
c. Negative zero (`-0`) MUST render as the string `0`.
d. The reference formatter is the **cyberphone JSON Canonicalization test
   corpus**, NOT the default `ryu` output. Rust implementations must use
   `ryu-js` (ECMAScript `Number.prototype.toString` semantics) or an
   equivalent.

### §4a.0.2 Duplicate keys rejected (RFC 8785 §3.1)

Implementations MUST reject JSON input containing duplicate object keys at
parse. Silently deduplicating duplicate keys is non-conformant. RFC 8259 §4
treats duplicate keys as SHOULD; FAMP upgrades this to MUST via RFC 8785 §3.1.

### §4a.0.3 No Unicode normalization

Canonical JSON MUST NOT apply Unicode normalization (NFC, NFD, NFKC, NFKD) to
string values. Bytes are passed through unchanged. A canonicalizer that
"cleans up" string content is non-conformant.

### §4a.0.4 Forbidden serde features

The `serde_json` features `arbitrary_precision` and `preserve_order` are
incompatible with JCS and MUST NOT be enabled by conforming Rust
implementations. `arbitrary_precision` changes number representation in ways
that break RFC 8785 §3.2.2.3; `preserve_order` retains insertion order in
place of the §3.2.3 UTF-16 sort.

### §4a.0.5 Forward reference

Two worked canonical-JSON examples (Example A, Example B) appear below in
this section; a full Ed25519 worked signature example using canonical JSON
is provided in §7.1c.

## §7.1 Signature (amendment — recipient binding)

The `to` field is part of the canonical JSON that is signed by the sender. A
signed envelope binds `to` the recipient: **a signed envelope addressed to
agent A MUST NOT be replayable to agent B**. This property is FAMP's
**recipient anti-replay** guarantee.

Any verifier that accepts an envelope without enforcing `envelope.to ==
self_principal` is non-conformant. The signature-over-canonical-JSON
machinery of §7.1a provides the cryptographic binding; this section makes
the policy normative.

## §7.1a Domain separation

The FAMP signature domain is separated from every other Ed25519 signing
context by a fixed prefix prepended to the canonical JSON input before
Ed25519 signing.

**Prefix (literal bytes):** `b"FAMP-sig-v1\x00"` — 12 bytes total: the 11
ASCII characters `F A M P - s i g - v 1` followed by one NUL byte.

**Prefix (hex):** `46 41 4d 50 2d 73 69 67 2d 76 31 00`

**Signing formula:**

```
signing_input   = prefix || canonical_json_bytes
signature       = Ed25519.sign(sk, signing_input)
```

where `canonical_json_bytes` is the RFC 8785 output (see §4a) of the
envelope **with the `signature` field omitted** from the object before
canonicalization.

Verification MUST apply the identical prefix in the identical position. The
prefix is prepended; it is **not** appended, **not** interleaved, and **not**
included as a JSON field in the envelope body.

**Rationale.** Domain separation prevents cross-protocol signature confusion
where a FAMP signature could be mistaken for a signature over arbitrary JSON
in an unrelated protocol that happens to share an Ed25519 key. Without a
prefix, an adversary could extract a FAMP signature and present it as a
valid signature in another system whose signed payloads happen to collide
with a FAMP canonical-JSON form.

A byte-level worked example (test keypair, canonical-JSON bytes, full
signing-input hex, 64-byte signature hex) is provided in §7.1c.

## §7.1b Ed25519 encoding

Ed25519 signing, key format, and verification semantics follow RFC 8032.

**Key and signature format (RFC 8032 §5.1.2, §5.1.6):**

- Ed25519 public keys are raw **32 bytes** (compressed Edwards point per
  RFC 8032 §5.1.2).
- Ed25519 signatures are raw **64 bytes** (`R || S` concatenation per
  RFC 8032 §5.1.6).

**Wire encoding (RFC 4648 §5):**

Both public keys and signatures are wire-encoded as **unpadded base64url**
per RFC 4648 §5, Table 2 alphabet (`A-Z a-z 0-9 - _`).

Encoded lengths:

| Field           | Raw bytes | Base64url chars |
|-----------------|-----------|-----------------|
| Public key      | 32        | 43              |
| Signature       | 64        | 86              |
| Idempotency key | 16        | 22              |

**Decoder rejection list.** Decoders MUST reject:

a. any `=` padding character;
b. the standard (non-url) alphabet characters `+` and `/`;
c. embedded whitespace (spaces, tabs, newlines) within the encoded string;
d. trailing garbage (any byte after the expected number of base64url chars).

**Strict verification (RFC 8032 §5.1.7).** Signature verification MUST use
the **strict** form: reject `R` that decodes to a small-order point, and
reject non-canonical `S` (enforce the upper bound `S < L` where `L` is the
Ed25519 group order). This matches `ed25519-dalek 2.2`'s `verify_strict`.
**Cofactor-tolerant verification** (the raw §5.1.7 equation
`[8]SB = [8]R + [8]kA`) is non-conformant for FAMP.

**Weak key rejection.** Weak (8-torsion) public keys MUST be rejected at
**trust-list ingress**, not only at verify time. A trust list containing a
small-order public key is itself malformed.

## §7.1c Worked signature example

*Placeholder — populated by Plan 02.*

## §6.1 Agent Card (revised)

*Placeholder — populated by Plan 03.*

## §6.3 Card versioning

*Placeholder — populated by Plan 03.*

## §13.1 Freshness and clock skew

*Placeholder — populated by Plan 03.*

## §13.2 Idempotency

*Placeholder — populated by Plan 03.*

## §9.5a EXPIRED vs deliver tiebreak

*Placeholder — populated by Plan 04.*

## §9.6a Terminal precedence

*Placeholder — populated by Plan 04.*

## §9.6b Conditional-lapse precedence

*Placeholder — populated by Plan 04.*

## §10.3a Supersession round counting

*Placeholder — populated by Plan 04.*

## §11.2a Capability snapshot

*Placeholder — populated by Plan 04.*

## §11.5a Competing-instance resolution

*Placeholder — populated by Plan 04.*

## §12.3a Transfer-timeout tiebreak

*Placeholder — populated by Plan 04.*

## §7.3a FSM-observable whitelist

*Placeholder — populated by Plan 04.*

## §8a Body schemas

*Placeholder — populated by Plan 05.*

## §3.6a Artifact identifiers

*Placeholder — populated by Plan 05.*

---

## v0.5.1 Changelog

Each entry below cites the reviewer finding that drove the change. Entries
are stable references of the form `v0.5.1-Δnn`.

- `v0.5.1-Δ04 — §4a Canonical JSON — PITFALLS P1/P2/P3 — RFC 8785 JCS made normative with §3.2.3 and §3.2.2.3 pull-quotes, duplicate-key rejection, no-Unicode-normalization clause.`
- `v0.5.1-Δ05 — §4a Canonical JSON — CONTEXT D-08 — serde arbitrary_precision and preserve_order forbidden; NaN/±Infinity rejected; integers > 2^53 serialized as strings.`
- `v0.5.1-Δ08 — §7.1a Domain separation — PITFALLS P5 — Fixed prefix b"FAMP-sig-v1\x00" (12 bytes); prepended to canonical JSON before Ed25519 sign/verify.`
- `v0.5.1-Δ09 — §7.1 — v0.5 reviewer finding (cross-recipient replay) — signature binds the to field; recipient anti-replay made normative.`
- `v0.5.1-Δ10 — §7.1b Ed25519 encoding — PITFALLS P4 — Raw 32/64-byte, unpadded base64url (RFC 4648 §5); verify_strict semantics normative; decoder rejection list specified.`
