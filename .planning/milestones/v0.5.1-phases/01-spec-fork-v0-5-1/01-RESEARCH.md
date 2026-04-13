# Phase 1: Spec Fork v0.5.1 — Research

**Researched:** 2026-04-13
**Domain:** Protocol specification authorship (docs-only; no Rust code)
**Confidence:** HIGH on RFC citations, decision pressure-tests, and change inventory; MEDIUM on body-schema field completeness (some fields are extrapolated from v0.5 prose — planner must confirm against §11.2 and §12.4 during drafting)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (verbatim D-01..D-29 — do NOT re-open)

**File location & structure**
- **D-01:** Spec lives at repo root as `FAMP-v0.5.1-spec.md`; v0.5 retained unchanged.
- **D-02:** Top-level section numbers preserved; additions are lettered sub-sections (e.g., §7.1a) to avoid renumbering.
- **D-03:** A **Changelog from v0.5** section at the end (before appendices); each entry shape `v0.5.1-Δnn — <section touched> — <finding id/reviewer> — <resolution summary>`. No inline diff markers.
- **D-04:** Spec-version constant block near the top: `FAMP_SPEC_VERSION = "0.5.1"`.

**Canonical JSON (SPEC-02)**
- **D-05:** Canonical JSON section is rewritten to say, verbatim: "Canonical JSON for FAMP is **RFC 8785 JSON Canonicalization Scheme (JCS)**." No paraphrasing; cite RFC 8785 §3.2.3 (UTF-16 sort) and §3.2.2.3 (ECMAScript number formatting) as normative.
- **D-06:** Duplicate JSON object keys are rejected at parse.
- **D-07:** Two normative worked examples (ASCII mixed-case + supplementary-plane emoji U+1F600) with exact byte sequences in hex.
- **D-08:** `arbitrary_precision` and `preserve_order` serde features MUST NOT be used by conforming implementations.

**Signatures & domain separation (SPEC-03/04/19)**
- **D-09:** Domain separation prefix is fixed ASCII: **`FAMP-sig-v1\0`** (12 bytes = 11 ASCII + 1 NUL). `sig = Ed25519.sign(sk, prefix || canonical_json_bytes)`.
- **D-10:** Byte-level worked example with fixed test keypair committed inline (hex canonical JSON, hex prefix, hex signing input, hex signature).
- **D-11:** `to` field is part of signed canonical JSON; explicit recipient anti-replay text.
- **D-12:** Raw 32-byte pubkeys, raw 64-byte sigs, unpadded base64url (RFC 4648 §5). Decoders reject padded input and standard (non-url) alphabet. `verify_strict` semantics normative.

**Agent Card & identity (SPEC-05/06)**
- **D-13:** Agent Card adds required `federation_credential` field; card is signed by federation-scoped credential (trust list published), not self-signed.
- **D-14:** `card_version` (integer, monotonic) and `min_compatible_version` (integer). In-flight commits bound to card N remain valid through resolution even if card rotates to N+1, provided new card's `min_compatible_version ≤ N`.

**Numeric defaults & idempotency (SPEC-07/08)**
- **D-15:** Clock skew ±60s, validity window 300s (RECOMMENDED); federation cap ±300s / 1800s.
- **D-16:** Idempotency key 128-bit random, unpadded base64url 22 chars, scope `(sender_principal, recipient_principal)`; dedup tuple `(id, idempotency_key, content_hash)`.

**State-machine hole resolutions (SPEC-09..16) — D-17..D-24**
- **D-17 (SPEC-09):** Ack-disposition ≠ terminal crystallization; terminal crystallizes only on `deliver`-with-terminal-status, `control:cancels`, or transfer-timeout reversion.
- **D-18 (SPEC-10):** Whitelist of FSM-inspected envelope/body fields: `{class, relation, body.interim, body.scope_subset, body.target, body.terminal_status}`. §7.3 "no body inspection" claim retracted.
- **D-19 (SPEC-11):** Transfer-timeout race: delegate's commit wins iff its `ts` precedes the timeout deadline; otherwise reversion wins. Loser gets `conflict:transfer_timeout`.
- **D-20 (SPEC-12):** EXPIRED vs in-flight `deliver`: `deliver` with `ts` strictly before EXPIRED deadline MUST be accepted and crystallize COMPLETED/FAILED; at/after deadline rejected `stale:expired`.
- **D-21 (SPEC-13):** Committer-side conditional lapse (`control:cancel_if_not_started`) wins over delivery-wins rule when both fire in same tick.
- **D-22 (SPEC-14):** Intermediate `COMMITTED_PENDING_RESOLUTION` state for competing commits; tiebreak = lexicographically smaller `id` (UUIDv7 time-ordered). Loser → REJECTED with `conflict:competing_instance`. INV-5 holds (public observable state still single terminal).
- **D-23 (SPEC-15):** `propose` that supersedes a prior `propose` continues the round counter (does NOT reset).
- **D-24 (SPEC-16):** Capability snapshot is taken at commit time and bound to committing card's `card_version`. Card rotations do not retroactively invalidate the commit.

**Body schemas (SPEC-17)**
- **D-25:** Body schemas defined inline, field-per-line tables, 5 schemas: `commit`, `propose`, `deliver`, `control`, `delegate`. `ack`/`announce`/`describe`/`request` retain v0.5 definitions (noted in changelog).
- **D-26:** `additionalProperties: false` semantics (translated as `deny_unknown_fields` at decode). Unknown fields rejected; extensions live under an explicit `extensions` map.
- **D-27:** Field shapes: `deliver` has `interim`, `terminal_status?`, artifact list; `commit` has `scope_subset`; `control` has `target: ControlTarget` + enumerated action; `delegate` has `form: assist|subtask|transfer` + ceiling fields; `propose` mirrors `commit` minus commitment binding.

**Artifact IDs & hashes (SPEC-18)**
- **D-28:** `sha256:<hex>` over canonical JSON of artifact body; lowercase hex; 64 chars. `sha<N>:` reserved.

**Spec-version constant (SPEC-20)**
- **D-29:** `FAMP_SPEC_VERSION = "0.5.1"` exact string; mismatched version rejected `unsupported_version`.

### Claude's Discretion
- Exact section ordering within each SPEC-xx rewrite (content preserved, changelog cites edit).
- Prose style and example variable names in worked examples.
- Whether body schemas render as tables, code blocks, or hybrid.
- Minor editorial fixes (typos, broken cross-refs) logged as `editorial` changelog entries.

### Deferred Ideas (OUT OF SCOPE)
- Multi-party commitment profiles.
- Cross-federation delegation.
- Streaming (token-by-token) deliver (`interim: true` is sufficient for v1).
- Additional hash algorithms beyond SHA-256.
- Real trust registry (interface boundary only).
- Python/TS binding commentary beyond "reserved" line.
- Level 1-only release profile.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SPEC-01 | `FAMP-v0.5.1-spec.md` forked with changelog citing review findings | §6 Change Inventory enumerates every Δnn; planner uses as task skeleton |
| SPEC-02 | Canonical JSON = RFC 8785 JCS (explicit, not paraphrase) | §2 Normative Citations — exact §3.2.3 + §3.2.2.3 text + two worked examples |
| SPEC-03 | Signature domain-separation byte format with hex-dump worked example | §2 (RFC 8032) + §5 Worked Ed25519 Example Skeleton |
| SPEC-04 | Signature covers `to` field (recipient binding) | §6 Δ09; explicit normative sentence in §5 skeleton |
| SPEC-05 | Agent Card federation credential (resolves circular self-signature) | §6 Δ11 — adds required `federation_credential`, removes `signature` self-sig |
| SPEC-06 | Card versioning for key rotation / in-flight commits | §3 hole D-14 pressure-test (HOLDS) + §6 Δ12 |
| SPEC-07 | Clock skew tolerance + validity window concrete defaults | §6 Δ13 — ±60s / 300s, fed cap ±300s / 1800s |
| SPEC-08 | Idempotency key format (128-bit random, scope) | §6 Δ14 |
| SPEC-09 | §9.6 terminal precedence — ack-disposition vs crystallization | §3 D-17 pressure-test (HOLDS) + §6 Δ15 |
| SPEC-10 | §7.3 "no body inspection" reconciled — whitelist | §3 D-18 pressure-test (HOLDS; note spec §7.1 already references body fields it then denies) + §6 Δ16 |
| SPEC-11 | Transfer-timeout race tiebreak | §3 D-19 pressure-test (ADJUST — see below) + §6 Δ17 |
| SPEC-12 | EXPIRED vs in-flight deliver | §3 D-20 pressure-test (HOLDS) + §6 Δ18 |
| SPEC-13 | Conditional-lapse precedence over delivery-wins | §3 D-21 pressure-test (HOLDS) + §6 Δ19 |
| SPEC-14 | Competing-instance commit intermediate state | §3 D-22 pressure-test (HOLDS) + §6 Δ20 |
| SPEC-15 | Supersession round counting | §3 D-23 pressure-test (HOLDS) + §6 Δ21 |
| SPEC-16 | Capability snapshot vs card-version drift | §3 D-24 pressure-test (HOLDS) + §6 Δ22 |
| SPEC-17 | Body schemas for commit, propose, deliver, control, delegate | §4 Body Schemas Draft + §6 Δ23 |
| SPEC-18 | Artifact ID scheme `sha256:<hex>` | §6 Δ24 (D-28) |
| SPEC-19 | Ed25519 encoding locked | §2 RFC 8032 + RFC 4648 citations, §6 Δ10 |
| SPEC-20 | Spec-version constant | §6 Δ01 (top-of-doc constant + envelope `famp` field bump) |
</phase_requirements>

## 1. Summary

Phase 1 is a 100% writing task. The planner's job is to sequence edits against `FAMP-v0.5-spec.md` into `FAMP-v0.5.1-spec.md` such that every SPEC-xx requirement lands in a specific lettered sub-section, every reviewer finding in PITFALLS.md maps to a Δnn changelog entry, and every normative claim about canonical JSON / Ed25519 / base64url / UUIDv7 is a **verbatim RFC citation** rather than a paraphrase.

Three load-bearing facts for the planner:

1. **Section numbering is frozen (D-02).** New content lands as §4a, §7.1a, §7.3a, §9.6a, §11.2a, §12.3a, §12.4a, §13.1a, §15.1a, §18a. No section gets renumbered. This massively simplifies downstream phase references.
2. **The worked Ed25519 example is not optional decoration** — it is conformance vector #1 (reused byte-for-byte by Phase 8). Its structure (§5 of this doc) must be byte-exact even if the actual hex bytes are computed during the plan step.
3. **One CONTEXT.md decision (D-19 transfer-timeout tiebreak) has a subtle ambiguity** that the planner must resolve before drafting §12.3a — see §3 SPEC-11. All other holes hold up to pressure-test.

The change inventory in §6 is 24 numbered deltas, which is also the minimum plan-task count for this phase (one Δ per task, or small groups of related Δ per task).

## 2. Normative Citations (must appear verbatim in the fork)

Source priority: RFCs cited here are stable, published, and directly referenced by CONTEXT.md decisions. All quotes are paraphrase-free targets — the spec fork SHOULD include a pull-quote block for each.

### 2.1 RFC 8785 — JSON Canonicalization Scheme (JCS)

URL: <https://www.rfc-editor.org/rfc/rfc8785>

**Cite §3.2.3 (Sorting of Object Properties)** — mandates UTF-16 code-unit order:

> "[...] JSON object members MUST be sorted based on the UTF-16 code units of their names."

And further (per PITFALLS Pitfall 1): supplementary-plane characters (U+10000+) sort by surrogate pair (D800–DFFF range), **not** by codepoint and **not** by UTF-8 bytes. The spec fork MUST note: "An implementation that compares keys as UTF-8 byte strings or as Rust `str::cmp` output is non-conformant."

**Cite §3.2.2 + §3.2.2.3 (Number Serialization)** — mandates ECMAScript `Number.prototype.toString`:

> "[...] JSON numbers MUST be represented as specified by Section 7.1.12.1 of ECMAScript (ECMA-262), which is equivalent to the 'Number.prototype.toString' method."

Required spec-fork language: (a) `NaN`, `±Infinity` rejected at serializer boundary with typed error; (b) integers > 2^53 represented as JSON strings per RFC 8785 §6 guidance; (c) `-0` renders as `0`; (d) reference formatter sourced from cyberphone test corpus (not `ryu` default).

**Cite §3.1 — duplicate keys rejected at parse** (per D-06). RFC 8259 §4 ("The names within an object SHOULD be unique") is SHOULD; RFC 8785 §3.1 tightens to input requirement. Spec fork MUST state "implementations that silently dedupe are non-conformant."

**Appendix B** — test vectors are **not reproduced** in the spec fork (licensing + length), but the fork MUST cite Appendix B as the authoritative conformance source and state that Phase 2's `famp-canonical` crate will wire them as a CI gate.

### 2.2 RFC 8032 — Ed25519 (EdDSA)

URL: <https://www.rfc-editor.org/rfc/rfc8032>

**Cite §5.1.6 (Sign)** for signing algorithm and **§5.1.7 (Verify)** for the verification equation. Required spec-fork language:

- Signature length: **exactly 64 bytes**, raw (R || S concatenation per §5.1.6).
- Public key length: **exactly 32 bytes**, raw (compressed Edwards point encoding per §5.1.2).
- Verification MUST be the **strict** form: reject `R` that decodes to a small-order point; reject non-canonical `S` (the upper bound `S < L` check, per §5.1.7 step 2). Phrasing for the fork: "Verification MUST match the `verify_strict` semantics of `ed25519-dalek 2.2` (rejects small-subgroup `A`, rejects non-canonical `S`). Cofactor-tolerant verification (the raw §5.1.7 equation `[8]SB = [8]R + [8]kA`) is non-conformant for FAMP."
- Weak public keys (8-torsion) MUST be rejected at trust-list ingress (not only at verify). Cite RustCrypto `ed25519-dalek::VerifyingKey::is_weak`.

**RFC 8032 §7.1 test vectors** — named in spec fork as Phase 2 CI gate (same pattern as RFC 8785 Appendix B).

### 2.3 RFC 9562 — UUIDs (including UUIDv7)

URL: <https://www.rfc-editor.org/rfc/rfc9562>

**Cite §5.7 (UUID Version 7)** for envelope `id` field. Required spec-fork language:

- `id` is a **UUIDv7** per RFC 9562 §5.7 (48-bit Unix timestamp ms + 4-bit version + 12-bit rand_a + 2-bit variant + 62-bit rand_b).
- Canonical string form: lowercase hyphenated (RFC 9562 §4) — `xxxxxxxx-xxxx-7xxx-yxxx-xxxxxxxxxxxx` where `y ∈ {8,9,a,b}`.
- **Load-bearing cross-validation** (per CAUS-07 in REQUIREMENTS.md): the first 48 bits of the UUIDv7 MUST be within the envelope's `ts` ± validity_window. This is the D-22 tiebreak foundation — UUIDv7 time-ordering is the disambiguator.
- Implementations MUST NOT use UUIDv4 for `id`. v4 loses the time-order property that D-22 depends on.

### 2.4 RFC 4648 — Base64url (unpadded)

URL: <https://www.rfc-editor.org/rfc/rfc4648>

**Cite §5 ("Base 64 Encoding with URL and Filename Safe Alphabet")** and **§3.2 ("Padding of Encoded Data")**. Required spec-fork language:

- Alphabet: `A-Z a-z 0-9 - _` exactly (RFC 4648 §5 Table 2). The `+`/`/` alphabet of RFC 4648 §4 is **rejected**.
- Padding: **no `=` characters**. Encoded length for 32-byte pubkey = 43 chars; for 64-byte sig = 86 chars; for 16-byte idempotency key = 22 chars.
- Decoders MUST reject: (a) any `=`; (b) `+` or `/`; (c) whitespace; (d) trailing garbage.
- Cite RustCrypto `base64::engine::general_purpose::URL_SAFE_NO_PAD` as the reference engine.

### 2.5 RFC 2119 / RFC 8174 — Normative language

URL: <https://www.rfc-editor.org/rfc/rfc8174> (updates RFC 2119 with lowercase disambiguation)

The spec fork MUST include a "Conventions" box near the top:

> "The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, NOT RECOMMENDED, MAY, and OPTIONAL in this document are to be interpreted as described in BCP 14 [RFC 2119] [RFC 8174] when, and only when, they appear in all capitals, as shown here."

v0.5 uses these keywords inconsistently (some lowercase, some RFC-capital). The fork MUST normalize to RFC-capital for all normative statements. This is an editorial Δ that should be bundled into one changelog entry, not a per-section fix.

### 2.6 RFC 8259 — JSON base reference

URL: <https://www.rfc-editor.org/rfc/rfc8259>

Cited only as the base reference that RFC 8785 extends. One sentence in §4 (Canonical JSON): "FAMP JSON messages are JSON documents per RFC 8259, canonicalized per RFC 8785 JCS."

## 3. State-Machine Hole Resolutions — Pressure-Tested

For each of SPEC-09..SPEC-16, pressure-test tabulates: v0.5 §ref, the PITFALLS finding that flagged it, CONTEXT.md resolution, verdict (HOLDS / ADJUST / BROKEN), and the final resolution text the planner should put in the fork.

### SPEC-09 — §9.6 Terminal Precedence (D-17)

- **v0.5 §ref:** §9.6 "Terminal precedence rule" (lines 560–575). Current text crystallizes terminal "at the point of semantic acknowledgment, not at the point of sending" — but conflates ack-disposition (which is any value in `{accepted, duplicate, stale, malformed, orphaned, refused}`) with "accepted" as the crystallization trigger.
- **PITFALLS flag:** v0.5 review finding: an `ack` with disposition `refused` or `stale` does not and MUST NOT crystallize a terminal state, but the §9.6 prose is ambiguous. Also INV-5 hole when ack never arrives.
- **CONTEXT resolution (D-17):** Separate (a) causality-metadata update (happens on every ack) from (b) terminal-state crystallization (happens only on: `deliver` with terminal_status, `control:cancels`, transfer-timeout reversion).
- **Verdict:** **HOLDS.** D-17 is cleaner than v0.5 because it decouples crystallization from the ack loop entirely. Side effect the planner must make explicit: the "ack with accepted disposition" language in §9.6 is **deleted**, not softened. The terminal state now crystallizes when the terminal-producing message is **validly processed by the FSM**, without requiring a downstream ack.
- **Final resolution text (§9.6a):** "A terminal state crystallizes when the FSM validly processes one of: (a) a `deliver` message carrying envelope-level `terminal_status ∈ {completed, failed}`; (b) a `control` message with relation `cancels` against a task in COMMITTED state; (c) a transfer-timeout reversion event (Section 12.3a) against a task in transitional state. Semantic acknowledgment (`ack`) updates causality metadata and delivery disposition, but does NOT crystallize terminal state. An `ack` with disposition `refused` or `stale` on a terminal-producing message is valid and does not reverse crystallization; the terminal state has already been decided by the FSM at processing time." [Cites PITFALLS §9.6 finding.]

### SPEC-10 — §7.3 "No Body Inspection" (D-18)

- **v0.5 §ref:** §7.3 (lines 382–385) — claims "Every state transition is determined by the tuple `(class, relation, terminal_status, current_state)` — all envelope-level fields. No body inspection is required for any state transition." This is **false** in the same spec: §9.5 mentions `interim: true` (a body field) as the no-transition signal, §10.4 mentions `scope_subset` (a body field) as the partial-acceptance signal, and §12.3 mentions `form: transfer` (a body field) as the ownership-transition trigger.
- **PITFALLS flag:** Reviewer-identified spec-internal contradiction. v0.5 §7.1 promotes `terminal_status` to envelope level specifically to make this claim true — but it only promoted one field out of four that the FSM actually inspects.
- **CONTEXT resolution (D-18):** Retract the "no body inspection" claim. Publish a whitelist of FSM-inspected body fields: `{body.interim, body.scope_subset, body.target, body.terminal_status}` — note `terminal_status` is already envelope-level per v0.5 §7.1, so the **body** whitelist is really `{interim, scope_subset, target}` plus the envelope `terminal_status`.
- **Verdict:** **HOLDS** — but D-18 lists `body.terminal_status` which is a slight mis-statement (v0.5 already moved it to envelope). Planner should correct to: "FSM inspects these envelope fields: `class`, `relation`, `terminal_status`, plus these body fields: `interim`, `scope_subset`, `target`. All other body content is opaque to the FSM. Extensions MUST NOT reuse these field names."
- **Final resolution text (§7.3a):** New subsection replacing the bold contradiction. "**FSM-observable field whitelist.** The conversation and task state machines are driven by a closed set of fields: (envelope) `class`, `relation`, `terminal_status`; (body) `interim`, `scope_subset`, `target`. No other fields participate in state transitions. Extensions MUST NOT define body fields named `interim`, `scope_subset`, `target`, or `terminal_status`. The v0.5 claim that 'no body inspection is required' is retracted; the whitelist is the normative replacement."

### SPEC-11 — Transfer-Timeout Race (D-19)

- **v0.5 §ref:** §12.3 (lines 765–773) — defines 5-minute `transfer_deadline` and auto-reversion, but does not specify what happens when the delegate's `commit` and the transferring agent's reversion are both in-flight.
- **PITFALLS flag:** Reviewer-flagged race: "transitional state" (§12.3 item 4) has no tiebreak rule.
- **CONTEXT resolution (D-19):** Delegate's commit wins iff its `ts` precedes the timeout deadline; else reversion wins. Loser → `conflict:transfer_timeout`.
- **Verdict:** **ADJUST — ambiguity in "precedes."** `ts` is the sender's clock. The receiver's clock is different. "Precedes the timeout deadline" by whose clock? If by delegate's `ts`, a delegate with a skewed-fast clock can always win the race by pre-dating. If by transferring agent's receipt time, the delegate has no way to know a priori whether it will win.
  - **Recommended adjustment:** Tiebreak is evaluated by the **transferring agent** (the one that will observe both the delegate's `commit` and its own timeout timer). Criterion: `delegate_commit.ts <= transfer_message.ts + transfer_deadline - clock_skew_tolerance` (i.e., `≤ ts_0 + 5min − 60s`). This gives a 60-second guard band within which the delegate is guaranteed to lose (sacrificing some legitimate late commits for unambiguous tiebreak). The delegate can self-check this bound before sending and, if inside the guard band, send a `control:cancels` instead.
  - Planner MUST reflect this in §12.3a and in the `control` body schema (`target: transfer_commit_race` target value).
- **Final resolution text (§12.3a):** "**Transfer-timeout tiebreak.** Let `ts_transfer` be the `ts` of the `delegate` message with `form: transfer`. Let `ts_deadline = ts_transfer + transfer_deadline` (default 5 minutes). Let `δ` be the federation clock skew tolerance (default 60 seconds). A `commit` from the delegate is considered on-time iff `delegate_commit.ts ≤ ts_deadline − δ`. On-time commits crystallize ownership-transfer and close the original commitment. Commits with `ts > ts_deadline − δ` are rejected with `conflict:transfer_timeout`; the transferring agent's auto-reversion wins and the original commitment reactivates (Section 12.3 item 5). The `δ` guard band ensures that no commit whose on-time status depends on clock-skew interpretation is accepted."

### SPEC-12 — EXPIRED vs In-Flight Deliver (D-20)

- **v0.5 §ref:** §9.5 state diagram (line 553) — "`Any non-terminal state ──(timeout)──► EXPIRED`" — with no tiebreak rule for a `deliver` message whose `ts` is before the EXPIRED threshold but which arrives after.
- **PITFALLS flag:** Reviewer noted this as an INV-5 hole identical in shape to SPEC-09 but on a different axis.
- **CONTEXT resolution (D-20):** `deliver.ts < EXPIRED_deadline` → accept and crystallize COMPLETED/FAILED; else reject `stale:expired`.
- **Verdict:** **HOLDS.** Same clock-skew question as SPEC-11, but less severe because the attacker-benefit is zero (why would a sender lie about `ts` to have their delivery accepted when they could just send it earlier?). Planner should still apply the same `δ` guard band for consistency: `deliver.ts ≤ EXPIRED_deadline − δ` accepts.
- **Final resolution text (§9.5a):** "**EXPIRED vs deliver tiebreak.** Let `ts_expire` be the task's computed EXPIRED deadline. Let `δ` be the federation clock skew tolerance. A `deliver` message is accepted and crystallizes its `terminal_status` iff `deliver.ts ≤ ts_expire − δ`. A `deliver` with `deliver.ts > ts_expire − δ` is rejected with `stale:expired` and the task transitions to EXPIRED. This rule applies identically to interim deliveries that happen to race expiration: they are accepted if on-time, rejected if off-time."

### SPEC-13 — Conditional-Lapse Precedence (D-21)

- **v0.5 §ref:** §11.4 (lines 712–725) — defines conditional commitments and says "If a condition becomes false, the commitment lapses." No ordering rule vs delivery-wins (§9.6 default).
- **PITFALLS flag:** Reviewer noted commit-side safety vs recipient-side finality conflict.
- **CONTEXT resolution (D-21):** Committer-side conditional lapse wins over delivery-wins when both fire in same tick.
- **Verdict:** **HOLDS.** The rationale (D-21: "committer's right to withdraw an unaccepted conditional is a safety property") is sound: if a condition has genuinely failed (e.g., access token expired, downstream refused delegation), the delivered work may be based on false premises. Safety > finality here.
- **Final resolution text (§9.6b, adjacent to §9.6a):** "**Conditional-lapse precedence.** A `control` message with relation `cancels` and disposition `condition_failed` (Section 11.4) sent by the committing agent takes precedence over a concurrent `deliver` with terminal status. The default 'delivery wins' rule (Section 9.6) is overridden when both messages are in-flight and the `control:condition_failed` is valid (i.e., the condition was machine-evaluable and has provably become false per Section 11.4). The task crystallizes to CANCELLED with cause `condition_failed`. The counter-party receiving the late `deliver` responds with `ack` disposition `orphaned` (the commitment no longer exists)."

### SPEC-14 — Competing-Instance Commits / INV-5 Hole (D-22)

- **v0.5 §ref:** §11.5 (lines 728–735) — says "resolved by: explicit supersedes OR federation policy OR task owner decision" and "NOT by message arrival order." This leaves a deterministic default unspecified; without one, the reference implementation has no testable behavior.
- **PITFALLS flag:** Reviewer INV-5 hole — two instances of the same principal can both reach COMMITTED without explicit supersession, violating single-terminal-state if each then delivers independently.
- **CONTEXT resolution (D-22):** Intermediate `COMMITTED_PENDING_RESOLUTION` pseudo-state; lexicographically smaller `id` wins (UUIDv7 time-ordered tiebreak); loser → REJECTED with `conflict:competing_instance`.
- **Verdict:** **HOLDS.** The UUIDv7 tiebreak is deterministic, requires no clock synchronization (the lex order is a pure function of bytes), and is testable. INV-5 is preserved because `COMMITTED_PENDING_RESOLUTION` is transient and internal — it is not a public terminal state, and the resolution is forced to occur within one transition step.
  - **One planner note:** D-22 says "public observable state is still a single terminal." Make this explicit by adding `COMMITTED_PENDING_RESOLUTION` to §9.5 as an **internal** state, with a note that it is not directly observable via protocol messages. The FSM table in §7.3a must include rows for `(COMMITTED_PENDING_RESOLUTION, commit)` and `(COMMITTED_PENDING_RESOLUTION, <any>)` → force-resolution.
  - **Zero-UUIDv7-collision assumption:** Two `commit` messages with the same `id` are a bug (UUIDv7 collision probability is negligible but non-zero). If they occur, the FSM MUST reject the second with `conflict:competing_instance` regardless of lex order. Planner should add this as a catch-all sentence.
- **Final resolution text (§11.5a):** "**Competing-instance resolution.** When two instances of the same principal issue `commit` messages for the same task within the same freshness window and neither explicitly supersedes the other, the task enters the internal state `COMMITTED_PENDING_RESOLUTION`. The FSM MUST deterministically resolve this state within one transition by selecting the commit with the **lexicographically smaller envelope `id`** (which, for UUIDv7, corresponds to the earlier-created message). The winning commit crystallizes the task into COMMITTED. The losing commit is rejected with `conflict:competing_instance`; the losing instance MUST be notified via `ack` with disposition `refused`. If two commits have identical `id` values (UUIDv7 collision), both are rejected as `conflict:competing_instance` and the task remains REQUESTED. The `COMMITTED_PENDING_RESOLUTION` state is internal; it MUST NOT appear in public protocol messages or provenance records. INV-5 holds because the public observable state is always one of the five terminal states plus REQUESTED/COMMITTED."

### SPEC-15 — Supersession Round Counting (D-23)

- **v0.5 §ref:** §10.3 Rule 4 (line 644) — "Each task MUST have a negotiation round limit. The default is 20 rounds (total messages with `proposes_against` relation within the task)." No guidance on whether a `supersedes` + re-`propose` resets the counter.
- **PITFALLS flag:** Reviewer-flagged round-limit circumvention via `supersedes` loops.
- **CONTEXT resolution (D-23):** Round counter continues from superseded round's number; does not reset.
- **Verdict:** **HOLDS.** The alternative (reset on supersession) is obviously exploitable. Planner MUST be explicit about the counting rule: "round counter is `count(proposes_against messages in task) + count(superseded proposes_against messages in task)`" — superseded messages still count toward the limit.
- **Final resolution text (§10.3a, amending Rule 4):** "**Rule 4a: Supersession does not reset round counting.** The negotiation round counter for a task is `count(messages with relation proposes_against in the task's conversation graph, including messages that have been subsequently superseded)`. A `supersedes` relation voids the prior message for commitment purposes but does NOT remove it from round accounting. This prevents a buggy or malicious agent from circumventing INV-11 via repeated supersession-then-re-propose cycles. When the counter reaches `max_negotiation_rounds` (default 20), the task transitions to EXPIRED as specified in INV-11."

### SPEC-16 — Capability Snapshot vs Card-Version Drift (D-24)

- **v0.5 §ref:** §11.2 (lines 693–697) — "**Capability snapshot binding.** A commitment is made against the agent's capability posture as advertised at the time of commitment." And §6.3 — "In-flight conversations that were initiated under a prior card version continue under the terms of the proposal/commit that referenced that version." These two statements are **almost but not exactly consistent**: §11.2 binds to "time of commitment," §6.3 binds to "prior card version." If an agent proposes under card v=3, counter-proposes under v=3, rotates to v=4, commits under v=4 — what does the commit bind to?
- **PITFALLS flag:** Reviewer-flagged ambiguity.
- **CONTEXT resolution (D-24):** Snapshot taken at commit time and bound to committing card's `card_version`. Subsequent rotations do not retroactively invalidate.
- **Verdict:** **HOLDS.** D-24 definitively picks "time of commit, not time of proposal" as the binding moment. This is consistent with D-14 (in-flight commits survive rotation iff `min_compatible_version ≤ N`). Planner should add a cross-reference between §6.3 and §11.2 so the two sections cannot drift apart again.
- **Final resolution text (§11.2a):** "**Capability snapshot binding — card version clarification.** The capability snapshot is captured at the moment the committing agent sends the `commit` message, not at the moment the underlying proposal was issued. The snapshot is bound to the committing card's `card_version` at commit time. If the committing agent's card rotates after commit, Section 6.3's continuity rule applies: the commit remains valid provided the new card's `min_compatible_version ≤ card_version_at_commit`. If the counter-party's card rotated between proposal and commit, the committing agent MAY choose to re-validate the counter-party's latest card before committing, but is not required to; a commit sent after a counter-party rotation is valid against the counter-party's card version current at commit receipt time, subject to the same `min_compatible_version` rule."

## 4. Body Schemas Draft (SPEC-17)

Fields derived from v0.5 §7.1 (envelope shape), §10.2 (proposal must-contain), §11.2 (commit binds), §12.4 (delegate message structure), §12.5 (delegation ceiling), §9.5 (state-machine inputs), §15.1 (error categories, relevant for `control.action`). Field tables are the **planner's draft** — the plan step will flesh out constraint notes (regexes, ranges, enum values) against PITFALLS Pitfall 6 (`deny_unknown_fields`) and Pitfall 2 (no `u64`/`i64` over 2^53).

All five body schemas share the rule: `additionalProperties: false`; unknown fields rejected at decode; extensions MUST live under the envelope-level `extensions` array (v0.5 §7.1), never inside the body.

### 4.1 `propose` body

| Field | JSON type | Req/Opt | Constraint notes |
|---|---|---|---|
| `scope` | object | REQUIRED | Opaque to FSM; domain-specific work description. MUST be present (INV-4 precursor). |
| `bounds` | object | REQUIRED | MUST include ≥2 keys from {`deadline`, `budget`, `hop_limit`, `policy_domain`, `authority_scope`, `max_artifact_size`, `confidence_floor`, `recursion_depth`} per §9.3 / INV-4. |
| `bounds.deadline` | string (RFC 3339) | OPTIONAL | Absolute time. |
| `bounds.budget` | object | OPTIONAL | `{amount: string, unit: string}` — amount is a string to avoid 2^53 precision loss (PITFALLS P2). |
| `bounds.hop_limit` | integer | OPTIONAL | ≥ 0 ≤ 2^53. |
| `bounds.policy_domain` | string | OPTIONAL | Opaque identifier. |
| `bounds.authority_scope` | string (enum) | OPTIONAL | One of §5.3 levels. |
| `bounds.max_artifact_size` | integer | OPTIONAL | Bytes, ≤ 2^53. |
| `bounds.confidence_floor` | number | OPTIONAL | 0.0–1.0. |
| `bounds.recursion_depth` | integer | OPTIONAL | ≥ 0 ≤ 255. |
| `terms` | object | OPTIONAL | §10.2 SHOULD-contain; opaque. |
| `delegation_permissions` | object | OPTIONAL | `{form_allowed: [assist|subtask|transfer], ceiling: {...}}`; if absent, delegation forbidden per INV-3. |
| `artifact_expectations` | object | OPTIONAL | Opaque; input/output format hints. |
| `policy_references` | array of string | OPTIONAL | Policy IDs. |
| `natural_language_summary` | string | SHOULD | Human/LLM-readable, no length cap; canonicalized per JCS (no Unicode normalization per PITFALLS P3). |
| `modifications` | array of string | OPTIONAL | Courtesy field per §10.3 Rule 3 — MUST NOT be used as normative source of truth. |
| `conditions` | array of object | OPTIONAL | For conditional proposals; §11.4. Each: `{expression: string, evaluator: string, deadline: string}`. |

### 4.2 `commit` body

Mirrors `propose` minus negotiation affordances, plus commitment-specific fields.

| Field | JSON type | Req/Opt | Constraint notes |
|---|---|---|---|
| `scope` | object | REQUIRED | MUST be present. MAY be narrower than the referenced proposal's scope iff `scope_subset = true`. |
| `scope_subset` | boolean | OPTIONAL (default false) | Partial acceptance flag per §10.4. If `true`, `scope` MUST be interpretable as a proper subset of the referenced proposal's scope. FSM inspects this field (§7.3a whitelist). |
| `bounds` | object | REQUIRED | Same shape as `propose.bounds`. MUST be within referenced proposal's bounds. |
| `accepted_policies` | array of string | REQUIRED | Policy IDs the committer accepts. |
| `delegation_permissions` | object | OPTIONAL | Frozen at commit; D-24 binding. |
| `reporting_obligations` | object | OPTIONAL | `{progress_frequency: string, interim_required: bool, final_report_format: string}`. |
| `terminal_condition` | object | REQUIRED | Machine-evaluable description of what constitutes completion; may be opaque. |
| `capability_snapshot` | object | REQUIRED | D-24: frozen snapshot of committer's capability posture at commit time. `{card_version: integer, capabilities: [...]}`. |
| `conditions` | array of object | OPTIONAL | §11.4 conditional commitment. Each: `{expression: string, evaluator: string, deadline: string}`. |
| `natural_language_summary` | string | SHOULD | Same rules as proposal. |

### 4.3 `deliver` body

| Field | JSON type | Req/Opt | Constraint notes |
|---|---|---|---|
| `interim` | boolean | REQUIRED | FSM-inspected (§7.3a). `false` means terminal delivery (requires envelope-level `terminal_status`); `true` means progress update (MUST NOT carry `terminal_status`). |
| `artifacts` | array of object | OPTIONAL | Each: `{id: "sha256:<hex>", media_type: string, size: integer}`. Per D-28. |
| `result` | object | OPTIONAL | Domain payload; opaque to FSM. |
| `usage_metrics` | object | OPTIONAL | `{tokens_used: integer, compute_ms: integer, cost: {amount: string, unit: string}}`. Numbers bounded by 2^53; large values as strings. |
| `error_detail` | object | OPTIONAL | REQUIRED iff envelope `terminal_status = failed`. `{category: ErrorCategory, message: string, diagnostic: object?}`. |
| `provenance` | object | REQUIRED on terminal | §14.2: `{originating_task, commitment_lineage, delegation_lineage?, artifact_lineage?, policy_context}`. Canonicalized per RFC 8785 (§14.3). On interim deliveries, provenance MAY be omitted. |
| `natural_language_summary` | string | SHOULD | Human/LLM readable. |

### 4.4 `control` body

| Field | JSON type | Req/Opt | Constraint notes |
|---|---|---|---|
| `target` | string (enum) | REQUIRED | FSM-inspected (§7.3a). One of: `task`, `conversation`, `commitment`, `delegation`, `proposal`, `transfer_commit_race`. Defines what the control action operates on. |
| `action` | string (enum) | REQUIRED | One of: `cancel`, `supersede`, `close`, `cancel_if_not_started`, `revert_transfer`. Combined with envelope `relation` per §7.3 table. |
| `disposition` | string (enum) | OPTIONAL | One of §15.1 categories where relevant: `condition_failed`, `capacity_exceeded`, `policy_blocked`, `unauthorized`, `conflict`, or a human reason. For conditional lapse (§11.4), MUST be `condition_failed`. |
| `reason` | string | SHOULD | Human-readable justification. |
| `affected_ids` | array of string | OPTIONAL | Extra targets (e.g., list of commitment IDs closed by a conversation close). |

### 4.5 `delegate` body

| Field | JSON type | Req/Opt | Constraint notes |
|---|---|---|---|
| `form` | string (enum) | REQUIRED | One of `assist`, `subtask`, `transfer`. Per §12.3. Not FSM-inspected directly (form governs downstream obligations, not state transitions of the parent task — except `transfer` which triggers §12.3a). |
| `commitment_ref` | string | REQUIRED | Commitment ID (not message ID) under which delegation is authorized. |
| `downstream` | string | REQUIRED | Principal identity of delegate (format `agent:<authority>/<name>`, per §5.1). |
| `scope` | object | REQUIRED | What portion is delegated. |
| `bounds` | object | REQUIRED | Same shape as `commit.bounds`. MUST be within parent commitment bounds. |
| `delegation_ceiling` | object | OPTIONAL | `{max_hops: integer, max_fanout: integer?, allowed_delegates: [string]?, forbidden_delegates: [string]?, policy_inheritance: boolean}`. Per §12.5. |
| `transfer_deadline` | string (RFC 3339) | REQUIRED iff `form=transfer` | Default 5 minutes from envelope `ts`. Per §12.3 item 5 and §12.3a tiebreak. |
| `natural_language_summary` | string | SHOULD | Human-readable. |

## 5. Worked Ed25519 Example Skeleton

This is conformance vector #1 (reused byte-for-byte in Phase 8). Planner MUST instantiate every `<HEX>` placeholder during task execution; the **structure** below is the normative output.

### 5.1 Test keypair (deterministic)

Use **RFC 8032 §7.1 Test 1** keypair so third parties can verify against the RFC directly:

```
secret_key (32 bytes):
  9d61b19deffd5a60ba844af492ec2cc4 4449c5697b326919703bac031cae7f60
public_key (32 bytes, raw):
  d75a980182b10ab7d54bfed3c964073a 0ee172f3daa62325af021a68f707511a
```

Spec fork MUST include a boxed warning: "This key pair is from RFC 8032 §7.1 Test 1 and is published worldwide. It MUST NOT be used for any production signing. Its sole purpose is to produce byte-identical conformance output across independent FAMP implementations."

### 5.2 Minimal envelope (pre-canonical)

```json
{
  "famp": "0.5.1",
  "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b",
  "from": "agent:example.test/alice",
  "to": "agent:example.test/bob",
  "scope": "standalone",
  "class": "ack",
  "causality": { "rel": "acknowledges", "ref": "01890a3b-1111-7222-8333-444444444444" },
  "authority": "advisory",
  "ts": "2026-04-13T00:00:00Z",
  "body": { "disposition": "accepted" }
}
```

(Note: `signature` field is excluded from the signing input, per §7.1.)

### 5.3 Canonical JSON (JCS output)

Planner computes the JCS canonicalization. For this example, the expected output is a single line (no whitespace), keys in UTF-16 sort order. Fields in this envelope are all ASCII, so sort order is predictable:

```
`authority` < `body` < `causality` < `class` < `famp` < `from` < `id` < `scope` < `to` < `ts`
```

Note that nested objects (`causality`, `body`) also have their keys sorted. Planner MUST compute and commit the exact bytes. Placeholder:

```
canonical_json_bytes (hex) = <HEX_A>
canonical_json_bytes (length) = <LEN_A> bytes
```

### 5.4 Domain-separation prefix (D-09)

```
prefix_bytes (12 bytes):
  46 41 4d 50 2d 73 69 67 2d 76 31 00
  (= b"FAMP-sig-v1\x00")
```

Byte-by-byte breakdown: `F`=0x46, `A`=0x41, `M`=0x4d, `P`=0x50, `-`=0x2d, `s`=0x73, `i`=0x69, `g`=0x67, `-`=0x2d, `v`=0x76, `1`=0x31, NUL=0x00.

### 5.5 Signing input (concatenation)

```
signing_input_bytes = prefix_bytes || canonical_json_bytes
signing_input_bytes (hex) = 46414d502d7369672d7631 00 <HEX_A>
signing_input_bytes (length) = 12 + <LEN_A> bytes
```

Explicit: the prefix is **prepended** to the canonical JSON. It is NOT appended, not interleaved, not included as a JSON field. Verification applies the identical prefix in the identical position.

### 5.6 Signature

```
signature (64 bytes, raw, Ed25519 over signing_input_bytes with secret_key):
  R (32 bytes) || S (32 bytes) = <HEX_B>
signature (unpadded base64url, 86 chars) = <B64_B>
```

### 5.7 Re-embedded envelope (on the wire)

The `signature` field in the envelope that gets sent over the wire contains `<B64_B>`. The envelope sent is:

```json
{
  "famp": "0.5.1",
  "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b",
  "from": "agent:example.test/alice",
  "to": "agent:example.test/bob",
  "scope": "standalone",
  "class": "ack",
  "causality": { "rel": "acknowledges", "ref": "01890a3b-1111-7222-8333-444444444444" },
  "authority": "advisory",
  "ts": "2026-04-13T00:00:00Z",
  "body": { "disposition": "accepted" },
  "signature": "<B64_B>"
}
```

Verification procedure (normative, spec fork §7.1b):

1. Parse the envelope (reject on duplicate keys, unknown fields, or bad base64url per §7.1).
2. Extract `signature` field value.
3. Construct a copy of the envelope **without** the `signature` field.
4. Canonicalize that copy per §4 (RFC 8785 JCS).
5. Construct `signing_input = b"FAMP-sig-v1\x00" || canonical_bytes`.
6. Look up sender's public key via `(from_principal, card_version_referenced_in_envelope_if_any)`.
7. Verify `signature` against `signing_input` using `verify_strict` (RFC 8032 §5.1.7 strict form — reject small-order `A`, reject non-canonical `S`).
8. On any failure, reject message with error category `unauthorized`.

## 6. Change Inventory

Numbered `v0.5.1-Δnn` deltas. Each row is a candidate plan task (or group). Column: `SPEC-xx` is the requirement satisfied; `Finding` cites PITFALLS or a v0.5 reviewer-flagged contradiction; `v0.5 §` is the source section touched; `Summary` is a one-line change description.

| Δ | SPEC-xx | v0.5 § | Finding | Summary |
|---|---|---|---|---|
| Δ01 | SPEC-20 | top-of-doc | PITFALLS P14 (spec drift) | Add "Spec-version constant" block: `FAMP_SPEC_VERSION = "0.5.1"`; mandate envelope `famp` field = `"0.5.1"` exact; reject mismatched as `unsupported_version` |
| Δ02 | SPEC-01 | front matter | N/A | Bump title to "FAMP v0.5.1" with subtitle "Reviewer-audited revision of v0.5"; add "Conventions" box citing RFC 2119/8174 |
| Δ03 | SPEC-01 (editorial) | global | N/A | Normalize all normative keywords to RFC-capital (MUST/SHOULD/MAY); single editorial Δ for the entire document |
| Δ04 | SPEC-02 | §4 (new) + global | PITFALLS P1, P2, P3 | Insert new §4a "Canonical JSON" immediately before §4 "Core Invariants" citing RFC 8785 §3.2.3 (UTF-16 sort) and §3.2.2.3 (ECMAScript numbers) verbatim; add normative sentences on duplicate-key rejection (§3.1) and no-Unicode-normalization (per P3) |
| Δ05 | SPEC-02 | §4a | D-08 | Add paragraph: serde `arbitrary_precision` and `preserve_order` features are non-conformant for FAMP; integers > 2^53 MUST serialize as strings; NaN/±Infinity rejected at serializer boundary |
| Δ06 | SPEC-02 | §4a | D-07 + P1 | Add worked example A (ASCII mixed-case key sort) with exact hex byte output |
| Δ07 | SPEC-02 | §4a | D-07 + P1 | Add worked example B (supplementary-plane U+1F600 emoji key) with exact hex byte output proving UTF-16 surrogate-pair sort |
| Δ08 | SPEC-03, SPEC-19 | §7.1 | PITFALLS P5 | Insert §7.1a "Domain separation prefix" with normative `prefix = b"FAMP-sig-v1\x00"` (12 bytes); specify `sign(sk, prefix || canonical_bytes)`; reference §5 worked example |
| Δ09 | SPEC-04 | §7.1 | v0.5 reviewer finding: cross-recipient replay | Amend §7.1 signature paragraph: "The `to` field is part of the canonical JSON signed by the sender. A signed envelope addressed to agent A MUST NOT be replayable to agent B." |
| Δ10 | SPEC-19 | §7.1 | PITFALLS P4 | Insert §7.1b "Ed25519 encoding" citing RFC 8032 §5.1.2/5.1.6/5.1.7; raw 32-byte pub / 64-byte sig; unpadded base64url per RFC 4648 §5; verify_strict normative; decoder MUST reject padded / non-url alphabet |
| Δ11 | SPEC-03 + SPEC-04 | §7.1c (new) | PITFALLS P5 | Insert §7.1c "Worked signature example" inline with the bytes from §5 of RESEARCH.md (test keypair, canonical JSON hex, prefix hex, signing input hex, signature hex) |
| Δ12 | SPEC-05 | §6.1 | v0.5 reviewer finding: circular self-signature | Replace `signature` field in Agent Card with `federation_credential` (required) + `federation_signature`; card is signed by federation-scoped credential whose pubkey is in the federation trust list; state trust-list distribution is federation-specific |
| Δ13 | SPEC-06 | §6.3 | D-14 | Pin card versioning rules: `card_version` (int, monotonic) and `min_compatible_version` (int); in-flight commits bound to card N remain valid iff new card's `min_compatible ≤ N`; cross-link to §11.2a (D-24) |
| Δ14 | SPEC-07 | §13.1 | D-15 + v0.5 §13.1 ("RECOMMENDED ±30 seconds") | Amend §13.1: clock skew RECOMMENDED ±60s (up from v0.5's ±30); validity window RECOMMENDED 300s; federation cap ±300s / 1800s (MUST NOT exceed); v0.5 explicit bump |
| Δ15 | SPEC-08 | §13.2 | D-16 | Amend §13.2: idempotency key is 128-bit cryptographic random, unpadded base64url 22 chars; scope = (sender_principal, recipient_principal); dedup tuple `(id, idempotency_key, content_hash)`; format enforced at ingress |
| Δ16 | SPEC-09 | §9.6 | PITFALLS: §9.6 terminal precedence hole | Insert §9.6a (see §3 SPEC-09 resolution text); delete the "semantic acknowledgment crystallizes" language and replace with FSM-processing-time crystallization |
| Δ17 | SPEC-10 | §7.3 | PITFALLS: §7.3 "no body inspection" contradiction | Insert §7.3a "FSM-observable field whitelist" (see §3 SPEC-10 resolution); retract v0.5's "no body inspection" claim explicitly |
| Δ18 | SPEC-11 | §12.3 | PITFALLS: transfer-timeout race | Insert §12.3a "Transfer-timeout tiebreak" with the δ-guard-band rule (see §3 SPEC-11 resolution); this is the one CONTEXT.md decision the planner must pressure-test further |
| Δ19 | SPEC-12 | §9.5 | PITFALLS: EXPIRED vs deliver race | Insert §9.5a "EXPIRED vs deliver tiebreak" with δ-guard-band rule |
| Δ20 | SPEC-13 | §9.6, §11.4 | PITFALLS: conditional-lapse precedence | Insert §9.6b "Conditional-lapse precedence" — committer-side `control:condition_failed` overrides delivery-wins default |
| Δ21 | SPEC-14 | §11.5 | PITFALLS: INV-5 hole, competing instances | Insert §11.5a "Competing-instance resolution" with `COMMITTED_PENDING_RESOLUTION` internal state + lex-UUIDv7 tiebreak |
| Δ22 | SPEC-15 | §10.3 | PITFALLS: supersession round circumvention | Insert §10.3a "Supersession does not reset round counting" |
| Δ23 | SPEC-16 | §11.2 | PITFALLS: capability snapshot / card-version drift | Insert §11.2a "Capability snapshot binding — card version clarification" cross-linking to §6.3 |
| Δ24 | SPEC-17 | §8.5-8.9, §10.2, §11.2, §12.4 | PITFALLS P6 (serde deny_unknown_fields discipline) | Insert §8a "Body schemas" containing the five tables from §4 of RESEARCH.md; each schema: additionalProperties=false; extensions belong to envelope `extensions` array, never body |
| Δ25 | SPEC-18 | §3.6, §14 | D-28 | Pin artifact IDs to `sha256:<hex>` lowercase 64 chars; hash is SHA-256 over the canonical JSON of the artifact body (not raw input); reserve `sha<N>:` prefix for future algorithms |
| Δ26 | SPEC-20 | §19 | D-29 | Amend §19 conformance level text to require implementations to emit `FAMP_SPEC_VERSION = "0.5.1"` exactly; reject mismatches as `unsupported_version` |
| Δ27 | (editorial) | §23 | D-deferred items | Update §23 "Open Questions": mark Q1 (multi-party), Q2 (streaming), Q3 (cross-federation) as "deferred to post-v0.5.1 per CONTEXT.md"; Q6 (deliver body structure) is RESOLVED by Δ24 |
| Δ28 | SPEC-01 | bottom of doc | — | Add "Changelog from v0.5" section listing Δ01..Δ27 in order, each row citing the finding + the SPEC-xx requirement |

Optional planner-discretion editorial deltas (log only if encountered):
- Typo corrections (free-form `editorial:typo` entries)
- Broken cross-references (v0.5 §X.Y references that no longer land on the correct section after lettered sub-sections are added)
- Consistent use of `MUST`/`SHOULD` instead of `must`/`should` (bundled into Δ03)

**Δ count:** 28 structural deltas. Expect plan to have 5–7 tasks, each bundling 3–6 related Δ (e.g., "Canonical JSON section" task = Δ04+Δ05+Δ06+Δ07; "Signature section" task = Δ08+Δ09+Δ10+Δ11).

## 7. Validation Architecture

Phase 1 is docs-only. There is no `cargo test`, no `just test`, no framework to configure. But "docs-only" does not mean "unverifiable" — the spec fork is a load-bearing artifact and must be validatable by grep + text-level checks.

### Test Framework
| Property | Value |
|---|---|
| Framework | `ripgrep` + `just spec-lint` shell target (new) |
| Config file | `Justfile` (existing — add `spec-lint` recipe) |
| Quick run command | `just spec-lint` |
| Full suite command | `just spec-lint` (single suite for a doc phase) |
| Phase gate command | `just spec-lint && just ci` |

### Phase Requirements → Test Map

Each grep-verifiable check anchors on a unique string that the spec-fork author must include. The check fails if the anchor is absent. This is the docs equivalent of a unit test.

| Req ID | Behavior | Test type | Automated command | File exists? |
|---|---|---|---|---|
| SPEC-01 | Changelog section exists with ≥25 `v0.5.1-Δ` entries | grep count | `rg -c '^v0\.5\.1-Δ' FAMP-v0.5.1-spec.md` ≥ 25 | ❌ Wave 0 |
| SPEC-02 | Canonical JSON section cites RFC 8785 verbatim | grep presence | `rg 'RFC 8785.*JSON Canonicalization Scheme' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-02 | Section cites §3.2.3 and §3.2.2.3 | grep presence | `rg 'RFC 8785.*3\.2\.3' && rg 'RFC 8785.*3\.2\.2\.3'` | ❌ Wave 0 |
| SPEC-03 | Domain-separation prefix literal present | grep presence | `rg 'FAMP-sig-v1\\x00\|46 41 4d 50 2d 73 69 67 2d 76 31 00' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-03 | Hex dump block for worked signature example | grep presence | `rg 'signing_input_bytes.*hex' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-04 | Explicit `to` field signing clause | grep presence | `rg 'to.*field.*signed\|recipient anti-replay' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-05 | Agent Card has `federation_credential` field | grep presence | `rg 'federation_credential' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-06 | `card_version` + `min_compatible_version` both present | grep presence | `rg 'card_version' && rg 'min_compatible_version'` | ❌ Wave 0 |
| SPEC-07 | Clock-skew value ±60s and window 300s both present | grep presence | `rg '±60\|60 seconds' && rg '300 seconds\|5 minutes'` | ❌ Wave 0 |
| SPEC-08 | Idempotency key spec: 128-bit + 22 chars + sender/recipient scope | grep presence | `rg '128-bit' && rg '22 char' && rg 'sender.*recipient.*scope'` | ❌ Wave 0 |
| SPEC-09..SPEC-16 | 8 state-machine hole resolutions each have a dedicated sub-section | grep count | `rg -c '^#### (§7\.3a\|§9\.5a\|§9\.6a\|§9\.6b\|§10\.3a\|§11\.2a\|§11\.5a\|§12\.3a)'` = 8 | ❌ Wave 0 |
| SPEC-17 | Body schemas for all 5 message classes | grep presence | `rg 'body schema.*commit' && ...propose && deliver && control && delegate` | ❌ Wave 0 |
| SPEC-18 | Artifact ID scheme `sha256:<hex>` | grep presence | `rg 'sha256:<hex>\|sha256:[0-9a-f]' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |
| SPEC-19 | RFC 8032 and RFC 4648 both cited | grep presence | `rg 'RFC 8032' && rg 'RFC 4648.*§5\|base64url.*unpadded'` | ❌ Wave 0 |
| SPEC-20 | `FAMP_SPEC_VERSION = "0.5.1"` exact string | grep presence | `rg 'FAMP_SPEC_VERSION = "0\.5\.1"' FAMP-v0.5.1-spec.md` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `just spec-lint` (fast — single ripgrep pass over one file)
- **Per wave merge:** `just spec-lint` (same — no heavier suite exists for a docs phase)
- **Phase gate:** `just spec-lint && just ci` (where `just ci` is the existing Phase 0 gate which still needs to pass since `just fmt`/`just lint` must not regress)

### Wave 0 Gaps

None of the checks above exist yet. Wave 0 of the plan must create:

- [ ] `Justfile` — add `spec-lint` recipe wrapping the ripgrep checks above (shell script or inline, roughly 20 lines of `rg` invocations with `|| exit 1`)
- [ ] `scripts/spec-lint.sh` (optional) — externalized shell script if `Justfile` inline grows unwieldy
- [ ] Empty `FAMP-v0.5.1-spec.md` file scaffold at repo root (so `spec-lint` has a file to grep against from task 1 onward)

No test framework install needed — `ripgrep` is already a dependency of `just` in the developer environment (and installable via `cargo install ripgrep` if not; CI `taiki-e/install-action` can add it if missing).

**Note on Phase 8 reuse:** The §5 worked Ed25519 example is planned as conformance vector #1 in Phase 8. When the planner computes the concrete bytes during plan execution, those bytes should be emitted as a `famp-conformance/fixtures/vector-01-signature.json` file too (or at minimum, the plan should include a task to extract them into that file during Phase 8). This is not a Phase 1 deliverable but should be flagged in the plan's "future cross-references" so it isn't lost.

## 8. Gaps for Planner to Resolve

CONTEXT.md locked a lot, but not everything. These are gaps the planner must decide during `/gsd:plan-phase 1`:

1. **D-19 transfer-timeout tiebreak — clock-skew guard band.** CONTEXT.md says "delegate's commit wins iff its `ts` precedes the timeout deadline" but does not specify whose clock. This research recommends adding a δ=60s guard band (§3 SPEC-11 ADJUST verdict) using the transferring agent's clock. **Planner MUST either accept this adjustment or document an alternative.**
2. **Agent Card `federation_signature` field name.** D-13 says `federation_credential` is the **credential reference**, but the spec also needs a **signature field** that contains the actual Ed25519 signature made by the federation over the card body. Name is not in CONTEXT.md — suggest `federation_signature` (explicit) over reusing `signature`.
3. **Body size limits per message class.** v0.5 §18 sets 1MB transport limit but does not distinguish per-class. No CONTEXT decision. Suggest: punt to Phase 7 transport binding; spec fork says "transport-level limit applies per §18; body schemas do not impose additional size caps except via `bounds.max_artifact_size`."
4. **§19 Conformance Levels wording.** CONTEXT D-29 requires version emission, but §19 currently describes L1/L2/L3 without tying them to the version constant. Planner decides whether to amend §19 (probably yes — one-line edit).
5. **Worked example `<HEX>` values.** Planner must actually compute these during plan execution (either by hand-computing JCS + invoking `openssl`/Python Ed25519, or by asking a task to run a one-off Rust script). The structure in §5 is locked; the bytes are not.
6. **`COMMITTED_PENDING_RESOLUTION` naming.** D-22 uses this name. Planner may choose a shorter name (`CPR`?) — trivial editorial call.
7. **§13.1 stale-commit handling.** v0.5 says `commit` messages MUST be rejected stale (line 841). D-20 now has a δ guard band for `deliver`. Should the same guard band apply to `commit` for consistency? Probably yes — planner decides.
8. **Spec-fork signing authority.** The spec fork itself (`FAMP-v0.5.1-spec.md`) is a document, not a FAMP message. But should it be signed (hash committed in the repo's `CHANGELOG.md` or `SECURITY.md`)? Not a protocol requirement; just a docs-hygiene question.

## 9. Open Risks

Risks that could bite even with a perfect plan:

1. **Worked-example bytes diverge from actual Phase 2 `famp-canonical` output.** If the planner hand-computes JCS bytes for the §5 worked example, and Phase 2's `serde_jcs` output differs (PITFALLS P1/P2), the spec becomes wrong rather than the code. Mitigation: compute the bytes using an **external reference** (cyberphone JS implementation, Python `jcs` library) and treat Rust as the implementation under test, not the source of truth. This matches PITFALLS P10.
2. **RFC 8785 §3.2.2.3 corner-case floats in the worked example.** If the example envelope contains any `number` literal (it doesn't, currently), the bytes can diverge between implementations. Keeping the worked example **integer-free** sidesteps this risk. Recommended: the example envelope uses only strings and booleans, no numbers. (Verify: the example in §5.2 has no numbers.)
3. **`ack` disposition enum drift.** v0.5 §7.4 defines 6 dispositions. The conditional-lapse resolution (Δ20) adds an `orphaned` semantic to acks. This is already in §7.4. No drift — but planner should verify.
4. **Δ27 §23 open-questions update.** Marking Q1/Q2/Q3 as "deferred" is an editorial statement about protocol governance that the downstream (Phases 2–8) will cite. If anyone later argues "but v0.5 had Q2 open," the fork commits are the provenance. Make Δ27 bulletproof.
5. **RFC 2119 normalization (Δ03) touching too many lines.** If v0.5 has hundreds of lowercase "must" / "should" occurrences, Δ03 could be a huge diff that drowns out the semantic changes. Mitigation: planner may elect to **not** do a global normalization pass in Phase 1 and instead only capitalize keywords in new/amended sections. Document the scope in the Δ03 entry.
6. **Agent Card JSON shape breaking existing tooling.** Δ12 replaces `signature` with `federation_credential` + `federation_signature`. Any v0.5 example in the spec (including the §6.1 JSON block) must be updated, and any cross-reference in §5.4 (federation verification) needs review. Planner should grep v0.5 for `signature` in the Agent Card context.
7. **`COMMITTED_PENDING_RESOLUTION` leaking into provenance.** D-22 says it's internal. But if an implementer serializes FSM state into provenance naively, the internal state will appear in signed provenance records. Spec fork must say explicitly: "MUST NOT appear in protocol messages or provenance records." This is in the §3 SPEC-14 resolution text.
8. **PROJECT.md vs FAMP-v0.5-spec.md drift.** The repo has both a `.planning/PROJECT.md` (project vision) and the spec file. If the planner finds a discrepancy (e.g., PROJECT says bilateral-only but v0.5 §23 Q1 discusses multi-party as an open question), Δ27 resolves it. Planner should grep PROJECT.md during fork authoring to catch any other drift.

## RESEARCH COMPLETE
