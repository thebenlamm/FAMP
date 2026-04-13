# Phase 1: Spec Fork v0.5.1 - Context

**Gathered:** 2026-04-13
**Status:** Ready for planning
**Mode:** `--auto` (Claude-picked defaults from PROJECT.md, ROADMAP.md success criteria, REQUIREMENTS.md SPEC-01..20, and docs/PITFALLS.md)

<domain>
## Phase Boundary

Produce `FAMP-v0.5.1-spec.md` — a forked, reviewer-audited version of `FAMP-v0.5-spec.md` that resolves every ambiguity and spec bug identified by the 4 parallel review agents. Pure documentation phase: no Rust code, no crates touched. Output is the interop contract that Phases 2–8 hold as authoritative. Every change from v0.5 is cited in a changelog section referencing the review finding that drove it.

**In scope:** spec text, body schemas (inline), state-machine hole resolutions, canonical-JSON binding, domain-separation prefix, numeric defaults, encoding formats, spec-version constant, changelog.

**Out of scope:** any `.rs` file, `Cargo.toml` edits, crate scaffolding beyond the spec file's location, Python/TS binding commentary beyond a "reserved" note.

</domain>

<decisions>
## Implementation Decisions

### File location and structure
- **D-01:** Spec lives at repo root as `FAMP-v0.5.1-spec.md` (parallel to `FAMP-v0.5-spec.md`). v0.5 file is retained unchanged for diff/audit.
- **D-02:** Top-level ordering preserved from v0.5 to keep section numbers stable; new sections are appended or added as lettered sub-sections (e.g., §7.1a "Domain separation prefix") to avoid renumbering downstream references.
- **D-03:** A **Changelog from v0.5** section is added at the end of the document (before any appendices). Each entry has the shape: `v0.5.1-Δnn — <section touched> — <finding id/reviewer> — <resolution summary>`. The changelog is the normative record of every diff; inline diff markers in section bodies are not used.
- **D-04:** A **Spec-version constant** block near the top of the document declares `FAMP_SPEC_VERSION = "0.5.1"` (string, exact casing) and states that implementations MUST emit this in envelope headers and Agent Cards where the v0.5 spec referenced a version string.

### Canonical JSON (SPEC-02)
- **D-05:** Canonical JSON section is rewritten to say, verbatim: "Canonical JSON for FAMP is **RFC 8785 JSON Canonicalization Scheme (JCS)**." No paraphrasing of sort rules, number formatting, or Unicode handling — instead, the spec cites RFC 8785 §3.2.3 (UTF-16 code unit sort) and §3.2.2.3 (ECMAScript `Number.prototype.toString` number formatting) as normative.
- **D-06:** The spec explicitly notes **duplicate JSON object keys are rejected at parse** (RFC 8259 §4 is silent; RFC 8785 §3.1 requires "member names must be unique" in the input). An implementation that silently dedupes is non-conformant.
- **D-07:** Two normative worked examples are included: (a) a simple object with mixed-case ASCII keys showing UTF-16 sort and whitespace stripping, and (b) an object containing a supplementary-plane character (e.g., emoji U+1F600) showing surrogate-pair sort order. Both examples include the exact byte sequence output in hex. **Ratified 2026-04-13 after plan-check: D-07 is LOCKED — both examples are mandatory in Phase 1, bytes computed from the same external JCS reference implementation used for the Ed25519 worked example. Deferral to Phase 2 is explicitly rejected.**
- **D-08:** The spec states that `serde_json`-style features `arbitrary_precision` and `preserve_order` are incompatible with JCS and MUST NOT be used by conforming implementations. (Motivated by PITFALLS.md reviewer finding.)

### Signatures & domain separation (SPEC-03, SPEC-04, SPEC-19)
- **D-09:** Domain separation prefix is a fixed ASCII byte string: **`FAMP-sig-v1\0`** (11 ASCII chars + one NUL byte, 12 bytes total). Applied as `sig = Ed25519.sign(sk, prefix || canonical_json_bytes)`. Verification applies the same prefix.
- **D-10:** A byte-level worked example is included: a minimal `ack` envelope, its canonical JSON (hex), the prefix (hex), the concatenated signing input (hex), and the resulting 64-byte signature (hex) over a fixed test key pair (test key committed in spec text, never for production use).
- **D-11:** The signature field binds `to`: the `to` (recipient) envelope field is part of the canonical JSON that is signed. The spec adds explicit text: "Recipient anti-replay is achieved by including `to` under signature; a signed envelope addressed to agent A cannot be replayed to agent B."
- **D-12:** Ed25519 encoding is locked: **raw 32-byte public keys**, **raw 64-byte signatures**, both wire-encoded as **unpadded base64url** (RFC 4648 §5, no `=` padding). Decoders MUST reject padded input and MUST reject the standard (non-url) alphabet. `verify_strict` semantics (reject non-canonical S, reject small-subgroup A) are normatively required.

### Agent Card & identity (SPEC-05, SPEC-06)
- **D-13:** Agent Card adds a required `federation_credential` field. Card signing is **not** circular self-signature: the card is signed by a federation-scoped credential whose public key is published in the federation trust list. Trust-list distribution is federation-specific and out of scope for v0.5.1 beyond stating the interface.
- **D-14:** Card versioning: the card declares `card_version` (integer, monotonic) and `min_compatible_version` (integer). In-flight commits that were bound to a card with `card_version = N` remain valid through resolution even if the card rotates to `card_version = N+1` mid-flow, **provided** the new card's `min_compatible_version ≤ N`. Fresh requests always use the latest card.

### Numeric defaults & idempotency (SPEC-07, SPEC-08)
- **D-15:** Clock skew tolerance: **±60 seconds**. Default envelope validity window: **300 seconds (5 minutes)**. Both are RECOMMENDED defaults in the spec; federations MAY tighten but MUST NOT loosen beyond a documented cap of **±300s / 1800s** respectively.
- **D-16:** Idempotency key: **128 bits of cryptographic randomness**, encoded as unpadded base64url (22 chars). Collision scope is the tuple `(sender_principal, recipient_principal)`; receivers MUST deduplicate using `(id, idempotency_key, content_hash)` per the replay cache rule.

### State-machine hole resolutions (SPEC-09 through SPEC-16)
- **D-17:** **SPEC-09 — §9.6 terminal precedence:** Ack-disposition and terminal-state-crystallization are distinct. The spec rewrite cleanly separates (a) how an `ack` disposition updates causality metadata (always) from (b) when a terminal status crystallizes the task FSM (only on `deliver` with terminal status, or `control` with cancellation, or transfer-timeout reversion). An ack on a terminal message does not itself crystallize.
- **D-18:** **SPEC-10 — §7.3 "no body inspection":** The claim is retracted. A normative whitelist of envelope-level fields that the FSM inspects is published: `{ class, relation, body.interim, body.scope_subset, body.target, body.terminal_status }`. All other body content remains opaque to the protocol layer. Extensions MAY add fields but MUST NOT reuse these names.
- **D-19:** **SPEC-11 — Transfer-timeout race:** Tiebreak rule: a transfer-timeout reversion that fires while a delegate's commit is in-flight resolves in favor of **the delegate's commit if its `ts` precedes the timeout deadline**; otherwise the reversion wins. The loser receives a `conflict:transfer_timeout` error class.
- **D-19.1:** **Ratified 2026-04-13 after RESEARCH §3 SPEC-11 pressure-test.** The tiebreak is evaluated with a **δ = 60-second clock-skew guard band** against the transferring agent's clock: the delegate commit is "on-time" iff `delegate_commit.ts ≤ ts_deadline − δ`. Rationale: absent δ, any two clocks disagreeing by <60s (within SPEC-07's allowed skew) produce non-deterministic tiebreak outcomes between implementations. δ matches the clock skew tolerance from D-15 for consistency. Loser still receives `conflict:transfer_timeout`. Plan 04 Task 2 implements this refinement.
- **D-20:** **SPEC-12 — EXPIRED vs in-flight deliver:** The default "delivery-wins" rule is amended: a `deliver` whose `ts` is strictly before the task EXPIRED deadline MUST be accepted and crystallize the task as COMPLETED/FAILED; a `deliver` at or after the deadline is rejected with `stale:expired`.
- **D-21:** **SPEC-13 — Conditional-lapse precedence:** Committer-side conditional lapse (a `control:cancel_if_not_started` or equivalent) **wins over delivery-wins** when both fire in the same tick. Rationale: the committer's right to withdraw an unaccepted conditional is a safety property.
- **D-22:** **SPEC-14 — Competing-instance commits (INV-5 hole):** An intermediate pseudo-state `COMMITTED_PENDING_RESOLUTION` is defined. Two concurrent commits from different instances of the same principal both enter this state; resolution picks the commit with the lexicographically smaller `id` (UUIDv7 time-ordered tiebreak). The loser transitions to REJECTED with `conflict:competing_instance`. INV-5 holds because the public observable state is still a single terminal.
- **D-23:** **SPEC-15 — Supersession round counting:** When a `propose` supersedes a prior `propose`, the round counter continues from the superseded round's number (does not reset). Rationale: prevents round-limit (INV-11) circumvention via supersession loops.
- **D-24:** **SPEC-16 — Capability snapshot vs card-version drift:** Capability snapshot is taken at the moment of `commit` and is bound to the committing card's `card_version`. Subsequent card rotations do not retroactively invalidate the commit. This resolves the contradiction in favor of commit-time binding, consistent with D-14.

### Body schemas (SPEC-17)
- **D-25:** Body schemas are defined **inline in the spec** (not as separate JSON Schema files) with a field-per-line table: field name, JSON type, required/optional, constraint notes. Five schemas: `commit`, `propose`, `deliver`, `control`, `delegate`. The `ack`, `announce`, `describe`, `request` classes retain their v0.5 definitions (noted in changelog).
- **D-26:** Every body schema declares `additionalProperties: false` semantics (translated to implementations as `deny_unknown_fields` at decode). Spec text states unknown fields are rejected; extensions live under an explicit `extensions` map.
- **D-27:** The `deliver` body schema includes `interim: bool`, `terminal_status: TerminalStatus?`, and an artifact list; `commit` includes `scope_subset` for partial-acceptance; `control` includes `target: ControlTarget` and an enumerated action; `delegate` includes `form: "assist" | "subtask" | "transfer"` plus delegation ceiling fields (`max_hops`, `max_fanout`, `allow`, `forbid`). `propose` mirrors `commit` minus commitment binding. Field-level details are worked out in the plan step.

### Artifact identifiers & hashes (SPEC-18)
- **D-28:** Artifact IDs use the scheme `sha256:<hex>`. Hash is SHA-256 over the canonical JSON of the artifact body (not over the raw input), **lowercase hex**, 64 characters. Alternative hash algorithms are reserved via the `sha<N>:` prefix but NOT defined in v0.5.1.

### Spec-version constant (SPEC-20)
- **D-29:** `FAMP_SPEC_VERSION = "0.5.1"` — exact string, case-sensitive. Implementations MUST emit this unchanged in any envelope header or Agent Card field that references the spec version. Upgrading to a newer spec requires changing the string; a message with a mismatched version is rejected with `unsupported_version`.

### Claude's Discretion
- Exact section ordering within each SPEC-xx rewrite (as long as normative content is preserved and changelog cites the edit).
- Prose style and example variable names in worked examples.
- Whether body schemas are rendered as tables, code blocks, or a hybrid — whichever reads cleanest on GitHub.
- Minor editorial fixes encountered while forking (typos, broken cross-references) — logged in changelog as `editorial` entries.

</decisions>

<specifics>
## Specific Ideas

- Changelog entries must be cross-referenceable: each gets a stable ID (`v0.5.1-Δnn`) that downstream phase plans can cite when they implement the corresponding behavior.
- Worked examples should use test keypairs and test envelopes that can be **reused byte-for-byte** as the first entries in Phase 8's conformance vector fixtures. Spec text and fixture file share the same bytes.
- Feel like RFCs, not like a blog post: numbered sections, normative MUST/SHOULD/MAY language (RFC 2119), no hedging in requirement text.
- When the spec text and RFC 8785 disagree on any edge case, **RFC 8785 wins** and the spec says so explicitly.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Source spec being forked
- `FAMP-v0.5-spec.md` — The v0.5 spec; every section is potentially touched. Section numbers cited in decisions above refer to this file.

### Reviewer audit & pitfalls
- `docs/PITFALLS.md` — 4-reviewer audit findings; every decision above cites a finding from here. The planner/executor uses this to verify no finding is left unresolved.
- `docs/SUMMARY.md` — Research synthesis across reviewers.
- `docs/ARCHITECTURE.md` — Crate DAG; relevant because spec's normative requirements must be realizable in the planned crate layout.
- `docs/FEATURES.md` — v1 feature set the spec must cover.
- `docs/STACK.md` — Tech stack; canonical-JSON and Ed25519 library choices constrain the spec's encoding normatives.

### External normative references (must be cited verbatim in spec)
- **RFC 8785** — JSON Canonicalization Scheme (JCS). Normative for canonical JSON section. <https://datatracker.ietf.org/doc/html/rfc8785>
- **RFC 8032** — Edwards-Curve Digital Signature Algorithm (EdDSA). Normative for Ed25519 signing. <https://datatracker.ietf.org/doc/html/rfc8032>
- **RFC 9562** — UUID (including UUIDv7). Normative for `id` field format.
- **RFC 4648 §5** — base64url encoding (unpadded variant).
- **RFC 2119 / RFC 8174** — Key words for normative requirement language.
- **RFC 8259** — JSON data interchange format (base reference for RFC 8785).

### Project context
- `.planning/PROJECT.md` — Vision, constraints, key decisions logged in prior phase.
- `.planning/REQUIREMENTS.md` — SPEC-01..SPEC-20 and their acceptance criteria.
- `.planning/ROADMAP.md` §"Phase 1: Spec Fork v0.5.1" — Success criteria (6 items) the spec fork must satisfy.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **None for this phase** — Phase 1 produces no code. The 13-crate workspace from Phase 0 exists and its crate names (`famp-canonical`, `famp-crypto`, `famp-envelope`, `famp-identity`, `famp-causality`, `famp-fsm`, `famp-protocol`, `famp-extensions`, `famp-transport`, `famp-transport-http`, `famp-conformance`, `famp-core`, umbrella `famp`) constrain how the spec refers to implementation layers in non-normative guidance text.

### Established Patterns
- **Docs live at repo root for the spec, in `docs/` for research artifacts** — Phase 0 placed `FAMP-v0.5-spec.md` at root and research docs in `docs/`. Phase 1 preserves this: `FAMP-v0.5.1-spec.md` at root.
- **Changelog discipline from v0.5 → v0.5.1 mirrors the commit-level discipline Phase 0 established** — every change cites a finding, every entry is auditable.

### Integration Points
- **Phase 2 (`famp-canonical`, `famp-crypto`) is the first consumer** — canonical JSON section must be unambiguous enough that the Phase 2 planner can write RFC 8785 test-vector assertions directly from the spec text, with no additional interpretation.
- **Phase 3 (`famp-envelope`) consumes the body schemas** — schemas must be specific enough that a `serde` struct with `deny_unknown_fields` maps one-to-one onto each body class.
- **Phase 5 (`famp-fsm`) consumes the hole resolutions** — each resolved hole must specify state, event, and outcome precisely enough that an exhaustive `match` compiles without guessing.
- **Phase 8 (`famp-conformance`) reuses the worked-example bytes as vector #1 and vector #2** — spec text and conformance fixtures share byte-for-byte content.

</code_context>

<deferred>
## Deferred Ideas

- **Multi-party commitment profiles** — explicitly out of scope per PROJECT.md; spec v0.5.1 reaffirms bilateral-only.
- **Cross-federation delegation** — deferred; spec v0.5.1 says "reserved".
- **Streaming (token-by-token) deliver** — v0.5.1 says `interim: true` is sufficient for v1.
- **Additional hash algorithms** beyond SHA-256 — `sha<N>:` prefix reserved, not specified.
- **Real trust registry** — spec defines the interface boundary only; registry is federation-specific.
- **Python/TS binding commentary in the spec** — a single "Reserved for future bindings" line only; no normative content.
- **Level 1 conformance profile** — v0.5.1 spec does not define a Level-1-only release profile. L2+L3 is the minimum conformance target.

</deferred>

---

*Phase: 01-spec-fork-v0-5-1*
*Context gathered: 2026-04-13*
*Mode: `--auto` — decisions sourced from PROJECT.md, ROADMAP.md success criteria, REQUIREMENTS.md SPEC-01..20, and docs/PITFALLS.md. Downstream planner/executor may revisit any decision by citing the source and rationale.*
