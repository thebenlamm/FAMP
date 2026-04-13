# Phase 1: Minimal Signed Envelope — Context

**Gathered:** 2026-04-13
**Status:** Ready for research → planning

<domain>
## Phase Boundary

`famp-envelope` encodes, decodes, and signature-verifies every message class the Personal Runtime actually emits (`request`, `commit`, `deliver`, `ack`, `control/cancel`) and rejects anything else at the type level. INV-10 (mandatory signature) is enforced compile-time, not at runtime. ENV-12 is narrowed to `cancel` only; ENV-09 is narrowed to omit `capability_snapshot` entirely. The wider v0.6-catalog message set (announce, describe, propose, delegate, supersede, close) is explicitly out of scope and must not be representable in types.

Transport, keyring, FSM, and examples are later phases.

</domain>

<decisions>
## Implementation Decisions

### A. Signed/Unsigned representation (INV-10 at type level)

- **D-A1:** Exactly two public envelope states: `UnsignedEnvelope<B>` and `SignedEnvelope<B>`. No third `VerifiedEnvelope` state. `SignedEnvelope::decode` verifies before constructing, so "signed" ≡ "verified by construction".
- **D-A2:** Flow is strictly: builders → `UnsignedEnvelope<B>` → sign → `SignedEnvelope<B>` → wire. There is NO public API that yields an unsigned on-wire envelope. `Option<Signature>` is explicitly rejected — a permanently-half-valid type is the wrong shape.
- **D-A3:** An internal `WireEnvelope` serde struct is permitted for decode plumbing, but is **not** public. `SignedEnvelope::decode(bytes, verifier)` is the only public decode path; it parses the wire struct, strips the `signature` field, rebuilds the signing input via `famp_crypto::canonicalize_for_signature`, runs `verify_strict`, and only then constructs `SignedEnvelope<B>`.
- **D-A4:** Signing API consumes or borrows `UnsignedEnvelope<B>` and returns `SignedEnvelope<B>` — no in-place mutation, no "envelope with optional sig" limbo.

### B. Message class ↔ body coupling

- **D-B1:** Envelope is generic over a sealed `BodySchema` trait:
  ```rust
  pub trait BodySchema: Serialize + DeserializeOwned + Sealed {
      const CLASS: MessageClass;
      const SCOPE: EnvelopeScope;
  }
  ```
  `UnsignedEnvelope<B: BodySchema>` and `SignedEnvelope<B: BodySchema>` — wrong `(class, body)` pairs are unrepresentable in normal typed use.
- **D-B2:** Trait is sealed so downstream crates cannot invent new body classes. Only the five shipped types implement it: `RequestBody`, `CommitBody`, `DeliverBody`, `AckBody`, `ControlBody`.
- **D-B3:** Internal decode dispatch on the envelope `class` field is permitted (and required — wire JSON is untyped) but is private. Public API is typed.
- **D-B4:** A parallel public `AnySignedEnvelope` enum is provided for router-style code:
  ```rust
  pub enum AnySignedEnvelope {
      Request(SignedEnvelope<RequestBody>),
      Commit(SignedEnvelope<CommitBody>),
      Deliver(SignedEnvelope<DeliverBody>),
      Ack(SignedEnvelope<AckBody>),
      Control(SignedEnvelope<ControlBody>),
  }
  ```
  Phase 3/4 transport/router code will dispatch on this. Typed decode is the primary ergonomic path; `AnySignedEnvelope::decode` is the secondary path for when the caller does not know the body class upfront.
- **D-B5:** **Narrowing is type-level absence, not `Option<_>`.**
  - `ControlBody` exposes only `cancel`. `supersede`, `close`, `cancel_if_not_started`, `revert_transfer` literally do not exist as variants in v0.7. Adding one is a v0.8+ breaking change.
  - `CommitBody` omits `capability_snapshot` entirely — it is not an `Option` field, it is not present. Documented inline with a pointer to v0.8 §11.2a.
  - Same rule for any other deferred field: absent from the type, not optional.

### C. ENV-14 Scope enforcement (standalone / conversation / task)

- **D-C1:** Scope rule lives on the body type via `BodySchema::SCOPE`, a plain `EnvelopeScope` enum (`Standalone`, `Conversation`, `Task`). No phantom-scope machinery, no `StandaloneEnvelope<B>`/`ConversationEnvelope<B>` wrappers.
- **D-C2:** Enforcement happens at decode time:
  1. Parse wire header + body.
  2. Assert `envelope.class == B::CLASS`.
  3. Assert the envelope's actual scope-bearing fields (which conversation/task IDs are present) are compatible with `B::SCOPE`.
  4. Failure → narrow typed error (`EnvelopeDecodeError` variant), not `ProtocolErrorKind::Other`.
- **D-C3:** **`request` is locked to `Standalone` scope for v0.7 Personal Runtime.** No conversation-bound `request` in this milestone. Rationale: defer conversation IDs from Phase 1 envelope logic; the Personal Runtime happy path does not need them. Revisit in v0.8 Negotiation/Causality.
- **D-C4:** Scope locks for the other four classes are to be finalized during research against §7.3a FSM-observable whitelist, but the *mechanism* (body-associated `SCOPE` const, decode-time cross-check) is fixed.

### D. Test strategy for ENV-15 / round-trip / adversarial decode

- **D-D1:** **Golden vector test — vector 0.** The externally-generated §7.1c worked-example `ack` bytes are committed as a fixture and asserted byte-for-byte through the full pipeline (decode → re-serialize → canonicalize → signing input → signature bytes). This is the byte-exact signature regression anchor for the whole library.
- **D-D2:** **Per-class round-trip tests** — one deterministic test per shipped class (`request`, `commit`, `deliver`, `ack`, `control/cancel`): build typed `UnsignedEnvelope<B>`, sign, serialize, decode through `SignedEnvelope<B>::decode`, assert semantic equality. Not proptest — deterministic fixtures with known-good inputs.
- **D-D3:** **`deny_unknown_fields` fixtures** — one committed JSON fixture per class with an injected unknown key, asserted to fail decode. At least one case must inject the unknown field **nested inside the body** (not only at envelope top level) to lock the contract at depth.
- **D-D4:** **Envelope-local adversarial decode cases** — included in Phase 1 (they validate the envelope API boundary itself):
  - missing `signature` field
  - malformed `signature` encoding (wrong length, bad base64url, padding present)
  - wrong `(class, body)` pairing (e.g., envelope `class="request"` with a commit-shaped body)
  - `control` body with `action="supersede"` or any non-`cancel` action
  - unknown body field at depth (see D-D3)
  - narrowed-`commit` body carrying `capability_snapshot` (must fail decode — the field is absent from the type)
  - Each adversarial case must fail with a **distinct, typed** `EnvelopeDecodeError` variant — no panics, no generic errors.
- **D-D5:** **proptest scope — small and typed.**
  - Per-body strategies only. No giant "arbitrary envelope JSON" generator.
  - Focus: round-trip stability and signing/verification invariants (encode → decode → re-encode is byte-stable; sign → verify succeeds; tampered canonical bytes → verify fails).
  - Opaque payload objects (`scope`, `bounds`, `result`, `provenance`) kept shallow and deterministic enough that failures are debuggable.
  - Broad transport-level negative matrices and full envelope-wide random adversarial suites are deferred to Phase 3 CONF-05/06/07.

### E. Decode API shape — typed vs untyped dispatch

- **D-E1:** **Both paths, typed is primary.**
  - Primary: `SignedEnvelope::<RequestBody>::decode(bytes, verifier) -> Result<SignedEnvelope<RequestBody>, EnvelopeDecodeError>` — used when the caller already knows the class.
  - Secondary: `AnySignedEnvelope::decode(bytes, verifier) -> Result<AnySignedEnvelope, EnvelopeDecodeError>` — used by router-style code (Phase 3/4 transports) that inspects the wire `class` field and dispatches.
- **D-E2:** Both paths share a private decode core: parse wire struct → verify signature → construct typed body. The two public entry points differ only in how they interpret the `class` field (typed path asserts it matches `B::CLASS`; Any path matches and dispatches).

### F. Error shape

- **D-F1:** Phase-local narrow enum `EnvelopeDecodeError` / `EnvelopeError` — follows the v0.6 pattern (see Plans 01-01 D-16, 02-01). Converts into the global `ProtocolErrorKind` at the crate boundary; does not leak `famp_core::ProtocolErrorKind::Other`.
- **D-F2:** One variant per adversarial case in D-D4 so tests can match exactly.

### Claude's Discretion

- Exact module layout inside `famp-envelope/src/` (one module per body vs `bodies/` submodule vs flat).
- Whether `UnsignedEnvelope<B>` exposes a typed builder (`EnvelopeBuilder<B>`) or just `new` + field assignment — pick the ergonomics that fit the round-trip tests.
- Naming of the sealed trait's sealing technique (private module, sealed supertrait, etc.).
- Exact `EnvelopeDecodeError` variant list beyond the D-D4 enumeration — add variants as research reveals more decode failure modes.
- Whether `AnySignedEnvelope` lives in `famp-envelope` directly or in a small `dispatch` module.

</decisions>

<specifics>
## Specific Ideas

- "Narrowed should mean not representable, not optional but unused." — strict rule for ENV-09 and ENV-12 narrowings. The type is the documentation; if it's `Option<_>`, a future drive-by PR can set it and break the narrowing silently.
- Treat the §7.1c worked example as **vector 0** — the first and most important regression test in the whole library. Any change to canonicalization, signing input construction, or base64url encoding that breaks vector 0 is an immediate hard stop.
- Prefer the existing v0.6 pattern: phase-local narrow error enums that convert into the global 15-category `ProtocolErrorKind` at the crate boundary. Proven twice in v0.6 Plans 01-01 and 02-01.
- `request` stays **standalone** in v0.7 — the fewer IDs Phase 1 has to plumb, the less Phase 3's `personal_two_agents` example has to set up.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec — envelope, signature, body schemas
- `FAMP-v0.5.1-spec.md` §7.1 — Recipient-binding amendment, `to`-field signing, replay anti-property
- `FAMP-v0.5.1-spec.md` §7.1a — Domain separation prefix (`b"FAMP-sig-v1\x00"`, 12 bytes), signing formula, prefix position (prepend, not append/interleave)
- `FAMP-v0.5.1-spec.md` §7.1b — Ed25519 wire encoding (raw 32/64 bytes, base64url-unpadded), decoder rejection list, `verify_strict` normativity
- `FAMP-v0.5.1-spec.md` §7.1c — Worked signature example (vector 0). **Sub-sections .0 through .8 are all load-bearing** — the Python-generated canonical bytes, signing input hex, and 64-byte signature hex must reproduce byte-for-byte in Rust.
- `FAMP-v0.5.1-spec.md` §7.3a — FSM-observable whitelist (envelope fields `class`, `relation`, `terminal_status`; body fields `interim`, `action`, `target`, `disposition`, `form`, `scope_subset`). Tells scope/class enforcement which fields can be inspected at decode without peeking into opaque payloads.
- `FAMP-v0.5.1-spec.md` §8a — Body schemas. `additionalProperties: false` semantics is normative (⇒ `deny_unknown_fields` everywhere).
- `FAMP-v0.5.1-spec.md` §8a.2 — `commit` body. **Read carefully**: `capability_snapshot` is REQUIRED in the spec. Phase 1 narrowing (ENV-09) intentionally omits it — this divergence must be documented inline in `CommitBody` with a pointer to v0.8 §11.2a.
- `FAMP-v0.5.1-spec.md` §8a.3 — `deliver` body. `interim` bool gates `terminal_status`; `error_detail` REQUIRED iff `terminal_status = failed`; `provenance` REQUIRED on terminal deliveries.
- `FAMP-v0.5.1-spec.md` §8a.4 — `control` body. **The full catalog lists 5 actions** (`cancel`, `supersede`, `close`, `cancel_if_not_started`, `revert_transfer`). ENV-12 narrowing keeps only `cancel`. The other four must not exist as variants in v0.7 `ControlBody`.

### Spec — canonicalization and identifiers (substrate from v0.6)
- `FAMP-v0.5.1-spec.md` §4a — RFC 8785 JCS: key sort, number formatting, duplicate key rejection, no Unicode normalization, forbidden serde features
- `FAMP-v0.5.1-spec.md` §3.6a — Artifact ID scheme (`sha256:<hex>`, canonical-JSON-of-body hash)
- `FAMP-v0.5.1-spec.md` §13.2 — Idempotency key scoping (16 bytes / 22 base64url chars)

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed), ENV-10, ENV-12 (cancel-only), ENV-14, ENV-15
- `.planning/ROADMAP.md` — Phase 1 goal, success criteria, dependency on v0.6 substrate
- `.planning/STATE.md` — Accumulated decisions from v0.5.1 and v0.6 (verify_strict-only, domain prefix prepended internally, narrow phase-local error enums, 15-category ProtocolErrorKind)

### v0.6 implementation precedents to mirror
- `crates/famp-canonical/src/lib.rs` — `canonicalize`, `from_slice_strict`, `artifact_id_for_value` (use, do not reimplement)
- `crates/famp-crypto/src/lib.rs` — `canonicalize_for_signature` (the ONLY sanctioned signing-input path), `verify_strict`, `FampSignature`, `TrustedVerifyingKey`
- `crates/famp-core/src/error.rs` — `ProtocolError`, `ProtocolErrorKind` (15-category flat enum) — target for phase-local error conversion
- `crates/famp-core/src/identity.rs` + `ids.rs` — `Principal`, `Instance`, `MessageId`, `ConversationId`, `TaskId`, `CommitmentId` (envelope fields reuse these, do not re-type)
- `crates/famp-core/src/scope.rs` — `AuthorityScope` (body `bounds.authority_scope` reuses this)
- `.planning/milestones/v0.6-phases/01-canonical-json-foundations/01-01-PLAN.md` D-16 — narrow phase-local error enum pattern precedent
- `.planning/milestones/v0.6-phases/02-crypto-foundations/` — sign/verify API precedent for consuming `FampSigningKey` / `TrustedVerifyingKey` from envelope code

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp_canonical::canonicalize` + `from_slice_strict` — the only parse/serialize path envelope code should use. Enforces RFC 8785 + duplicate-key rejection already.
- `famp_crypto::canonicalize_for_signature` — prepends the 12-byte domain prefix; envelope sign/verify code calls this, never hand-assembles signing input.
- `famp_crypto::FampSigningKey` + `TrustedVerifyingKey` — opaque key wrappers. `SignedEnvelope::decode` takes a `&TrustedVerifyingKey` (or a trust-lookup closure) to run `verify_strict`.
- `famp_core::{Principal, Instance, MessageId, ConversationId, TaskId, CommitmentId}` — envelope header fields are typed, not raw `String`. Reuse directly.
- `famp_core::ProtocolErrorKind` — 15-category sink; phase-local `EnvelopeDecodeError` converts into the appropriate variant (`Malformed`, `SignatureInvalid`, `Unauthorized`, etc.) at the public boundary.

### Established Patterns
- **Compile-time unrepresentability over runtime rejection** — applied in v0.6 Phase 3 (`#[deny(unreachable_patterns)]` consumer stub for `ProtocolErrorKind`). Phase 1 extends this to INV-10 via type-state and to ENV-12/ENV-09 via variant absence.
- **Phase-local narrow error enums** — precedent in Plans 01-01 and 02-01. Do not reach for `ProtocolErrorKind::Other`.
- **`canonicalize_for_signature` is the only sanctioned signing path** — envelope code never concatenates prefix bytes by hand.
- **Externally-generated conformance vectors (PITFALLS P10)** — §7.1c bytes come from Python `jcs` + `cryptography`. Never self-generate a fixture the library is supposed to validate against.

### Integration Points
- `famp-envelope` sits on top of `famp-canonical`, `famp-crypto`, `famp-core`. Declares them as workspace deps.
- Phase 2 (`famp-fsm`) will consume `SignedEnvelope<B>` and inspect the §7.3a whitelist fields to drive task transitions. Envelope types must expose those fields at a stable accessor surface.
- Phase 3 (`famp-transport` + `MemoryTransport`) will consume `AnySignedEnvelope::decode` for routing. The Any-enum shape is a hard contract for that phase.
- Phase 4 (`famp-transport-http`) pre-routing middleware will call `AnySignedEnvelope::decode` inside a `tower` layer before handler dispatch (TRANS-09).

</code_context>

<deferred>
## Deferred Ideas

- `announce`, `describe`, `propose`, `delegate` message classes — v0.8+ Federation Profile (ENV-04/05/08/11).
- `supersede`, `close`, `cancel_if_not_started`, `revert_transfer` control actions — v0.8+ (ENV-12 full form).
- `capability_snapshot` on `CommitBody` — v0.8 §11.2a Identity & Cards.
- 11 causal relations (ENV-13) — v0.9 Causality & Replay Defense.
- Conversation-bound `request` — v0.8+ negotiation/causality. v0.7 locks `request` to `Standalone`.
- Freshness window / clock-skew validation (§13.1) — v0.9.
- Replay cache + idempotency-key scoping enforcement — v0.9 (the field is present in the envelope, but no enforcement in Phase 1).
- Full envelope-wide random adversarial proptest suite — Phase 3 CONF-05/06/07 + v0.14 adversarial conformance.
- `VerifiedEnvelope` third type-state — explicitly rejected; reconsider only if "parsed but not yet verified" becomes a legitimate downstream state.
- `famp-envelope` FFI / Python / TS bindings — deferred to post-v1 per project constraints.

</deferred>

---

*Phase: 01-minimal-signed-envelope*
*Context gathered: 2026-04-13*
