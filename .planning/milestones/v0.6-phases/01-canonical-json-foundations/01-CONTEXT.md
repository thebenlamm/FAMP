# Phase 1: Canonical JSON Foundations - Context

**Gathered:** 2026-04-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship `famp-canonical`: a stable `Canonicalize` API wrapping `serde_jcs`, with externally-sourced RFC 8785 Appendix B vectors enforced as a hard CI gate, the SEED-001 go/no-go decision recorded in-repo with rationale, and a written fallback plan on disk *before* the decision is made. Includes duplicate-key strict parsing, UTF-16 key sort fixtures, ECMAScript number formatting verified against cyberphone, and the `sha256:<hex>` artifact-ID helper.

Out of scope: the strong `ArtifactId` type (lives in `famp-core`, Phase 3), signing/verification (`famp-crypto`, Phase 2), envelope schemas (later milestone).

</domain>

<decisions>
## Implementation Decisions

### Public API shape (Canonicalize)
- **D-01:** Ship BOTH a free function and a blanket-impl trait. The free function is the primary path; the trait is a thin sugar layer delegating to it.
- **D-02:** Signatures:
  ```rust
  pub fn canonicalize<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CanonicalError>;

  pub trait Canonicalize: Serialize {
      fn canonicalize(&self) -> Result<Vec<u8>, CanonicalError> {
          canonicalize(self)
      }
  }
  impl<T: Serialize + ?Sized> Canonicalize for T {}
  ```
- **D-03:** Rationale: downstream crates will mostly call `canonicalize(&envelope)`; trait reads cleanly at call sites and matches spec language. Trait-only was rejected because it adds friction for generic utility code.

### Ingress parsing (duplicate-key rejection)
- **D-04:** Canonicalization and strict parsing are **separate public API surfaces**. `canonicalize()` operates on typed in-memory values (trusted). Duplicate-key rejection lives on the parse path for inbound JSON bytes.
- **D-05:** Expose explicit strict parse helpers as the sanctioned ingress for signed JSON bytes:
  ```rust
  pub fn from_slice_strict<T: DeserializeOwned>(input: &[u8]) -> Result<T, CanonicalError>;
  pub fn from_str_strict<T: DeserializeOwned>(input: &str) -> Result<T, CanonicalError>;
  ```
- **D-06:** Implement via a custom serde `Deserializer`/visitor path that errors on duplicate object keys during deserialization (not a separate pre-scan tokenizer). A pre-scan is the documented fallback if serde-based enforcement turns out impractical, but it's second choice — two parser surfaces are worse than one.
- **D-07:** Docs-only / schema-level enforcement was rejected. Duplicate-key rejection is a FAMP protocol guarantee and must live in a real parser path.

### SEED-001 decision workflow
- **D-08:** Fallback plan written FIRST, on disk at `famp-canonical/docs/fallback.md`, **before** running any RFC 8785 vectors against `serde_jcs`. This matches roadmap success criterion #6 literally and prevents late-failure schedule pressure.
- **D-09:** No parallel fallback crate / feature flag. Don't build two implementations now — that's premature. The fallback is a written plan, not code, until the gate actually fails.
- **D-10:** Fixed sequence (Phase 1 task ordering):
  1. Define wrapper API + error surface (D-02, D-05, D-16)
  2. Write `famp-canonical/docs/fallback.md` (~500 LoC plan, RFC 8785 §3 key sort + number formatter + UTF-8 pass-through)
  3. Wire conformance vectors + duplicate-key rejection tests + UTF-16 key sort fixtures
  4. Run the gate (Appendix B vectors, float corpus sample, supplementary-plane fixtures)
  5. Record SEED-001 decision ("keep `serde_jcs`" or "fork now") in-repo with evidence — must cite which vectors passed/failed
- **D-11:** Decision doc location: update `.planning/SEED-001.md` (or equivalent seed doc) with the evidence and outcome at step 5. Keep it co-located with the planning artifacts, not buried in commit messages.

### Float corpus CI budget
- **D-12:** Deterministic **sampled** subset runs on every PR. Full 100M corpus runs nightly + on release tags + manual workflow dispatch. Full 100M is a required **release gate**, not per-PR best-effort.
- **D-13:** Sample properties (MUST):
  - Fixed seed (committed in-repo)
  - Committed sample-generation recipe (reproducible; any dev can regenerate the same sample)
  - Sample size chosen to catch formatter drift, not token-large for prestige
  - Deterministic ordering so failures point at a specific input
- **D-14:** Sample size target: Claude's Discretion during planning — start around 100K–1M, adjust based on GHA runtime. Document the chosen N + justification in `famp-canonical/tests/float_corpus.rs` header.
- **D-15:** Full 100M on every PR was rejected — unknown whether it fits GHA free-tier budget, and paying that cost on every PR is the wrong trade when a nightly catch-all exists.

### Error surface (CanonicalError)
- **D-16:** Keep `CanonicalError` **narrow and phase-appropriate**. Do NOT overfit to the 15-category protocol error enum from spec §15.1 — that lives in `famp-core` (Phase 3).
- **D-17:** Phase 1 variants (minimum viable set):
  - `Serialize` — upstream serde serialization failure
  - `InvalidJson` — malformed input bytes on strict-parse path
  - `DuplicateKey` — duplicate object key detected during strict parse (carries key name)
  - `NonFiniteNumber` — NaN / ±Infinity encountered (RFC 8785 forbids)
  - `InternalCanonicalization` — escape hatch for `serde_jcs` internal failures
- **D-18:** Use `thiserror` (per tech-stack). Never return `anyhow::Result` from `famp-canonical` public API.

### Artifact ID helper
- **D-19:** `sha256:<hex>` helpers live in `famp-canonical` (not deferred to Phase 2/3), because the byte-to-hash-string logic is a canonicalization concern.
- **D-20:** Expose two helpers in Phase 1:
  ```rust
  pub fn artifact_id_for_canonical_bytes(bytes: &[u8]) -> ArtifactIdString;
  pub fn artifact_id_for_value<T: Serialize + ?Sized>(value: &T)
      -> Result<ArtifactIdString, CanonicalError>;
  ```
- **D-21:** `ArtifactIdString` is a newtype-or-alias *placeholder* in Phase 1 — it's a `String` (or `Cow<'static, str>`) wrapper, not the strongly-typed `ArtifactId` that lands in `famp-core` (Phase 3). Phase 3 will refactor Phase 1's helpers to return the strong type; Phase 1 just needs the byte→`sha256:<hex>` primitive to exist and be correct.
- **D-22:** `sha2` dependency lives in `famp-canonical` for this helper. No dependency on `famp-crypto` (Phase 2 hasn't shipped when Phase 1 lands).

### Claude's Discretion
- Exact float corpus sample size (start ~100K–1M, tune to GHA budget)
- Internal module layout inside `famp-canonical` (one file vs split by concern)
- Exact wording of `CanonicalError` `Display` messages
- Whether `ArtifactIdString` is a literal alias `type ArtifactIdString = String` or a thin newtype — pick whichever refactors cleanest to Phase 3's strong type
- Test fixture filenames and directory layout under `famp-canonical/tests/vectors/`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec
- `FAMP-v0.5.1-spec.md` §7.1 — canonical JSON requirement, signature-over-canonical-bytes
- `FAMP-v0.5.1-spec.md` SPEC-02 — RFC 8785 JCS locked as canonical form (not paraphrase)
- `FAMP-v0.5.1-spec.md` SPEC-18 — `sha256:<hex>` artifact identifier scheme

### External standards
- RFC 8785 — JSON Canonicalization Scheme (authoritative): https://datatracker.ietf.org/doc/html/rfc8785
- RFC 8785 Appendix B — conformance test vectors (hard CI gate)
- cyberphone `jcs` reference implementation + 100M-float corpus: https://github.com/cyberphone/json-canonicalization
- RFC 9562 — UUIDv7 (not directly used in Phase 1, but referenced by downstream phases)

### Project planning
- `.planning/ROADMAP.md` §"Phase 1: Canonical JSON Foundations" — success criteria, research flag
- `.planning/REQUIREMENTS.md` — CANON-01..07, SPEC-02, SPEC-18
- `.planning/PROJECT.md` — tech-stack section (`serde_jcs 0.2.0`, `sha2 0.11.0`, fallback rationale)
- `.planning/STATE.md` — SEED-001 open blocker, carried decisions from v0.5.1
- `CLAUDE.md` — project-level tech stack table (rows #2, #3, #5, #6 relevant to Phase 1)

### Seed docs
- SEED-001 (`.planning/SEED-001.md` or equivalent) — `serde_jcs` conformance gate decision — THIS PHASE RESOLVES IT

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **None** — repository is fresh. v0.5.1 was a docs-only milestone; no Rust crates exist yet. Phase 1 is the first code to land.

### Established Patterns
- **Workspace dependency pinning** (from tech-stack decision): every crate version lives in root `[workspace.dependencies]`, member crates reference via `{ workspace = true }`. Phase 1 is the first crate to exercise this pattern.
- **Error handling**: `thiserror` in libraries, `anyhow` only in bins/tests. `famp-canonical` is a library → `thiserror` only.
- **Clippy strict** + `unsafe_code = "forbid"` at workspace root (TOOL-07 shipped).
- **`cargo nextest`** is the test runner (TOOL-04 shipped) — write fixtures assuming nextest.

### Integration Points
- This crate is a **leaf** in Phase 1 — nothing depends on it yet inside this repo.
- Downstream (Phase 2): `famp-crypto` will call `canonicalize()` to produce bytes for Ed25519 signing, and will consume the domain-separation prefix applied to those bytes.
- Downstream (Phase 3): `famp-core` will refactor `ArtifactIdString` → strong `ArtifactId` type.
- Nothing in Phase 1 depends on Phase 2 or 3.

</code_context>

<specifics>
## Specific Ideas

- **API ergonomics target:** `canonicalize(&envelope)` at call sites should feel as natural as `serde_json::to_vec(&envelope)` — the trait exists to make that possible without forcing a trait import.
- **Fallback-plan-first discipline:** the point of writing `fallback.md` before running the gate is psychological as much as technical. When the gate fails at midnight on a Friday, a written plan prevents panic design.
- **Duplicate-key rejection = protocol guarantee, not hygiene.** Treat it with the same seriousness as signature verification. If serde-based rejection turns out to require unsafe gymnastics, escalate rather than relaxing the guarantee.
- **Float corpus sample:** "meaningful not prestige." Stability and determinism matter more than raw count.

</specifics>

<deferred>
## Deferred Ideas

- **Strong `ArtifactId` type** — Phase 3 (`famp-core`). Phase 1 ships a string placeholder helper.
- **Pre-scan tokenizer fallback for duplicate-key rejection** — documented as backup in fallback plan, only implemented if serde-visitor path proves infeasible.
- **Parallel fallback crate implementation** — explicitly rejected as premature. Revisit only if `serde_jcs` fails the gate.
- **Full 100M float corpus on every PR** — rejected; nightly + release-tag only.
- **15-category protocol error enum integration** — Phase 3 work. Phase 1 keeps `CanonicalError` narrow.
- **Domain-separation prefix logic** — Phase 2 (`famp-crypto`). Phase 1 produces raw canonical bytes; Phase 2 wraps them for signing.

</deferred>

---

*Phase: 01-canonical-json-foundations*
*Context gathered: 2026-04-12*
