# Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example — Context

**Gathered:** 2026-04-13
**Status:** Ready for research → planning

<domain>
## Phase Boundary

A single developer runs `request → commit → deliver → ack` end-to-end in one binary (`cargo run --example personal_two_agents`), with every message signed and verified against a local-file TOFU keyring, and the three adversarial cases (CONF-05 unsigned / CONF-06 wrong-key / CONF-07 canonical divergence) fail closed with distinct typed errors on `MemoryTransport`. This phase ships four things and nothing else:

1. **`famp-transport`** — a pure, byte-oriented `Transport` trait plus an in-process `MemoryTransport` implementation (~50 LoC of actual transport logic). No signature verification, no envelope decoding, no keyring awareness inside the trait or the in-process impl.
2. **`famp-keyring`** — a new crate providing `Keyring`, `TrustedVerifyingKey`, a line-oriented file format, and TOFU semantics (first pin wins, conflict always rejects, no silent overwrites).
3. **Runtime glue in `crates/famp/src/`** — the orchestration module that composes transport + envelope + crypto + keyring + FSM. This is where signature verification, envelope decoding, FSM stepping, and the envelope↔FSM adapter (Phase 2 D-D3) live. No new `famp-runtime` crate.
4. **`crates/famp/examples/personal_two_agents.rs`** — single binary driving the four-message happy path over `MemoryTransport`, printing a typed conversation trace, exiting 0. Three adversarial tests live next to this example, exercising the runtime glue on top of `MemoryTransport` with test-only raw injection.

**Explicitly out of scope for Phase 3** (even though mentioned in adjacent context): HTTP transport, TLS, axum, reqwest, middleware layers, `.well-known` Agent Card distribution, cancellation-safe spawn channels, Agent Cards of any kind, pluggable trust stores, federation credentials. Phase 4 will build on top of whatever this phase locks — none of it will be re-designed there.

</domain>

<decisions>
## Implementation Decisions

### A. Principal ↔ Pinned Pubkey Binding (the load-bearing decision)

- **D-A1 (contract reinterpretation, REQUIREMENTS.md UPDATE REQUIRED):** KEY-01's literal wording "principal = raw Ed25519 pubkey" directly contradicts v0.6 Phase 3's shipped `famp_core::Principal`, which is a parsed `agent:<authority>/<name>` identity (see `crates/famp-core/src/identity.rs:19`). **Phase 3 does NOT redefine `Principal`.** Rolling back the v0.6 identity type would throw away an intentional layering decision and contaminate every downstream crate. Instead, the correct reading of KEY-01 for v0.7 is:

  > **Each principal is authenticated by exactly one pinned Ed25519 public key in Personal Profile v0.7.**

  The binding is the keyring itself, not a type-equality claim. **Action item for planner:** update `.planning/REQUIREMENTS.md` KEY-01 wording in the same commit that lands `famp-keyring`, with a pointer to this D-A1 in the commit body. This is a real contract fix, not a cosmetic edit.

- **D-A2:** The keyring value type is a **newtype**, not a raw `ed25519_dalek::VerifyingKey`:
  ```rust
  pub struct TrustedVerifyingKey(ed25519_dalek::VerifyingKey);
  ```
  Constructing a `TrustedVerifyingKey` is the only way a key enters the trust boundary — it is produced by the keyring loader/CLI parser/TOFU-pin path, never by ad-hoc code. Raw `VerifyingKey` remains usable anywhere it already is (e.g. `famp-crypto::verify_strict`), but the keyring public API speaks exclusively in `TrustedVerifyingKey`. Matches the v0.6 "narrow by absence" precedent: untrusted keys literally cannot be stored in the keyring type.

- **D-A3:** Keyring shape:
  ```rust
  pub struct Keyring { map: HashMap<Principal, TrustedVerifyingKey> }

  impl Keyring {
      pub fn new() -> Self;
      pub fn load_from_file(path: &Path) -> Result<Self, KeyringError>;
      pub fn save_to_file(&self, path: &Path) -> Result<(), KeyringError>;
      pub fn with_peer(self, principal: Principal, key: TrustedVerifyingKey)
          -> Result<Self, KeyringError>;       // reject on conflict
      pub fn get(&self, principal: &Principal) -> Option<&TrustedVerifyingKey>;
      pub fn pin_tofu(&mut self, principal: Principal, key: TrustedVerifyingKey)
          -> Result<(), KeyringError>;         // first-sight only
  }
  ```
  `pin_tofu` is the ONLY mutation path after construction, and it fails closed if the principal already has a different pinned key. There is no `replace`, no `override`, no `force` variant.

- **D-A4:** Principal is the stable routing key everywhere downstream: transport address, FSM task owner, keyring lookup, envelope `from` field. No parallel "raw-pubkey-as-ID" universe exists in v0.7.

### B. Keyring File Format + TOFU Semantics

- **D-B1:** On-disk line format:
  ```text
  # FAMP v0.7 TOFU keyring
  # One entry per principal; principal is agent:<authority>/<name>
  # pubkey is base64url-unpadded 32-byte Ed25519 verifying key
  agent:local/alice  11qYAY7gqW8...
  agent:local/bob    nWGxne_9WmC...
  ```
  **Grammar (normative for v0.7):**
  - One entry per line.
  - Separator between principal and pubkey: one or more ASCII spaces or tabs (`[ \t]+`).
  - Full-line comments: any line whose first non-whitespace character is `#` is ignored.
  - Blank lines (including whitespace-only) are ignored.
  - **No inline trailing comments** in v0.7 — a `#` anywhere after the pubkey is a parse error.
  - Trailing newline at EOF allowed but not required; round-trip save writes trailing `\n`.
  - Line ending: `\n` only on write; `\r\n` tolerated on read for cross-platform sanity.

- **D-B2:** Validation rules, all enforced at load:
  - Principal parses via `Principal::from_str` (delegates to v0.6 parser; propagates error).
  - Pubkey is base64url-unpadded; strict decode (reject padding, reject non-URL-safe alphabet).
  - Decoded pubkey is exactly 32 bytes; `VerifyingKey::from_bytes` must succeed (weak-key rejection already enforced by `famp-crypto` per v0.6 Phase 2).
  - **Duplicate principal entries → reject** with `KeyringError::DuplicatePrincipal { principal }`.
  - **Same pubkey under two different principals → reject** with `KeyringError::DuplicatePubkey { principals: (p1, p2) }`. (One-to-one pinning.)
  - Unknown/unparseable line structure → reject with line number in the error.

- **D-B3:** TOFU semantics — "first pin wins, conflict always rejects":
  - On first sight of an unknown principal with a valid signature, the runtime MAY call `pin_tofu` if the caller has opted into TOFU mode (example binaries do).
  - On seeing a known principal with a DIFFERENT key: always reject. There is no auto-rotate, no prompt, no override. **"Pinning is sticky" is the one property that matters.**
  - Keyring never overwrites a pinned key automatically in v0.7. Key rotation is v0.8+.

- **D-B4:** CLI flag bootstrap (KEY-03):
  - Flag syntax: `--peer agent:<authority>/<name>=<base64url-unpadded-pubkey>` (use `=` not `:` because principals contain `:`).
  - File is loaded first; CLI flags are merged via `Keyring::with_peer` one at a time.
  - **If a CLI entry conflicts with a file entry → reject.** No precedence games.
  - Both file path and `--peer` flag are optional; binary must accept either, both, or only-CLI for purely ephemeral runs.

- **D-B5:** Round-trip test (KEY-02 explicit requirement): committed fixture `crates/famp-keyring/tests/fixtures/two_peers.keyring` is loaded, saved to a `tempfile`, and byte-compared. Byte-identical round trip is required. This locks the save format: alphabetical order by principal string, exactly two spaces as separator, trailing `\n`, no trailing-blank-line churn.

- **D-B6:** `KeyringError` is a phase-local narrow enum (precedent: v0.6 Plans 01-01, 02-01, Phase 1 `EnvelopeDecodeError`, Phase 2 `TaskFsmError`). It does NOT convert into `ProtocolErrorKind` inside `famp-keyring` — mapping happens at the runtime boundary in `crates/famp/src/`, same rule as every other v0.7 crate.

### C. Transport Trait + MemoryTransport

- **D-C1:** Transport is **byte-oriented and principal-addressed**. It knows nothing about envelopes, signatures, canonicalization, or the FSM. The trait:
  ```rust
  pub struct TransportMessage {
      pub sender: Principal,
      pub recipient: Principal,
      pub bytes: Vec<u8>,        // raw wire bytes — signed envelope JSON
  }

  pub trait Transport {
      type Error: std::error::Error + Send + Sync + 'static;

      async fn send(&self, msg: TransportMessage) -> Result<(), Self::Error>;
      async fn recv(&self, as_principal: &Principal)
          -> Result<TransportMessage, Self::Error>;
  }
  ```
  Rationale: mirrors the eventual HTTP mental model — HTTP inbox is per-principal, bytes are opaque to the layer, sender/recipient are addressing metadata, not envelope payload. Phase 4 `HttpTransport` implements this trait with zero shape change.

- **D-C2:** `Principal` is the routing address. No separate `Address` type, no `SocketAddr`-style abstraction, no URL in the trait surface. If Phase 4 HTTP needs URL-per-principal, that mapping lives **inside** `HttpTransport`'s constructor, not in the trait.

- **D-C3:** **Signature verification is NOT in the transport trait.** Decision C's whole point: transport hands up raw bytes, runtime glue decodes + verifies + dispatches. This guarantees both `MemoryTransport` and `HttpTransport` exercise identical verification code paths. Default trait methods that do verification would couple transport to `famp-envelope` and `famp-keyring` — rejected.

- **D-C4:** `MemoryTransport` implementation shape:
  ```
  struct MemoryTransport {
      inboxes: Arc<Mutex<HashMap<Principal, mpsc::UnboundedReceiver<TransportMessage>>>>,
      senders: Arc<Mutex<HashMap<Principal, mpsc::UnboundedSender<TransportMessage>>>>,
  }
  ```
  - **Shared inbox hub keyed by principal.** One `tokio::sync::mpsc::unbounded_channel` per principal.
  - `send(msg)` looks up `msg.recipient`'s sender half and pushes; unknown recipient returns `MemoryTransportError::UnknownRecipient`.
  - `recv(&principal)` waits on that principal's receiver half.
  - `register(principal)` creates both halves for a new agent (example calls this for alice and bob at startup).
  - Unbounded channels: no backpressure — personal profile, single binary, ≤20 messages per run. Bounded would complicate the trace and blow the ~50 LoC budget for zero gain.

- **D-C5:** Async-in-trait: **use native AFIT (async fn in trait), not `async-trait` crate.** Toolchain is `rust 1.87+` (Cargo.toml pin). AFIT is stable since 1.75 for trait definitions without dyn dispatch. Phase 3 does not need `dyn Transport`; both `MemoryTransport` and `HttpTransport` are used as concrete types in typed generic contexts (example binaries, tests). If a future phase needs `dyn Transport`, it can box at that site. No macro dependency for v0.7.

- **D-C6:** `MemoryTransport` itself is ~50 LoC (per TRANS-02 requirement) EXCLUDING: the `test-util` feature gate, the `TransportMessage` struct (shared), and the error enum. Counting only the `impl Transport for MemoryTransport` body + the inbox hub struct.

- **D-C7:** `MemoryTransportError` is a phase-local narrow enum: `UnknownRecipient { principal }`, `InboxClosed { principal }`. No other variants.

### D. Runtime Glue Home + Adversarial Injection

- **D-D1:** The runtime orchestration module lives in **`crates/famp/src/runtime/`** (inside the existing top-level `famp` crate). No new `famp-runtime` crate. Abstraction is cheaper added later than removed later; personal v0.7 doesn't know what wants to stabilize.

- **D-D2:** `crates/famp/` dependency graph:
  ```
  famp (top crate, has examples/)
    ├── famp-core
    ├── famp-canonical
    ├── famp-crypto
    ├── famp-envelope
    ├── famp-fsm
    ├── famp-transport
    └── famp-keyring          (new in Phase 3)
  ```
  `famp-transport` does NOT depend on `famp-envelope` or `famp-keyring`. `famp-keyring` depends on `famp-core` + `famp-crypto` only. All composition happens in `crates/famp/src/runtime/`.

- **D-D3:** Runtime glue responsibilities (pseudocode):
  ```
  loop {
      let msg = transport.recv(&me).await?;              // raw bytes
      let env = decode_signed_envelope(&msg.bytes)?;     // famp-envelope
      let pinned = keyring.get(&env.from())
          .ok_or(RuntimeError::UnknownSender)?;
      verify_strict(pinned, &env)?;                      // famp-crypto
      cross_check_recipient(&msg.recipient, &env)?;      // D-D5 below
      let fsm_input = fsm_input_from_envelope(&env);     // Phase 2 D-D3 adapter
      task_fsm.step(fsm_input)?;                         // famp-fsm
      // emit next message, if any, back via transport.send()
  }
  ```
  This is the ~20-line adapter Phase 2 D-D3 committed to, expanded with sig-verification and the sender cross-check. Lives ONLY here — neither `famp-envelope` nor `famp-fsm` grow a dependency because of it.

- **D-D4:** `ack` handling: **`ack` is transport-only and does NOT call `TaskFsm::step`.** Phase 2 explicitly shipped 4 legal FSM arrows (plus `control/cancel`); `ack` is not among them. In Phase 3 runtime glue, `ack` messages are decoded, sig-verified, cross-checked, logged to the conversation trace, and returned without touching the FSM. This decision is **locked here** so Phase 2 does not need to be reopened. The roadmap SC#3 phrasing "`request`, `commit`, `deliver`, and `ack`" refers to the wire-trace order, not FSM transitions.

- **D-D5:** **Sender identity cross-check (the fifth locked decision):** On receive, the runtime MUST verify BOTH:
  1. The signature is valid under the pinned key for `envelope.from()`.
  2. **If the envelope carries a recipient field** (e.g. `to`), it matches the transport-layer `msg.recipient`.

  **Phase 1 envelope `to` field status is currently unverified** — researcher MUST check `.planning/phases/01-minimal-signed-envelope/01-CONTEXT.md` and `crates/famp-envelope/src/` for whether body/common envelope carries a recipient Principal. Three outcomes:
  - **If present:** cross-check is mandatory; mismatch returns a distinct `RuntimeError::RecipientMismatch { transport, envelope }`. CONF-05/06/07 test matrix gains a fourth case: transport-recipient / envelope-recipient mismatch returns the typed error (bonus coverage, not a new CONF requirement).
  - **If absent:** D-D5.2 is a no-op for v0.7, runtime still enforces D-D5.1, and researcher flags this in RESEARCH.md for v0.8 (Federation Profile will need the field added + middleware enforcement). Do NOT retroactively modify Phase 1 body schemas to add it — Phase 1 is shipped.
  - **If ambiguous:** researcher escalates before planning.

  Planner must resolve this before writing Plan 03-0x for the runtime glue.

- **D-D6:** Adversarial injection mechanism (CONF-05/06/07):
  ```rust
  // crates/famp-transport/src/memory.rs
  impl MemoryTransport {
      #[cfg(feature = "test-util")]
      pub async fn send_raw_for_test(&self, msg: TransportMessage)
          -> Result<(), MemoryTransportError> {
          // bypass any validation, push bytes as-is
      }
  }
  ```
  - `famp-transport` exposes a `test-util` feature flag.
  - `crates/famp/Cargo.toml` has `famp-transport = { path = "...", features = ["test-util"] }` **only under `[dev-dependencies]`**, not in `[dependencies]`. Production build cannot reach `send_raw_for_test`.
  - The three adversarial tests live in `crates/famp/tests/adversarial.rs` (integration tests), construct valid-looking envelopes with surgically injected defects, push them via `send_raw_for_test`, and assert the runtime glue returns the expected typed error variant for each.
  - **Rejected alternatives:** a separate `AdversarialMemoryTransport` wrapper (more machinery for no gain); an unsafe/test-only constructor on `SignedEnvelope` that bypasses signing (widens the more important envelope boundary — BAD).

- **D-D7:** CONF-07 canonical-divergence test construction: hand-crafted JSON bytes with a **valid Ed25519 signature computed over a DIFFERENT canonical form** than the bytes actually on the wire. The runtime re-canonicalizes the received bytes, re-verifies against the pinned key, and detects the mismatch via `famp-crypto`'s verify-strict pathway. Fixture bytes committed under `crates/famp/tests/fixtures/conf-07-canonical-divergence.json`. This exercises the real verification code path, not a stand-in. **Why not whitespace mutation that invalidates the signature?** Because that is indistinguishable from CONF-06 (wrong key) — it does not test the canonicalization check specifically.

- **D-D8:** `RuntimeError` is a phase-local narrow enum inside `crates/famp/src/runtime/error.rs`:
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum RuntimeError {
      #[error("unknown sender: {0}")]
      UnknownSender(Principal),
      #[error("signature verification failed")]
      SignatureInvalid(#[source] famp_crypto::VerifyError),
      #[error("canonicalization divergence detected")]
      CanonicalDivergence,
      #[error("transport recipient {transport} does not match envelope recipient {envelope}")]
      RecipientMismatch { transport: Principal, envelope: Principal },
      #[error("transport error")]
      Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
      #[error("envelope decode error")]
      Decode(#[source] famp_envelope::DecodeError),
      #[error("keyring error")]
      Keyring(#[source] famp_keyring::KeyringError),
      #[error("fsm error")]
      Fsm(#[source] famp_fsm::TaskFsmError),
  }
  ```
  Each CONF-0x test asserts a SPECIFIC variant — CONF-05 → `SignatureInvalid` (unsigned envelope fails decode's INV-10 check, may surface as `Decode` — researcher to confirm); CONF-06 → `SignatureInvalid`; CONF-07 → `CanonicalDivergence`. The variants must be distinguishable, not collapsed into a single "BadMessage".

### E. Example Binary

- **D-E1:** `crates/famp/examples/personal_two_agents.rs` wires:
  - Two `Principal`s: `agent:local/alice`, `agent:local/bob`.
  - Two Ed25519 keypairs generated with `rand` inside the binary (not loaded from disk — this is the personal happy path, no persistent identity yet).
  - One `MemoryTransport` instance, with both principals `register`ed.
  - Two `Keyring`s — one per agent — each containing ONLY the other's `TrustedVerifyingKey` (pre-pinned via `with_peer`, not TOFU).
  - Two `tokio::spawn`ed tasks, one per agent, each running a runtime loop over its inbox.
  - Alice sends `request` to Bob; Bob commits, delivers (`interim=false`, `terminal_status=completed`), acks; Alice receives ack; both tasks complete; binary prints a typed trace; exits 0.

- **D-E2:** "Typed conversation trace" format for CONF-03: printed as `[seq] SENDER → RECIPIENT: CLASS (state: FROM → TO)` lines, one per message, in the exact order the wire saw them. Sequence number is a local counter, NOT a protocol field. Trace format is not an external contract — planner picks the exact string layout.

- **D-E3:** The example is exercised by both a manual `cargo run` and an integration test `crates/famp/tests/example_happy_path.rs` that invokes it as a subprocess and asserts exit-code 0 + expected trace lines. CI gate.

### Claude's Discretion

- Exact file layout inside `crates/famp-keyring/src/` (`lib.rs` + `file_format.rs` + `error.rs`, or flat) — whatever reads cleanest.
- Exact file layout inside `crates/famp/src/runtime/` (one module or a handful).
- Whether `Keyring::load_from_file` returns `(Self, Vec<Warning>)` or hard-errors on first issue — prefer hard-error for v0.7 simplicity unless a concrete use case emerges in planning.
- Exact name of the envelope→FSM adapter function (`fsm_input_from_envelope` vs `derive_fsm_input` vs `TaskTransitionInput::from_envelope`).
- Whether `TransportMessage` is `Clone` or `Copy`-of-handle — prefer owned `Vec<u8>` for v0.7 (matches Phase 2 D-A5 "no lifetimes at crate boundary" rule).
- Exact `[features]` table wording in `famp-transport/Cargo.toml`.
- Whether the adversarial tests live in one `adversarial.rs` file with three `#[tokio::test]`s or three files — one file preferred.
- Whether CONF-07 fixture is pre-generated (committed bytes) or generated at test time from a deterministic seed — pre-generated preferred (matches v0.6 §7.1c vector-0 precedent).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec — transport, trust, and wire semantics
- `FAMP-v0.5.1-spec.md` §7.1 — canonical signing/verification rules; CONF-07 (canonicalization divergence) fixture must exercise this exact path.
- `FAMP-v0.5.1-spec.md` §7.1c — worked Ed25519 example (v0.6 Phase 2 byte-exact gate); Phase 3 does not extend this, but the CONF-07 fixture draws on the same conventions.
- `FAMP-v0.5.1-spec.md` §18 — 1 MB body limit; noted here for Phase 4 reference; NOT enforced in `MemoryTransport` for v0.7.
- `FAMP-v0.5.1-spec.md` §14.3 / INV-10 — unsigned messages unreachable at type level. `MemoryTransport` does not re-enforce INV-10 — it is already enforced by `famp-envelope` decode (Phase 1). CONF-05 test asserts the decode path's typed error.
- `FAMP-v0.5.1-spec.md` §5.1, §5.2 — `Principal` and `Instance` identity formats. v0.6 `famp-core::Principal` already parses these; D-A1 reaffirms this is the trust binding axis in v0.7.
- `FAMP-v0.5.1-spec.md` §7.3a — FSM-observable whitelist. Phase 3 runtime glue uses the same extraction the Phase 2 FSM expects.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — **TRANS-01, TRANS-02, KEY-01 (wording update required per D-A1), KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07.** Also note TRANS-05 and TRANS-08 explicitly absent.
- `.planning/ROADMAP.md` Phase 3 — 5 success criteria (Transport trait + MemoryTransport, TOFU keyring, same-process example, adversarial cases, keyring round-trip).
- `.planning/PROJECT.md` — "narrow by absence, not by option" rule; no Agent Card, no federation credential, no pluggable trust store in v0.7.

### Prior phase outputs (direct dependencies of this phase's runtime glue)
- `.planning/phases/01-minimal-signed-envelope/01-CONTEXT.md` — envelope type-state, `AnySignedEnvelope` shape, `SignedEnvelope` decode pipeline. **D-D5 researcher task: confirm whether the common envelope header carries a recipient `to: Principal` field.**
- `.planning/phases/01-minimal-signed-envelope/01-RESEARCH.md` — body schemas, field extraction precedents.
- `.planning/phases/01-minimal-signed-envelope/01-03-PLAN.md` — decode pipeline final shape (where CONF-05 INV-10 error surfaces).
- `.planning/phases/02-minimal-task-lifecycle/02-CONTEXT.md` — **D-D3** (Phase 3 owns the envelope↔FSM adapter), **D-B5** (`TerminalStatus` home), **D-F2** (deterministic fixture arrows that the example binary exercises end-to-end).
- `.planning/phases/02-minimal-task-lifecycle/02-01-PLAN.md` — `MessageClass` + `TerminalStatus` lifted into `famp-core`; runtime glue imports from there.
- `.planning/phases/02-minimal-task-lifecycle/02-RESEARCH.md` — FSM transition table that the adapter feeds.

### v0.6 implementation precedents
- `crates/famp-core/src/identity.rs` — `Principal` + `Instance` parsers. D-A1 depends on this being the authoritative identity type.
- `crates/famp-core/src/error.rs` — `ProtocolErrorKind` boundary sink; not used inside `famp-keyring`, `famp-transport`, or `famp-runtime` internals.
- `crates/famp-crypto/src/` — `verify_strict`, weak-key rejection, base64url-unpadded codec. Keyring file parser REUSES the crypto crate's base64 codec — do not reimplement.
- `crates/famp-canonical/src/` — canonical JSON. CONF-07 fixture exercises this re-canonicalization path.
- `crates/famp-envelope/src/` — `SignedEnvelope`, decode pipeline, `INV-10` enforcement. Runtime glue calls decode here; CONF-05 surfaces as a typed decode error.
- `crates/famp-fsm/src/` — `TaskFsm::step`, `TaskTransitionInput`, `TaskFsmError`. Runtime glue owns the adapter from decoded envelope → `TaskTransitionInput`.
- `.planning/milestones/v0.6-phases/03-core-types-invariants/03-CONTEXT.md` — exhaustive consumer stub precedent, narrow-enum precedent. Same patterns apply to `KeyringError`, `MemoryTransportError`, `RuntimeError`.

### Technology stack references (from project CLAUDE.md — already researched)
- `tokio 1.51.1` — `tokio::sync::mpsc::unbounded_channel` is the MemoryTransport backbone.
- `ed25519-dalek 2.2.0` — `VerifyingKey` is what `TrustedVerifyingKey` wraps.
- `base64 0.22.1` — `URL_SAFE_NO_PAD` engine for keyring file pubkey codec.
- `thiserror 2.0.18` — all four Phase 3 error enums.
- No new crates introduced in Phase 3 beyond what v0.6 + Phase 1/2 already pulled.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp_core::Principal` — parsed `agent:<authority>/<name>` identity. **THE routing key across Phase 3** (transport address, keyring key, runtime cross-check).
- `famp_core::MessageClass`, `famp_core::TerminalStatus` — lifted into core by Phase 2; runtime glue imports directly when building `TaskTransitionInput`.
- `famp_crypto::verify_strict` — called from runtime glue for CONF-06 / happy-path verification.
- `famp_crypto` base64url codec — reused by `famp-keyring` file parser; do NOT reimplement.
- `famp_canonical::canonicalize` — exercised by CONF-07 fixture via `famp_envelope` decode.
- `famp_envelope::SignedEnvelope`, `AnySignedEnvelope`, decode pipeline — runtime glue consumes; CONF-05 (unsigned) surfaces here.
- `famp_fsm::TaskFsm`, `TaskTransitionInput`, `TaskFsmError` — runtime glue drives these per Phase 2 D-D3.
- `crates/famp-transport/src/lib.rs` — Phase 0 stub with a smoke test; Phase 3 replaces the body but does NOT create a new crate.
- `crates/famp/Cargo.toml` — top crate exists with examples directory convention already set up from v0.5.1/v0.6.

### Established Patterns
- **Phase-local narrow error enums** — `KeyringError`, `MemoryTransportError`, `RuntimeError` follow the v0.6 + Phase 1/2 pattern. No crate-internal use of `ProtocolErrorKind`.
- **Owned types at crate boundaries** — `TransportMessage` owns its `Vec<u8>`, `Principal`, and both endpoint identities. No lifetimes in public Phase 3 types.
- **"Narrow by absence"** — no `Option<FederationCredential>`, no feature-gated Agent Card, no stubbed-out pluggable trust store. Personal v0.7 does not reach for these.
- **Compile-time layering** — `famp-transport` has ZERO dependencies on envelope/fsm/keyring; composition is top-crate-only.
- **Phase 0 stub pattern** — `crates/famp-transport/` already exists as a stub; Phase 3 fills it in rather than scaffolding from scratch.
- **`rust 1.87+` toolchain** — native AFIT (async fn in trait) is stable; no `async-trait` macro dependency.

### Integration Points
- **`famp-transport` ↔ `crates/famp/` runtime glue:** raw-bytes in/out + `Principal` routing. One-way dependency; transport knows nothing of the composition above.
- **`famp-keyring` ↔ `crates/famp/` runtime glue:** `Keyring::get(&Principal) -> Option<&TrustedVerifyingKey>` is the sole lookup API from glue.
- **`crates/famp/src/runtime/` ↔ everything:** the only place that imports `famp-envelope` + `famp-fsm` + `famp-transport` + `famp-keyring` together.
- **`crates/famp/examples/personal_two_agents.rs` ↔ runtime:** uses the library surface of `crates/famp`. Driven in CI via a subprocess test in `crates/famp/tests/`.
- **`crates/famp/tests/adversarial.rs` ↔ `famp-transport` `test-util` feature:** the only place that enables the feature flag.
- **Phase 4 reuse lane:** every decision here is also the contract for Phase 4's HTTP transport. `famp-transport-http` will implement the same `Transport` trait, plug into the same runtime glue, and reuse the same `famp-keyring`, CONF-05/06/07 test shape, and example structure. Phase 3 must not hard-wire anything memory-specific into the runtime glue.

</code_context>

<specifics>
## Specific Ideas

- **"Principal stays semantic; trust is a binding, not an equality."** D-A1 is the headline: v0.6 `Principal` is the stable identity type, keyring is `HashMap<Principal, TrustedVerifyingKey>`, and KEY-01's wording is updated to match. Rolling back the core type is a bigger cost than any implementation simplification could earn back.
- **"Transport is bytes; composition is the top crate."** No signature verification in the `Transport` trait, no envelope decoding in `MemoryTransport`, no FSM awareness anywhere below `crates/famp/src/runtime/`. This is the layering Phase 4 depends on.
- **"First pin wins, conflict always rejects."** TOFU in Personal Profile is one rule: you see a principal's key once, and from then on, mismatches are fatal. No auto-rotate, no prompt, no override flag. Pinning is sticky.
- **"Test-only raw injection at the transport boundary, not the envelope boundary."** Weakening `SignedEnvelope` construction would compromise the more important INV-10 invariant. Keep the hostile-sender escape hatch at the transport layer, behind a Cargo feature, reachable only from dev-deps.
- **"Narrow error variants per adversarial case."** CONF-05, CONF-06, and CONF-07 must each return a distinct `RuntimeError` variant. The adversarial matrix has to be able to tell them apart — collapsing into a single "BadMessage" defeats the point of the test suite.
- **"ack is wire-level, not FSM-level."** The Phase 2 FSM has four legal arrows; ack is not among them, and will not be. Phase 3 handles ack in the runtime trace without adding it to `TaskFsm::step`.
- **"Sender cross-check belongs in the runtime."** Decision D-5 from the discussion: verify signature under the pinned key AND (if present) cross-check envelope recipient against transport recipient. Prevents the keyring from becoming "any valid key will do".
- **"No new abstraction until it earns its keep."** No `famp-runtime` crate, no generic `TrustStore` trait, no middleware layer, no pluggable codec. Everything composes in `crates/famp/src/runtime/` — which is the one place allowed to know the whole picture.

</specifics>

<deferred>
## Deferred Ideas

- **`.well-known` Agent Card distribution (TRANS-05)** — v0.8 Identity & Cards.
- **Cancellation-safe spawn-channel send path (TRANS-08)** — v0.9 Causality & Replay Defense.
- **Pluggable `TrustStore` trait + federation credential** — v0.8+.
- **Key rotation / multi-key principals** — v0.9+; v0.7 is one-principal-one-pinned-key.
- **Agent Card self-signature resolution** — v0.8 Identity & Cards.
- **Bounded-channel backpressure + flow control on transport** — not needed for personal profile; revisit if/when a real high-throughput transport lands.
- **`dyn Transport` / trait objects** — v0.7 uses concrete types with generics. Add `dyn` support only if a concrete caller needs it.
- **Inline trailing comments in keyring file format** — v0.7 is full-line comments only. Revisit if ops tooling asks for it.
- **Keyring auto-rotation / TOFU-override / quarantine-then-allow** — rejected for v0.7. Pinning is sticky; any revisit is a v0.8+ decision with an explicit rationale.
- **`HttpTransport` + `rustls` + axum + reqwest + signature middleware** — Phase 4 only.
- **`famp keygen` / `famp serve` CLI subcommands** — v0.8+ CLI milestone; v0.7 ships example binaries only.
- **`stateright` model check over the runtime glue** — v0.14 Adversarial Conformance.
- **Conformance Level 2/3 badges** — v0.14.
- **Envelope `to` field retroactive addition** — if D-D5 research shows no recipient field in v0.7 envelopes, DO NOT add it in Phase 3. Phase 1 is shipped; any schema addition lands in v0.8 Identity & Cards, not here.

</deferred>

---

*Phase: 03-memorytransport-tofu-keyring-same-process-example*
*Context gathered: 2026-04-13*
