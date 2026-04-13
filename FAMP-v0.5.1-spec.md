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

*Placeholder — populated by Plan 02.*

## §7.1a Domain separation

*Placeholder — populated by Plan 02.*

## §7.1b Ed25519 encoding

*Placeholder — populated by Plan 02.*

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

*Empty — populated by Plan 06.*
