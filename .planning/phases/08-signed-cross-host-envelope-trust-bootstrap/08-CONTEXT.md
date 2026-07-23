# Phase 8: Signed Cross-Host Envelope + Trust Bootstrap - Context

**Gathered:** 2026-07-23
**Status:** Ready for planning

> **[--auto]** Discussion ran in autonomous mode. Every gray area below was
> auto-resolved to the recommended option and logged inline. Ben should skim
> `<decisions>` before `/gsd-plan-phase 8` — any decision he wants changed is a
> one-line edit here, then re-plan.

<domain>
## Phase Boundary

Deliver the **wire format + trust machinery** for cross-host FAMP, not the live
two-machine delivery cycle (that is Phase 9). Concretely, this phase makes four
things true:

1. A cross-host envelope carries the forward-compatible federation fields
   (sender/receiver domain + key_id, nonce, expiry; capability/approval
   omitted-when-empty) and round-trips through canonical JSON byte-exact
   (WIRE-02).
2. That envelope is Ed25519-signed under the `FAMP-sig-v1\0` domain prefix, and
   an unsigned / signature-invalid envelope is rejected at the receiving
   gateway **before it touches the local bus** (WIRE-01, INV-10 on the
   cross-host path).
3. `famp peer export` on machine A → out-of-band move → `famp peer import` on
   machine B (and the reverse) establishes mutual TOFU key trust, with **no key
   material ever crossing FAMP itself** (TRUST-01).
4. An envelope signed by an unpinned key is rejected with no state created and
   no implicit trust (TRUST-02).

**This is a v1.0 own-machines-first phase.** Two machines Ben controls,
hand-copied keys, full network control, **no public relay**. The gateway
ingress-verify is built as a pure function this phase; Phase 9 wires it to the
live HTTP transport and proves the full `request → commit → deliver → ack`
cycle across the wire.

**Explicitly out of scope** (deferred to v1.1 / v2.0+, per PROJECT.md and the
FAMP-Sec spec): public-internet relay, cross-person trust, the signed peer
directory, active nonce/replay caching, and the entire FAMP-Sec
capability/approval/tool-admission plane. The capability/approval envelope
fields are reserved wire real-estate this phase — **carried and omit-when-empty,
never interpreted.**

</domain>

<decisions>
## Implementation Decisions

### Envelope shape (WIRE-02)
- **D-01 [auto → recommended]:** **One envelope, one signature.** Extend the
  existing `famp-envelope` wire type with the federation fields as **optional,
  `skip_serializing_if`-omitted-when-empty** additions — `from_domain`,
  `to_domain`, `sender_key_id`, `nonce`, `expiry`, plus reserved `capability` /
  `approval`. All are covered by the *single existing INV-10 signature*. No
  nested/double-signed outer wrapper. Rationale: this is the least-surprise
  reading of PROJECT.md's "extend `famp-envelope` … forward-compatible without a
  wire break," and it keeps exactly one canonical signing input to reason about.
  *Rejected:* a separate `CrossHostEnvelope` wrapper with its own second
  signature (nested double-sign — redundant crypto, two canonical forms to keep
  byte-exact).
- **D-02 [auto → recommended]:** **Local path stays byte-identical.** Because
  every new field is omit-when-empty, a local-bus envelope (crypto-dropped)
  serializes to the exact bytes it does today. The cross-host path is the only
  place the gateway populates + signs the federation fields. This preserves the
  v0.9 local-bus wire and the existing RFC 8785 / §7.1c interop vectors
  unchanged. Preserve the no-`serde(flatten)` / no-`serde(tag)` discipline
  (`wire.rs` top-of-file warning) when adding fields.
- **D-03 [auto → recommended]:** **Domain = `Principal.authority`.**
  `Principal` is already `agent:<authority>/<name>`; `from_domain` / `to_domain`
  derive from the sender/receiver authority. `key_id` = a stable fingerprint of
  the Ed25519 verifying key (recommend `b64url(sha256(pubkey))` truncated to a
  documented length, human-comparable). Researcher confirms exact key_id
  derivation + length against any spec §; planner locks it.

### nonce / expiry enforcement scope
- **D-04 [auto → recommended]:** **Carry + sign both; actively enforce
  neither this phase.** `nonce` (random 128-bit) and `expiry` (absolute
  timestamp) are populated and covered by the signature so the wire is
  v1.1-ready, but Phase 8 does **not** build a replay cache and does **not**
  reject on expiry. Format is validated only (nonce present/well-formed; expiry
  parses and is after `ts`). Rationale: Phase 8 proves the *spine* — signature +
  trust. Active anti-replay + expiry rejection add clock-skew and shared-state
  failure modes that belong to the public-internet layer (v1.1); folding them in
  now would make a Phase 9 E2E failure ambiguous between "spine" and "new
  layer." Matches the own-machines-first thesis in PROJECT.md and the
  serial-scoped-negative-cache-is-v2.0 note in the FAMP-Sec spec.

### Trust bootstrap surface (TRUST-01)
- **D-05 [auto → recommended]:** **Reuse the peer-card + keyring line format.**
  Add `famp peer export --as <name>` emitting a **single, copy/paste-safe line**
  (principal + `b64url` pubkey + a human-readable key_id/fingerprint for
  eyeball verification over Signal), building on the existing `famp info`
  peer-card. Add `famp peer import [<file>|-]` that parses via the existing
  `famp-keyring` `parse_line` and pins via `Keyring::pin_tofu`. No key material
  ever traverses FAMP — the transport is Ben's clipboard / Signal. Rationale:
  the roadmap explicitly says this phase "wires the preserved `famp-keyring`
  into the gateway's cross-host path"; `file_format.rs` already has
  `serialize_entry` / `parse_line`, and TOFU pin-with-conflict-detection already
  exists in `Keyring::pin_tofu`.
- **D-06 [auto → recommended]:** **Dedicated gateway peer keyring on disk.**
  The gateway reads/writes a peer keyring at a stable path under `~/.famp/`
  (recommend `~/.famp/gateway/peers.keyring`), separate from any per-session
  identity store. `import` writes to it; ingress-verify reads from it. Planner
  confirms the exact path against `famp home`/`paths.rs` conventions.

### Ingress verify + rejection semantics (WIRE-01, TRUST-02)
- **D-07 [auto → recommended]:** **Verify is a pure, transport-agnostic
  function.** Build `verify_inbound(bytes, &keyring) -> Result<SignedEnvelope,
  RejectReason>` in `famp-gateway`: canonical-decode → `verify_strict` over the
  `FAMP-sig-v1\0` signing input (reuse `famp-crypto`) → look up the sender
  Principal in the pinned keyring and require the signing key to match. It takes
  bytes + keyring as *input* (data-as-input, not synthetic wire routing) so it's
  unit-testable in-process and Phase 9 just feeds it the HTTP body. This is the
  **protocol-grade boundary** decision already locked in Recent Decisions: the
  gateway verifies Ed25519/INV-10 at the edge and carries the verified sender
  key inward — it does NOT strip envelopes and re-inject chat-grade into the
  crypto-dropped local bus.
- **D-08 [auto → recommended]:** **Two distinct, loud rejection reasons, no
  state, no bus write.** A rejected envelope produces zero local-bus writes and
  zero pinned/registry state, and is logged at `warn` with the sender
  principal + key_id and a reason that distinguishes **`invalid_signature`**
  (bad crypto / unsigned) from **`unpinned_key`** (unknown peer). When Phase 9
  wires the transport, this surfaces as an HTTP 4xx. Rationale: an operator (and
  the Phase 9 E2E) must be able to tell "the bytes were tampered" apart from "I
  never imported that peer" — collapsing them into one error reproduces the
  v0.9 flat-error incident class (see `register error disambiguation`).

### Claude's Discretion
- Exact `key_id` derivation function + truncation length (D-03) — researcher to
  confirm against spec/precedent, planner locks.
- Exact on-disk peer-keyring path (D-06) — planner confirms against
  `paths.rs` / `famp home`.
- CLI noun/verb final spelling (`famp peer export/import` vs a `gateway`
  subcommand) — planner picks against existing clap tree in `cli/mod.rs`;
  recommend top-level `famp peer` to mirror the existing `famp info` peer-card.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase intent & requirements
- `.planning/ROADMAP.md` §"Phase 8: Signed Cross-Host Envelope + Trust
  Bootstrap" — goal + 4 success criteria (the acceptance contract).
- `.planning/REQUIREMENTS.md` — WIRE-01, WIRE-02, TRUST-01, TRUST-02 exact text.
- `.planning/PROJECT.md` §"Current Milestone: v1.0 Federation Profile — Gateway
  Core" — own-machines-first thesis, "no public relay," and the explicit
  "Explicitly NOT v1.0" deferral list (relay, cross-person trust, peer
  directory, FAMP-Sec plane).
- `ARCHITECTURE.md` — Layer 0 primitive / Layer 1 bus / Layer 2 gateway model;
  `FAMP-sig-v1\0` domain-prefix + INV-10 invariant statements.

### Reused primitives (extend / wire, do not re-derive)
- `crates/famp-envelope/src/wire.rs` — `WireEnvelope<B>`; the no-`flatten` /
  no-`tag` serde discipline the new fields must respect (D-01/D-02).
- `crates/famp-envelope/src/envelope.rs` — `UnsignedEnvelope` / `SignedEnvelope`
  type-state, `sign()` / `decode()` / `encode()`; the single INV-10 signature
  site.
- `crates/famp-envelope/src/version.rs` — `FAMP_SPEC_VERSION = "0.5.2"`.
- `crates/famp-crypto/src/keys.rs`, `verify.rs` — `FampSigningKey`,
  `TrustedVerifyingKey`, `FampSignature`, `verify_strict`.
- `crates/famp-keyring/src/lib.rs` — `Keyring::pin_tofu` (TOFU with
  conflict-detection), `get`, `load_from_file` / `save_to_file`.
- `crates/famp-keyring/src/file_format.rs` — `serialize_entry` / `parse_line`
  (the export/import blob format, D-05).
- `crates/famp-core/src/identity.rs` — `Principal { authority, name }`
  (`from_domain`/`to_domain` source, D-03).

### Gateway (Phase 7 skeleton this phase builds on)
- `crates/famp-gateway/src/{lib,principal,registry,error,main}.rs` — Design A
  local-proxy skeleton; add ingress-verify here (D-07).
- `.planning/phases/07-broker-liveness-fork-gateway-skeleton/07-VERIFICATION.md`
  — what Phase 7 actually delivered (LIVE-01/02, GW-04); note the gateway has
  **no transport/HTTP ingress yet** — that's Phase 9.

### Out-of-scope guardrail (read to know what NOT to build)
- FAMP-Sec security-plane spec (draft 0.2, in `~/Downloads`, fragile) —
  capability/approval/tool-admission semantics are **v2.0+**. This phase only
  reserves the omit-when-empty wire fields; it interprets none of them.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp-keyring` `pin_tofu` + `parse_line`/`serialize_entry`: the entire
  TRUST-01/TRUST-02 mechanism (pin, conflict-detect, one-line blob) already
  exists — Phase 8 wires it, doesn't reinvent it.
- `famp-crypto` `verify_strict` + `FAMP-sig-v1\0` prefix: the WIRE-01 signature
  check is a call into existing, KAT-tested code.
- `famp info` peer-card command (`crates/famp/src/cli/info/`): the natural base
  for `famp peer export`'s output.
- `famp-envelope` type-state (`UnsignedEnvelope::sign` → `SignedEnvelope`): the
  cross-host envelope rides the same sign/verify path; only the field set grows.

### Established Patterns
- **No `serde(flatten)` / `serde(tag)` in the envelope** (wire.rs warning) —
  new fields are plain `Option` members with `skip_serializing_if`, or the
  `deny_unknown_fields` gate breaks and byte-exactness regresses.
- **Data-as-input over synthetic wire routing** (memory: prefer pure cores) —
  ingress verify takes `(bytes, keyring)` as parameters (D-07), no forged
  synthetic messages.
- **Loud, disambiguated errors** (memory: register-error-disambiguation) —
  `invalid_signature` vs `unpinned_key` split (D-08), never a flat "rejected."
- **`ChildGuard` RAII** for any test spawning broker/gateway children (memory:
  test-child-guard-convention).

### Integration Points
- `verify_inbound` (new, `famp-gateway`) → `GatewayRegistry` back()/deliver:
  only a verified `SignedEnvelope` reaches the proxied-principal delivery path.
- `famp peer import` → `~/.famp/gateway/peers.keyring` (D-06) → the same file
  `verify_inbound` reads. Single source of pinned truth.
- Phase 9 seam: `verify_inbound` is called by the (Phase 9) HTTP transport
  handler; Phase 8 delivers and unit-tests the function, not the live socket.

</code_context>

<specifics>
## Specific Ideas

- Prove TRUST-01 with a **single-machine round-trip test**: `export` produces a
  blob, feed it to `import`, assert the key is pinned and a matching-key
  envelope verifies while a wrong-key envelope is rejected — no second physical
  machine needed for the phase's own gate (the real two-machine run is a Phase 9
  / Phase 10 concern).
- Keep the export blob **one line, Signal-paste-safe** (no multi-line PEM), with
  a short human fingerprint so Ben can read the last 6–8 chars aloud to confirm
  the paste survived.

</specifics>

<deferred>
## Deferred Ideas

- **Active nonce/replay cache + expiry rejection** — wire fields carried+signed
  now (D-04), enforcement is v1.1 (public-internet layer).
- **Public-internet dumb relay** — v1.1 (Recent Decisions: relay = availability
  dependency, not trust; own-machines-first has no relay).
- **Cross-person trust + signed peer directory** — v1.1.
- **FAMP-Sec capability/approval/tool-admission plane** — v2.0+; only reserved
  wire fields here.
- **Live two-process HTTP cross-host cycle** — Phase 9 (GW-01/02/03).
- **Deferred federation test triage** (`crates/famp/tests/_deferred_v1/`) —
  Phase 10 (TEST-01/02).

</deferred>

---

*Phase: 8-Signed Cross-Host Envelope + Trust Bootstrap*
*Context gathered: 2026-07-23*
