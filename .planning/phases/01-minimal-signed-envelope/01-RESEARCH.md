# Phase 1: Minimal Signed Envelope — Research

**Researched:** 2026-04-13
**Domain:** FAMP envelope encoding / decoding / signature verification (`famp-envelope` crate)
**Confidence:** HIGH (stack is frozen by CLAUDE.md; substrate shipped in v0.6; spec §7.1/§7.1c is byte-locked)

## Summary

Phase 1 builds `famp-envelope` on top of the shipped v0.6 substrate (`famp-canonical`, `famp-crypto`, `famp-core`). The CLAUDE.md tech stack is frozen, CONTEXT.md locks all major design decisions (type-state `UnsignedEnvelope<B>`/`SignedEnvelope<B>`, sealed `BodySchema` trait, parallel `AnySignedEnvelope` router enum, phase-local narrow `EnvelopeDecodeError`, proptest per-body strategies, §7.1c vector 0 as the load-bearing regression anchor). Research focuses on the narrow remaining questions: exact envelope field set, body field tables, serde patterns that actually compose `deny_unknown_fields` with the envelope-body split, and the decode/verify wiring against `famp_crypto::canonicalize_for_signature`.

The critical serde gotcha is that `#[serde(deny_unknown_fields)]` is incompatible with both `#[serde(flatten)]` and internally-tagged enums in serde 1.0.228 (long-standing, documented, `unreleased` for fix as of this writing). CONTEXT.md's design sidesteps this entirely: the envelope is a plain struct generic over `B: BodySchema` with `body: B` as a regular (non-flattened) field. No tagged enum is nested inside the envelope. `AnySignedEnvelope` does manual dispatch on the already-parsed `class` field — serde never sees an internally-tagged enum. This is the only pattern that reliably combines `deny_unknown_fields` with discriminated body variants in current serde.

**Primary recommendation:** Define `Envelope<B: BodySchema>` as a plain `#[derive(Deserialize)] #[serde(deny_unknown_fields)]` struct whose `body` field has type `B`. Each `BodySchema` impl is also a plain `deny_unknown_fields` struct. `AnySignedEnvelope::decode` first decodes to `Envelope<serde_json::Value>` to read the `class` field, then re-decodes the body into the typed variant. All sign/verify routes through `famp_crypto::canonicalize_for_signature` + `verify_strict` — envelope code never touches the domain prefix or hand-assembles signing bytes.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**A. Signed/Unsigned representation (INV-10 at type level)**

- **D-A1:** Exactly two public envelope states: `UnsignedEnvelope<B>` and `SignedEnvelope<B>`. No third `VerifiedEnvelope` state. `SignedEnvelope::decode` verifies before constructing, so "signed" ≡ "verified by construction".
- **D-A2:** Flow is strictly: builders → `UnsignedEnvelope<B>` → sign → `SignedEnvelope<B>` → wire. There is NO public API that yields an unsigned on-wire envelope. `Option<Signature>` is explicitly rejected — a permanently-half-valid type is the wrong shape.
- **D-A3:** An internal `WireEnvelope` serde struct is permitted for decode plumbing, but is **not** public. `SignedEnvelope::decode(bytes, verifier)` is the only public decode path; it parses the wire struct, strips the `signature` field, rebuilds the signing input via `famp_crypto::canonicalize_for_signature`, runs `verify_strict`, and only then constructs `SignedEnvelope<B>`.
- **D-A4:** Signing API consumes or borrows `UnsignedEnvelope<B>` and returns `SignedEnvelope<B>` — no in-place mutation, no "envelope with optional sig" limbo.

**B. Message class ↔ body coupling**

- **D-B1:** Envelope is generic over a sealed `BodySchema` trait with `CLASS: MessageClass` and `SCOPE: EnvelopeScope` associated consts.
- **D-B2:** Trait is sealed — only the five shipped types implement it: `RequestBody`, `CommitBody`, `DeliverBody`, `AckBody`, `ControlBody`.
- **D-B3:** Internal decode dispatch on envelope `class` is private; public API is typed.
- **D-B4:** `AnySignedEnvelope` enum is public for router-style code in Phase 3/4; typed decode is primary, Any-decode is secondary.
- **D-B5:** **Narrowing is type-level absence, not `Option<_>`.** `ControlBody` exposes only `cancel`. `CommitBody` omits `capability_snapshot` entirely. Adding one is a v0.8+ breaking change.

**C. ENV-14 Scope enforcement**

- **D-C1:** Scope rule on the body via `BodySchema::SCOPE: EnvelopeScope` (`Standalone | Conversation | Task`). No phantom-scope wrappers.
- **D-C2:** Enforced at decode: assert `envelope.class == B::CLASS` and cross-check scope-bearing fields against `B::SCOPE`. Failure → typed `EnvelopeDecodeError` variant, never `ProtocolErrorKind::Other`.
- **D-C3:** **`request` is locked to `Standalone` scope for v0.7.** Conversation-bound request defers to v0.8.
- **D-C4:** Scope locks for the other four classes to be finalized during research against §7.3a whitelist; mechanism is fixed.

**D. Test strategy for ENV-15 / round-trip / adversarial decode**

- **D-D1:** **Golden vector test — vector 0.** §7.1c worked example committed as fixture, asserted byte-for-byte through the full pipeline.
- **D-D2:** **Per-class round-trip** — one deterministic test per class; not proptest.
- **D-D3:** **`deny_unknown_fields` fixtures** — one per class with an injected unknown key; at least one injects the unknown field **nested inside the body**.
- **D-D4:** **Envelope-local adversarial decode cases** included in Phase 1: missing signature; malformed signature encoding; wrong (class, body) pairing; `control.action` other than `cancel`; unknown body field at depth; narrowed-commit body carrying `capability_snapshot`. Each must fail with a **distinct, typed** `EnvelopeDecodeError` variant.
- **D-D5:** **proptest scope — small and typed.** Per-body strategies only. No giant "arbitrary envelope JSON" generator. Focus: round-trip stability and sign/verify invariants. Broad adversarial fuzzing deferred to Phase 3 CONF-05/06/07.

**E. Decode API shape — typed vs untyped dispatch**

- **D-E1:** **Both paths, typed is primary.** `SignedEnvelope::<B>::decode(bytes, verifier)` primary; `AnySignedEnvelope::decode(bytes, verifier)` secondary.
- **D-E2:** Both share a private decode core.

**F. Error shape**

- **D-F1:** Phase-local narrow `EnvelopeDecodeError` / `EnvelopeError`. Converts into `ProtocolErrorKind` at the crate boundary — does not leak `ProtocolErrorKind::Other`.
- **D-F2:** One variant per adversarial case in D-D4.

### Claude's Discretion

- Exact module layout inside `famp-envelope/src/` (one module per body vs `bodies/` submodule vs flat).
- Whether `UnsignedEnvelope<B>` exposes a typed builder or just `new` + field assignment.
- Naming of the sealed trait's sealing technique (private module, sealed supertrait, etc.).
- Exact `EnvelopeDecodeError` variant list beyond D-D4.
- Whether `AnySignedEnvelope` lives in `famp-envelope` directly or in a small `dispatch` module.

### Deferred Ideas (OUT OF SCOPE)

- `announce`, `describe`, `propose`, `delegate` message classes (v0.8+).
- `supersede`, `close`, `cancel_if_not_started`, `revert_transfer` control actions (v0.8+).
- `capability_snapshot` on `CommitBody` (v0.8 §11.2a).
- 11 causal relations (ENV-13) (v0.9).
- Conversation-bound `request` (v0.8+).
- Freshness window / clock-skew validation (v0.9).
- Replay cache + idempotency-key scoping enforcement (v0.9).
- Full envelope-wide random adversarial proptest suite (Phase 3 + v0.14).
- `VerifiedEnvelope` third type-state (explicitly rejected).
- FFI / Python / TS bindings (post-v1).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ENV-01 | `famp-envelope` crate with typed `Envelope` struct matching v0.5.1 §7.1 | §7.1c.2 locks the exact envelope field set (`famp`, `id`, `from`, `to`, `scope`, `class`, `causality?`, `authority`, `ts`, `body`, `signature`, plus optional `terminal_status`/`idempotency_key`/`extensions`). See "Envelope Schema Shape" below. |
| ENV-02 | `deny_unknown_fields` everywhere | Serde pattern section — plain nested structs per body; NO `flatten`, NO internally-tagged enum at envelope level (both break `deny_unknown_fields` in serde 1.0.228). |
| ENV-03 | Mandatory signature enforcement on decode (INV-10) | Type-state: `Option<Signature>` rejected per D-A2; `SignedEnvelope::decode` is the only public path and it verifies before constructing. `WireEnvelope` private; public API cannot express "unsigned on wire". |
| ENV-06 | `ack` body schema (disposition restricted to 5-state FSM) | §8a / §7.1c.2 use `{"disposition": "accepted"}`. Values for v0.7 narrowed FSM: `accepted`, `rejected`, `received`, `completed`, `failed`, `cancelled` — see Ack Body section; final set TBD against Phase 2 FSM. |
| ENV-07 | `request` body schema | Request body is the `scope`+`bounds` shape inherited from v0.5 §7.4 (opaque `scope` object, required `bounds` per §9.3). See Request Body section. |
| ENV-09 (narrowed) | `commit` body without capability_snapshot | §8a.2 makes `capability_snapshot` REQUIRED; narrowing omits it as type-level absence (D-B5). Inline `// v0.8 §11.2a` doc comment in `CommitBody`. |
| ENV-10 | `deliver` body + envelope `terminal_status` | §8a.3 `interim: bool` gates envelope-level `terminal_status`; `error_detail` required iff `terminal_status = failed`; `provenance` required on terminal. Cross-field validation at decode. |
| ENV-12 (cancel-only) | `control` body restricted to `cancel` | §8a.4 full catalog is `{cancel, supersede, close, cancel_if_not_started, revert_transfer}`; v0.7 exposes only `cancel` — as type-level absence, not Option (D-B5). `target` field stays but defaults to `task`. |
| ENV-14 | Scope enforcement (standalone/conversation/task) | `BodySchema::SCOPE` const + decode-time cross-check (D-C1/C2). Request locked to Standalone per D-C3. |
| ENV-15 | Signed round-trip test per class | §7.1c vector 0 as Vector 0; one deterministic per-class test; proptest per-body strategies (D-D1/D2/D5). |
</phase_requirements>

## Standard Stack

**All versions are frozen by CLAUDE.md — do not re-litigate.** `famp-envelope` declares these as workspace dependencies (already pinned in root `Cargo.toml`).

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` | 1.0.228 | Derive `Serialize`/`Deserialize` on envelope + body structs | Already workspace dep; sole supported path for `serde_jcs` integration |
| `serde_json` | 1.0.149 | `Value` intermediate for strip-signature + re-canonicalize flow | Already workspace dep; same reference impl serde_jcs is built on |
| `famp-canonical` | path | `canonicalize(&Value)`, `from_slice_strict` | **Only sanctioned parse/canonicalize path**; enforces RFC 8785 + duplicate-key rejection. Envelope code MUST NOT call `serde_jcs` directly. |
| `famp-crypto` | path | `canonicalize_for_signature`, `verify_strict`, `FampSignature`, `TrustedVerifyingKey`, `FampSigningKey` | **Only sanctioned signing-input path** — prepends domain prefix internally. Envelope code MUST NOT touch `DOMAIN_PREFIX` directly. |
| `famp-core` | path | `Principal`, `Instance`, `MessageId`, `ConversationId`, `TaskId`, `CommitmentId`, `ProtocolErrorKind`, `AuthorityScope` | Envelope fields reuse these typed newtypes directly. |
| `thiserror` | 2.0.18 | Derive `EnvelopeDecodeError` / `EnvelopeError` | Libs always use typed errors; never `anyhow` in a `famp-*` crate. |

### Supporting (dev-dependencies)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | 1.11.0 | Per-body strategies for round-trip + sign/verify invariants | Per D-D5; strategies per body variant, no giant envelope generator |
| `insta` | 1.47.2 | Snapshot canonical bytes + signing-input bytes | Asserts byte stability on §7.1c Vector 0 and each per-class fixture |
| `hex` | workspace | Decode §7.1c.3/.5/.6 hex fixtures in tests | Already dev-dep in famp-crypto |
| `base64` | 0.22.1 | URL_SAFE_NO_PAD — already wrapped by `FampSignature::from_b64url` | Envelope code should never call `base64` directly; go through `FampSignature`/`TrustedVerifyingKey` b64url ctors |

**Not needed this phase:** `stateright` (FSM work lands in Phase 2), `simd-json`, `sonic-rs`, `ed25519-dalek` (only reached via `famp-crypto`).

## Architecture Patterns

### Recommended Crate Layout

```
crates/famp-envelope/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public re-exports; crate-level invariants
    ├── error.rs            # EnvelopeDecodeError + conversion to ProtocolErrorKind
    ├── scope.rs            # EnvelopeScope enum (Standalone | Conversation | Task)
    ├── class.rs            # MessageClass enum (Request, Commit, Deliver, Ack, Control)
    ├── wire.rs             # Private WireEnvelope serde struct — decode plumbing only
    ├── envelope.rs         # UnsignedEnvelope<B> + SignedEnvelope<B> type-state
    ├── body/
    │   ├── mod.rs          # sealed BodySchema trait
    │   ├── request.rs      # RequestBody + RequestScope (Standalone)
    │   ├── commit.rs       # CommitBody — NO capability_snapshot
    │   ├── deliver.rs      # DeliverBody + TerminalStatus plumbing
    │   ├── ack.rs          # AckBody + AckDisposition enum
    │   └── control.rs      # ControlBody — cancel-only enum
    └── dispatch.rs         # AnySignedEnvelope + AnySignedEnvelope::decode
```

**Why this shape:** one file per body keeps `deny_unknown_fields` struct diffs scoped and makes the ENV-12/ENV-09 narrowings one-file reviews. The `body/mod.rs` seal technique: private supertrait in a private module (`mod private { pub trait Sealed {} }`), exposed via `pub trait BodySchema: /* ... */ + private::Sealed`.

### Envelope Schema Shape (from §7.1c)

The §7.1c.2 minimal `ack` envelope is **the canonical field set**. Extracted verbatim (keys reordered alphabetically per RFC 8785):

```json
{
  "authority": "advisory",
  "body": { "disposition": "accepted" },
  "causality": { "ref": "...", "rel": "acknowledges" },
  "class": "ack",
  "famp": "0.5.1",
  "from": "agent:example.test/alice",
  "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b",
  "scope": "standalone",
  "to": "agent:example.test/bob",
  "ts": "2026-04-13T00:00:00Z"
}
```

After signing, `"signature"` is added as an envelope sibling (§7.1c.7).

**Required envelope fields** (all classes):

| Field | Rust type | Notes |
|-------|-----------|-------|
| `famp` | `&'static str` = `"0.5.1"` | **MUST** equal `FAMP_SPEC_VERSION` exact; reject on any other value (§19 — `unsupported_version`). Suggest a private `FampVersion` unit-ish struct that serialize/deserialize as the literal `"0.5.1"`. |
| `id` | `MessageId` | UUIDv7 from `famp-core`. Already has `Serialize`/`Deserialize` via hyphenated form (rejects 32-char simple form). |
| `from` | `Principal` | `famp-core`; deserialize-validated. |
| `to` | `Principal` | **Load-bearing for §7.1 recipient anti-replay.** Inside signing input. |
| `scope` | `EnvelopeScope` | Enum `{ Standalone, Conversation, Task }`. §7.1c.2 uses `"standalone"`. Snake-case serde rename. |
| `class` | `MessageClass` | Enum `{ Request, Commit, Deliver, Ack, Control }`. Snake-case. |
| `authority` | `AuthorityScope` | From `famp-core` — 5-level ladder. |
| `ts` | `String` (RFC 3339) | For v0.7 keep as wrapped `Timestamp(String)` newtype; defer full `time::OffsetDateTime` parsing to v0.9 freshness-window work. §7.1c.2 uses `"2026-04-13T00:00:00Z"`. Canonical form is the bytes that arrived — do not round-trip through a parsed type. |
| `body` | `B: BodySchema` | Plain generic, NOT flatten, NOT tagged. |

**Optional envelope fields:**

| Field | Rust type | Notes |
|-------|-----------|-------|
| `causality` | `Option<Causality>` | `{rel: Relation, ref: MessageId}`. v0.7 only needs `acknowledges`, `requests`, `commits`, `delivers`, `cancels` (not all 11 ENV-13 relations). `ref` is `MessageId` newtype. **Serialize note:** `#[serde(skip_serializing_if = "Option::is_none")]` so round-trip is byte-stable on envelopes that lack it (§7.1c includes it; request bodies may not). |
| `terminal_status` | `Option<TerminalStatus>` | Enum `{ Completed, Failed, Cancelled }`. REQUIRED iff body is `deliver` with `interim=false`. Present only on `deliver` class per v0.7 (Phase 2 FSM will confirm). Cross-field validation at decode. |
| `idempotency_key` | `Option<IdempotencyKey>` | 16-byte / 22-char base64url-unpadded newtype. Format-validate at decode per §7.1b, but **no enforcement of uniqueness in v0.7** — that's v0.9. |
| `extensions` | `Option<BTreeMap<String, Value>>` | Per §7.1. v0.7 accepts and round-trips but does not dispatch on critical-extensions. |
| `signature` | `FampSignature` on `SignedEnvelope<B>` only; **absent** from `WireEnvelope` decode path after strip | See "Signature Binding" below. |

**Serialize order does not matter** — `famp_canonical::canonicalize` re-sorts UTF-16 code-unit key order before signing/verifying. Rust struct field order is irrelevant.

### Body Variants (v0.7 shipped set)

#### `RequestBody` — Standalone scope
Inherits the v0.5 §7.4 shape. CONTEXT.md D-C3 locks it to `Standalone` for v0.7.

| Field | Type | Req/Opt | Notes |
|-------|------|---------|-------|
| `scope` | `serde_json::Value` (opaque) | REQUIRED | Domain-specific work description; opaque to FSM. |
| `bounds` | `Bounds` | REQUIRED | Same struct used by propose/commit/delegate. See "Bounds" below. |
| `natural_language_summary` | `Option<String>` | OPTIONAL | No length cap; no Unicode normalization (PITFALLS P3). |

`const CLASS: MessageClass = MessageClass::Request;`
`const SCOPE: EnvelopeScope = EnvelopeScope::Standalone;`

#### `CommitBody` — narrowed per ENV-09
§8a.2 minus `capability_snapshot`. Inline doc: `// v0.7 narrowing — capability_snapshot omitted; defers to v0.8 §11.2a Identity & Cards.`

| Field | Type | Req/Opt | Notes |
|-------|------|---------|-------|
| `scope` | `serde_json::Value` | REQUIRED | Same rules as RequestBody.scope. |
| `scope_subset` | `Option<bool>` (default false on serialize via `skip_serializing_if`) | OPTIONAL | FSM-inspected (§7.3a). |
| `bounds` | `Bounds` | REQUIRED | |
| `accepted_policies` | `Vec<String>` | REQUIRED | Policy IDs. |
| `delegation_permissions` | `Option<Value>` | OPTIONAL | Opaque in v0.7; `famp-delegate` is v0.11. |
| `reporting_obligations` | `Option<Value>` | OPTIONAL | Opaque. |
| `terminal_condition` | `serde_json::Value` | REQUIRED | Opaque. |
| `conditions` | `Option<Vec<Value>>` | OPTIONAL | §11.4 conditional commitment — opaque. |
| `natural_language_summary` | `Option<String>` | OPTIONAL | |

**Decode check:** if the incoming JSON has a `capability_snapshot` key, it is an unknown field under `deny_unknown_fields` and MUST fail with `EnvelopeDecodeError::UnknownBodyField { class: Commit, field: "capability_snapshot" }`. Test fixture D-D4 explicitly covers this.

`const CLASS = Commit; const SCOPE = EnvelopeScope::Task;` (commits bind to a task)

#### `DeliverBody` — §8a.3 full

| Field | Type | Req/Opt | Notes |
|-------|------|---------|-------|
| `interim` | `bool` | REQUIRED | §7.3a FSM-inspected. `false` ↔ envelope-level `terminal_status` MUST be present; `true` ↔ `terminal_status` MUST be absent. **Cross-field validation at decode.** |
| `artifacts` | `Option<Vec<Artifact>>` | OPTIONAL | `{id: ArtifactId, media_type: String, size: u64}`. `ArtifactId` from `famp-core` (`sha256:<hex>`). |
| `result` | `Option<Value>` | OPTIONAL | Opaque payload. |
| `usage_metrics` | `Option<Value>` | OPTIONAL | Opaque in v0.7. |
| `error_detail` | `Option<ErrorDetail>` | CONDITIONAL | REQUIRED iff envelope `terminal_status == Failed`. `{category: ErrorCategory, message: String, diagnostic: Option<Value>}`. Cross-field validation. |
| `provenance` | `Option<Value>` | CONDITIONAL | REQUIRED on terminal deliveries; OPTIONAL on interim. Opaque in v0.7. |
| `natural_language_summary` | `Option<String>` | OPTIONAL | |

`const CLASS = Deliver; const SCOPE = EnvelopeScope::Task;`

#### `AckBody` — §7.1c uses `{"disposition": "accepted"}`
v0.5 `ack` body is small. Disposition values must match the 5-state FSM Phase 2 will ship. Candidate set (to be confirmed against Phase 2 FSM):

| Field | Type | Req/Opt | Notes |
|-------|------|---------|-------|
| `disposition` | `AckDisposition` | REQUIRED | Enum: `accepted`, `rejected`, `received`, `completed`, `failed`, `cancelled`. §7.1c vector 0 uses `accepted`. Final enum set locked during Phase 2 FSM planning; for Phase 1, ship the superset above and let Phase 2 tighten. |
| `reason` | `Option<String>` | OPTIONAL | Human-readable. |

`const CLASS = Ack; const SCOPE = EnvelopeScope::Task;` (ack references a task/commitment via causality.ref)

**Open decision:** Ack scope may be Standalone in v0.7 if we keep §7.1c vector 0 as canonical (which declares `"scope": "standalone"`). **Resolution for planner:** Ack is Standalone in v0.7 to match Vector 0 byte-for-byte. Document inline; revisit in Phase 2 FSM planning if the task FSM needs task-scoped ack.

#### `ControlBody` — ENV-12 cancel-only narrowing

§8a.4 full catalog has `action ∈ {cancel, supersede, close, cancel_if_not_started, revert_transfer}`. v0.7 exposes only `cancel` — **as a variant enum with one variant**, not `Option<Action>`.

```rust
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlBody {
    pub target: ControlTarget,  // v0.7: only `Task` — same narrowing rule applies
    pub action: ControlAction,  // enum { Cancel } — single variant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<ControlDisposition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_ids: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlAction { Cancel }  // v0.7 — single variant. v0.8+ adds others.

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlTarget { Task }  // v0.7 — task only. v0.8+ adds conversation/commitment/etc.
```

Decoding `{"action": "supersede"}` fails as "unknown enum variant" — serde's own error, surfaced through `EnvelopeDecodeError::InvalidControlAction` via typed mapping.

`const CLASS = Control; const SCOPE = EnvelopeScope::Task;`

### Shared Bounds Struct

Used by `RequestBody`, `CommitBody` (and later `PropBody`, `DelegateBody` in v0.8+). §9.3 requires ≥2 keys from the set. Keep the struct loose in v0.7 (all fields optional) and validate the "≥2 keys" rule at decode with a custom post-deserialize check.

```rust
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,  // RFC 3339; deferred parsing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,    // {amount: String, unit: String}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_scope: Option<AuthorityScope>,  // famp-core
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_artifact_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_floor: Option<f64>,  // 0..=1 — reject NaN/Inf at decode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursion_depth: Option<u8>,
}
```

**Critical:** `budget.amount` is STRING not NUMBER. This avoids 2^53 precision loss (PITFALLS P2, §8a). Per-body proptest generators MUST NOT produce numeric amounts.

## Serde Pattern Recommendations

This is the section where training-data intuition is most likely to be wrong. **Read the serde incompatibility table carefully.**

### The `deny_unknown_fields` Composition Table

| Pattern | Works with `deny_unknown_fields`? | Evidence |
|---------|-----------------------------------|----------|
| Plain struct with nested plain structs | ✅ YES | Standard serde usage |
| Struct with `#[serde(flatten)]` field | ❌ **NO** — `deny_unknown_fields` is silently ignored on the outer struct | Long-standing serde issue (`serde#1547`, `serde#1600`). `deny_unknown_fields` does not propagate through `flatten` because the flatten implementation uses `deserialize_any` / buffered approach that loses the schema boundary. Confidence: HIGH (documented, reproduced, upstream open). |
| Internally-tagged enum `#[serde(tag = "type")]` | ❌ **BROKEN** with `deny_unknown_fields` — intermittent false negatives + false positives | serde's internally-tagged enum implementation also buffers via `Content`; `deny_unknown_fields` fails on tag siblings. `serde#1358`. |
| Externally-tagged enum (default enum repr) | ✅ works, but wire format is `{"Variant": {...}}` — **WRONG for FAMP** where the discriminator is at envelope level, not body level | N/A — not the envelope shape. |
| Adjacently-tagged enum `#[serde(tag = "t", content = "c")]` | ⚠️ Works but wrong shape (adds a `content` wrapper field) | Not the FAMP envelope shape. |
| Plain generic struct `Envelope<B>` with `body: B` where `B` is a concrete plain struct | ✅ YES | **This is CONTEXT.md's choice and it is the only pattern that works.** Each monomorphization is a concrete struct with full `deny_unknown_fields` enforcement. |

**Conclusion:** CONTEXT.md's design is not just a preference — it is the **only** serde pattern in 1.0.228 that actually composes `deny_unknown_fields` at both envelope and body level. Any drive-by refactor to "clean up" the envelope using `#[serde(flatten)]` or an internally-tagged `Body` enum silently breaks ENV-02. Lock this in the plan.

### How `AnySignedEnvelope` Decode Works Without Tagged Enums

```rust
// dispatch.rs (sketch)
pub enum AnySignedEnvelope {
    Request(SignedEnvelope<RequestBody>),
    Commit(SignedEnvelope<CommitBody>),
    Deliver(SignedEnvelope<DeliverBody>),
    Ack(SignedEnvelope<AckBody>),
    Control(SignedEnvelope<ControlBody>),
}

impl AnySignedEnvelope {
    pub fn decode(bytes: &[u8], verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>
    {
        // Step 1: parse once as serde_json::Value via famp_canonical::from_slice_strict
        //   — this already rejects duplicate keys (§4a)
        let value: serde_json::Value = famp_canonical::from_slice_strict(bytes)?;

        // Step 2: peek the `class` field (wire-level string)
        let class_str = value.get("class").and_then(Value::as_str)
            .ok_or(EnvelopeDecodeError::MissingClass)?;

        // Step 3: dispatch to per-class typed decode
        //   (each re-deserializes the same Value into Envelope<B> via serde_json::from_value)
        match class_str {
            "request" => Ok(Self::Request(SignedEnvelope::<RequestBody>::decode_value(value, verifier)?)),
            "commit"  => Ok(Self::Commit(SignedEnvelope::<CommitBody>::decode_value(value, verifier)?)),
            "deliver" => Ok(Self::Deliver(SignedEnvelope::<DeliverBody>::decode_value(value, verifier)?)),
            "ack"     => Ok(Self::Ack(SignedEnvelope::<AckBody>::decode_value(value, verifier)?)),
            "control" => Ok(Self::Control(SignedEnvelope::<ControlBody>::decode_value(value, verifier)?)),
            other     => Err(EnvelopeDecodeError::UnknownClass { found: other.into() }),
        }
    }
}
```

The private `decode_value` core is shared with the typed `SignedEnvelope::<B>::decode` path (D-E2) and does: class assertion, scope assertion, signature strip, canonicalize, verify_strict, construct.

## Signature Binding (what gets signed)

From §7.1a + §7.1c + the shipped `famp_crypto::canonicalize_for_signature`:

1. Start with the envelope **minus** the `signature` field (§7.1c.2 = `WireEnvelope` sans signature).
2. Canonicalize via `famp_canonical::canonicalize(&value)` — returns RFC 8785 bytes (324 bytes for Vector 0).
3. Prepend the 12-byte domain prefix `b"FAMP-sig-v1\x00"` — done internally by `famp_crypto::canonicalize_for_signature` or by `sign_canonical_bytes`; envelope code calls these, never hand-assembles.
4. `ed25519_dalek::SigningKey::sign(&signing_input)` — this happens inside `FampSigningKey`.
5. Attach base64url-unpadded signature as `signature` sibling.

Verification (per §7.1c.8):
1. Parse bytes via `famp_canonical::from_slice_strict` (duplicate-key reject + strict decode).
2. Extract signature string → `FampSignature::from_b64url` (rejects padding, standard alphabet, wrong length).
3. **Remove the `signature` field from the parsed `Value`.** Use `Value::as_object_mut().unwrap().remove("signature")`.
4. `famp_crypto::canonicalize_for_signature(&stripped_value)` returns `DOMAIN_PREFIX || canonical_bytes`.
5. Pass those bytes to `verify_strict` via `famp_crypto::verify_canonical_bytes(&verifier, &canonical_bytes, &sig)` — **note**: `verify_canonical_bytes` itself prepends the prefix, so step 4 above is slightly wrong if we call it. Cleaner: strip signature → call `famp_crypto::verify_value(&verifier, &stripped_value, &sig)` which canonicalizes + prefixes internally.
6. On `Err(CryptoError::VerificationFailed)` → `EnvelopeDecodeError::SignatureInvalid`.

**API call summary (exact paths):**
- **Sign:** `famp_crypto::sign_value(&signing_key, &wire_sans_signature)` → `FampSignature`. One call. Do not hand-canonicalize.
- **Verify:** `famp_crypto::verify_value(&trusted_key, &wire_sans_signature, &signature)` → `Result<(), CryptoError>`. One call.

Envelope code never touches `DOMAIN_PREFIX`, never calls `canonicalize` followed by manual prefix bytes, never reaches for `ed25519_dalek` directly. This is enforced by the v0.6 API surface and is a **hard review gate**.

### Vector 0 Pipeline — the load-bearing test

| Step | Expected bytes (hex) | From |
|------|---------------------|------|
| Canonical JSON (stripped) | `7b2261757468...` (324 bytes) | §7.1c.3 |
| Domain prefix | `46414d502d7369672d7631 00` (12 bytes) | §7.1c.4 |
| Signing input | prefix ‖ canonical (336 bytes) | §7.1c.5 |
| Signature (raw) | `9366aaced854c7898735908d2e2d973208905fd80e2f93fe505710f58f0ed1fc92e3b9d7a19b30b2cf184f703552dafcf91ca81321f57fa689d1a96865d0b608` | §7.1c.6 |
| Signature (b64url) | `k2aqzthUx4mHNZCNLi2XMgiQX9gOL5P-UFcQ9Y8O0fyS47nXoZswss8YT3A1Utr8-RyoEyH1f6aJ0aloZdC2CA` | §7.1c.6 |

Test keypair: RFC 8032 §7.1 Test 1 — secret `9d61b19d...7f60`, pubkey `d75a9801...511a`. Already in `famp-crypto` fixtures.

The Phase 1 Vector 0 test must:
1. Deserialize the §7.1c.7 wire envelope through `SignedEnvelope::<AckBody>::decode` with a `TrustedVerifyingKey::from_bytes(&[0xd7, 0x5a, ...])` and assert Ok.
2. Round-trip: serialize that typed envelope back → byte-compare canonical bytes against §7.1c.3 expected → insta snapshot.
3. Re-sign the stripped envelope with the Test 1 secret and assert the resulting `FampSignature` matches §7.1c.6 byte-for-byte (Ed25519 is deterministic per RFC 8032 §5.1.6).

If any of these fail, the whole `famp-envelope` crate is non-conformant — hard stop.

## Type-Level Enforcement Patterns

### Signed/Unsigned type-state (INV-10)

```rust
pub struct UnsignedEnvelope<B: BodySchema> {
    pub famp: FampVersion,
    pub id: MessageId,
    pub from: Principal,
    pub to: Principal,
    pub scope: EnvelopeScope,
    pub class: MessageClass,  // always equals B::CLASS — constructor enforces
    pub authority: AuthorityScope,
    pub ts: Timestamp,
    pub causality: Option<Causality>,
    pub terminal_status: Option<TerminalStatus>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub extensions: Option<BTreeMap<String, Value>>,
    pub body: B,
}

pub struct SignedEnvelope<B: BodySchema> {
    inner: UnsignedEnvelope<B>,
    signature: FampSignature,
    // No public constructor except via sign() / decode()
}

impl<B: BodySchema> UnsignedEnvelope<B> {
    pub fn sign(self, sk: &FampSigningKey) -> Result<SignedEnvelope<B>, EnvelopeError> {
        let wire_value = self.to_wire_value()?;  // serde_json::to_value
        let sig = famp_crypto::sign_value(sk, &wire_value)?;
        Ok(SignedEnvelope { inner: self, signature: sig })
    }
}

impl<B: BodySchema> SignedEnvelope<B> {
    pub fn decode(bytes: &[u8], verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError> { /* strip → verify → construct */ }

    pub fn encode(&self) -> Result<Vec<u8>, EnvelopeError> {
        // Serialize inner to Value, insert signature b64url, canonicalize
        // — or plain serde_json::to_vec if wire form is non-canonical
    }

    pub fn body(&self) -> &B { &self.inner.body }
    pub fn from_principal(&self) -> &Principal { &self.inner.from }
    // ... read accessors only
}
```

**Why this shape works:**
- `UnsignedEnvelope::sign` consumes `self` and returns `SignedEnvelope` → no "envelope with sig=None" limbo.
- `SignedEnvelope::new` is private — only `decode` or `sign` can produce one. So "signed" ≡ "verified by construction".
- `Option<Signature>` does not appear anywhere in the public API — enforces D-A2.

### Unrepresentable `(class, body)` mismatches (D-B1/B2)

```rust
mod private { pub trait Sealed {} }

pub trait BodySchema:
    Serialize + DeserializeOwned + private::Sealed + Sized + 'static
{
    const CLASS: MessageClass;
    const SCOPE: EnvelopeScope;
}

impl private::Sealed for RequestBody {}
impl private::Sealed for CommitBody {}
// ... five only
```

At decode, `SignedEnvelope::<B>::decode_value` asserts `parsed.class == B::CLASS` and `parsed.scope == B::SCOPE`. Any mismatch → `EnvelopeDecodeError::ClassMismatch { expected, got }` or `ScopeMismatch { expected, got }`.

### ENV-12 cancel-only

Enforced at the type level by `ControlAction` being an enum with a single variant `Cancel`. Serde rejects `"supersede"` as unknown variant at deserialize time — no runtime check needed.

### ENV-09 no capability_snapshot

Enforced by the field being absent from `CommitBody`. `deny_unknown_fields` on `CommitBody` rejects any incoming `capability_snapshot` key as `UnknownField`.

## Round-Trip Pitfalls (canonicalization + serde)

1. **`f64` for `confidence_floor`.** RFC 8785 §3.2.2.3 mandates ECMAScript number formatting. `serde_jcs` handles this via `ryu-js`, but only if the JSON number literal survives. Do not let serde turn `0.5` into anything weird. Trust `serde_json` default number handling; do NOT enable `arbitrary_precision` (CLAUDE.md forbids it — changes canonicalization).
2. **NaN / Infinity.** RFC 8785 forbids them. `serde_json` by default serializes `f64::NAN` as `null` (!) — an implicit data corruption. Reject at decode on the proptest boundary by filtering NaN/Inf out of `confidence_floor` generators.
3. **`budget.amount` is STRING.** If a test fixture uses a numeric amount, it breaks 2^53 semantics. All proptest strategies: `any::<u64>().prop_map(|x| x.to_string())`.
4. **Timestamps as opaque strings.** Do not parse `ts` through `time::OffsetDateTime` and reserialize — you may lose the exact byte representation that was signed. Keep as `Timestamp(String)` newtype that validates RFC 3339 shape on decode (regex or simple len+char check) but preserves the input bytes on re-serialize.
5. **UUIDs are hyphenated 36-char.** `famp-core` already rejects the simple form. Do not accept a 32-char UUID.
6. **Unicode normalization is FORBIDDEN (PITFALLS P3 / §4a).** `serde_json` does not normalize; `serde_jcs` does not normalize. Do not introduce any text-processing step on `natural_language_summary`. Round-trip must be byte-exact.
7. **Duplicate keys.** `serde_json::from_slice` accepts duplicate keys silently (last-wins). **This is why envelope code must use `famp_canonical::from_slice_strict`** which rejects duplicates per RFC 8785.
8. **Empty `extensions` map vs absent.** `Some(empty map)` serializes as `"extensions":{}` but absent field is no key at all — different canonical bytes, different signature. Use `#[serde(skip_serializing_if = "Option::is_none")]` and at the UX level, prefer `None` over `Some(BTreeMap::new())`.
9. **`serde_json::Value` key ordering.** `Value::Object` is backed by `BTreeMap` only if `preserve_order` feature is OFF (default). **DO NOT enable `preserve_order`** — CLAUDE.md forbids it. This means when we decode `value.remove("signature")` the rest of the object preserves canonical order by virtue of BTreeMap's sorted-key invariant, and re-canonicalizing is byte-stable.
10. **Signature strip must happen on `Value`, not on the typed struct.** Round-trip: `bytes → from_slice_strict → Value → clone → value_no_sig → canonicalize → verify`. Never: decode to typed → serialize back → canonicalize. The second path risks a field you don't know about being dropped on the typed decode step, producing a different signing input.

## Error Taxonomy — `EnvelopeDecodeError`

One variant per adversarial case in D-D4 plus ancillary variants for decode plumbing. Phase-local narrow enum; converts to `famp_core::ProtocolErrorKind` at the crate boundary via `impl From<EnvelopeDecodeError> for ProtocolError`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeDecodeError {
    // Wire-level (pre-verify)
    #[error("malformed envelope JSON: {0}")]
    MalformedJson(#[from] famp_canonical::CanonicalError),  // includes dup-key, bad UTF-8

    #[error("missing required envelope field: {field}")]
    MissingField { field: &'static str },

    #[error("unknown envelope field: {field}")]
    UnknownEnvelopeField { field: String },

    #[error("unknown body field at depth: {class}.{field}")]
    UnknownBodyField { class: MessageClass, field: String },

    #[error("envelope.famp = {found:?}; expected \"0.5.1\"")]
    UnsupportedVersion { found: String },

    #[error("envelope.class = {found:?} not a known message class")]
    UnknownClass { found: String },

    #[error("envelope.class = {got} does not match expected {expected}")]
    ClassMismatch { expected: MessageClass, got: MessageClass },

    #[error("envelope.scope = {got} does not match expected {expected} for class {class}")]
    ScopeMismatch { class: MessageClass, expected: EnvelopeScope, got: EnvelopeScope },

    // Signature (INV-10)
    #[error("envelope is unsigned — signature field absent")]
    MissingSignature,

    #[error("signature encoding malformed")]
    InvalidSignatureEncoding(#[from] famp_crypto::CryptoError),

    #[error("signature verification failed (verify_strict)")]
    SignatureInvalid,

    // Cross-field
    #[error("control.action = {found:?}; v0.7 supports only `cancel`")]
    InvalidControlAction { found: String },

    #[error("deliver.interim = true but envelope.terminal_status is set")]
    InterimWithTerminalStatus,

    #[error("deliver.interim = false but envelope.terminal_status is absent")]
    TerminalWithoutStatus,

    #[error("deliver.error_detail required when terminal_status = failed")]
    MissingErrorDetail,

    #[error("deliver.provenance required on terminal delivery")]
    MissingProvenance,

    #[error("bounds requires ≥2 keys from §9.3 set; got {count}")]
    InsufficientBounds { count: usize },

    #[error("body field validation failed: {0}")]
    BodyValidation(String),
}
```

**Conversion into `ProtocolErrorKind`:**
- All of `Malformed*`, `Missing*`, `Unknown*`, `Unsupported*`, `InvalidControlAction`, `Interim*`, `Terminal*`, `Missing{ErrorDetail,Provenance}`, `InsufficientBounds`, `BodyValidation` → `ProtocolErrorKind::Malformed`.
- `MissingSignature`, `InvalidSignatureEncoding`, `SignatureInvalid` → `ProtocolErrorKind::Unauthorized`.
- `UnsupportedVersion` → `ProtocolErrorKind::UnsupportedVersion`.
- **Never** route to `ProtocolErrorKind::Other`.

One D-D4 variant maps to one error variant maps to exactly one test assertion.

## Proptest Strategy

Per D-D5, strategies are small and typed. One strategy per body variant; one strategy for envelope header fields; combination via `prop_oneof` only at the top-level `AnySignedEnvelope` level if needed.

```rust
// tests/prop_roundtrip.rs (sketch)

fn arb_principal() -> impl Strategy<Value = Principal> { /* ... */ }
fn arb_message_id() -> impl Strategy<Value = MessageId> { Just(MessageId::new_v7()) }

fn arb_bounds_min2() -> impl Strategy<Value = Bounds> {
    // Generate at least 2 of the 8 bound fields — avoids InsufficientBounds rejection
    // Use prop_flat_map to pick a subset of size ≥ 2
}

fn arb_request_body() -> impl Strategy<Value = RequestBody> {
    (prop::json::value(), arb_bounds_min2(), option::of(".*"))
        .prop_map(|(scope, bounds, nls)| RequestBody { scope, bounds, natural_language_summary: nls })
}

fn arb_envelope_header<B: BodySchema>(body: B) -> impl Strategy<Value = UnsignedEnvelope<B>> { /* ... */ }

proptest! {
    #[test]
    fn request_sign_verify_roundtrip(body in arb_request_body()) {
        let (sk, vk) = keypair_fixture();
        let unsigned: UnsignedEnvelope<RequestBody> = arb_envelope_header(body).new_tree(...).current();
        let signed = unsigned.clone().sign(&sk).unwrap();
        let bytes = signed.encode().unwrap();
        let decoded = SignedEnvelope::<RequestBody>::decode(&bytes, &vk).unwrap();
        prop_assert_eq!(unsigned, decoded.into_unsigned());
    }

    #[test]
    fn tampered_canonical_fails(body in arb_request_body()) {
        let (sk, vk) = keypair_fixture();
        let signed = /* ... */;
        let mut bytes = signed.encode().unwrap();
        bytes.last_mut().map(|b| *b ^= 0x01);  // flip a byte
        let res = SignedEnvelope::<RequestBody>::decode(&bytes, &vk);
        prop_assert!(matches!(res, Err(EnvelopeDecodeError::MalformedJson(_) | Err(EnvelopeDecodeError::SignatureInvalid))));
    }
}
```

**Shrinkability pointers:**
- Keep opaque `Value` generators SHALLOW (max depth 2, max keys 3). Proptest shrinking on deep JSON is painful.
- For `scope` and `bounds` objects, prefer hand-built fixed generators over `prop::json::value()` — the debug output is much cleaner.
- For failure-reproduction, use `insta` to snapshot the failing canonical bytes on first run, then freeze the seed.

## Fixture Strategy

Committed test fixtures under `crates/famp-envelope/tests/fixtures/`:

```
fixtures/
├── vector_0/                 # §7.1c — load-bearing
│   ├── envelope.json         # §7.1c.7 wire form
│   ├── canonical.hex         # §7.1c.3 expected canonical bytes
│   ├── signing_input.hex     # §7.1c.5 expected (prefix || canonical)
│   ├── signature.hex         # §7.1c.6 64-byte raw
│   └── signature.b64url      # §7.1c.6 base64url form
├── roundtrip/                # D-D2 — one per class
│   ├── request.json
│   ├── commit.json
│   ├── deliver_interim.json
│   ├── deliver_terminal.json
│   ├── ack.json              # = vector_0/envelope.json — alias ok
│   └── control_cancel.json
├── adversarial/              # D-D3 + D-D4
│   ├── missing_signature.json
│   ├── bad_signature_padded.json
│   ├── bad_signature_stdalphabet.json
│   ├── unknown_envelope_field.json
│   ├── unknown_body_field_top.json
│   ├── unknown_body_field_nested.json  # D-D3 depth requirement
│   ├── class_body_mismatch.json        # envelope.class=request with commit body
│   ├── control_supersede.json          # ENV-12 enforcement
│   ├── commit_with_capability_snapshot.json  # ENV-09 narrowing
│   ├── deliver_interim_with_terminal_status.json
│   └── deliver_failed_without_error_detail.json
```

Each adversarial fixture has a paired assertion in `tests/adversarial.rs` matching against a specific `EnvelopeDecodeError` variant. Insta snapshots for `roundtrip/` canonical bytes.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RFC 8785 canonicalization | Custom sorter | `famp_canonical::canonicalize` | Already shipped; 12/12 conformance gate; `serde_jcs` wrapper with fallback plan |
| Strict JSON parse with dup-key rejection | `serde_json::from_slice` + manual check | `famp_canonical::from_slice_strict` | Already shipped; enforces RFC 8785 decode side |
| Signing input assembly | `DOMAIN_PREFIX.to_vec() + canonical` | `famp_crypto::sign_value` / `verify_value` | Hard rule from v0.6 STATE.md — envelope code never touches the prefix |
| Ed25519 verify path | `VerifyingKey::verify` | `famp_crypto::verify_value` (routes to `verify_strict`) | Non-strict verify is forbidden per §7.1b + STATE.md |
| Base64url signature decode | `base64::decode` + length check | `FampSignature::from_b64url` | Rejects padding, standard alphabet, wrong length already |
| UUID parsing | `uuid::Uuid::parse_str` directly | `MessageId::from_str` (or derive `Deserialize`) | Already rejects 32-char simple form |
| `Principal` parsing | Hand-written `agent:<authority>/<name>` regex | `famp_core::Principal::Deserialize` | Shipped in v0.6 |
| Tagged-enum wire format for bodies | `#[serde(tag = "class")]` on an outer `Body` enum | Generic `Envelope<B>` + `AnySignedEnvelope` manual dispatch | serde's internally-tagged + `deny_unknown_fields` is broken (see table above) |
| Signature strip | Manual string munging | `Value::as_object_mut().unwrap().remove("signature")` | `BTreeMap` backing of `serde_json::Value` preserves canonical key order on strip |

## Common Pitfalls

### Pitfall 1: `#[serde(flatten)]` silently disables `deny_unknown_fields`
**What goes wrong:** Adding `#[serde(flatten)] body: B` on the envelope (to get a flat wire shape) silently drops the `deny_unknown_fields` guarantee on the envelope struct. Tests pass, production accepts unknown extension fields, ENV-02 broken.
**Why it happens:** Open upstream serde issue — `flatten` uses `Content` buffering which bypasses the per-struct unknown-field check.
**How to avoid:** **No `flatten` on any envelope or body struct.** The body is a nested `"body": {...}` field on the wire, NOT flattened siblings. §7.1c confirms this wire shape.
**Warning signs:** Any PR that introduces `#[serde(flatten)]` anywhere in `famp-envelope/src/`.

### Pitfall 2: Internally-tagged `Body` enum + `deny_unknown_fields`
**What goes wrong:** Someone "refactors" to `#[derive(Deserialize)] #[serde(tag = "class", deny_unknown_fields)] pub enum Body { Request(RequestBody), ... }`. Compiles. Tests for happy path pass. Adversarial unknown-field tests flake or false-positive.
**Why it happens:** Same `Content` buffering issue applied to internally-tagged enums.
**How to avoid:** CONTEXT.md locks the design as generic `Envelope<B>`. Document the ban explicitly in a top-of-file comment in `envelope.rs`: `// CRITICAL: do NOT refactor to an internally-tagged Body enum. See RESEARCH.md Pitfall 2.`
**Warning signs:** Any `#[serde(tag = ...)]` on a body-related type.

### Pitfall 3: Typed decode → re-serialize → canonicalize drops extension fields
**What goes wrong:** Verify flow does `from_slice_strict → Envelope<B> → serde_json::to_value → canonicalize → verify`. But `Envelope<B>` deliberately rejects unknown top-level fields under `deny_unknown_fields`. That's correct. However, if a future extension passes a new optional envelope field and the typed struct doesn't know about it, verify fails — which is the desired behavior BUT it conflates "unknown field" with "signature mismatch" at the error layer.
**Why it happens:** Verify uses the typed struct instead of the raw `Value`.
**How to avoid:** **Verify flow uses `Value`, not the typed struct.** Flow: `from_slice_strict → Value → extract sig → remove sig from Value → verify_value(vk, &value_no_sig, &sig)`. THEN deserialize the typed body from the same `Value`. The typed deserialization is a separate, subsequent step.

### Pitfall 4: `f64::NAN` serializes as `null`
**What goes wrong:** A proptest generates `confidence_floor = f64::NAN`. `serde_json` serializes as `null`. Round-trip yields `Some(null)` which deserializes back as `None`. Test "passes" despite data corruption. Worse: the signing input changes silently.
**Why it happens:** `serde_json` default behavior on non-finite floats.
**How to avoid:** Filter NaN/Inf from the generator: `any::<f64>().prop_filter("finite", |f| f.is_finite() && *f >= 0.0 && *f <= 1.0)`. Also: validate `confidence_floor.is_finite()` at decode.

### Pitfall 5: `preserve_order` feature accidentally enabled transitively
**What goes wrong:** A dependency quietly enables `serde_json/preserve_order`. Envelope decode starts producing `serde_json::Value` with insertion order instead of sorted order. Canonicalization still works (goes through the canonical serializer), BUT `Value::remove("signature")` now preserves insertion order, and a subsequent `serde_json::to_value` round-trip produces a different byte layout → signing input changes → verify fails on vector 0.
**Why it happens:** Cargo feature unification across the workspace.
**How to avoid:** Add a CI gate: `cargo tree -e features -i serde_json` must NOT show `preserve_order`. Also: re-canonicalize through `famp_canonical::canonicalize` rather than relying on `Value`'s in-memory ordering — canonicalization is order-independent.

### Pitfall 6: Timestamp round-trip through `time::OffsetDateTime`
**What goes wrong:** Parsing `"2026-04-13T00:00:00Z"` into a `time::OffsetDateTime` and reserializing may produce `"2026-04-13T00:00:00.000000000Z"` or `"2026-04-13T00:00:00+00:00"`. Canonical bytes change. Vector 0 breaks.
**Why it happens:** RFC 3339 allows multiple equivalent encodings; parser libraries pick one on output.
**How to avoid:** Keep `ts` as an opaque `String` (or `Timestamp(String)` newtype with format validation but no normalization). Full RFC 3339 parsing lands in v0.9 for freshness-window work — and even there, the parsed value is for comparison only, never for re-serialization.

### Pitfall 7: `Option<Signature>` creeps in via "convenience"
**What goes wrong:** A reviewer asks for "just a constructor that builds an envelope and you can sign later". Developer adds `pub signature: Option<FampSignature>` for ergonomics. INV-10 dead.
**Why it happens:** Premature ergonomic pressure before the type-state API is fully wired.
**How to avoid:** D-A2 explicit — `Option<Signature>` is rejected. Lock in a failing compile test: a `compile_fail` doctest that tries to construct an envelope with `signature: None` and asserts the type does not exist.

### Pitfall 8: Forgetting to strip `signature` before canonicalizing for verify
**What goes wrong:** Verify computes canonical bytes over the full envelope INCLUDING the signature field. Signature never matches. Silent verify failure on every message.
**Why it happens:** §7.1 says "envelope with signature field removed"; easy to miss on implementation.
**How to avoid:** Encapsulate the strip in the private `decode_value` core. Document the strip as step 3 in the function header. Cover with a dedicated test: sign → decode with strip → ok; decode without strip → fail (adversarial).

### Pitfall 9: `ts` in signing input must exactly match the wire bytes
**What goes wrong:** Canonicalization re-encodes strings per RFC 8785 §3.2.2.2. For ASCII timestamps this is identity, but a reviewer adds a "normalize to UTC" helper that turns `+00:00` into `Z` — now the canonical bytes differ from wire bytes, signing input differs, vector 0 breaks.
**How to avoid:** No normalization anywhere. Pitfalls P3 (no Unicode normalization) extends to all string fields, including `ts`.

### Pitfall 10: `famp: "0.5.1"` serialization as anything other than literal
**What goes wrong:** Reviewer models `famp` as an enum `FampVersion::V0_5_1` and serde renames it to `"V051"` or `"0_5_1"`. Vector 0 breaks immediately.
**How to avoid:** `struct FampVersion;` with hand-written `Serialize`/`Deserialize` that emit/expect exactly `"0.5.1"`. Test: insta snapshot of `serde_json::to_string(&FampVersion)` == `"\"0.5.1\""`.

## Code Examples

### Canonical `SignedEnvelope::decode` flow

```rust
// envelope.rs (core decode sketch)
impl<B: BodySchema> SignedEnvelope<B> {
    pub fn decode(bytes: &[u8], verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>
    {
        // 1. Strict parse — dup-key and bad UTF-8 rejected here
        let mut value: serde_json::Value = famp_canonical::from_slice_strict(bytes)
            .map_err(EnvelopeDecodeError::MalformedJson)?;

        Self::decode_value(value, verifier)
    }

    fn decode_value(mut value: serde_json::Value, verifier: &TrustedVerifyingKey)
        -> Result<Self, EnvelopeDecodeError>
    {
        let obj = value.as_object_mut()
            .ok_or(EnvelopeDecodeError::MalformedJson(/* ... */))?;

        // 2. Extract and remove the signature field
        let sig_str = obj.remove("signature")
            .ok_or(EnvelopeDecodeError::MissingSignature)?
            .as_str()
            .ok_or(EnvelopeDecodeError::MissingSignature)?
            .to_string();

        let signature = FampSignature::from_b64url(&sig_str)
            .map_err(EnvelopeDecodeError::InvalidSignatureEncoding)?;

        // 3. Verify — canonicalize + prefix is all inside famp_crypto::verify_value
        famp_crypto::verify_value(verifier, &value, &signature)
            .map_err(|_| EnvelopeDecodeError::SignatureInvalid)?;

        // 4. Deserialize typed envelope from the stripped Value
        //    (note: we serialize the Value back into a decode — cheap because we already own the Value)
        let inner: UnsignedEnvelope<B> = serde_json::from_value(value)
            .map_err(|e| /* map to Unknown{Envelope,Body}Field, MissingField, Malformed */)?;

        // 5. Class + scope cross-checks
        if inner.class != B::CLASS {
            return Err(EnvelopeDecodeError::ClassMismatch { expected: B::CLASS, got: inner.class });
        }
        if inner.scope != B::SCOPE {
            return Err(EnvelopeDecodeError::ScopeMismatch {
                class: B::CLASS, expected: B::SCOPE, got: inner.scope,
            });
        }

        // 6. Version check
        // (FampVersion deserialize already enforces exact "0.5.1" — no extra step)

        Ok(SignedEnvelope { inner, signature })
    }
}
```

### Sign flow

```rust
impl<B: BodySchema> UnsignedEnvelope<B> {
    pub fn sign(self, sk: &FampSigningKey) -> Result<SignedEnvelope<B>, EnvelopeError> {
        // Enforce self.class == B::CLASS (constructor invariant; re-assert for safety)
        debug_assert_eq!(self.class, B::CLASS);

        // Serialize to Value (without a signature field, since UnsignedEnvelope has no such field)
        let value = serde_json::to_value(&self).map_err(EnvelopeError::Serialize)?;

        // Sign — famp_crypto canonicalizes and prepends prefix internally
        let signature = famp_crypto::sign_value(sk, &value)
            .map_err(EnvelopeError::Crypto)?;

        Ok(SignedEnvelope { inner: self, signature })
    }
}
```

### Vector 0 test

```rust
// tests/vector_0.rs
#[test]
fn vector_0_roundtrip_byte_exact() {
    let envelope_json = include_str!("fixtures/vector_0/envelope.json");
    let expected_canonical = hex::decode(include_str!("fixtures/vector_0/canonical.hex").trim()).unwrap();
    let expected_sig_hex = include_str!("fixtures/vector_0/signature.hex").trim();

    // RFC 8032 Test 1 public key
    let pk_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    let pk_bytes: [u8; 32] = hex::decode(pk_hex).unwrap().try_into().unwrap();
    let verifier = TrustedVerifyingKey::from_bytes(&pk_bytes).unwrap();

    // 1. Decode — must succeed
    let signed = SignedEnvelope::<AckBody>::decode(envelope_json.as_bytes(), &verifier)
        .expect("vector 0 must decode");

    // 2. Re-canonicalize the stripped envelope — must match §7.1c.3 exactly
    let stripped_value = signed.to_unsigned_value();  // strips sig field
    let actual_canonical = famp_canonical::canonicalize(&stripped_value).unwrap();
    assert_eq!(actual_canonical, expected_canonical, "canonical bytes must match §7.1c.3");

    // 3. Re-sign with RFC 8032 Test 1 secret — must match §7.1c.6 exactly
    let sk_bytes: [u8; 32] = hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
        .unwrap().try_into().unwrap();
    let sk = FampSigningKey::from_bytes(sk_bytes);
    let sig = famp_crypto::sign_value(&sk, &stripped_value).unwrap();
    assert_eq!(hex::encode(sig.to_bytes()), expected_sig_hex, "signature must match §7.1c.6");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[serde(flatten)]` for envelope-body composition | Plain nested `body: B` field | Long-standing serde limitation (pre-1.0) | Required — flatten breaks `deny_unknown_fields` |
| Internally-tagged `Body` enum | Generic `Envelope<B>` + manual `AnySignedEnvelope` dispatch | serde#1358 unresolved | Required — tagged enums break `deny_unknown_fields` |
| `ed25519-dalek 1.x` `Keypair` API | `SigningKey` / `VerifyingKey::verify_strict` | dalek 2.0 (2023) | Already adopted in `famp-crypto` |
| Hand-written base64 | `base64 0.22` `Engine::URL_SAFE_NO_PAD` | `base64 0.21` | Wrapped inside `FampSignature`/`TrustedVerifyingKey` already |

**Deprecated / outdated:**
- Any `base64::encode`/`decode` free function calls — wrapped, do not use.
- `ed25519_dalek::Keypair` — replaced by `SigningKey` in 2.x.
- `ed25519_dalek::VerifyingKey::verify` (non-strict) — unreachable from `famp-crypto` public API.

## Open Questions

1. **Exact `AckDisposition` enum set.**
   - What we know: §7.1c vector 0 uses `accepted`. v0.7 5-state FSM will emit a subset of `{accepted, rejected, received, completed, failed, cancelled}`.
   - What's unclear: Final enum set; whether `rejected` is legal under FSM-02 narrowing (it's narrowed out of state names, but may still be a valid ack disposition).
   - **Recommendation for planner:** Ship the superset `{accepted, rejected, received, completed, failed, cancelled}` in Phase 1 with `deny_unknown_fields`. Phase 2 FSM plan tightens to exactly the FSM emission set; the tightening is a backward-compatible narrowing of the enum variant list.

2. **Scope lock for `ack`.**
   - What we know: §7.1c vector 0 declares `"scope": "standalone"`. Vector 0 is byte-load-bearing.
   - What's unclear: CONTEXT.md D-C4 says scope locks for non-request classes are TBD. If ack is locked to `Task` scope, vector 0 breaks.
   - **Recommendation for planner:** Lock `AckBody::SCOPE = Standalone` to preserve vector 0. Document with §7.1c cross-reference. Revisit in Phase 2 FSM planning — if the task FSM needs task-scoped ack, that's a v0.8 breaking change, not v0.7.

3. **`commit`, `deliver`, `control` scope locks.**
   - What we know: The §7.3a whitelist and §8a schemas name `scope_subset` (body) and `target` (body) as FSM-observable, but the envelope-level `scope` string is independent.
   - What's unclear: Whether commits and deliveries are `Task` scoped uniformly. Almost certainly yes (a commit binds to a task), but the spec doesn't exhaustively enumerate envelope-scope locks per class.
   - **Recommendation for planner:** Lock Commit=Task, Deliver=Task, Control=Task for v0.7. This matches Personal Runtime's `request → commit → deliver → ack` happy path where everything after the initial request is task-bound. Revisit if Phase 2 FSM conflicts.

4. **`Bounds` "≥2 keys" validation — decode-time or sign-time?**
   - What we know: §9.3 / INV-4 says bounds MUST contain ≥2 keys.
   - What's unclear: Whether this is a decode-time reject or a sign-time check.
   - **Recommendation:** Both. At decode, count populated `Option` fields and reject `InsufficientBounds` if < 2. At sign, same check in `UnsignedEnvelope::sign`. Tests cover both directions.

5. **`conversation_id` / `task_id` envelope fields.**
   - What we know: §7.1c vector 0 (a `standalone` ack) contains NO `conversation_id` or `task_id` envelope fields — scope is carried as `"scope": "standalone"` only, and the causality reference is the task binding.
   - What's unclear: Whether `Task` scope needs an explicit `task_id` field or whether `causality.ref` is sufficient.
   - **Recommendation:** Do NOT add `conversation_id` / `task_id` envelope fields in v0.7. Task binding is expressed via `causality.ref` pointing at the requesting MessageId (which becomes the task ID in Phase 2's FSM). Simpler wire, matches vector 0. Revisit in v0.8 Causality if required.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo-nextest 0.9.132` + built-in `cargo test` for doctests |
| Config file | `.config/nextest.toml` (workspace-level, shipped in v0.6) |
| Quick run command | `cargo nextest run -p famp-envelope` |
| Full suite command | `just ci` (workspace-level, includes clippy + fmt + nextest + audit) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENV-01 | `famp-envelope` crate compiles with typed `Envelope<B>` | unit (compile) | `cargo check -p famp-envelope` | ❌ Wave 0 (crate is stub) |
| ENV-01 | Envelope field set matches §7.1c | fixture | `cargo nextest run -p famp-envelope vector_0` | ❌ Wave 0 |
| ENV-02 | `deny_unknown_fields` at envelope level | fixture | `cargo nextest run -p famp-envelope adversarial::unknown_envelope_field` | ❌ Wave 0 |
| ENV-02 | `deny_unknown_fields` nested in body | fixture | `cargo nextest run -p famp-envelope adversarial::unknown_body_field_nested` | ❌ Wave 0 |
| ENV-03 | Missing signature rejected | fixture | `cargo nextest run -p famp-envelope adversarial::missing_signature` | ❌ Wave 0 |
| ENV-03 | Bad signature encoding rejected | fixture | `cargo nextest run -p famp-envelope adversarial::bad_signature_padded` | ❌ Wave 0 |
| ENV-03 | Invalid signature (tampered bytes) rejected | unit | `cargo nextest run -p famp-envelope sig::tampered` | ❌ Wave 0 |
| ENV-03 | `Option<Signature>` unreachable | compile_fail doctest | `cargo test -p famp-envelope --doc` | ❌ Wave 0 |
| ENV-06 | Ack body round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::ack` | ❌ Wave 0 |
| ENV-07 | Request body round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::request` | ❌ Wave 0 |
| ENV-09 | Commit body round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::commit` | ❌ Wave 0 |
| ENV-09 | `capability_snapshot` rejected as unknown | fixture | `cargo nextest run -p famp-envelope adversarial::commit_with_capability_snapshot` | ❌ Wave 0 |
| ENV-10 | Deliver interim round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::deliver_interim` | ❌ Wave 0 |
| ENV-10 | Deliver terminal round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::deliver_terminal` | ❌ Wave 0 |
| ENV-10 | `interim=true` + terminal_status fails | fixture | `cargo nextest run -p famp-envelope adversarial::deliver_interim_with_terminal_status` | ❌ Wave 0 |
| ENV-10 | `terminal_status=failed` without `error_detail` fails | fixture | `cargo nextest run -p famp-envelope adversarial::deliver_failed_without_error_detail` | ❌ Wave 0 |
| ENV-12 | `control.action=cancel` round-trip | fixture | `cargo nextest run -p famp-envelope roundtrip::control_cancel` | ❌ Wave 0 |
| ENV-12 | `control.action=supersede` rejected | fixture | `cargo nextest run -p famp-envelope adversarial::control_supersede` | ❌ Wave 0 |
| ENV-14 | Class/body mismatch rejected | fixture | `cargo nextest run -p famp-envelope adversarial::class_body_mismatch` | ❌ Wave 0 |
| ENV-14 | Scope/body mismatch rejected | unit | `cargo nextest run -p famp-envelope scope::mismatch` | ❌ Wave 0 |
| ENV-15 | Vector 0 byte-exact round-trip | fixture | `cargo nextest run -p famp-envelope vector_0::byte_exact` | ❌ Wave 0 |
| ENV-15 | Per-body proptest sign/verify | proptest | `cargo nextest run -p famp-envelope prop_roundtrip` | ❌ Wave 0 |
| ENV-15 | Per-body proptest tampered canonical | proptest | `cargo nextest run -p famp-envelope prop_tampered` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo nextest run -p famp-envelope`
- **Per wave merge:** `cargo nextest run -p famp-envelope` + `cargo clippy -p famp-envelope --all-targets -- -D warnings`
- **Phase gate:** `just ci` green on the full workspace before `/gsd:verify-work`

### Wave 0 Gaps
All test infrastructure for `famp-envelope` is net new — the crate currently exists as a stub with no source files beyond Cargo.toml. Wave 0 must create:

- [ ] `crates/famp-envelope/src/{lib.rs,error.rs,scope.rs,class.rs,wire.rs,envelope.rs}` — public types and private decode core
- [ ] `crates/famp-envelope/src/body/{mod.rs,request.rs,commit.rs,deliver.rs,ack.rs,control.rs}` — body schemas with sealed trait
- [ ] `crates/famp-envelope/src/dispatch.rs` — `AnySignedEnvelope`
- [ ] `crates/famp-envelope/Cargo.toml` — add workspace deps: `serde`, `serde_json`, `thiserror`, `famp-canonical`, `famp-crypto`, `famp-core`; dev-deps: `proptest`, `insta`, `hex`
- [ ] `crates/famp-envelope/tests/fixtures/vector_0/{envelope.json,canonical.hex,signing_input.hex,signature.hex,signature.b64url}` — §7.1c bytes, committed
- [ ] `crates/famp-envelope/tests/fixtures/roundtrip/{request,commit,deliver_interim,deliver_terminal,ack,control_cancel}.json` — per-class deterministic fixtures
- [ ] `crates/famp-envelope/tests/fixtures/adversarial/*.json` — D-D3 + D-D4 cases (≥11 files)
- [ ] `crates/famp-envelope/tests/{vector_0.rs,roundtrip.rs,adversarial.rs,prop_roundtrip.rs}` — integration test files
- [ ] Framework install: none (nextest + proptest + insta already in workspace dev-deps from v0.6)

## Sources

### Primary (HIGH confidence)
- `FAMP-v0.5.1-spec.md` §7.1, §7.1a, §7.1b, §7.1c.0–.8, §7.3a, §8a.2, §8a.3, §8a.4 — envelope schema, signature binding, body schemas, FSM whitelist. **§7.1c is byte-load-bearing.**
- `.planning/phases/01-minimal-signed-envelope/01-CONTEXT.md` — locked design decisions
- `crates/famp-crypto/src/{prefix.rs,sign.rs,verify.rs,keys.rs}` — shipped v0.6 sign/verify API surface; confirms `sign_value` / `verify_value` as the only callable paths
- `crates/famp-core/src/ids.rs` — `MessageId`/`ConversationId`/`TaskId`/`CommitmentId` with hyphenated-UUID serde
- `CLAUDE.md` — frozen tech stack (HIGH for all version numbers; all verified on crates.io 2026-04-12 per the research memo embedded in CLAUDE.md)

### Secondary (MEDIUM confidence, verified against primary)
- serde incompatibility of `deny_unknown_fields` + `flatten` — long-standing open issue (`serde#1547`, `serde#1600`). Confirmed against training data; no recent upstream fix merged as of 1.0.228. Risk: if serde ships a fix in 1.0.229+, refactor becomes possible (not required). **Mitigation:** add a nightly CI check that compiles a minimal repro and asserts the current (broken) behavior, so a silent upstream fix gets a visible signal.
- serde incompatibility of `deny_unknown_fields` + internally-tagged enums — `serde#1358`. Same status.

### Tertiary (LOW — flagged)
- None. All critical claims trace to primary sources.

## Metadata

**Confidence breakdown:**
- Envelope schema field set: HIGH — taken directly from §7.1c.2 (normative worked example)
- Body schemas (request/commit/deliver/ack/control): HIGH — §8a.2/.3/.4 for commit/deliver/control; ack from §7.1c.2; request inherited from v0.5 §7.4 via §8a closing note
- Serde pattern (no flatten, no internally-tagged enum): HIGH — known long-standing limitation, reproducible
- Sign/verify wiring: HIGH — v0.6 code already shipped, read directly
- Type-state INV-10 enforcement: HIGH — design locked in CONTEXT.md
- `AckDisposition` final enum set: MEDIUM — Phase 2 FSM will tighten
- Scope locks for non-request classes: MEDIUM — §7.3a whitelist names the fields but not envelope-level scope-per-class; recommendation above is inference, not direct spec quote

**Research date:** 2026-04-13
**Valid until:** 2026-05-13 (30 days — all primary sources are frozen spec and already-shipped code)
