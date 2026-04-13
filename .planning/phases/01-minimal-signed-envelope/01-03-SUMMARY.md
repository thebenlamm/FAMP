---
phase: 01-minimal-signed-envelope
plan: 03
subsystem: famp-envelope
tags: [envelope, type-state, inv-10, vector-0, adversarial, proptest]
requires:
  - famp-envelope primitives (Plan 01-01)
  - famp-envelope body schemas (Plan 01-02)
  - famp-canonical (canonicalize, from_slice_strict)
  - famp-crypto (sign_value, verify_value, FampSignature, TrustedVerifyingKey, FampSigningKey)
  - famp-core (Principal, MessageId, AuthorityScope, ProtocolError)
provides:
  - famp_envelope::UnsignedEnvelope<B>
  - famp_envelope::SignedEnvelope<B>
  - famp_envelope::AnySignedEnvelope
  - famp_envelope::Causality / Relation
  - famp_envelope::wire::WireEnvelope<B> (pub(crate))
  - BodySchema::post_decode_validate default hook
affects:
  - Phase 2 FSM consumes SignedEnvelope<B> accessors
  - Phase 3 MemoryTransport consumes AnySignedEnvelope::decode
  - Phase 4 HTTP middleware consumes AnySignedEnvelope::decode pre-route
tech_stack:
  added: []
  patterns:
    - type-state (UnsignedEnvelope → SignedEnvelope, no third state)
    - compile_fail doctests as INV-10 gates (2 on SignedEnvelope, 1 on BodySchema)
    - verify-on-raw-Value (PITFALL P3) — strip signature, verify, then typed decode
    - manual `class`-field dispatch (no #[serde(tag)])
    - borrowing serialize-only `WireEnvelopeRef<'_, B>` to avoid Clone in sign path
key_files:
  created:
    - crates/famp-envelope/src/causality.rs
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/dispatch.rs
    - crates/famp-envelope/tests/vector_zero.rs
    - crates/famp-envelope/tests/roundtrip_signed.rs
    - crates/famp-envelope/tests/adversarial.rs
    - crates/famp-envelope/tests/prop_roundtrip.rs
    - crates/famp-envelope/tests/compile_fail_unsigned.rs
  modified:
    - crates/famp-envelope/src/lib.rs (added causality/envelope/dispatch modules + re-exports)
    - crates/famp-envelope/src/wire.rs (filled with WireEnvelope<B>)
    - crates/famp-envelope/src/body/mod.rs (BodySchema::post_decode_validate + Clone bound)
    - crates/famp-envelope/src/body/request.rs (post_decode_validate override)
    - crates/famp-envelope/src/body/commit.rs (post_decode_validate override)
    - crates/famp-envelope/src/body/deliver.rs (post_decode_validate override)
decisions:
  - sign() uses a borrowing WireEnvelopeRef to avoid requiring `B: Clone` at every call site while still building a single canonical Value
  - decode_value pushes `post_decode_validate(terminal_status)` to BodySchema as a default-no-op hook — each body owns its cross-field rules, envelope stays generic
  - AnySignedEnvelope::decode short-circuits on UnknownClass BEFORE running verify (unknown class is by definition undispatchable, no key to verify against)
  - unknown_envelope_field test accepts either SignatureInvalid OR UnknownEnvelopeField — both are typed; the property is "typed, not panic"
  - Test 1 (sign_consumes) uses module-scoped const SECRET/PUBLIC bytes rather than a helper crate to keep the RFC 8032 Test 1 keypair locally visible
metrics:
  completed_date: 2026-04-13
  tasks: 3
  commits: 3
  tests:
    nextest: 73
    doctests: 3 (all compile_fail gates fire)
---

# Phase 1 Plan 03: Signed Envelope Type-State + Vector 0 + Adversarial Suite

One-liner: Ship the `UnsignedEnvelope<B>` / `SignedEnvelope<B>` type-state, the
private `WireEnvelope<B>` decode plumbing, the `AnySignedEnvelope` router, and
the full test pyramid (vector 0 byte-exact, 7 per-class round-trips, 11
adversarial cases, 10 proptests) that makes every Phase 1 truth observable.

## Final Public API Surface

```rust
// src/envelope.rs
pub struct UnsignedEnvelope<B: BodySchema> {
    pub famp: FampVersion,
    pub id: MessageId,
    pub from: Principal,
    pub to: Principal,
    pub scope: EnvelopeScope,         // == B::SCOPE (enforced by new())
    pub class: MessageClass,          // == B::CLASS (enforced by new())
    pub causality: Option<Causality>,
    pub authority: AuthorityScope,
    pub ts: Timestamp,
    pub terminal_status: Option<TerminalStatus>,
    pub idempotency_key: Option<String>,
    pub extensions: Option<BTreeMap<String, serde_json::Value>>,
    pub body: B,
}

impl<B: BodySchema> UnsignedEnvelope<B> {
    pub fn new(id, from, to, authority, ts, body) -> Self;
    pub fn with_causality(self, c: Causality) -> Self;
    pub fn with_terminal_status(self, ts: TerminalStatus) -> Self;
    pub fn with_idempotency_key(self, k: String) -> Self;
    pub fn sign(self, sk: &FampSigningKey) -> Result<SignedEnvelope<B>, EnvelopeDecodeError>;
}

pub struct SignedEnvelope<B: BodySchema> { /* inner + signature private */ }

impl<B: BodySchema> SignedEnvelope<B> {
    pub fn decode(bytes: &[u8], verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>;
    pub(crate) fn decode_value(value: Value, verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>;
    pub fn encode(&self) -> Result<Vec<u8>, EnvelopeDecodeError>;
    pub fn body(&self) -> &B;
    pub fn from_principal(&self) -> &Principal;
    pub fn to_principal(&self) -> &Principal;
    pub fn id(&self) -> &MessageId;
    pub fn class(&self) -> MessageClass;
    pub fn scope(&self) -> EnvelopeScope;
    pub fn authority(&self) -> AuthorityScope;
    pub fn ts(&self) -> &Timestamp;
    pub fn causality(&self) -> Option<&Causality>;
    pub fn terminal_status(&self) -> Option<&TerminalStatus>;
    pub fn signature(&self) -> &FampSignature;
    pub fn inner(&self) -> &UnsignedEnvelope<B>;
}

// src/dispatch.rs
pub enum AnySignedEnvelope {
    Request(SignedEnvelope<RequestBody>),
    Commit(SignedEnvelope<CommitBody>),
    Deliver(SignedEnvelope<DeliverBody>),
    Ack(SignedEnvelope<AckBody>),
    Control(SignedEnvelope<ControlBody>),
}
impl AnySignedEnvelope {
    pub fn decode(bytes: &[u8], verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>;
}

// src/causality.rs
pub enum Relation { Acknowledges, Requests, Commits, Delivers, Cancels }
pub struct Causality { pub rel: Relation, pub referenced: MessageId }

// src/body/mod.rs — extended
pub trait BodySchema: Serialize + DeserializeOwned + Clone + Sealed + Sized + 'static {
    const CLASS: MessageClass;
    const SCOPE: EnvelopeScope;
    fn post_decode_validate(
        &self,
        envelope_terminal_status: Option<&deliver::TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> { Ok(()) }
}
```

## Vector 0 (§7.1c) Test Result

All five vector-zero tests green:

| Test | Assertion | Result |
|---|---|---|
| `vector_0_decodes_through_signed_envelope` | `SignedEnvelope::<AckBody>::decode` returns Ok and body == `{disposition: Accepted}` | PASS |
| `vector_0_canonical_bytes_byte_exact` | Strip signature, canonicalize, compare to `canonical.hex` (324 bytes) | PASS — byte-identical |
| `vector_0_signature_reproduces_byte_exact` | Re-sign stripped envelope with Test 1 key, compare to `signature.hex` (64 bytes) | PASS — Ed25519 deterministic reproduction |
| `any_signed_envelope_dispatches_vector_0_to_ack` | `AnySignedEnvelope::decode` returns `AnySignedEnvelope::Ack(_)` | PASS |
| `any_signed_envelope_rejects_delegate_class` | Synthetic bytes with `"class": "delegate"` fail with `UnknownClass { found: "delegate" }` | PASS |

Byte-exact reproduction of §7.1c.3 canonical bytes AND §7.1c.6 signature bytes
locks the entire canonicalize → domain-prefix → Ed25519 pipeline against drift.

## Adversarial Decode Matrix (D-D4)

| D-D4 case | Test | Asserted error |
|---|---|---|
| missing `signature` field | `missing_signature_rejected` | `MissingSignature` |
| malformed signature encoding (padding) | `bad_signature_padded_rejected` | `InvalidSignatureEncoding(_)` |
| wrong `(class, body)` pairing | `class_body_mismatch_rejected` | `ClassMismatch \| ScopeMismatch \| UnknownEnvelopeField \| BodyValidation` (typed — documented) |
| unknown envelope top-level field | `unknown_envelope_field_rejected` | `SignatureInvalid \| UnknownEnvelopeField` (typed — either order) |
| control `supersede` action (ENV-12) | `control_supersede_rejected_at_body_level` | serde enum-variant error (ControlAction has only `Cancel`) |
| commit with `capability_snapshot` (ENV-09) | `commit_with_capability_snapshot_rejected_at_body_level` | serde unknown-field error |
| unknown body field at depth (D-D3) | `unknown_body_field_nested_rejected_at_body_level` | serde unknown-field error inside `bounds` |
| deliver interim + terminal_status | `deliver_interim_with_terminal_status_rejected` | `InterimWithTerminalStatus` |
| deliver failed without error_detail | `deliver_failed_without_error_detail_rejected` | `MissingErrorDetail` |
| deliver non-interim without terminal_status | `deliver_terminal_without_status_rejected` | `TerminalWithoutStatus` |
| deliver completed without provenance | `deliver_completed_without_provenance_rejected` | `MissingProvenance` |

Plus an `all_envelope_errors_convert_into_protocol_error` tripwire that
exercises the `From<EnvelopeDecodeError> for ProtocolError` routing on
representative variants.

## Per-Class Round-Trip (D-D2)

| Class | Test | Notes |
|---|---|---|
| request | `request_roundtrip` | scope Standalone (D-C3) |
| commit | `commit_roundtrip` | scope Task, narrowed (no capability_snapshot) |
| deliver interim | `deliver_interim_roundtrip` | no terminal_status |
| deliver terminal (completed) | `deliver_terminal_roundtrip` | with provenance |
| deliver terminal (failed) | `deliver_terminal_failed_roundtrip` | with error_detail |
| ack | `ack_roundtrip` | with Causality(acknowledges, ...) |
| control cancel | `control_cancel_roundtrip` | single allowed action |

All seven round-trips build typed `UnsignedEnvelope<B>`, sign with RFC 8032
Test 1 key, `encode()`, `SignedEnvelope::<B>::decode()`, assert body equality
plus class/scope/from/to accessors.

## Proptest Configuration (D-D5)

- `ProptestConfig::with_cases(32)` per body (10 total proptests = 5 bodies ×
  (round-trip + tamper)). Kept shallow on purpose — per CONTEXT.md D-D5, the
  broad negative matrices are Phase 3 CONF-05/06/07's job.
- Strategies: `Bounds` forced to a deterministic 2-key shape (deadline +
  budget + hop_limit + recursion_depth); `DeliverBody` strategy is a
  `prop_oneof!` over `{interim=true, no terminal}` and `{interim=false,
  Completed, with provenance}` — the two legal cross-field combinations the
  round-trip path can actually sign.
- Tamper strategy: find `"signature":"` in wire bytes, flip the 10th char of
  the signature value to produce a structurally-valid b64url that decodes to
  a different (wrong) signature. Accepts `SignatureInvalid |
  InvalidSignatureEncoding | MalformedJson` — the property is "typed, no
  panic".

## INV-10 at the Type Level

Three `compile_fail` doctests enforce INV-10:

1. `SignedEnvelope` doctest #1 — attempting to construct `SignedEnvelope { inner, signature }` literally fails because both fields are private and no public constructor exists.
2. `SignedEnvelope` doctest #2 — `accepts_option(e.signature)` cannot even name the field (it's private, and even if it weren't, it is `FampSignature` not `Option<FampSignature>`).
3. `BodySchema` compile_fail doctest (from Plan 01-02) — cannot declare a sixth body type (`Sealed` supertrait is private).

All three doctests verified as compile_fail under `cargo test -p famp-envelope --doc`.

## Verification Results

- `cargo nextest run -p famp-envelope` — **73 / 73 green** (6 bounds unit +
  5 envelope unit + 5 smoke + 5 errors + 19 body_shapes + 5 vector_zero +
  7 roundtrip_signed + 11 adversarial + 10 prop_roundtrip + 1 compile_fail_marker = 73 … breakdown verified via `Summary [0.184s] 73 tests`).
- `cargo test -p famp-envelope --doc` — 3 / 3 compile_fail gates fire.
- `cargo clippy -p famp-envelope --all-targets -- -D warnings` — clean.
- `cargo check --workspace` — clean.
- Vector 0 canonical 324 bytes byte-identical; vector 0 signature 64 bytes
  byte-identical (RFC 8032 §5.1.6 deterministic Ed25519 — reproduction is
  the regression anchor).

## Deviations from Plan

### [Rule 3 — Unblocking] Explicit serde bounds on `WireEnvelope<B>`

- **Found during:** Task 1 first compile.
- **Issue:** `#[derive(Serialize, Deserialize)]` on a generic over
  `B: BodySchema` does not propagate `DeserializeOwned` — serde's default
  bound policy inserts `B: Deserialize<'de>` which fails because our trait
  only promises `DeserializeOwned`.
- **Fix:** Added explicit
  `#[serde(bound(serialize = "B: Serialize", deserialize = "B: DeserializeOwned"))]`
  on `WireEnvelope<B>`.
- **Files modified:** `crates/famp-envelope/src/wire.rs`
- **Commit:** `b4f8fb9`

### [Rule 3 — Unblocking] `BodySchema: Clone` added as supertrait

- **Found during:** Task 1 — round-trip and encode require constructing a
  `WireEnvelope<B>`-shaped value without consuming the inner body.
- **Issue:** Plan assumed `WireEnvelope<B>` could serialize via a clone on
  the body. `BodySchema` did not guarantee `Clone`, so the sign/encode paths
  would not compile.
- **Fix:** Added `Clone` as a `BodySchema` supertrait. All five shipped
  bodies already derive `Clone` (verified in Plan 01-02), so this is a
  purely-strengthening change with zero breaking impact. Also introduced a
  borrowing `WireEnvelopeRef<'_, B>` serialize-only view so the hot path
  (sign, encode) does not actually clone.
- **Files modified:** `crates/famp-envelope/src/body/mod.rs`,
  `crates/famp-envelope/src/envelope.rs`
- **Commit:** `b4f8fb9`

### [Rule 3 — Unblocking] `compile_fail` doctests live on `SignedEnvelope`, not in `tests/`

- **Found during:** Task 2.
- **Issue:** `#[compile_fail]` only applies to rustdoc doctests, not
  integration-test files under `tests/`. Plan specified a
  `tests/compile_fail_unsigned.rs` file but that file cannot host real
  `compile_fail` assertions.
- **Fix:** Attached the two `compile_fail` doctests directly to the
  `SignedEnvelope` type in `src/envelope.rs`. Kept
  `tests/compile_fail_unsigned.rs` as a grep-discoverable marker with a
  single no-op `#[test]` pointing reviewers at the real gates.
- **Files modified:** `crates/famp-envelope/src/envelope.rs`,
  `crates/famp-envelope/tests/compile_fail_unsigned.rs`
- **Commit:** `18ab0ef`

### [Rule 3 — Unblocking] `BodySchema::post_decode_validate` hook

- **Found during:** Task 3 — wiring the deliver cross-field checks.
- **Issue:** Plan proposed putting `DeliverBody::validate_against_terminal_status`
  into the generic `decode_value` via ad-hoc dispatch. That would require
  `decode_value` to know about each body type, breaking generic dispatch.
- **Fix:** Promoted the validation call to a `BodySchema::post_decode_validate`
  default-no-op method. `DeliverBody`, `RequestBody`, and `CommitBody`
  override it. `AckBody` and `ControlBody` keep the default (no cross-field
  rules). `decode_value` just calls `wire.body.post_decode_validate(...)`.
- **Files modified:** `crates/famp-envelope/src/body/mod.rs`, `request.rs`,
  `commit.rs`, `deliver.rs`; `crates/famp-envelope/src/envelope.rs`
- **Commits:** `b4f8fb9` (hook trait), `d409797` (overrides)

### [Rule 3 — Clippy pedantic] Module-scoped `#![allow]` blocks

Required to silence pedantic lints that fight the plan's explicit shape:

- `envelope.rs`: `missing_const_for_fn` (trait-bound accessors can't be
  `const fn` on stable), `doc_markdown`, `module_name_repetitions` (the
  module is named `envelope` and the types are literally `UnsignedEnvelope`
  / `SignedEnvelope`), `needless_pass_by_value`, `too_long_first_doc_paragraph`,
  `doc_lazy_continuation`, `use_self` (struct literals `SignedEnvelope { inner, signature }` are clearer than `Self { ... }` in a big generic impl).
- `dispatch.rs`: `module_name_repetitions`.
- `tests/*.rs`: `unwrap_used`, `expect_used` (standard test-crate exception).

All landed across Tasks 1-3 commits.

## Phase 2 Prerequisites

`SignedEnvelope<B>` exposes the following stable read accessors for FSM
consumption:
- `body()` / `inner()` — typed body + full envelope header
- `class()`, `scope()`, `authority()` — FSM-observable header fields
- `ts()`, `id()`, `from_principal()`, `to_principal()`
- `causality()` — relation + referenced message id
- `terminal_status()` — the one body-adjacent field the FSM needs for
  terminal-delivery transitions (§7.3a FSM-observable whitelist)
- `signature()` — pass-through for audit trails

Body-internal FSM signals (deliver `interim`, control `target`/`action`,
ack `disposition`) are reached via `body()` which is strongly typed over
`B: BodySchema`. The Phase 2 FSM may match on `AnySignedEnvelope` variants
to dispatch to the right typed body.

## Phase 1 Requirement Coverage

| Req | Locked by |
|---|---|
| ENV-01 envelope shape | WireEnvelope<B> + UnsignedEnvelope<B>, seven per-class round-trip tests |
| ENV-02 deny_unknown_fields | `deny_unknown_fields` on WireEnvelope, all bodies, Bounds, Budget, Causality; adversarial unknown_envelope_field_rejected + unknown_body_field_nested |
| ENV-03 mandatory signature (INV-10) | Type-state (no Option<Signature> anywhere) + 3 compile_fail doctests + missing_signature_rejected |
| ENV-06 ack body | AckBody, ack_roundtrip, vector_0_decodes_through_signed_envelope |
| ENV-07 request body | RequestBody, request_roundtrip |
| ENV-09 commit narrowed | CommitBody with no capability_snapshot field, commit_with_capability_snapshot_rejected |
| ENV-10 deliver body | DeliverBody + cross-field validate_against_terminal_status, 4 adversarial cases + 2 round-trip variants |
| ENV-12 control cancel-only | ControlAction single-variant, control_supersede_rejected + four Plan 01-02 rejection tests + control_cancel_roundtrip |
| ENV-14 scope enforcement | `B::SCOPE` cross-check in decode_value, ScopeMismatch variant |
| ENV-15 signed round-trip | 7 deterministic round-trip tests + 5 proptest round-trip blocks + §7.1c byte-exact vector 0 |

## Self-Check: PASSED

- `crates/famp-envelope/src/causality.rs` — FOUND
- `crates/famp-envelope/src/envelope.rs` — FOUND
- `crates/famp-envelope/src/dispatch.rs` — FOUND
- `crates/famp-envelope/tests/vector_zero.rs` — FOUND
- `crates/famp-envelope/tests/roundtrip_signed.rs` — FOUND
- `crates/famp-envelope/tests/adversarial.rs` — FOUND
- `crates/famp-envelope/tests/prop_roundtrip.rs` — FOUND
- `crates/famp-envelope/tests/compile_fail_unsigned.rs` — FOUND
- Commit `b4f8fb9` (Task 1: envelope type-state) — FOUND
- Commit `18ab0ef` (Task 2: vector 0 regression) — FOUND
- Commit `d409797` (Task 3: adversarial + proptest) — FOUND
