# Phase 3: Core Types & Invariants - Context

**Gathered:** 2026-04-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship `famp-core`: the shared value-type substrate every downstream FAMP crate depends on. Delivers strict-ASCII `Principal` / `Instance` identity types with separate parsers, distinct UUIDv7 newtypes for `MessageId` / `ConversationId` / `TaskId` / `CommitmentId`, a parsed `ArtifactId` owning the `sha256:<hex>` invariant, the flat wire-stable §15.1 `ProtocolErrorKind` enum (15 categories), an `AuthorityScope` enum with an explicit `satisfies()` ladder, and an `invariants` module with one doc-bearing public item per INV-1..INV-11.

**Out of scope:**
- Envelope schema, message bodies, signature field → `famp-envelope` (v0.7)
- FSM / task lifecycle / terminal-state enforcement → `famp-fsm` (v0.7)
- Agent Card / trust store / federation credential → `famp-identity` (v0.8+)
- Causality ref types / freshness / replay cache → `famp-causality` (v0.9)
- Conversion between upstream lib errors (`CanonicalError`, `CryptoError`) and `ProtocolErrorKind` → lives at the boundary crates that actually build wire-error responses, NOT in `famp-core`
- Runtime enforcement of INV-1..INV-11 beyond what the type system gives for free — Phase 3 ships scaffolding, not policy engines

</domain>

<decisions>
## Implementation Decisions

### Identity parsing — strict syntax, separate types

- **D-01:** `Principal` and `Instance` are **separate types with separate parsers**. `Principal` MUST NOT silently accept an instance-bearing string by trimming `#...`, and `Instance` MUST NOT silently accept a principal-only string. Misuse is a parse error, not a coerced success.
- **D-02:** Wire forms locked:
  - `Principal`: `agent:<authority>/<name>`
  - `Instance`: `agent:<authority>/<name>#<instance_id>`
- **D-03:** ASCII-only for `authority`, `name`, and `instance_id`. No Unicode, no IDNA, no punycode, no case-folding, no whitespace (leading, trailing, or embedded), no empty segments, no trailing separators. Case-sensitive round-trip: what goes in comes out byte-for-byte.
- **D-04:** Authority rules (DNS-style hostname syntax, conservative):
  - Labels separated by `.`
  - Each label matches `[A-Za-z0-9-]+`
  - Label MUST NOT start or end with `-`
  - Underscore `_` REJECTED
  - Total authority length ≤ 253 bytes
  - ≥ 1 label required
- **D-05:** Name rules:
  - ASCII `[A-Za-z0-9._-]+`
  - Length 1..=64
  - MUST NOT contain `/`, `#`, `:`, or whitespace
- **D-06:** Instance-id rules (same character set as name, deliberately):
  - ASCII `[A-Za-z0-9._-]+`
  - Length 1..=64
  - MUST NOT contain `/`, `#`, `:`, or whitespace
- **D-07:** No automatic normalization. Authority is NOT auto-lowercased; the spec already locks a case-sensitive wire form and normalizing here would desync bytes from signatures. Validation only — never transformation.
- **D-08:** Parse errors live in **narrow, identity-local error types**: `ParsePrincipalError` and `ParseInstanceError`. These MUST NOT be `From`-converted into `ProtocolErrorKind` — the boundary crate (envelope / transport) decides whether a parse failure maps to `malformed` or something else.
- **D-09:** Both types implement `Display`, `FromStr`, `Serialize`, `Deserialize` — the string form is the canonical serde form. Round-trip property test: `parse(display(x)) == x` for every generated valid input.

### ID newtypes — distinct, human-readable serde

- **D-10:** `MessageId`, `ConversationId`, `TaskId`, `CommitmentId` are **distinct newtypes** wrapping `uuid::Uuid`. The compiler rejects cross-type assignment. No `AsRef<Uuid>` blanket implementations on the public surface that would let callers launder one into another — if they need the inner UUID, they call a named accessor.
- **D-11:** Serde form: canonical hyphenated UUID string (e.g. `01890a3b-1c2d-7e3f-8a1b-0c2d3e4f5a6b`). Parse from canonical form only. No raw-byte serde, no integer serde, no `Uuid::simple` unhyphenated form. Deserializer rejects anything that isn't exactly hyphenated.
- **D-12:** UUIDv7 generation lives in `famp-core` as `MessageId::new_v7()` / `TaskId::new_v7()` / etc. (one per type). Centralizing generation prevents downstream crates from inventing their own ID schemes or picking v4 by accident. Requires `uuid` crate `v7` + `serde` features (already in workspace deps).
- **D-13:** Every wire-facing type in Phase 3 implements the full round-trip quad: `Display`, `FromStr`, `Serialize`, `Deserialize`. No implicit serde derivation of wire shape — every representation is chosen deliberately and tested with a fixture.

### ArtifactId — parsed, type-owned invariant

- **D-14:** `ArtifactId` is a **parsed type owned by `famp-core`**, not a transparent `String` alias and not re-exported from `famp-canonical`. `famp-core` owns the type-level invariant; `famp-canonical` owns the byte/string hashing helpers. The dependency direction (core → canonical for hashing, canonical → core for the typed return) will be revisited when a concrete caller needs it — Phase 3 just defines the type.
- **D-15:** Exact wire form accepted in Phase 3: `sha256:<64 lowercase hex chars>`. Uppercase hex REJECTED. Only `sha256` accepted as the algorithm tag.
- **D-16:** Internal shape — tighter option preferred:
  ```rust
  pub struct ArtifactId(String);  // invariant: matches sha256:<64-lc-hex>
  ```
  with invariant-checked constructors. Internally the type MAY reserve room for future algorithm tags (private enum or parsed struct), but the **public API does not expose an `algorithm` accessor in Phase 3** — premature generalization. Revisit when a second hash algorithm actually lands.
- **D-17:** Implements `Display`, `FromStr`, `TryFrom<String>`, `TryFrom<&str>`, `Serialize`, `Deserialize`. Round-trip property test.
- **D-18:** Parse errors live in a narrow `ParseArtifactIdError`. Same rule as D-08: no automatic conversion into `ProtocolErrorKind`.

### Protocol error enum — flat wire-stable kind + optional richer wrapper

- **D-19:** `ProtocolErrorKind` is a **flat enum of unit variants**, one per §15.1 category, exactly 15 entries. No structured context inside variants — that breaks exhaustive match and couples wire stability to context shape.
  ```rust
  pub enum ProtocolErrorKind {
      Malformed,
      Unsupported,
      Unauthorized,
      Stale,
      Duplicate,
      Orphaned,
      OutOfScope,
      CapacityExceeded,
      PolicyBlocked,
      CommitmentMissing,
      DelegationForbidden,
      ProvenanceIncomplete,
      Conflict,
      ConditionFailed,
      Expired,
  }
  ```
- **D-20:** Serde form: snake_case string (`"malformed"`, `"out_of_scope"`, `"commitment_missing"`, etc.). Matches spec §15.1 verbatim. Hard gate: a fixture test asserts every variant's wire string against the spec table so a rename is a compile/test failure.
- **D-21:** Provide an optional richer wrapper struct for internal error plumbing:
  ```rust
  pub struct ProtocolError {
      pub kind: ProtocolErrorKind,
      pub detail: Option<String>,
  }
  ```
  The wrapper is a convenience for internal use. The **wire envelope** (later phase) will carry `{ "error": "<kind>", "detail": "..." }` as two sibling fields, NOT a serde-tagged algebraic representation. `ProtocolError` itself does NOT derive `Serialize` — wire shape is the envelope crate's job.
- **D-22:** **No `From<CanonicalError>` or `From<CryptoError>` into `ProtocolErrorKind`.** Upstream errors stay in their crates. Mapping from `CanonicalError` / `CryptoError` to a `ProtocolErrorKind` category happens **explicitly at the boundary crate** that builds the wire error (envelope / transport). This prevents the "everything becomes `malformed`" or "everything becomes `unauthorized`" anti-pattern.
- **D-23:** `ProtocolErrorKind` and `ProtocolError` both `impl std::error::Error` via `thiserror`. No `anyhow` in `famp-core` public API.
- **D-24:** Exhaustive-match verification: at least one downstream consumer stub (test-only module inside `famp-core/tests/`) performs `match kind { ... }` over every variant. Adding a variant without updating the stub fails CI — that's the compile-check promise from the roadmap SC-#3.

### INV-1..INV-11 scaffolding — `invariants` module with public doc-bearing items

- **D-25:** Dedicated `pub mod invariants` with one public item per INV. Preferred shape: a `pub const INV_N: &str = "INV-N";` per invariant, with the **doc comment carrying the full invariant statement** copied from spec `FAMP-v0.5-spec.md` §3 (INV-1..INV-11).
- **D-26:** Constant values are deliberately minimal (`"INV-10"`). The real payload is the rustdoc text, which future crates link to via intra-doc links (`[`famp_core::invariants::INV_10`]`). This satisfies CORE-05's "every future crate can link to them" requirement with stable, deep-linkable rustdoc items.
- **D-27:** No marker types, no trait-based "invariant tagging" — overbuilt for Phase 3. Enforcement of each INV happens in the crates that actually model the behavior (envelope, fsm, transport); `famp-core` ships the documentation anchor.
- **D-28:** Test: a small fixture iterates over the public names (`INV_1..INV_11`) and asserts each exists and each doc comment is non-empty (`include_str!` of `lib.rs` or a `doc(cfg)` trick — planner to pick mechanism). Guards against silent deletion.

### AuthorityScope enum — wire-stable, explicit satisfies()

- **D-29:** Enum of 5 unit variants matching spec §5.3 exactly:
  ```rust
  pub enum AuthorityScope {
      Advisory,
      Negotiate,
      CommitLocal,
      CommitDelegate,
      Transfer,
  }
  ```
- **D-30:** Serde / wire form: snake_case strings — `"advisory"`, `"negotiate"`, `"commit_local"`, `"commit_delegate"`, `"transfer"`. `Display` / `FromStr` match. Fixture test locks every variant's wire string against the spec table.
- **D-31:** **Do NOT derive `Ord` / `PartialOrd`.** The ladder is semantic, not lexical, and auto-derived ordering couples correctness to declaration order — a hazard the moment someone reorders variants alphabetically.
- **D-32:** Expose an explicit ladder via `satisfies`:
  ```rust
  impl AuthorityScope {
      pub fn satisfies(self, required: Self) -> bool { ... }
  }
  ```
  Semantics (locked):
  - `Transfer` satisfies `Transfer`, `CommitDelegate`, `CommitLocal`, `Negotiate`, `Advisory`
  - `CommitDelegate` satisfies `CommitDelegate`, `CommitLocal`, `Negotiate`, `Advisory`
  - `CommitLocal` satisfies `CommitLocal`, `Negotiate`, `Advisory`
  - `Negotiate` satisfies `Negotiate`, `Advisory`
  - `Advisory` satisfies only `Advisory`
  - Planner writes an exhaustive 5×5 truth table test.
- **D-33:** A private `rank(self) -> u8` helper is acceptable as an implementation detail of `satisfies`; it MUST NOT be `pub` — ranks leak declaration order into the public API.

### Cross-cutting

- **D-34:** Every wire-facing public type implements the round-trip quad: `Display`, `FromStr` (or `TryFrom<&str>`), `Serialize`, `Deserialize`. No type ships with "serde shape TBD."
- **D-35:** Parse-error types (`ParsePrincipalError`, `ParseInstanceError`, `ParseArtifactIdError`) are **narrow, type-local, and distinct from `ProtocolErrorKind`**. Boundary crates translate on construction of wire errors; `famp-core` never does.
- **D-36:** `thiserror` for every error enum. `#![forbid(unsafe_code)]` continues from Phase 0/1/2. Workspace inheritance for all Cargo.toml metadata.
- **D-37:** Follow Phase 1/2 API pattern: free functions / constructors primary, traits only as thin sugar. No trait-heavy API design in Phase 3 — types are the product.

### Claude's Discretion

- Internal module layout (`identity.rs`, `ids.rs`, `artifact.rs`, `error.rs`, `scope.rs`, `invariants.rs`)
- Exact `thiserror` variant wording for parse errors
- Whether `FromStr` or `TryFrom<&str>` is the primary entrypoint (implement the other in terms of it)
- Choice of proptest strategies for round-trip fuzzing
- Whether `impl Debug` for any type needs customization (most can derive)
- Whether `AuthorityScope::rank` exists as a private helper or is inlined into `satisfies`
- Precise mechanism for the `invariants` doc-presence test (build script, `include_str!`, or trybuild-style)
- Length caps chosen for name / instance-id (1..=64 is the decision; MAY document a rationale but not widen)

</decisions>

<specifics>
## Specific Ideas

- Mirror Phase 1 and Phase 2's API discipline exactly: free constructors + narrow error enums + external fixtures as hard gates. Phase 3 should feel like the third entry in the same series, not a new style.
- The "separate Principal and Instance parsers" rule is a deliberate redundancy — a downstream crate holding a `Principal` should be structurally unable to accidentally treat it as `Instance`-shaped, and vice versa. The type system is doing spec INV-1 work for us.
- `ProtocolErrorKind` is the wire-error vocabulary for every FAMP crate that will ever exist. Get the enum names and wire strings byte-exact to spec §15.1 now; renaming later breaks every federation.
- `AuthorityScope::satisfies` is the one place Phase 3 ships behavior beyond types. The 5×5 truth table is small enough to write by hand — and it must be written by hand, not derived.
- Centralizing `MessageId::new_v7()` in `famp-core` is the choke point that prevents a later "oops we used v4 in the envelope crate" regression.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec (base v0.5 — §15 and §5 are NOT amended in v0.5.1)
- `FAMP-v0.5-spec.md` §3 — Invariants INV-1..INV-11 (full text; source for `invariants` module doc comments)
- `FAMP-v0.5-spec.md` §5.1 — Principal identity wire form (`agent:<authority>/<name>`)
- `FAMP-v0.5-spec.md` §5.2 — Instance identity wire form (`agent:<authority>/<name>#<instance-id>`)
- `FAMP-v0.5-spec.md` §5.3 — Authority scope (5-level ladder, advisory → transfer)
- `FAMP-v0.5-spec.md` §15.1 — Error categories (the 15 wire strings — source of truth for `ProtocolErrorKind` serde form)
- `FAMP-v0.5-spec.md` §15.2 — Error distinctions (parse vs policy vs negotiation vs execution vs provenance)

### Spec (v0.5.1 fork — where it touches Phase 3)
- `FAMP-v0.5.1-spec.md` §3.6a — Artifact identifiers (`sha256:<hex>` scheme, locked in Phase 1 D-19)
- `FAMP-v0.5.1-spec.md` — Spec version constant (pointer only; Phase 3 does not gate on it)

### Requirements
- `.planning/REQUIREMENTS.md` — CORE-01..06 (rows and acceptance criteria)

### Prior phase context (mandatory reading)
- `.planning/phases/01-canonical-json-foundations/01-CONTEXT.md` — Phase 1 API pattern (free fn + trait sugar), narrow error enum discipline, artifact-ID helpers at `famp-canonical` (D-19/D-20)
- `.planning/phases/02-crypto-foundations/02-CONTEXT.md` — Phase 2 newtype discipline, `TrustedVerifyingKey` as compile-enforced invariant pattern, `CryptoError` narrowness, decision to NOT map upstream errors into protocol-category enums

### Research / pitfalls (context for decisions)
- `.planning/research/PITFALLS.md` — if it contains identity-parsing or UUID-version hazards, surface them during research
- `.planning/research/ARCHITECTURE.md` — Crate-level boundaries for `famp-core` vs `famp-canonical` vs `famp-crypto` vs future `famp-envelope`
- `.planning/PROJECT.md` — Tech stack row 4 (`uuid 1.23` with `v7` + `serde` features) and `thiserror 2.x` discipline

### Upstream dependency docs
- `uuid 1.23` docs.rs — `Uuid::new_v7`, `serde` hyphenated form, `FromStr` behavior
- `thiserror 2.x` — derive macro usage for narrow typed errors
- RFC 9562 — UUIDv7 format (reference only; `uuid` crate implements it)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp-canonical::artifact_id_for_*` (Phase 1 D-19/D-20) — `sha2`-backed `sha256:<hex>` generator; Phase 3 `ArtifactId::from_str` must round-trip whatever this helper produces. A boundary fixture test will pair the two.
- `famp-canonical::CanonicalError` — stays in `famp-canonical`; `famp-core` does NOT wrap it (D-22)
- `famp-crypto::CryptoError` — stays in `famp-crypto`; same rule (D-22)
- Phase 1/2 vector-harness pattern — Phase 3's `ProtocolErrorKind` wire-string fixture and `AuthorityScope` wire-string fixture mirror it at smaller scale

### Established Patterns
- **Free function / constructor primary, traits as sugar** (Phase 1 D-01/D-02, Phase 2 D-01) — `famp-core` follows exactly
- **Narrow phase-appropriate error enum via `thiserror`** (Phase 1 D-16, Phase 2 D-26/D-27) — `famp-core` ships multiple narrow enums: `ProtocolErrorKind`, `ParsePrincipalError`, `ParseInstanceError`, `ParseArtifactIdError`. They do NOT cross-convert.
- **FAMP-owned newtypes wrap upstream types** (Phase 2 D-06 `FampSigningKey` wraps `ed25519_dalek::SigningKey`) — Phase 3 `MessageId` / `ConversationId` / `TaskId` / `CommitmentId` wrap `uuid::Uuid` the same way
- **Compile-enforced invariants over runtime discipline** (Phase 2 D-10 `TrustedVerifyingKey`) — Phase 3 uses distinct newtypes so the compiler rejects ID swaps
- **External fixtures as hard CI gate** (Phase 1 D-12, Phase 2 D-18) — Phase 3's fixture is the spec §15.1 and §5.3 wire-string tables, locked by test
- **`#![forbid(unsafe_code)]` + workspace inheritance** — continues

### Integration Points
- `famp-core/Cargo.toml` adds deps: `uuid = { workspace = true, features = ["v7", "serde"] }`, `serde = { workspace = true, features = ["derive"] }`, `serde_json = { workspace = true }` (dev-deps for round-trip fixture tests), `thiserror = { workspace = true }`. Crucially: NO dep on `famp-canonical` or `famp-crypto` — `famp-core` sits below them in the dep DAG for everything except the eventual `ArtifactId` producer bridge (which lives in `famp-canonical`, not here).
- Public API is consumed by: `famp-envelope` (v0.7 Phase 1 — builds messages that carry `Principal`, `Instance`, `MessageId`, `AuthorityScope`, and wire errors keyed on `ProtocolErrorKind`), `famp-fsm` (v0.7 Phase 2), and eventually every downstream crate.
- CI `just ci` recipe: add `cargo nextest run -p famp-core` step; Phase 3 introduces no new nightly workflow.

</code_context>

<deferred>
## Deferred Ideas

- **`ProtocolError` serde / wire encoding** — the wrapper struct ships in Phase 3 but its `Serialize` impl is deferred to `famp-envelope`, where the `{ "error": "...", "detail": "..." }` sibling-field layout actually belongs
- **Second hash algorithm for `ArtifactId`** — internal shape reserves room but the public API is `sha256`-only in Phase 3. Revisit when a concrete second algorithm lands (likely never for v1).
- **`AuthorityScope::rank` as public API** — private helper only; exposing ranks leaks declaration-order semantics. Revisit only if a downstream crate demonstrates a need that `satisfies` can't cover.
- **Runtime enforcement of INV-1..INV-11** — Phase 3 ships rustdoc anchors, not policy. Enforcement lives in `famp-envelope` (INV-10), `famp-fsm` (INV-5), and later federation crates.
- **`From<CanonicalError>` / `From<CryptoError>` into `ProtocolErrorKind`** — explicitly rejected (D-22). If future envelope code wants an ergonomic helper, it lives in `famp-envelope` as `fn map_canonical_err(e: CanonicalError) -> ProtocolErrorKind`, written by hand.
- **`TripleIdentity` composite (principal, instance, message-id)** — not needed in Phase 3; INV-1 is satisfied structurally by every message carrying three separate fields, not by a composite type
- **Generation helpers for non-UUID IDs** — `ArtifactId::new(...)` helper belongs in `famp-canonical` (it needs the hasher), not `famp-core`
- **`no_std` support for `famp-core`** — deferred; matches Phase 2 D-deferred rule

</deferred>

---

*Phase: 03-core-types-invariants*
*Context gathered: 2026-04-13*
