# Pitfalls Research

**Domain:** Protocol reference library (signed messages, canonical JSON, state machines, federation trust) in Rust
**Researched:** 2026-04-12
**Confidence:** HIGH on JCS/Ed25519/serde (verified via RFC + ed25519-dalek source + serde issues); MEDIUM on protocol-history lessons (community sources); MEDIUM on Rust FSM ergonomics (community experience).

> This document **extends** the prior-review findings already captured in `.planning/PROJECT.md` (spec §9.6 holes, §7.3 body-inspection, INV-5 race, negotiation round counting, Agent Card circular self-sig, missing domain separation, SHA-256 artifact encoding, body schemas undefined). It does not restate those. Phase numbers refer to the roadmap sketch in PROJECT.md (Phase 0 toolchain → Phase 1 spec fork → Phase 2 canonical+crypto → Phase 3 core/envelope/identity → Phase 4 causality+FSM → Phase 5 negotiate/delegate/provenance → Phase 6 transport → Phase 7 conformance/adversarial/integration).

---

## Critical Pitfalls

### Pitfall 1: JCS key sort uses UTF-8 byte order instead of UTF-16 code-unit order

**What goes wrong:** RFC 8785 §3.2.3 *mandates* that object members be sorted by **UTF-16 code-unit** order, not UTF-8 bytes. For ASCII and BMP-below-U+E000 keys these are identical, so the bug is invisible for 99% of test inputs. It surfaces the first time a key contains a supplementary-plane character (emoji, CJK Ext B, historic scripts, math symbols): U+10000 encodes as a surrogate pair in UTF-16 (D800 DC00) and therefore sorts **before** U+E000, but its UTF-8 bytes (F0 90 80 80) sort **after**. Two "conformant" implementations produce different byte strings, signature verification fails, and the debugging trail is non-obvious because canonical output looks visually identical in most editors.

**Why it happens:** Rust's `BTreeMap<String, _>` and `str::cmp` both compare by UTF-8 bytes. `cmp` on `&str` is byte-wise, not UTF-16-wise. Using idiomatic Rust collections gets you the wrong answer silently. The reference C and Go implementations linked from RFC 8785 explicitly re-encode each key to UTF-16 before sorting — Rust ports that skip this step are broken.

**How to avoid:** Implement `jcs_key_cmp(a: &str, b: &str) -> Ordering` that walks both strings as `char` iterators and compares the UTF-16 encoding (`char::encode_utf16`) code unit by code unit. Unit-test against RFC 8785 Appendix B test vectors **plus** hand-crafted keys containing at least one character from each of: BMP PUA (U+E000), supplementary plane (U+1F600 emoji), and a high-BMP CJK char. Do NOT rely on `BTreeMap` for canonical serialization.

**Warning signs:** Conformance tests pass ASCII-only fixtures but fail the moment you add emoji to a fixture. `cargo test` clean locally but another implementation produces different bytes for the same input. Hash mismatches only for specific payloads.

**Phase to address:** **Phase 2** (`famp-canonical` crate). Must be baked into the canonicalizer from day one with supplementary-plane test vectors *in the initial commit*, not added later.

---

### Pitfall 2: JCS number serialization cannot be delegated to Rust's default float formatting

**What goes wrong:** RFC 8785 §3.2.2 defines number output via the ECMAScript 2017 `Number.prototype.toString` / `ToString(Number)` algorithm — specifically the "shortest roundtrip" form per the Grisu2/Ryu family, but with ECMAScript-specific rounding and exponent threshold rules (integer exponents ≥ 21 switch to exponent form; `-0` renders as `0`; `NaN`/`±Infinity` are errors). Rust's `f64::to_string`, `ryu`, and `serde_json`'s float formatter each produce subtly different output in edge cases (rounding halfway cases, `1e21` vs `1000000000000000000000`, `-0` handling). Any of these mismatches causes signature failure.

**Why it happens:** "Just use `ryu`" sounds right — ryu *is* shortest-roundtrip — but ECMAScript's algorithm diverges from ryu on rounding ties and on the exponent-vs-fixed threshold. The reference JCS test suite catches this, but only if you actually run it against millions of IEEE-754 samples (the cyberphone/json-canonicalization repo publishes a 100M-sample test file for this exact reason).

**How to avoid:** (a) Use or port a known-good ECMAScript number formatter; the cyberphone reference C implementation is the authoritative source. (b) Run the published 100M-sample float test file as part of CI, not just `cargo test`. (c) Reject `NaN`, `±Infinity`, and any number where `is_finite()` is false *at the serializer boundary* with a typed error. (d) For integers that won't survive round-trip through f64 (abs value > 2^53), represent as JSON strings per RFC 8785 §6 guidance; add a lint/compile-time check that no protocol field uses `i64`/`u64` directly in serialized types.

**Warning signs:** Self-round-trip tests pass (`serialize → parse → serialize` gives same bytes) but cross-implementation tests fail on specific numeric values. Failures cluster around powers of 10 near 1e21 or near half-ULP boundaries.

**Phase to address:** **Phase 2**. Pull in the cyberphone test vectors during crate bootstrap. If the library treats floats as protocol primitives at all — even in provenance metrics — the 100M-sample test is mandatory before declaring `famp-canonical` done.

---

### Pitfall 3: Unicode normalization is silently applied somewhere in the pipeline

**What goes wrong:** RFC 8785 explicitly does **not** normalize strings — it preserves whatever code points the input contains. But a helpful library elsewhere in the stack (HTTP framework, logging middleware, a database round-trip, a "defensive" upfront `nfc()` call) applies NFC or NFKC on the way in or out, and now "café" (precomposed U+00E9) and "café" (e + U+0301 combining) hash to different canonical bytes. Sender signs one form, receiver verifies the other, signature fails for "no reason."

**Why it happens:** Canonical JSON guidance online frequently conflates "canonical" with "normalized." Well-meaning contributors add `unicode-normalization` deps. HTTP frameworks, filesystems (HFS+!), and some JSON parsers normalize without documenting it.

**How to avoid:** (a) Document in the canonicalizer doc comments: "This implementation preserves input code points byte-exactly; it does NOT apply NFC/NFKC. Normalization, if required by the application, is the caller's responsibility and must happen *before* signing." (b) Add a CI grep that fails if `unicode-normalization` appears in `Cargo.lock`. (c) Conformance vector: sign a message with combining-character content; verification must succeed round-trip but **fail** against an NFC-normalized copy, proving the canonicalizer is not normalizing.

**Warning signs:** Signatures pass in unit tests and fail only when a message traverses the full HTTP path. Verification failures correlate with input containing non-ASCII.

**Phase to address:** **Phase 2** for the canonicalizer contract, **Phase 6** for the HTTP transport round-trip test that proves no middleware mutation occurs.

---

### Pitfall 4: Ed25519 verification accepts small-subgroup / low-order public keys

**What goes wrong:** A malicious or malformed public key with order dividing 8 (the curve's cofactor) can produce signatures that verify for many messages, because the cofactored verification equation `[8][S]B = [8]R + [8][k]A'` admits solutions that the strict equation rejects. Worse, an attacker who controls the public key registration can generate keys such that a *single* signature is valid for multiple messages. For a federation registry that accepts arbitrary agent-supplied public keys, this is a trust-anchor break.

**Why it happens:** Historical `ed25519-dalek` (<1.0) `verify()` used the cofactored equation; current `verify()` still permits keys the RFC 8032 "strict" mode would reject. Only `VerifyingKey::verify_strict()` rejects `signature_R.is_small_order() || point.is_small_order()`. Developers paste `pk.verify(msg, &sig)` from a tutorial and ship.

**How to avoid:** (a) In `famp-crypto`, wrap `ed25519-dalek` and expose *only* `verify_strict`. Mark raw `verify` as `#[deprecated]` at the module boundary or `pub(crate)`-hide it. (b) Additionally, when *accepting* a public key (Agent Card registration, federation trust list import), call `VerifyingKey::is_weak()` or replicate the small-order check and reject at ingress. (c) Conformance vector: include known-weak public keys from the `ed25519-dalek` test suite as a "must reject" fixture.

**Warning signs:** No runtime symptom — this is a latent vulnerability that manifests only under attack. Detection is code review + test-vector coverage, not observation.

**Phase to address:** **Phase 2** (`famp-crypto`). Non-negotiable. Also gate at **Phase 3** (`famp-identity`) when pubkeys are accepted into the trust store.

---

### Pitfall 5: Domain separation string is added, but is hashed *after* canonicalization instead of *before*

**What goes wrong:** The spec fork adds a domain-separation prefix to signed payloads (PROJECT.md Context item 4). The natural-feeling implementation is `sign(canonicalize(msg) ++ b"famp-v0.5.1")` or `sign(b"famp-v0.5.1" ++ canonicalize(msg))`. Both work, but they must be **identical on both sides** and must be **documented at the byte level**. A common subtle bug: one side prepends the prefix to the canonical JSON bytes, the other includes the prefix as a JSON field inside the object and then canonicalizes. Both pass their own round-trip tests. Neither interops.

**Why it happens:** "Domain separation" is folklore-level advice in most blog posts; the mechanical implementation is rarely spelled out. Different NIST/IETF specs use different conventions (SHA-2 vs SHA-3 input framing, COSE's sig_structure, JWS's signing input). Without a reference byte-level example in the spec fork, two implementers will disagree.

**How to avoid:** (a) Spec fork (Phase 1) MUST include a **worked example with hex dumps**: input message, canonical JSON bytes, signing input bytes (prefix concatenation shown byte-wise), signature bytes, public key bytes, verification result. (b) Define the prefix as a fixed ASCII string with a length byte or null terminator so concatenation ambiguity is impossible (recommend `b"FAMP-v0.5.1-envelope-sig\x00"` or Tink-style length-prefixed). (c) Ship this worked example as conformance vector #1.

**Warning signs:** Two independent implementations each self-verify but cross-verification fails with `SignatureError` on *every* message, not just edge cases.

**Phase to address:** **Phase 1** (spec fork defines the byte format), **Phase 2** (`famp-crypto` implements it), **Phase 7** (cross-implementation conformance vector confirms).

---

### Pitfall 6: Serde tagged-enum dispatch silently accepts messages that should fail class-check

**What goes wrong:** Message class in FAMP is dispatched via some field (likely `"class"` or `"type"`). The idiomatic Rust pattern is `#[serde(tag = "class")]` on an enum with one variant per class. Two failure modes: (a) **Untagged fallback** — if any developer later adds `#[serde(untagged)]` variant or a catch-all, serde will try variants in source order and return the first that deserializes, producing "successful" parse of garbage into whatever variant happens to be permissive first. Error messages become useless ("data did not match any variant"). (b) **Extra fields ignored** — by default, serde ignores unknown fields; a message with `"class":"commit"` plus a typo in a body field (e.g., `"artifcts"` instead of `"artifacts"`) deserializes to a `Commit` with empty artifacts, signature verifies against the original bytes, and the receiver silently acts on an incomplete commit.

**Why it happens:** Serde defaults favor flexibility over strictness. `#[serde(deny_unknown_fields)]` is opt-in and easy to forget. Untagged enums are cargo-culted from JSON-API tutorials. Error reporting with untagged enums is infamously bad (multiple serde issues open on this).

**How to avoid:** (a) Use `#[serde(tag = "class", deny_unknown_fields)]` on the top-level message enum and on *every* body struct. Add a workspace-wide clippy lint or a `build.rs` grep that fails CI if any protocol struct lacks `deny_unknown_fields`. (b) Never use `#[serde(untagged)]` in protocol types — if polymorphism is needed, implement `Deserialize` by hand with explicit dispatch and explicit error messages. (c) Conformance vector: "message with typo in required field" → expected result = parse error, not silent-success.

**Warning signs:** Logs show successfully parsed messages with unexpectedly empty vec/option fields. Senders complain that fields they sent "don't make it." Conformance tests that use strict field schemas pass, but loose-field tests also pass (they should fail).

**Phase to address:** **Phase 3** (`famp-core`, `famp-envelope`). Establish the `deny_unknown_fields` discipline in the first protocol struct written; retrofitting is painful once 50 structs exist.

---

### Pitfall 7: State machine stored as `enum State { ... }` borrowing from message data

**What goes wrong:** A Rust beginner, excited about zero-copy, writes `enum ConvState<'a> { Negotiating(&'a Proposal), Committed(&'a Commit) }`. Lifetimes propagate into every function, storing a state value in a `HashMap` becomes impossible, async tasks can't hold state across `.await` points, and the entire codebase rewrites itself into `Arc<Mutex<_>>` in a panic. Two weeks lost to lifetime hell for zero performance benefit (FSM transitions are nanosecond-scale; state is hundreds of bytes not megabytes).

**Why it happens:** Rust tutorials emphasize borrowing. FAMP state objects are small, owned, and long-lived — the opposite of the borrow-optimization sweet spot. Beginners don't yet have the calibration to say "this should just be `Clone`."

**How to avoid:** (a) Rule in `famp-fsm` module docs: **no lifetimes in state types.** All state variants are `#[derive(Clone, Debug)]` and own their data via `Arc<str>`, `Box<[u8]>`, `SmallVec`, or plain `String`/`Vec`. (b) Use `Arc` for shared immutable fields (artifact hashes, principal IDs) to keep clones cheap. (c) The type of a conversation state should be `ConvState` not `ConvState<'_>`. If `cargo check` suggests adding a lifetime parameter to a state enum, that's a sign to use `Arc` or `Clone` instead. (d) Compiler-checked terminal states (Key Decision in PROJECT.md) should use the standard "typestate" pattern with zero-sized marker types, which is orthogonal to lifetimes and does not introduce them.

**Warning signs:** `cargo check` errors mentioning `'static`, "does not live long enough," or "lifetime may not live long enough" in FSM code. FSM state can't be stored in a `HashMap<ConvId, State>`. `tokio::spawn` complaints about `'static` bounds.

**Phase to address:** **Phase 0** (learning phase — document rule before code exists), **Phase 4** (`famp-fsm`, when state types are first written).

---

### Pitfall 8: `Transport` trait is designed with borrowed buffers, locking out async and dyn-dispatch

**What goes wrong:** First-draft `Transport` trait: `fn send(&self, msg: &Envelope) -> Result<&Response, Error>`. Three problems pile up: (a) returning `&Response` is impossible for any async transport that owns its connection, (b) async methods in traits without `async fn` desugaring create `impl Future + '_` soup, (c) `dyn Transport` object-safety breaks as soon as a method has generic type parameters or GATs.

**Why it happens:** Beginners write sync-first traits and retrofit async. The Rust async-trait ecosystem changed significantly in 2023-2024 (native async-fn-in-trait stabilized in 1.75) and blog posts from before that window give outdated `#[async_trait]` advice that won't compose with the newer patterns.

**How to avoid:** (a) Define `Transport` with **owned inputs and outputs**: `async fn send(&self, msg: Envelope) -> Result<Response, TransportError>`. (b) Use native `async fn` in trait (Rust ≥1.75) for internal use, and if `dyn Transport` is required for plugging in transports at runtime, provide a `DynTransport` wrapper that boxes the future (`Pin<Box<dyn Future<Output = ...> + Send + '_>>`). (c) Make `MemoryTransport` and `HttpTransport` the two concrete implementations that pin the trait shape — if both can implement it cleanly, the trait is right. (d) Explicitly decide generic (monomorphized, faster, binary bloat) vs trait object (runtime plug-in, slower, smaller binary); for a library with ≤3 transports, generic is almost always correct.

**Warning signs:** `#[async_trait]` appearing in the codebase at all (a smell, not a bug) — modern Rust rarely needs it. Transport trait has lifetimes on method signatures. Library consumers can't store a `Box<dyn Transport>`.

**Phase to address:** **Phase 6** (`famp-transport`). Pin the trait shape by implementing both `MemoryTransport` and `HttpTransport` in the same phase; don't ship with only one.

---

### Pitfall 9: Async cancellation drops an in-flight state transition mid-commit

**What goes wrong:** A client does `tokio::time::timeout(5s, transport.send(commit)).await`. The timeout fires while `send` is mid-flight. The future is dropped at an `.await` point. From the client's view the commit "failed" (retry). From the server's view, the commit was received, persisted, and the response was lost on the wire. Client retries; server sees a duplicate commit attempt that violates INV-5 (single terminal state). Protocol dead.

**Why it happens:** Rust async futures are cancel-by-drop. Code written as if `await` points are atomic is wrong: *any* `.await` is a potential drop site. "Cancellation safety" is the term of art; most transport code in the wild is NOT cancellation-safe. `tokio` documentation for `select!` warns about this explicitly, but `timeout()` is less obviously cancellable.

**How to avoid:** (a) Document cancellation-safety contract for every `Transport` method in doc comments: "Cancellation of this future after the request has been transmitted MAY result in the operation having taken effect server-side." (b) Require that commit-class operations are **idempotent by `idempotency_key`** end-to-end — retries of the same logical commit must be safe. (Note: PROJECT.md Context mentions idempotency key collision surface as an existing concern; this pitfall compounds it — keys must be both unique AND honored by retry logic.) (c) For state-mutating transport calls, spawn the send into a `tokio::task` owned by the conversation controller, and let the controller (not the caller's timeout) own cancellation decisions. (d) Adversarial test in Phase 7: inject random future-drop at every `.await` point in the commit path using a custom `CancelOnAwait` wrapper; protocol invariants must hold.

**Warning signs:** Duplicate commits in logs. Production reports "I committed but the server says I didn't" or vice versa. Flaky integration tests that pass 99% of the time.

**Phase to address:** **Phase 6** (transport contract), **Phase 7** (adversarial cancellation tests). Do not ship without the drop-injection test.

---

### Pitfall 10: Conformance vectors are generated by the same code they're meant to validate

**What goes wrong:** "Conformance vectors" committed to the repo are produced by running `famp-canonical::canonicalize()` on sample inputs and dumping the output. The tests then verify that `canonicalize(input) == committed_output`. This tests **reproducibility of the bug**, not **correctness**. If the JCS sort is wrong (Pitfall 1), the vectors are wrong in the same direction, and every test passes. A second implementation will fail against these "conformance" vectors with no way to distinguish "my impl is wrong" from "FAMP's vectors are wrong."

**Why it happens:** It's the easiest way to generate vectors. Adversarial review (PROJECT.md workflow preference) catches this only if the reviewer specifically asks "where did these bytes come from?"

**How to avoid:** (a) Canonical-JSON vectors must come from an **external** source: (i) RFC 8785 Appendix B test vectors verbatim, (ii) cyberphone/json-canonicalization published test data (C and Go reference implementations), (iii) hand-computed hex dumps for FAMP-specific cases. Commit the **source** (reference impl name + version) alongside each vector. (b) Crypto vectors: use RFC 8032 test vectors for Ed25519 raw sign/verify, add FAMP-specific vectors only for the signing-input-construction layer (which is FAMP-novel). (c) For FAMP-specific vectors, generate with a **second implementation** (even a quick-and-dirty Python script using `cryptography` + an existing JCS library) and diff the bytes against the Rust output before committing. If only Rust has produced them, they're not conformance vectors — they're regression vectors (still useful, but labeled differently in the repo).

**Warning signs:** `test-vectors/README.md` has no "source" column. All vectors have timestamps within the same git commit. No vector file was authored by any non-Rust tool.

**Phase to address:** **Phase 2** (first canonical vectors), **Phase 7** (full conformance suite). Adversarial review (per CLAUDE.md workflow) must explicitly check vector provenance.

---

### Pitfall 11: `proptest` generators accidentally cover only "already canonical" inputs

**What goes wrong:** Property test for canonicalization: `prop_assert_eq!(canonicalize(msg.clone()), canonicalize(canonicalize(msg)))` (idempotence). Looks right. But the `proptest` generator for the message type produces `BTreeMap<String, Value>`, which is already key-sorted. The property is tested only on inputs that are already canonical, and the canonicalizer's sort step is never exercised by proptest. Bugs in sort (Pitfall 1) slip through 10,000 random iterations.

**Why it happens:** Using collections with deterministic iteration order (BTreeMap, IndexMap, Vec with sorted Strings) as generator output elides the interesting entropy. Developers trust property-test iteration counts without examining the *shape* of inputs.

**How to avoid:** (a) Generators for canonicalizer tests must produce inputs that are explicitly **non-canonical** at the input side: use `Vec<(String, Value)>` with randomized order, inject duplicate keys, inject mixed-normalization Unicode, inject numbers that require re-formatting (`1.0`, `1e0`, `10e-1`), inject whitespace. (b) Measure generator coverage: add an assertion that counts "how many generated inputs were already byte-identical to their canonical form" — if >10%, the generator is broken. (c) Include a hand-written "pathological inputs" table alongside proptest: the deliberate adversarial cases (surrogate pairs, 1e21-1, combining marks, duplicate keys) that proptest will rarely hit by chance.

**Warning signs:** Proptest runs 10K cases in <100ms (should be slow because JCS is O(n log n) on non-trivial inputs). Coverage reports show the sort function as uncovered. Proptest shrinks always converge to empty object.

**Phase to address:** **Phase 2** for canonicalizer proptest, **Phase 4** for FSM proptest, **Phase 7** for full coverage review.

---

### Pitfall 12: Agent Card key rotation breaks all in-flight commitments

**What goes wrong:** Agent A has three in-flight conversations with Agent B. A rotates its signing key (legitimate rotation, not compromise). A publishes a new Agent Card with the new public key. B's next inbound message on any of the three conversations fails signature verification (B is using the newly-fetched card with the new key, but the old messages signed with the old key). Either rotation breaks all in-flight work (unacceptable for a long-running task protocol), or messages become unverifiable in post-hoc audit (unacceptable for provenance).

**Why it happens:** Key-rotation design often treats "the current key" as the lookup, rather than "the key *at the time the message was signed*." The card's version field exists (spec mentions card-version binding, PROJECT.md notes capability-snapshot contradiction) but is not consistently used for signature verification lookup.

**How to avoid:** (a) Agent Cards must be **immutable and versioned**: a rotation publishes a new card at `card_v=N+1`, the old card at `card_v=N` stays resolvable indefinitely. (b) Every signed envelope includes the *signing card version* (not just the principal ID) in the signed fields. Verifier resolves `(principal_id, card_version) → public_key` from an immutable store. (c) Trust store retention policy: keep old cards at least as long as the longest permissible commitment/delegation (spec must define this — flag for Phase 1 spec fork). (d) Adversarial test: rotate key mid-conversation; existing messages must still verify; new messages on the same conversation use the new card version; both card versions resolvable.

**Warning signs:** Verification failures correlate with recent key rotations. Audit logs cannot re-verify historical messages. "We had to pause all conversations to rotate keys."

**Phase to address:** **Phase 1** (spec fork: define immutable versioned cards + retention), **Phase 3** (`famp-identity` implements versioned resolution), **Phase 7** (rotation adversarial test).

---

### Pitfall 13: Extension registry (§17.4) decays into a junk drawer the reference implementation never exercises

**What goes wrong:** `famp-extensions` ships with a registry structure and the INV-9 fail-closed rule. Six months later, the reference implementation itself uses zero critical extensions and one non-critical extension (which is trivially ignored in tests). The fail-closed path is never exercised on realistic traffic. The first external implementation that tries to use a critical extension hits a bug in the reference's "unknown critical" handler that has never run in production. PROJECT.md explicitly flags this concern ("§17.4 warns about this — make sure reference uses them") — this pitfall operationalizes the warning.

**Why it happens:** Extension points are "for future use." Future never comes in the reference codebase. The first real user is the test of a code path that has only been code-reviewed, never executed.

**How to avoid:** (a) The reference implementation MUST ship with **at least one critical and one non-critical extension** that the reference itself exercises end-to-end in conformance tests — even if those extensions are trivial (e.g., `x-famp-test-critical` that adds a no-op required field, and `x-famp-test-optional` that adds a hint). (b) Conformance vectors include a "receiver does not know critical extension X, MUST fail-closed" case, with two reference implementations: one that knows X (verifies), one that doesn't (rejects). (c) Code coverage gate: `famp-extensions` must have >90% line coverage, specifically on the "unknown critical → error" path.

**Warning signs:** `grep -r 'x-famp' .` returns no real usages. Extension-registry code has zero coverage in `cargo tarpaulin`. Fail-closed error type has no runtime test triggering it.

**Phase to address:** **Phase 5** (`famp-extensions`, alongside the features that would plausibly use extensions), **Phase 7** (conformance vectors for unknown-critical rejection).

---

### Pitfall 14: Spec drift — v0.5.1 fork advances but code stays on v0.5 assumptions in one forgotten corner

**What goes wrong:** Spec fork (Phase 1) is treated as one-and-done. Later, during Phase 4 FSM work, a reviewer finds another edge case, the fork gets a v0.5.2 bump, but the patch only updates the FSM module — the envelope module still encodes to v0.5.1 field layout in one error path. Tests pass because the error path is untested. First production use of that error path breaks interop.

**Why it happens:** Spec version is not represented as a compile-time or runtime check inside the code. "Which version does this code target" exists only in the spec document and in git commits.

**How to avoid:** (a) Define `pub const FAMP_SPEC_VERSION: &str = "v0.5.1";` in `famp-core`. Every envelope carries this in a `spec_version` field of the signed payload (already plausibly required by spec, confirm in Phase 1). (b) Parser rejects envelopes whose `spec_version` does not match — no "accept any" mode in v1. (c) When spec is bumped, the version constant is bumped in the same PR as the spec fork change; CI fails if `.planning/spec-version` (or whatever sentinel) disagrees with the constant. (d) All modules import the constant; no string literals for version anywhere in the codebase (grep check in CI).

**Warning signs:** Multiple string literal versions ("v0.5.1") scattered across the codebase. Spec has been bumped but `Cargo.toml` workspace version hasn't. No single source of truth for "what version is this code."

**Phase to address:** **Phase 1** establishes the sentinel, every subsequent phase must keep it coherent. Phase 7 adds a lint.

---

### Pitfall 15: Build-time spiral from a wide crate graph

**What goes wrong:** Twelve member crates in the workspace (per PROJECT.md Active list). Each pulls `serde`, `serde_json`, `ed25519-dalek`, `thiserror`, `tokio`. Each is a separate compilation unit with its own dependency resolution. `cargo build` cold takes 4+ minutes. `cargo test` iteration loop becomes 20+ seconds per change. Development velocity craters; beginner user (per PROJECT.md) loses confidence that the Rust toolchain is usable.

**Why it happens:** "One crate per concern" is good modularity but bad compile time. Rust incremental compilation is per-crate; modifying a type in `famp-core` rebuilds every downstream crate. Serde derive + generics are notoriously slow to compile.

**How to avoid:** (a) Start with **fewer crates** than PROJECT.md's Active list suggests — merge `famp-canonical`+`famp-crypto`+`famp-core`+`famp-envelope` into a single `famp` crate initially; split later only when public API boundaries demand it. (b) Use `cargo-nextest` for test execution (2-4x faster than `cargo test`). (c) Set `[profile.dev] debug = "line-tables-only"` in `Cargo.toml` — skips most debuginfo, cuts link time significantly. (d) Use `sccache` for CI and encourage local install. (e) Measure cold+warm build time at end of Phase 0 and add a CI alert if warm rebuild on `famp-core` type change exceeds 30s. (f) Reserve workspace-crate-per-concern granularity for *public API surface* distinctions (FFI boundary, optional feature), not for internal modularity — internal modules are free.

**Warning signs:** `cargo check` on a one-line change takes >10s. `cargo test` feedback loop exceeds 30s. Beginner user stops iterating and batches changes.

**Phase to address:** **Phase 0** (workspace scaffold — pick initial crate granularity here; changing later is costly). Revisit at **Phase 5** boundary.

---

### Pitfall 16: Happy-path two-node integration test hides concurrent adversarial failures

**What goes wrong:** The headline test in PROJECT.md Active list is "two-node integration test exercising negotiate → commit → delegate → deliver." It passes. Demo looks great. But the test runs synchronously, one message at a time, no jitter, no concurrent second conversation. All the interesting bugs — INV-5 races, competing-instance commits (already flagged in PROJECT.md), cancellation-at-await (Pitfall 9), key rotation (Pitfall 12) — live in concurrent paths that the happy-path test never enters.

**Why it happens:** Integration tests are written to demonstrate the protocol works. Adversarial tests are often deferred until the protocol is "stable," but "stable" is assessed *by* the integration tests, creating circularity.

**How to avoid:** (a) Pair every happy-path integration test with an **adversarial variant** executed on the same scenario: random jitter (0-50ms), parallel second conversation between same two nodes, random future-drop injection, random message reordering in `MemoryTransport`. (b) Use `loom` (concurrency model checker for Rust) or `shuttle` to systematically explore interleavings on the FSM transition paths. (c) CI runs adversarial variants on every PR, not just happy paths.

**Warning signs:** Adversarial suite (PROJECT.md Active list includes one — good) is behind a feature flag or "slow" marker and only runs nightly. Integration tests green but production reports duplicate commits / state divergence.

**Phase to address:** **Phase 7** ships adversarial alongside happy-path, not after.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|---|---|---|---|
| Use `BTreeMap<String, Value>` for canonical serialization | 10-line implementation, passes ASCII tests | Silently broken on supplementary-plane keys (Pitfall 1); no way to detect without external vectors | Never — bake JCS sort correctly from day one |
| `serde_json::Value` as the internal wire type | Fast prototyping; flexible | Erases type information; invites Pitfall 6 silent acceptance; every field is `Option<Value>` at the use site | Only in tests/fixtures; never in the signed-payload data path |
| Skip `deny_unknown_fields` on protocol structs | Fewer test churns when spec evolves | Silent acceptance of typos and future-version fields; Pitfall 6 | Never on signed types; acceptable on debug/log types |
| `async_trait::async_trait` macro on the `Transport` trait | Works on older Rust; simple | Boxes every future (allocation per call); incompatible with native async-in-trait patterns | Only if targeting Rust <1.75; v1 targets latest stable so: no |
| Generate conformance vectors from own implementation | Easy to produce many vectors | Pitfall 10 — tests become regression tests, not conformance | Label as "regression vectors" in a separate directory; never claim them as conformance |
| Single `famp` crate instead of 12-crate workspace | Fast compile; simple mental model; fewer Pitfall 15 risks | Public API surface is coarse; harder to ship as separate versioned pieces later | Strongly recommended for Phase 0-3; split later at API boundaries |
| `unwrap()` in protocol code paths "for now" | Fast writing | Turns protocol violations into panics, kills calling process, adversary DoS vector | Only in tests and examples; never in `famp-*` crate library code — use `thiserror` from Phase 2 onward |
| Ignore key rotation until "someone asks for it" | No Phase 3 identity complexity | Pitfall 12 — retrofitting versioned cards is a spec-level change | Never — design versioned cards in Phase 1 spec fork even if rotation UX comes later |
| Skip `loom`/`shuttle` because `proptest` is "close enough" | Proptest covers data-shape entropy | Misses concurrency interleavings that cause Pitfalls 9 and 16 | Acceptable if `MemoryTransport` is single-threaded AND the library is documented as single-threaded; FAMP is neither |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|---|---|---|
| `ed25519-dalek` | Use `verify()` from tutorial snippet | Wrap and expose only `verify_strict()`; reject weak keys at ingress (Pitfall 4) |
| `serde` tagged enums | `#[serde(untagged)]` for polymorphism | Manual `Deserialize` impl with explicit class dispatch; always `deny_unknown_fields` (Pitfall 6) |
| `reqwest`/`hyper` (HTTP transport) | Let client add `Accept-Encoding: gzip` by default | Disable all content encodings — signed bytes must reach the parser byte-exact; any middleware that mutates body bytes breaks signatures |
| `tokio::time::timeout` | Wrap state-mutating sends directly | Timeout only observes completion; ownership of the send future stays with the conversation controller (Pitfall 9) |
| `tracing` / logging middleware | Log the parsed message | Log the raw canonical bytes *and* a signature fingerprint; parsed logs can't be used to reproduce signature failures |
| TLS (transport layer) | Rely solely on TLS for authentication | TLS authenticates the *server*, not the *agent*; every message must still be Ed25519-signed regardless of TLS (INV-10 in PROJECT.md) |
| Filesystem persistence | Write state with default `write_all` | Use atomic temp-file+rename for state checkpoints (per global CLAUDE.md rule); crash mid-write = corrupt state = spec-invariant violation on restart |
| `serde_json` parsing | Default parser | Configure to reject duplicate keys (not default in serde_json!); set recursion limit; reject on trailing data — strictness belongs at the parser boundary |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|---|---|---|---|
| Recanonicalize on every verify | Verify throughput <1k ops/s | Cache canonical bytes on the envelope after first canonicalization; invalidate on any body mutation (ideally: envelope is immutable after construction) | Verification hot path (>100 msg/s per node) |
| Clone entire message for each FSM transition | Latency climbs linearly with conversation size | Use `Arc` for the immutable parts (artifact lists, principal IDs); only clone the mutable state wrapper | Long conversations (>100 messages) or high-throughput agents |
| `BTreeMap<String, _>` as canonical output builder | O(n log n) sort per message, plus allocation churn | Build in a `Vec<(String, _)>` and sort once with JCS-aware comparator; reuse the buffer | When messages have >50 fields; matters at scale |
| `String` allocations for every field name during canonicalization | Allocator pressure | Use `&'static str` field names (compile-time known) and a `Cow<'static, str>` for dynamic keys | Tight loops over many small messages |
| Replay cache as unbounded `HashMap` | Memory growth over uptime | Bounded LRU or time-sliced window keyed to freshness rule (spec §causality) | Long-running nodes (>24h uptime) |
| Adversarial test suite as `#[test]` alongside unit tests | `cargo test` slow; developers stop running it | Separate `adversarial` test binary run by CI and by `cargo test --test adversarial`, not by default `cargo test` | Before Phase 7 ships |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---|---|---|
| Accepting any Ed25519 public key shape | Small-subgroup forgery (Pitfall 4) | `verify_strict` + weak-key rejection at all ingress points |
| Domain separation applied inconsistently | Cross-protocol signature reuse (sign a FAMP commit, attacker replays as a JWS assertion somewhere else) | Single, byte-defined, length-prefixed prefix documented in spec fork with hex example (Pitfall 5) |
| Signing over parsed representation instead of canonical bytes | Malleability: sender and receiver parse differently | Always sign `canonicalize(payload)` bytes; verifier recanonicalizes from received bytes and compares; never trust a pre-canonicalized field |
| Idempotency key as freeform string | Collision = INV-5 violation or replay | Require fixed-width random bytes (e.g., 128-bit) base64url; validate format on ingress |
| Trust store accepts Agent Cards without federation attestation | Circular trust (PROJECT.md Context item 4); any bad actor spins up a card | Phase 1 spec fork defines federation credential; Phase 3 identity rejects self-signed-only cards |
| `panic!` on protocol violation | Single malformed message crashes node → DoS | `thiserror` everywhere; `forbid(clippy::unwrap_used)` in library crates; `panic = "abort"` only in tests |
| Log full messages including signatures at INFO | Signature harvesting for replay on another network | Log at DEBUG; redact signatures from INFO/WARN logs; never log private keys at any level (add test) |
| No recipient binding in signed fields | Signed message can be replayed to a different recipient who also trusts the sender | Spec fork must include recipient principal ID in signed fields (PROJECT.md Context item 4 mentions this gap — explicit flag for Phase 1) |
| Clock skew tolerance too generous | Replay window widens | Phase 1 spec fork fixes tolerance (e.g., ±60s); implementation enforces strictly with monotonic clock for local operations |

---

## "Looks Done But Isn't" Checklist

- [ ] **Canonical JSON:** Often missing supplementary-plane sort test — verify a vector with emoji in a key exists and passes cross-implementation (Pitfall 1)
- [ ] **Canonical JSON:** Often missing the cyberphone 100M-sample number test — verify CI runs it, not just unit tests (Pitfall 2)
- [ ] **Canonical JSON:** Often normalized somewhere upstream — verify a combining-character round-trip through full HTTP stack (Pitfall 3)
- [ ] **Ed25519 verify:** Often uses non-strict `verify` — grep for `.verify(` and confirm none outside `verify_strict` wrapper (Pitfall 4)
- [ ] **Domain separation:** Often under-documented — verify spec fork has a hex-dump worked example and a conformance vector (Pitfall 5)
- [ ] **Serde structs:** Often missing `deny_unknown_fields` — CI grep check in place (Pitfall 6)
- [ ] **FSM state types:** Often have lifetimes — `cargo check` on `famp-fsm` shows zero lifetime parameters on state enums (Pitfall 7)
- [ ] **Transport trait:** Often sync-first with borrowed returns — confirm both `MemoryTransport` and `HttpTransport` compile against the same trait without `#[async_trait]` (Pitfall 8)
- [ ] **Cancellation:** Often untested — adversarial suite has a "drop future at every await" test (Pitfall 9)
- [ ] **Conformance vectors:** Often self-generated — `test-vectors/README.md` has a "source" column and at least one vector from an external tool (Pitfall 10)
- [ ] **Proptest generators:** Often produce canonical inputs — generator coverage metric asserts <10% already-canonical (Pitfall 11)
- [ ] **Agent Cards:** Often mutable — verify card resolver uses `(principal_id, card_version)` tuple (Pitfall 12)
- [ ] **Extensions:** Often unused in reference — reference ships with at least one critical extension exercised by conformance tests (Pitfall 13)
- [ ] **Spec version:** Often a loose string — verify single constant, CI check for drift, no literal version strings (Pitfall 14)
- [ ] **Build times:** Often not measured — Phase 0 exit criterion records warm rebuild time; CI alerts on regression (Pitfall 15)
- [ ] **Integration tests:** Often happy-path only — adversarial variants run on every PR, not nightly (Pitfall 16)
- [ ] **INV-5 single terminal state:** Often violated under concurrency — `loom` or `shuttle` test covers competing-commit interleavings
- [ ] **Replay cache:** Often unbounded — bounded by freshness rule, test proves eviction works
- [ ] **Key rotation:** Often UX-only — in-flight commits survive a rotation in adversarial test
- [ ] **HTTP transport:** Often trusts framework to leave body intact — middleware-audit test verifies signed bytes arrive byte-exact

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---|---|---|
| JCS sort bug (Pitfall 1) | LOW if caught in Phase 2; HIGH if caught in Phase 7 | Fix sort comparator; regenerate all conformance vectors from fixed impl *and* external reference; bump spec patch version if any vectors committed |
| Number serialization bug (Pitfall 2) | LOW in Phase 2; MEDIUM after | Replace number formatter with reference port; run 100M-sample test; regenerate affected vectors |
| Domain separation byte-format disagreement (Pitfall 5) | HIGH | Spec errata + version bump + coordinated cut-over with other implementations; no silent fix possible |
| Serde `deny_unknown_fields` missing (Pitfall 6) | LOW in any phase | Add attribute, fix test expectations, add CI grep check |
| FSM lifetime hell (Pitfall 7) | MEDIUM in Phase 4; HIGH in Phase 5+ | Rewrite state types as owned; cascade fix to all callers; typically 1-3 days for a beginner |
| Cancellation unsafe transport (Pitfall 9) | MEDIUM-HIGH | Refactor send path into spawned task + channel; add drop-injection test |
| Conformance vectors self-generated (Pitfall 10) | LOW in Phase 2; MEDIUM in Phase 7 | Relabel as regression vectors; generate new conformance vectors from external reference |
| Card rotation breaks in-flight (Pitfall 12) | HIGH if discovered post-release | Spec change: versioned cards; implementation change across `famp-identity` + verifier paths; migration for any stored state |
| Extension registry dead code (Pitfall 13) | LOW if caught in Phase 5; MEDIUM in Phase 7 | Add trivial reference extensions and conformance vectors; add coverage gate |
| Build time spiral (Pitfall 15) | LOW in Phase 0; MEDIUM in Phase 3+ | Merge crates; reducing 12→4 crates typically regains 60-80% of warm rebuild time |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---|---|---|
| 1. JCS UTF-16 sort | Phase 2 | Supplementary-plane test vector in initial commit; cross-impl diff before Phase 2 complete |
| 2. Number serialization | Phase 2 | Cyberphone 100M-sample test in CI |
| 3. Unicode normalization leak | Phase 2 (canonicalizer) + Phase 6 (HTTP) | Combining-char round-trip through full transport stack |
| 4. Non-strict Ed25519 verify | Phase 2 + Phase 3 | `verify_strict` wrapper is the only pub function; weak-key rejection at trust store ingress |
| 5. Domain separation byte ambiguity | Phase 1 (spec) + Phase 2 (impl) | Hex-dump worked example in spec fork; conformance vector #1 |
| 6. Serde silent acceptance | Phase 3 | Workspace lint / CI grep for `deny_unknown_fields` on all protocol structs |
| 7. FSM lifetime hell | Phase 0 (rule) + Phase 4 (code) | No lifetime parameters on any state enum in `famp-fsm` |
| 8. Transport trait shape | Phase 6 | Both MemoryTransport and HttpTransport implement without `#[async_trait]` |
| 9. Async cancellation | Phase 6 + Phase 7 | Drop-injection adversarial test across all `.await` points on commit path |
| 10. Self-generated conformance vectors | Phase 2 + Phase 7 | Vector manifest shows external-source provenance for every canonical vector |
| 11. Proptest generator coverage | Phase 2 + Phase 4 + Phase 7 | Generator metric asserts <10% already-canonical inputs |
| 12. Key rotation breaks in-flight | Phase 1 (spec) + Phase 3 (impl) + Phase 7 (test) | Rotation-mid-conversation adversarial test passes |
| 13. Extension registry dead code | Phase 5 + Phase 7 | Reference ships ≥1 critical and ≥1 non-critical extension exercised by conformance |
| 14. Spec-version drift | Phase 1 (constant) + ongoing | Single constant source; CI check no literal version strings |
| 15. Build time spiral | Phase 0 | Warm-rebuild benchmark; CI alert on regression |
| 16. Happy-path-only integration | Phase 7 | Adversarial variants on every PR, not nightly |

---

## Sources

- [RFC 8785: JSON Canonicalization Scheme (JCS)](https://www.rfc-editor.org/rfc/rfc8785) — normative spec for canonicalization (§3.2.2 number rules, §3.2.3 UTF-16 key sort)
- [cyberphone/json-canonicalization](https://github.com/cyberphone/json-canonicalization) — reference C/Java/JS/Go implementations, 100M-sample float test corpus
- [lattice-substrate/json-canon](https://github.com/lattice-substrate/json-canon) — Go JCS implementation, source of supplementary-plane sorting discussion
- [nlohmann/json Discussion #2612: RFC 8785 JSON Canonicalisation Scheme](https://github.com/nlohmann/json/discussions/2612) — Grisu2 vs ECMAScript number formatting divergence
- [CBOR/dCBOR Determinism chapter](https://cborbook.com/part_2/determinism.html) — cross-format determinism considerations
- [dalek-cryptography/curve25519-dalek issue #663](https://github.com/dalek-cryptography/curve25519-dalek/issues/663) — `verify_strict` cofactor equation discrepancy vs documentation
- [ed25519-dalek VerifyingKey docs](https://docs.rs/ed25519-dalek/latest/ed25519_dalek/struct.VerifyingKey.html) — `verify_strict`, `is_weak`, small-order rejection
- [SlowMist: Understanding the Principles and Scalability Issues of Ed25519](https://slowmist.medium.com/understanding-the-principles-and-scalability-issues-of-ed25519-6c2232f27290) — cofactor and malleability attack background
- [Serde Enum Representations](https://serde.rs/enum-representations.html) — tagged/untagged semantics and attribute reference
- [Tinkerer: Serde Errors When Deserializing Untagged Enums Are Bad](https://www.gustavwengel.dk/serde-untagged-enum-errors-are-bad) — documented poor error messages in untagged fallthrough
- [serde-rs/serde issue #2447](https://github.com/serde-rs/serde/issues/2447) — untagged deserialization error opacity
- [serde-rs/serde issue #1560](https://github.com/serde-rs/serde/issues/1560) — empty untagged variants serialize unintuitively
- [SWICG ActivityPub HTTP Signature Report](https://swicg.github.io/activitypub-http-signature/) — real-world canonical signing interop failures across Fediverse implementations
- [ActivityPub w3c/activitypub issue #203 — LDS public key URI](https://github.com/w3c/activitypub/issues/203) — key distribution and versioning problems that inform Pitfall 12
- `.planning/PROJECT.md` (this repo) — prior-review findings that this document extends, not restates
- Global CLAUDE.md rules (atomic writes, never-downgrade-errors, adversarial review workflow) — applied throughout

---
*Pitfalls research for: FAMP v0.5 Rust reference implementation*
*Researched: 2026-04-12*
