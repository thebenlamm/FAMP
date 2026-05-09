# v1.0 Trigger Unweld: Two Ship Gates, No Clock

**Status:** Approved (Ben, 2026-05-09)
**Supersedes:** v0.9 close note "v1.0 readiness trigger named" (single fused trigger + 4-week clock)
**Authority over:** `.planning/MILESTONES.md` v0.9 close blurb, MEMORY entry `project_v10_trigger.md`, ROADMAP language about v1.0 gating, vector-pack scheduling

## Summary

The v0.9 close named one trigger that was actually two welded primitives: cross-host stress signal AND conformance signal. They have different activation conditions and different right answers for "when does this ship." Unweld them into two independent ship gates. Retire the 4-week clock — it was anti-mummification insurance for the fused trigger; once unwelded, neither gate has a mummification risk that a clock solves.

## What was

From the v0.9 milestone close (2026-05-04):

> "v1.0 trigger named: Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. 4-week clock starts at v0.9.0; if untriggered, federation framing is reconsidered. Conformance vector pack ships at the same trigger."

This bundled three things into one event:

1. **Cross-host stress** — proves the federation premise survives a real use case.
2. **Conformance signal** — proves byte-exact canonicalization, signature scheme, and on-wire envelope format work when exercised by code the original author didn't write.
3. **Anti-mummification clock** — guarantees federation framing gets reconsidered if no real demand surfaces.

These were welded because at v0.9 close there was one expected event (Sofer cross-host) that would satisfy all three. Ben's symmetric-cross-machines use case (laptop ↔ home dev server, two equal agents) cleanly satisfies (1) but not (2): naming collisions, bidirectional reachability, and mutual peer trust all show up; spec-ambiguity catches do not.

## Decision

Replace the single trigger with two independent ship gates and retire the clock.

### Gate A — Gateway gate

**Activation:** Ben exchanges a signed envelope between symmetric agents on two of his machines (laptop ↔ home dev server). Two equal-role agents, bidirectional traffic, mutual peer trust exercised in real use, sustained for ~2 weeks of working dogfood.

**Unlocks:**
- `famp-gateway` crate construction (Layer 2, wraps `famp-transport-http` + `famp-keyring`)
- Reactivation of `crates/famp/tests/_deferred_v1/` (~27 tests)
- Tag `v1.0.0` (no RC — see Rationale)

**Does not unlock:** the conformance vector pack.

### Gate B — Conformance gate

**Activation:** A 2nd implementer (Sofer or anyone else) commits to interop and exercises the wire format against their own code lineage.

**Unlocks:**
- The conformance vector pack (deferred from v0.5.1 wrap; see `.planning/WRAP-V0-5-1-PLAN.md`)
- Whichever release tag is current at that moment (could be `v1.0.x`, could be `v1.1.0` — semver decides on shape of changes)

**Does not unlock:** the gateway. Gateway is already shipped via Gate A.

### Retired

- **The 4-week clock.** It was anti-mummification insurance for the fused trigger. With gates separated, Gate A's user is Ben himself (no mummification risk; he'll build it when he needs it), and Gate B is inherently demand-driven (no one to mummify for — the vector pack is worthless until a 2nd implementer exists, then load-bearing the moment they do).
- **The "single named event" framing.** "Sofer or named equivalent" survives, but only as Gate B's activation condition. Gate A is activated by Ben's own use case.
- **The `-rc` tag for the gateway.** RC is for releases gated on external signoff. A one-operator release isn't. Semver handles "ships with rough edges" via `v1.0.0` → `v1.0.1`.

## Rationale

### Why unweld

The fused trigger was solving two problems that have different right answers:

| Problem | Right activation | Right artifact |
|---|---|---|
| Cross-host stress | When the operator needs it | Working `famp-gateway` |
| Conformance signal | When 2nd implementer exists | Vector pack |

Welding them forces one to wait on the other. Specifically: the vector pack would have shipped at the moment cross-host worked — into an empty room, since both sides would still be running Ben's code. The conformance pack's whole reason to exist is interop with someone else's lineage. Shipping it when both sides are the same code is theatre.

Unwelding lets the gateway ship when it's needed and the vector pack ship when it's earned.

### Why no clock

The 4-week clock was guarding against the local-case-black-hole risk: v0.9 is so good that federation never gets built. With Gate A unwelded:

- Gate A's risk profile: Ben needs the gateway to do real work. If he doesn't need it, federation framing should be reconsidered — that's the original signal, and it doesn't need a clock to fire. The presence or absence of Ben's need *is* the signal.
- Gate B's risk profile: a 2nd implementer either commits or doesn't. A clock can't manufacture interop demand; it can only force premature shipping.

Both gates are event-driven. Time-driven framing was an artifact of fusing them.

### Why symmetric-cross-machines counts for Gate A but not B

Symmetric collab on two of Ben's machines stresses:

- Naming collisions (`dk` on laptop vs `dk` on server)
- Bidirectional reachability (NAT, routing, peer registry)
- Mutual peer trust (both sides need each other's keys/certs)
- Cross-OS canonicalization (macOS ↔ Linux line endings, Unicode, float edges in JCS)
- TLS/rustls behavior across heterogeneous builds
- Ed25519 verification across compiler/target combinations
- Clock skew under real network latency

What it does *not* stress: independent re-derivation of the spec from text. Both ends share Ben's mental model. If the spec sentence "INV-10 requires domain prefix `FAMP-sig-v1\0`" is ambiguous, Ben will read it the same way on both sides and the test will pass. A 2nd implementer reading the same sentence and disagreeing is the only way that ambiguity surfaces.

This is the ~60% / ~40% split. Gate A banks the transport, OS-heterogeneity, and crypto-stack-heterogeneity work. Gate B banks the spec-clarity work.

### Why this is honest

The retrospect sentence the v1.0 announcement should make readers nod at:

> "v1.0 shipped when Ben needed cross-host. Conformance shipped when someone else needed interop. We stopped pretending those were the same event."

## Implementation implications

### Pre-build spike (this week)

Before writing `famp-gateway`, dogfood the `v0.8.1-federation-preserved` tag on Ben's laptop ↔ home dev server. v0.8.1 has HTTPS daemons + TOFU pinning already working. Run two equal agents across machines, exchange envelopes, capture the friction.

**Rationale:** real friction signal beats guessing. Cost: one afternoon. Output: a friction log that informs `famp-gateway` design choices (peer registry shape, naming convention at the gateway boundary, error semantics for unreachable peers, mailbox semantics across hosts when a name exists on multiple machines).

**Do not** invest in fixing v0.8.1 friction in the v0.8 codebase. Capture, then write `famp-gateway` against the friction log.

### Spec-text test discipline (when building Gate A)

Wire-format tests in `famp-gateway` assert against:

- The literal byte sequence of the domain prefix (`FAMP-sig-v1\0`)
- JCS canonicalization vectors derived from the spec text, not from current implementation output
- INV-10 invariants stated in protocol terms ("every envelope is signed under the domain prefix"), not in implementation terms ("the `signed_envelope` field is non-None")

**Rationale:** banks spec-validation work that a 2nd implementer can later inherit. When Gate B fires, the existing tests should already serve as conformance vectors against the spec — the vector pack becomes a packaging job, not a write-from-scratch job.

This discipline is what bridges Gate A's ~60% conformance signal toward the ~40% Gate B closes.

### Documentation churn

Files that need updating when this spec is committed:

- `.planning/MILESTONES.md` — v0.9 close blurb's "v1.0 trigger named" paragraph rewritten as two-gate framing pointing here.
- `.planning/STATE.md` — any v1.0-gated dormant seeds re-tagged Gate A or Gate B (currently "2 v1.0-gated dormant seeds" per v0.9 deferred items).
- MEMORY entry `project_v10_trigger.md` — superseded by an entry pointing here.
- `ARCHITECTURE.md` — the v1.0 section's "trigger" reference updated; layered model itself is unchanged.
- `.planning/WRAP-V0-5-1-PLAN.md` DEFERRED banner — vector-pack ship condition rephrased as "Gate B: 2nd implementer commits to interop."
- `.planning/seeds/SEED-001-serde-jcs-conformance-gate.md` — if it references the welded trigger, update to Gate B.

### What does *not* change

- Layer 0 / Layer 1 / Layer 2 architecture model (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` stay transport-neutral; `famp-bus` stays Layer 1; `famp-gateway` is still the Layer 2 crate that wraps `famp-transport-http` + `famp-keyring`).
- INV-10 (every wire envelope signed over canonical JSON under `FAMP-sig-v1\0`).
- The 5-state task FSM.
- The MCP tool surface (gains transparent remote routing in v1.0; signatures unchanged).
- The "named equivalent" language — survives as Gate B's activation condition.

## Open items

None gating. Items deferred to plan-writing:

- Gateway-boundary naming convention (`name@host-fingerprint`? `name@hostname`? something else?). Decision deferred to the v1.0 gateway plan, informed by the pre-build spike.
- Peer registry shape under v1.0 (carry-forward of v0.8 `peers.toml`? new format? gateway-only and hidden from local bus?). Decision deferred to gateway plan.
- Mailbox semantics when a name exists on multiple hosts (one mailbox per `name@host`? unified across hosts via gateway routing?). Decision deferred.

## Edge cases acknowledged

- **Symmetric-Ben blind spot.** Both ends being Ben's code means he can mutate the protocol on both sides in the same commit. Mitigation is the spec-text test discipline above. The class of bug that escapes — shared mental model on ambiguous spec language — is exactly the bug Gate B exists to catch. The split is honest about this gap rather than hiding it.

- **What if Ben's symmetric cross-machine never sustains 2 weeks?** Then Gate A doesn't fire, and federation framing should be reconsidered — same outcome as the original 4-week clock would have produced, but driven by absence-of-need rather than absence-of-time. This is intentional.

- **What if a 2nd implementer commits before Ben's symmetric use is live?** Then Gate B fires first, and the vector pack ships against whatever shape the gateway has at that moment (possibly `v0.9.x` if gateway isn't built yet). Unlikely path; design tolerates it.
