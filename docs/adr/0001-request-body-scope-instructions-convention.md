# ADR 0001: Request-body `scope` convention — `instructions` key (PROVISIONAL)

**Status:** Proposed, PROVISIONAL
**Date:** 2026-04-24
**Context commit:** `fddc24d49fe5f964c978455adf775705a09da9ad`

## Context

The v0.5.1 envelope spec types `RequestBody.scope` as
`serde_json::Value` (arbitrary JSON) without prescribing a shape.
In practice, first beta usage (recorded in maintainer feedback notes
on 2026-04-24) revealed that senders want to attach prose task content
to a new task, and the implementation was silently dropping it:
`build_request_envelope` hardcoded `scope: {}` and the NewTask arm
never read `args.body`. Receivers saw `scope:{}` and had no way to
infer the task content, forcing a second `deliver` call as a
workaround.

## Decision

Prose task content attached to a `famp send --new-task --body <text>`
call lands in `RequestBody.scope` under the key `instructions` as
a JSON string:

    "scope": { "instructions": "<body text>" }

The key is centralised as the public constant
`famp_envelope::body::request::REQUEST_SCOPE_INSTRUCTIONS_KEY`.

This convention is **PROVISIONAL**. It is NOT lifted into the v0.5.1
spec fork. Re-evaluation gate: ~10 real cross-agent exchanges (or
earlier if a peer-ecosystem convention emerges).

## Alternatives considered

- **`natural_language_summary`** — rejected: semantic lie. The
  summary is a short title; the body may be multi-KB prose.
- **Require structured JSON** — rejected: hostile DX. Most sends
  are free-form instructions.
- **Auto-deliver combo (send request + follow-up deliver)** —
  rejected: wrong envelope class; forces a two-message handshake
  and muddles causality.
- **Key name `prose`** — rejected: literary / niche.
- **Key name `description`** — rejected: collides with OpenAPI /
  JSON-Schema `description` semantics.
- **Key name `text`** — rejected: implies plaintext, forecloses
  future `content-type` negotiation.
- **Key name `content`** — rejected: too generic; MIME-adjacent.
- **Key name `instructions`** — SELECTED: matches LLM-era API
  conventions (Anthropic / OpenAI `instructions` params), is
  self-documenting, carries no prior semantic baggage in the
  envelope schema.

## Consequences

- Signature coverage: `scope` is already inside canonical JSON under
  the `FAMP-sig-v1\0` domain prefix (INV-10). No signature change.
- Backward compatibility: sends with no `--body` continue to produce
  `scope:{}` — existing tests stay green.
- Spec drift: none. v0.5.1 spec fork is untouched.
- Conformance: a PROVISIONAL vector ships at
  `crates/famp-envelope/tests/fixtures/provisional/request-scope-instructions.json`.
  Normative Level 2 loaders must not include it.

## Re-evaluation

Revisit after ~10 real cross-agent exchanges have populated scope
with real instructions. Candidates to revisit:
- Promote to normative (lift into v0.5.1 fork).
- Rename (one-line change at the constant).
- Re-type `RequestBody.scope` as a typed enum with
  `{ Instructions(String), Structured(Value) }` variants.
