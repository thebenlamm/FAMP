# Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example — Research

**Researched:** 2026-04-13
**Domain:** In-process async transport, TOFU key management, runtime glue composition, adversarial conformance testing
**Confidence:** HIGH (all findings verified against actual shipped codebase — no assumed claims about existing APIs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**A. Principal ↔ Pinned Pubkey Binding**
- D-A1: KEY-01 is reinterpreted — `Principal` stays as `famp_core::Principal` (`agent:<authority>/<name>`). Each principal is authenticated by exactly one pinned `Ed25519` public key. REQUIREMENTS.md KEY-01 wording must be updated in the same commit as `famp-keyring`.
- D-A2: Keyring value type is `TrustedVerifyingKey` from `famp_crypto` (already exists — DO NOT create a second wrapper). `pub struct TrustedVerifyingKey(ed25519_dalek::VerifyingKey)` is the ingress-vetted newtype in `famp_crypto`. The keyring stores `famp_crypto::TrustedVerifyingKey` values.
- D-A3: Keyring API shape: `Keyring { map: HashMap<Principal, TrustedVerifyingKey> }` with `new`, `load_from_file`, `save_to_file`, `with_peer` (reject on conflict), `get`, `pin_tofu` (first-sight only).
- D-A4: `Principal` is the stable routing key everywhere: transport address, FSM task owner, keyring lookup, envelope `from` field.

**B. Keyring File Format + TOFU Semantics**
- D-B1: Line format — `agent:local/alice  <base64url-unpadded-pubkey>`. One entry per line. Separator: one or more spaces/tabs. Full-line comments with `#`. Blank lines ignored. No inline trailing comments. Trailing newline on write, `\r\n` tolerated on read.
- D-B2: Validation at load: Principal via `Principal::from_str`, pubkey via `TrustedVerifyingKey::from_b64url` (already exists in `famp_crypto`), 32 bytes exactly, `DuplicatePrincipal` and `DuplicatePubkey` errors with line numbers.
- D-B3: TOFU semantics — "first pin wins, conflict always rejects." No auto-rotate, no override, no prompt.
- D-B4: CLI flag: `--peer agent:<authority>/<name>=<base64url-unpadded-pubkey>` (use `=` not `:` as separator). File loaded first, CLI flags merged via `with_peer`. Conflict rejects.
- D-B5: Round-trip fixture: `crates/famp-keyring/tests/fixtures/two_peers.keyring` — load → save → byte-compare. Save format: alphabetical order by principal string, two spaces as separator, trailing `\n`.
- D-B6: `KeyringError` is phase-local narrow enum. Does NOT convert into `ProtocolErrorKind` inside `famp-keyring`. Mapping happens in `crates/famp/src/runtime/`.

**C. Transport Trait + MemoryTransport**
- D-C1: `Transport` trait is byte-oriented and principal-addressed (no envelope knowledge):
  ```rust
  pub struct TransportMessage { pub sender: Principal, pub recipient: Principal, pub bytes: Vec<u8> }
  pub trait Transport {
      type Error: std::error::Error + Send + Sync + 'static;
      async fn send(&self, msg: TransportMessage) -> Result<(), Self::Error>;
      async fn recv(&self, as_principal: &Principal) -> Result<TransportMessage, Self::Error>;
  }
  ```
- D-C2: `Principal` is the routing address. No `Address` type, no URL in the trait.
- D-C3: Signature verification is NOT in the transport trait. Raw bytes up, runtime glue decodes + verifies.
- D-C4: `MemoryTransport` uses `Arc<Mutex<HashMap<Principal, mpsc::UnboundedSender/Receiver>>>`. Unbounded channels. `register(principal)` creates halves at startup. Unknown recipient → `MemoryTransportError::UnknownRecipient`.
- D-C5: Native AFIT (async fn in trait), NOT `async-trait` crate. Rust 1.87+ is pinned in workspace.
- D-C6: ~50 LoC counting only `impl Transport for MemoryTransport` body + inbox hub struct.
- D-C7: `MemoryTransportError` variants: `UnknownRecipient { principal }`, `InboxClosed { principal }`.

**D. Runtime Glue + Adversarial Injection**
- D-D1: Runtime orchestration lives in `crates/famp/src/runtime/` (inside existing `famp` crate). No new `famp-runtime` crate.
- D-D2: Dependency graph: `famp` top crate imports all sub-crates. `famp-transport` has ZERO deps on `famp-envelope`/`famp-keyring`. `famp-keyring` depends on `famp-core` + `famp-crypto` only.
- D-D3: Runtime glue loop: `transport.recv` → `peek_sender` → keyring lookup → `AnySignedEnvelope::decode` → cross-check recipient → `fsm_input_from_envelope` → `task_fsm.step` → `transport.send` response.
- D-D4: `ack` is wire-level only — does NOT call `TaskFsm::step`. Runtime decodes, sig-verifies, logs trace, returns. This is locked; Phase 2 FSM is NOT reopened.
- D-D5: Sender cross-check: BOTH (1) signature valid under pinned key for `envelope.from()`, AND (2) envelope `to_principal()` matches transport `msg.recipient`. VERIFIED: `SignedEnvelope<B>` exposes `to_principal()` [VERIFIED: crates/famp-envelope/src/envelope.rs]. Cross-check is MANDATORY in Phase 3 — mismatch → `RuntimeError::RecipientMismatch`. D-D5 research conclusion: `to` field IS present.
- D-D6: Adversarial injection: `#[cfg(feature = "test-util")] pub async fn send_raw_for_test(...)` on `MemoryTransport`. Only accessible via `famp-transport = { features = ["test-util"] }` in `[dev-dependencies]` of `crates/famp/Cargo.toml`.
- D-D7: CONF-07 fixture: hand-crafted JSON with a valid Ed25519 signature computed over DIFFERENT canonical bytes than the on-wire payload. Pre-generated bytes committed at `crates/famp/tests/fixtures/conf-07-canonical-divergence.json`.
- D-D8: `RuntimeError` enum with distinct variants for each adversarial case (see Architecture Patterns section).

**E. Example Binary**
- D-E1: `crates/famp/examples/personal_two_agents.rs` — two principals `agent:local/alice` and `agent:local/bob`, keypairs generated with `rand`, one `MemoryTransport`, two `Keyring`s pre-pinned, two `tokio::spawn`ed tasks, happy-path `request → commit → deliver → ack`.
- D-E2: Trace format: `[seq] SENDER → RECIPIENT: CLASS (state: FROM → TO)` lines, in wire order.
- D-E3: Integration test at `crates/famp/tests/example_happy_path.rs` — spawns example as subprocess, asserts exit-code 0 and expected trace lines. CI gate.

### Claude's Discretion
- Exact file layout inside `crates/famp-keyring/src/` (`lib.rs` + `file_format.rs` + `error.rs` or flat).
- Exact file layout inside `crates/famp/src/runtime/` (one module or several).
- Whether `Keyring::load_from_file` returns `(Self, Vec<Warning>)` or hard-errors first — prefer hard-error for v0.7.
- Exact name of the envelope→FSM adapter function (`fsm_input_from_envelope` vs `derive_fsm_input`).
- Whether `TransportMessage` is `Clone` — prefer owned `Vec<u8>` for v0.7.
- Exact `[features]` table wording in `famp-transport/Cargo.toml`.
- Whether adversarial tests live in one `adversarial.rs` or three files — one file preferred.
- Whether CONF-07 fixture is pre-generated or generated at test time — pre-generated preferred.

### Deferred Ideas (OUT OF SCOPE)
- HTTP transport, TLS, axum, reqwest. Phase 4.
- `.well-known` Agent Card (TRANS-05). v0.8.
- Cancellation-safe spawn-channel send path (TRANS-08). v0.9.
- Pluggable `TrustStore` trait, federation credentials. v0.8+.
- Key rotation, multi-key principals. v0.9+.
- `dyn Transport` / trait objects. Add only when a concrete caller needs it.
- Bounded-channel backpressure. Revisit for high-throughput transport.
- Inline trailing comments in keyring file format. v0.8+ if ops asks.
- `stateright` for transport layer model checking. v0.14.
- `HttpTransport` + rustls. Phase 4.
- `famp keygen`/`famp serve` CLI subcommands. v0.8+ CLI milestone.
- Keyring auto-rotation/TOFU-override. Pinning is sticky through v0.7.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TRANS-01 | `famp-transport` crate with `Transport` trait (async send + incoming stream) | D-C1/C5: native AFIT, byte-oriented, principal-addressed; `tokio::sync::mpsc` backbone |
| TRANS-02 | `MemoryTransport` in-process impl (~50 LoC), dev-dep | D-C4/C6: unbounded mpsc channels per-principal, register+send+recv; test-util feature flag |
| KEY-01 | `HashMap<Principal, VerifyingKey>` keyring (reinterpreted per D-A1) | `famp_core::Principal` is the key; `famp_crypto::TrustedVerifyingKey` is the value |
| KEY-02 | Keyring load/save from local file, format committed and round-trip tested | D-B1/B2/B5: deterministic sort, two-space separator, `\n`, fixture at `tests/fixtures/two_peers.keyring` |
| KEY-03 | CLI-flag bootstrap `--peer agent:<auth>/<name>=<pubkey>` | D-B4: `=` separator, `with_peer` merge, conflict rejects |
| EX-01 | `famp/examples/personal_two_agents.rs` — `request → commit → deliver → ack` over `MemoryTransport`, typed trace, exit 0 | D-E1/E2/E3: two spawned tasks, pre-pinned keyrings, subprocess test |
| CONF-03 | Happy-path two-node integration over `MemoryTransport` | Covered by EX-01 + `example_happy_path.rs` integration test |
| CONF-05 | Unsigned message rejected (MemoryTransport) | `EnvelopeDecodeError::MissingSignature` → `RuntimeError::Decode`; raw injection via `send_raw_for_test` |
| CONF-06 | Wrong-key signature rejected (MemoryTransport) | `EnvelopeDecodeError::SignatureInvalid` → `RuntimeError::Decode`; inject signed with unknown key |
| CONF-07 | Canonicalization divergence detected (MemoryTransport) | `RuntimeError::CanonicalDivergence`; pre-generated fixture bytes, re-canonicalize path |

</phase_requirements>

---

## Summary

Phase 3 composes four independently tested components — `famp-transport`, `famp-keyring`, runtime glue in `crates/famp/src/runtime/`, and the `personal_two_agents` example — using exclusively the crate APIs that Phases 1 and 2 have already shipped. The research confirms that no new external crate dependencies are needed: `tokio::sync::mpsc::unbounded_channel` (already in workspace) provides the `MemoryTransport` backbone; `famp_crypto::TrustedVerifyingKey` (already exists) is the correct keyring value type — the CONTEXT.md D-A2 should be read as "reuse this type," not "create a new one"; `famp_crypto::TrustedVerifyingKey::from_b64url` is already the correct ingress parser for keyring file loading.

A critical architectural constraint is verified: `AnySignedEnvelope::decode(bytes, verifier)` takes a `TrustedVerifyingKey` as input. This means the runtime glue must extract the `from: Principal` from raw bytes BEFORE calling decode (to do the keyring lookup), then pass the pinned key into decode. This requires a lightweight `peek_sender(bytes: &[u8]) -> Result<Principal, RuntimeError>` helper in the runtime module — a ~5-line JSON inspection step. This is not an API gap; it is the intended sequence and prevents the transport layer from ever seeing key material.

The D-D5 question is definitively resolved: `SignedEnvelope<B>` exposes `to_principal() -> &Principal` [VERIFIED: `crates/famp-envelope/src/envelope.rs`], so recipient cross-check is mandatory and straightforward. CONF-05 will surface as `RuntimeError::Decode(EnvelopeDecodeError::MissingSignature)` — the envelope decode layer enforces INV-10 before the runtime's keyring lookup even runs.

**Primary recommendation:** Sequence plans as (1) `famp-transport` + `MemoryTransport` + `test-util` feature, (2) `famp-keyring` + file format + round-trip fixture, (3) runtime glue including `peek_sender` + adversarial test matrix, (4) example binary + subprocess CI test.

---

## Standard Stack

### Core (all already in workspace — no new crates)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio::sync::mpsc` | 1.51.1 (workspace) | `UnboundedSender/Receiver` per-principal inbox | Already workspace dep; only viable async runtime in this ecosystem |
| `famp_crypto::TrustedVerifyingKey` | workspace | Keyring value type (ingress-vetted pubkey newtype) | Already enforces weak-key rejection, b64url decode, 32-byte length; reuse, don't re-implement |
| `famp_core::Principal` | workspace | Routing key, keyring HashMap key, envelope `from`/`to` | Locked identity type per D-A1 |
| `famp_envelope::AnySignedEnvelope` | workspace | Runtime dispatch on decoded wire bytes | Locked decode path from Phase 1 |
| `famp_fsm::TaskFsm`, `TaskTransitionInput` | workspace | FSM stepping in runtime glue | Locked from Phase 2 |
| `thiserror 2.0.18` | workspace | `KeyringError`, `MemoryTransportError`, `RuntimeError` | Mandatory for all library errors |
| `rand` | needs workspace addition | Keypair generation in `personal_two_agents.rs` example binary | Standard; `FampSigningKey::from_bytes([u8; 32])` needs random seed |

[VERIFIED: Cargo.toml workspace dependencies checked 2026-04-13]

### Note on `rand` dependency

The example binary generates keypairs at startup (`rand` → `OsRng`). `rand` is not yet in the workspace. The planner must add it to `[workspace.dependencies]` and to `crates/famp/[dev-dependencies]` (or `[dependencies]` of the example). Check if `ed25519-dalek 2.2.0` re-exports a rand-compatible interface via the `rand_core` feature.

[VERIFIED: `ed25519-dalek = { version = "2.2.0", default-features = false, features = ["std", "zeroize"] }` in Cargo.toml — `rand_core` feature is NOT currently enabled. The example can either: (a) add `rand` + enable `rand_core` feature on `ed25519-dalek`, or (b) use a fixed test seed. CONTEXT.md D-E1 says "generated with `rand` inside the binary." Planner must add `rand` to workspace deps.]

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` | not in workspace | CLI flag `--peer` parsing in example binary | Only needed if the example binary accepts CLI args; check CONTEXT.md D-B4 — yes, KEY-03 requires it |
| `tempfile` | not in workspace | Keyring round-trip test (save to temp, compare) | Only in `famp-keyring` dev-dependencies |

[ASSUMED: `clap` version — workspace does not currently include it. If KEY-03 is only exercised via unit tests (not a real CLI flag), `clap` may not be needed in Phase 3. The example `personal_two_agents.rs` per D-E1 uses pre-pinned keyrings via `with_peer`, not CLI flags. KEY-03 CLI path may be a simpler struct-based argument parser in the example rather than full `clap`. Planner should decide: implement `--peer` via `clap`, or test KEY-03 via unit tests on `Keyring::with_peer` directly.]

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tokio::sync::mpsc::unbounded_channel` | `bounded` channel | Bounded adds backpressure; overkill for ≤20 messages in one binary; CONTEXT.md D-C4 explicitly chose unbounded |
| Native AFIT | `async-trait` crate | `async-trait` adds a macro dep and boxes futures; AFIT is stable on 1.75+, workspace pins 1.87; CONTEXT.md D-C5 mandates native AFIT |
| `famp_crypto::TrustedVerifyingKey` as keyring value | raw `ed25519_dalek::VerifyingKey` | Raw key bypasses weak-key rejection and base64url ingress checks; `TrustedVerifyingKey` is already the ingress-vetted newtype |
| `BTreeMap` for keyring save ordering | `HashMap` + sort-on-save | `BTreeMap` would enforce ordering automatically; CONTEXT.md D-A3 uses `HashMap` (matching the runtime lookup pattern) with sort-on-save; either works |

---

## Architecture Patterns

### Crate Dependency Graph (Phase 3 additions)

```
famp (top crate — examples/, src/runtime/)
  ├── famp-core              (Principal, MessageClass, TerminalStatus)
  ├── famp-canonical         (canonicalize — used in CONF-07 path)
  ├── famp-crypto            (TrustedVerifyingKey, FampSigningKey, verify_value)
  ├── famp-envelope          (AnySignedEnvelope, SignedEnvelope<B>, EnvelopeDecodeError)
  ├── famp-fsm               (TaskFsm, TaskTransitionInput, TaskFsmError)
  ├── famp-transport         (Transport trait, TransportMessage, MemoryTransport)  ← NEW
  └── famp-keyring           (Keyring, KeyringError)                               ← NEW

famp-transport
  ├── famp-core              (Principal — routing address only)
  └── tokio                  (mpsc::unbounded_channel)
  # ZERO deps on famp-envelope, famp-keyring, famp-fsm

famp-keyring
  ├── famp-core              (Principal — HashMap key, FromStr parser)
  └── famp-crypto            (TrustedVerifyingKey — ingress-vetted value type)
  # ZERO deps on famp-transport, famp-envelope, famp-fsm
```

[VERIFIED: actual workspace layout from `Cargo.toml` and `crates/famp-transport/Cargo.toml` checked 2026-04-13]

### Pattern 1: Transport Trait with Native AFIT

```rust
// crates/famp-transport/src/lib.rs
// Source: CONTEXT.md D-C1; Rust 1.75+ stable AFIT

pub struct TransportMessage {
    pub sender: Principal,
    pub recipient: Principal,
    pub bytes: Vec<u8>,
}

pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn send(&self, msg: TransportMessage) -> Result<(), Self::Error>;
    async fn recv(&self, as_principal: &Principal) -> Result<TransportMessage, Self::Error>;
}
```

**What:** Byte-oriented, principal-addressed, no envelope semantics. `type Error` associated type (not a generic bound) allows each implementation to expose a typed error without boxing.

**When to use:** Every transport implementation — `MemoryTransport` now, `HttpTransport` in Phase 4 — implements this exact trait.

**Key constraint:** No `dyn Transport` in Phase 3. Both `MemoryTransport` and any future transport are used as concrete types in generic contexts. Phase 4 may add a type-erased path if needed.

### Pattern 2: MemoryTransport Inbox Hub

```rust
// crates/famp-transport/src/memory.rs
// Source: CONTEXT.md D-C4

use famp_core::Principal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use super::{Transport, TransportMessage};

pub struct MemoryTransport {
    senders: Arc<Mutex<HashMap<Principal, mpsc::UnboundedSender<TransportMessage>>>>,
    // receivers held per-principal; recv() polls the principal's receiver
    // Option<Receiver> or a second HashMap<Principal, Receiver>
}
```

**LoC budget:** ~50 LoC for `struct MemoryTransport` + `impl Transport for MemoryTransport` (send + recv). `register(principal)` and `send_raw_for_test` are additional methods outside this count (per D-C6).

**Receiver storage challenge:** `mpsc::UnboundedReceiver` is not `Clone`. The `recv` method needs exclusive access to the receiver for a given principal. One approach: `Arc<Mutex<HashMap<Principal, mpsc::UnboundedReceiver<...>>>>` — `recv` locks, takes the receiver out, awaits it, then re-inserts. Alternatively use `Arc<tokio::sync::Mutex<...>>` and hold the lock across the await (acceptable for in-process, no deadlock risk in single-task examples). Planner decides exact receiver storage shape.

### Pattern 3: Keyring API

```rust
// crates/famp-keyring/src/lib.rs
// Source: CONTEXT.md D-A3

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::collections::HashMap;

pub struct Keyring {
    map: HashMap<Principal, TrustedVerifyingKey>,
}

impl Keyring {
    pub fn new() -> Self;
    pub fn load_from_file(path: &Path) -> Result<Self, KeyringError>;
    pub fn save_to_file(&self, path: &Path) -> Result<(), KeyringError>;
    pub fn with_peer(self, p: Principal, k: TrustedVerifyingKey) -> Result<Self, KeyringError>;
    pub fn get(&self, p: &Principal) -> Option<&TrustedVerifyingKey>;
    pub fn pin_tofu(&mut self, p: Principal, k: TrustedVerifyingKey) -> Result<(), KeyringError>;
}
```

**Save ordering:** Collect `map.iter()` into a `Vec`, sort by `principal.to_string()`, emit each as `{principal}  {key_b64url}\n`. Two-space separator is locked (D-B5).

### Pattern 4: Runtime Glue — Two-Phase Decode

**Critical finding:** `AnySignedEnvelope::decode(bytes, verifier)` requires a `TrustedVerifyingKey` upfront. The sender's identity must be extracted from raw bytes BEFORE decode so the runtime can look up the pinned key in the keyring. This requires a `peek_sender` helper:

```rust
// crates/famp/src/runtime/mod.rs
// Source: verified from crates/famp-envelope/src/dispatch.rs

fn peek_sender(bytes: &[u8]) -> Result<Principal, RuntimeError> {
    let value: serde_json::Value = famp_canonical::from_slice_strict(bytes)
        .map_err(|e| RuntimeError::Decode(EnvelopeDecodeError::MalformedJson(e)))?;
    let from_str = value.get("from")
        .and_then(|v| v.as_str())
        .ok_or(RuntimeError::Decode(EnvelopeDecodeError::MissingField { field: "from" }))?;
    Principal::from_str(from_str)
        .map_err(|_| RuntimeError::Decode(EnvelopeDecodeError::MissingField { field: "from" }))
}
```

Full runtime loop:
```rust
// Source: CONTEXT.md D-D3

async fn run_runtime_loop<T: Transport>(
    me: &Principal,
    transport: &T,
    keyring: &Keyring,
    task_fsm: &mut TaskFsm,
    signing_key: &FampSigningKey,
) -> Result<(), RuntimeError> {
    loop {
        let msg = transport.recv(&me).await
            .map_err(|e| RuntimeError::Transport(Box::new(e)))?;

        // Phase 1: peek sender from raw bytes (keyring lookup)
        let sender = peek_sender(&msg.bytes)?;
        let pinned = keyring.get(&sender)
            .ok_or(RuntimeError::UnknownSender(sender.clone()))?;

        // Phase 2: decode + verify (verify_strict runs inside AnySignedEnvelope::decode)
        let env = AnySignedEnvelope::decode(&msg.bytes, pinned)
            .map_err(RuntimeError::Decode)?;

        // Phase 3: cross-check transport recipient vs envelope recipient
        let env_to = env.to_principal();
        if env_to != &msg.recipient {
            return Err(RuntimeError::RecipientMismatch {
                transport: msg.recipient,
                envelope: env_to.clone(),
            });
        }

        // Phase 4: FSM step (ack is NOT stepped)
        if env.class() != MessageClass::Ack {
            let input = fsm_input_from_envelope(&env)?;
            task_fsm.step(input).map_err(RuntimeError::Fsm)?;
        }

        // Phase 5: log trace, emit response if needed
    }
}
```

**CONF-07 canonical divergence detection:** `AnySignedEnvelope::decode` calls `verify_value` internally [VERIFIED: `crates/famp-envelope/src/envelope.rs` `decode_value` → `verify_value`], which re-canonicalizes the received bytes and verifies the signature over the canonical form. A CONF-07 message has a valid signature over DIFFERENT canonical bytes — `verify_value` produces `CryptoError::VerificationFailed`, which surfaces as `EnvelopeDecodeError::SignatureInvalid`, which maps to `RuntimeError::Decode(EnvelopeDecodeError::SignatureInvalid)`.

**Wait — CONF-06 and CONF-07 both surface as `EnvelopeDecodeError::SignatureInvalid`?**

Yes, at the `EnvelopeDecodeError` level. The distinction between CONF-06 (wrong key) and CONF-07 (canonical divergence) is observable at the TEST CONSTRUCTION level (different injection methods), not necessarily at the error variant level — both fail signature verification. The CONTEXT.md D-D8 lists `RuntimeError::CanonicalDivergence` as a distinct variant, which means the runtime glue needs to DISTINGUISH between the two BEFORE calling decode. One approach: for CONF-07 specifically, the runtime can attempt re-canonicalization of the raw bytes and compare, then produce `CanonicalDivergence` if canonical form differs from what was signed. Alternatively, decode can be extended. This is an open question that the planner must resolve — see Open Questions.

### Pattern 5: `RuntimeError` Enum

```rust
// crates/famp/src/runtime/error.rs
// Source: CONTEXT.md D-D8

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown sender: {0}")]
    UnknownSender(Principal),

    #[error("envelope decode or signature verification failed")]
    Decode(#[source] famp_envelope::EnvelopeDecodeError),

    #[error("canonicalization divergence detected")]
    CanonicalDivergence,

    #[error("transport recipient {transport} does not match envelope recipient {envelope}")]
    RecipientMismatch { transport: Principal, envelope: Principal },

    #[error("transport error")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("keyring error")]
    Keyring(#[source] famp_keyring::KeyringError),

    #[error("fsm error")]
    Fsm(#[source] famp_fsm::TaskFsmError),
}
```

**CONF-05 mapping:** Unsigned message → `decode` returns `EnvelopeDecodeError::MissingSignature` → `RuntimeError::Decode(MissingSignature)`. This is INV-10 enforced by the envelope layer — CONF-05 literally cannot bypass the decode path.

**CONF-06 mapping:** Wrong-key signature → `decode` returns `EnvelopeDecodeError::SignatureInvalid` → `RuntimeError::Decode(SignatureInvalid)`.

**CONF-07 mapping:** Must be `RuntimeError::CanonicalDivergence`. See Open Question #1 for how to distinguish this from CONF-06.

### Pattern 6: `KeyringError` Enum

```rust
// crates/famp-keyring/src/error.rs
// Source: CONTEXT.md D-B2/D-B6

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("duplicate principal at line {line}: {principal}")]
    DuplicatePrincipal { principal: Principal, line: usize },

    #[error("duplicate pubkey at line {line}: already pinned to {existing}")]
    DuplicatePubkey { existing: Principal, line: usize },

    #[error("malformed entry at line {line}: {reason}")]
    MalformedEntry { line: usize, reason: String },

    #[error("conflict: principal {principal} already pinned to a different key")]
    KeyConflict { principal: Principal },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {0}")]
    Crypto(#[from] famp_crypto::CryptoError),
}
```

### Pattern 7: `MemoryTransportError` Enum

```rust
// crates/famp-transport/src/memory.rs
// Source: CONTEXT.md D-C7

#[derive(Debug, thiserror::Error)]
pub enum MemoryTransportError {
    #[error("unknown recipient: {principal}")]
    UnknownRecipient { principal: Principal },

    #[error("inbox closed for: {principal}")]
    InboxClosed { principal: Principal },
}
```

### Anti-Patterns to Avoid

- **Signature verification inside `Transport` trait or `MemoryTransport`:** Would couple transport to `famp-envelope` and `famp-keyring`. Rejected per D-C3.
- **`async-trait` crate:** Not needed; native AFIT is stable on 1.75+. Workspace pins 1.87. Adding `async-trait` would create a macro dependency for no gain.
- **New `famp-runtime` crate:** Orchestration lives in `crates/famp/src/runtime/`. A separate crate would add a workspace member with no clear API boundary — D-D1 explicitly rejects this.
- **`ProtocolErrorKind` inside `famp-keyring` or `famp-transport`:** Phase-local narrow error enums only. Conversion to `ProtocolErrorKind` happens at the `crates/famp/src/runtime/` boundary.
- **Re-implementing base64url decode for keyring file parsing:** `TrustedVerifyingKey::from_b64url` in `famp_crypto` already does strict decode + weak-key rejection. Call it directly.
- **Calling `verify_value` directly in runtime glue to distinguish CONF-06 from CONF-07:** This would duplicate the decode logic. See Open Question #1 for the correct approach.
- **`bounded` mpsc channels in MemoryTransport:** Adds backpressure complexity for a personal-profile binary with ≤20 messages. D-C4 mandates unbounded.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Base64url strict decode for pubkeys | Custom b64 parser | `famp_crypto::TrustedVerifyingKey::from_b64url` | Already handles strict decode, padding rejection, non-URL-safe alphabet rejection, weak-key check |
| `Principal` string parsing in keyring loader | Custom parser | `Principal::from_str` from `famp_core` | Already handles `agent:` prefix, authority validation, name validation |
| Signature verification in runtime | Direct dalek call | `AnySignedEnvelope::decode` (calls `verify_value` → `verify_strict` internally) | Phase 1 enrolled the domain-separation prefix; bypassing decode would lose scope/class validation |
| Async channel infrastructure | Custom message queue | `tokio::sync::mpsc::unbounded_channel` | Battle-tested, wakes futures efficiently, already in workspace |
| Test key generation in example | Hand-rolled PRNG | `FampSigningKey::from_bytes(seed)` with a fixed test seed or `rand::rngs::OsRng` | Seed-based generation is reproducible; OsRng is the correct choice for "real" ephemeral keys |
| Canonical JSON in runtime | Call `serde_json::to_vec` | `famp_canonical::canonicalize` | RFC 8785 sort order is non-trivial; wrong impl produces CONF-07-style divergence silently |

**Key insight:** Every piece of infrastructure needed in Phase 3 already exists in the workspace. The phase's job is composition, not new primitives.

---

## Runtime State Inventory

Phase 3 is a greenfield composition phase — no rename, refactor, or migration. This section is omitted.

---

## Common Pitfalls

### Pitfall 1: Receiver Ownership in `MemoryTransport::recv`

**What goes wrong:** `mpsc::UnboundedReceiver<T>` is not `Clone` and cannot be shared. Calling `recv(&self, as_principal)` with `&self` (shared reference) means the receiver must live behind a lock. If the receiver is stored as `Arc<Mutex<Option<UnboundedReceiver<...>>>>`, concurrent `recv` calls for the same principal will see `None` after the first caller takes the receiver.

**Why it happens:** The design stores receivers and senders in the same `HashMap`. `recv` must obtain exclusive access to the specific principal's receiver without locking the entire map for the full await duration (which would deadlock other principals' sends).

**How to avoid:** Two options:
1. Store senders and receivers in SEPARATE `Arc<Mutex<HashMap<...>>>` structures. `recv` locks the receiver map, removes the receiver, awaits it (outside the lock), then re-inserts it.
2. Use `Arc<tokio::sync::Mutex<UnboundedReceiver<...>>>` per-principal so the lock can be held across an await. This is safe in single-task examples but may cause issues if two `recv` callers race on the same principal.

In a personal-profile example with one task per principal, option 2 (per-principal receiver mutex) is cleanest and avoids the remove/re-insert dance.

**Warning signs:** `cannot borrow as mutable because it is also borrowed as immutable` during `recv`; or `recv` returning `None` on the second call.

### Pitfall 2: CONF-06 vs CONF-07 Distinction

**What goes wrong:** Both CONF-06 (wrong key) and CONF-07 (canonical divergence) produce `EnvelopeDecodeError::SignatureInvalid` from `AnySignedEnvelope::decode`. If the runtime maps all `SignatureInvalid` to `RuntimeError::Decode`, the two adversarial cases are indistinguishable — CONTEXT.md D-D8 requires `RuntimeError::CanonicalDivergence` as a distinct variant for CONF-07.

**Why it happens:** `verify_value` canonicalizes the received bytes and checks the signature — it does not separately report "canonical form mismatch" vs "wrong key."

**How to avoid:** The CONF-07 fixture is constructed so that the wire bytes are VALID JSON but their canonical form differs from the bytes that were signed. The runtime glue can detect this BEFORE decode: re-canonicalize the raw bytes, compare to the wire bytes. If they differ, emit `RuntimeError::CanonicalDivergence` immediately (the signature would fail anyway). This is a ~5-line pre-decode check. See Open Question #1 for the exact implementation choice.

**Warning signs:** CONF-07 test asserting `RuntimeError::CanonicalDivergence` but getting `RuntimeError::Decode(SignatureInvalid)`.

### Pitfall 3: `TrustedVerifyingKey` Already Exists — Don't Create a Second Newtype

**What goes wrong:** CONTEXT.md D-A2 says "constructing a `TrustedVerifyingKey` is the only way a key enters the trust boundary." A naive reading might lead to creating a NEW `TrustedVerifyingKey` in `famp-keyring`. But `famp_crypto::TrustedVerifyingKey` already IS that ingress-vetted newtype with weak-key rejection.

**Why it happens:** D-A2 describes the semantic, not the type location.

**How to avoid:** `famp-keyring` depends on `famp-crypto` and uses `famp_crypto::TrustedVerifyingKey` directly as the keyring value type. No new wrapper type.

### Pitfall 4: AFIT Trait Bounds in Generic Contexts

**What goes wrong:** Functions like `fn run_runtime_loop<T: Transport>(...)` will work, but error messages involving trait bounds in async contexts can be confusing. A `Send` bound may be required if the `T: Transport` value crosses a `.await` in a `tokio::spawn` context.

**Why it happens:** `async fn` in traits generates an associated `Future` type; for `tokio::spawn`, the future must be `Send`.

**How to avoid:** Add `where T: Transport + Send + Sync` in the runtime glue's generic bounds. `MemoryTransport` will derive `Send + Sync` if its inner `Arc<Mutex<...>>` fields are `Send`. `Mutex` from `std::sync` is `Send`; `tokio::sync::Mutex` is also `Send`.

**Warning signs:** `the trait bound T: Send is not satisfied` at `tokio::spawn`.

### Pitfall 5: Keyring Save Ordering — Must Be Deterministic

**What goes wrong:** Iterating a `HashMap` produces non-deterministic order. If `save_to_file` iterates the map directly, the round-trip test (`load → save → byte-compare`) will fail intermittently.

**Why it happens:** `HashMap` iteration order is randomized in Rust by default (SipHash with a random seed).

**How to avoid:** In `save_to_file`, collect keys into a `Vec<&Principal>`, sort by `principal.to_string()`, then emit lines in sorted order. This is the locked format per D-B5.

### Pitfall 6: `famp-transport` Test-Util Feature Gate

**What goes wrong:** If `send_raw_for_test` is accidentally available in production builds (e.g., because `famp-transport` appears in `[dependencies]` without feature filtering), the adversarial injection path is reachable in production.

**Why it happens:** Cargo feature flags are additive. If `crates/famp/Cargo.toml` includes `famp-transport` in `[dependencies]` with `features = ["test-util"]`, all builds get the test path.

**How to avoid:** `crates/famp/Cargo.toml` must have:
```toml
[dependencies]
famp-transport = { path = "../famp-transport" }  # NO test-util here

[dev-dependencies]
famp-transport = { path = "../famp-transport", features = ["test-util"] }
```

In `famp-transport/Cargo.toml`:
```toml
[features]
test-util = []
```

Gate with `#[cfg(feature = "test-util")]` on the `send_raw_for_test` method.

---

## Code Examples

### Keyring File Format (canonical two-peer fixture)

```
# FAMP v0.7 TOFU keyring
# One entry per principal; principal is agent:<authority>/<name>
# pubkey is base64url-unpadded 32-byte Ed25519 verifying key
agent:local/alice  11qYAY7gqW8nRGajN3MiST7fMSIlcvTkBY7K4Pmx5MQ
agent:local/bob    nWGxne_9WmC3IwkRHLFzRXcnc3xnEp_QD6JaVIPZWWU
```

Rules: entries sorted alphabetically by principal string, two-space separator, trailing `\n` on file, `#` lines are comments, blank lines ignored on read. [VERIFIED: D-B1/D-B5]

### CONF-07 Fixture Construction

```python
# Pre-generation script (Python, run once, output committed)
# Wire bytes: valid JSON with a signature computed over DIFFERENT canonical bytes
# The key insight: sign {"b": 1, "a": 2} (non-canonical), embed in wire as {"a": 2, "b": 1}
# canonical(wire) = {"a":2,"b":1} ≠ canonical(signed-payload) = {"a":2,"b":1} ... hmm

# Better: sign {"a": 1, "extra": null} but strip "extra" from wire payload
# canonical(wire) = {"a":1} ≠ {"a":1,"extra":null} = canonical(signed-payload)
# → signature was over canonical({"a":1,"extra":null}) but wire is canonical({"a":1})
# → verify_value on wire bytes → sig check fails → SignatureInvalid (NOT CanonicalDivergence)
```

Actually the CONF-07 fixture needs to produce divergence that the runtime catches as `CanonicalDivergence` rather than just `SignatureInvalid`. See Open Question #1 for the design.

### Adversarial Test Shape

```rust
// crates/famp/tests/adversarial.rs
// Source: CONTEXT.md D-D6

#[tokio::test]
async fn conf_05_unsigned_rejected() {
    // Build a raw JSON blob with no "signature" field
    let raw = b"{\"famp\":\"0.5.1\",\"class\":\"request\",...}"; // no signature field
    let (transport, keyring, mut fsm) = setup_two_agents();
    transport.send_raw_for_test(TransportMessage {
        sender: alice(),
        recipient: bob(),
        bytes: raw.to_vec(),
    }).await.unwrap();
    let result = run_single_recv(&bob(), &transport, &keyring, &mut fsm).await;
    assert!(matches!(
        result,
        Err(RuntimeError::Decode(EnvelopeDecodeError::MissingSignature))
    ));
}

#[tokio::test]
async fn conf_06_wrong_key_rejected() {
    // Sign with alice's key, but bob's keyring only knows carol's key for alice
    let msg = sign_envelope_with_wrong_key();
    let result = run_single_recv(&bob(), &transport, &keyring, &mut fsm).await;
    assert!(matches!(
        result,
        Err(RuntimeError::Decode(EnvelopeDecodeError::SignatureInvalid))
    ));
}

#[tokio::test]
async fn conf_07_canonical_divergence_rejected() {
    let bytes = include_bytes!("fixtures/conf-07-canonical-divergence.json");
    // ... inject via send_raw_for_test ...
    assert!(matches!(result, Err(RuntimeError::CanonicalDivergence)));
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `async-trait` macro for async fns in traits | Native AFIT (stable) | Rust 1.75 (Nov 2023) | Removes proc-macro dep; workspace 1.87 makes this the default |
| `VerifyingKey` direct as keyring value | `TrustedVerifyingKey` newtype (ingress-vetted) | v0.6 crypto foundations | Weak-key rejection enforced at ingress, not forgotten at use sites |
| Generic `Error` or `Box<dyn Error>` in library APIs | Phase-local narrow error enums + `thiserror 2.x` | v0.6 pattern established | Typed match at call sites; compiler catches unhandled error variants |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `clap` crate is needed for KEY-03 `--peer` CLI flag in example binary | Standard Stack | If KEY-03 is exercised only via unit tests on `Keyring::with_peer`, `clap` is not needed in Phase 3; low risk |
| A2 | CONF-07 requires a pre-decode canonicalization check to produce `RuntimeError::CanonicalDivergence` vs `Decode(SignatureInvalid)` | Common Pitfalls / Open Questions | High risk — if the planner does not add this check, CONF-07 and CONF-06 are indistinguishable at the error level; adversarial tests will fail |
| A3 | `rand` crate must be added to workspace deps for `personal_two_agents.rs` keypair generation | Standard Stack | If example uses fixed seeds instead, `rand` is not needed; low risk to functionality, medium risk to example realism |

**If this table is empty:** All claims in this research were verified or cited. In this case: A1, A2, A3 require planner decisions.

---

## Open Questions

1. **How to produce `RuntimeError::CanonicalDivergence` as a DISTINCT variant from `RuntimeError::Decode(SignatureInvalid)` (CONF-07)**

   **What we know:** `AnySignedEnvelope::decode` returns `EnvelopeDecodeError::SignatureInvalid` for BOTH wrong-key and canonical-divergence scenarios, because both fail `verify_strict`. `verify_strict` does not report "which" check failed.

   **What's unclear:** How does the runtime glue distinguish CONF-07 (canonical divergence) from CONF-06 (wrong key) to emit the correct `RuntimeError` variant?

   **Three options:**
   - **Option A (pre-decode check):** Before calling `decode`, re-canonicalize the raw bytes with `famp_canonical::canonicalize`. If `from_slice_strict(bytes)` parses to a `Value` whose canonical form differs from `bytes`, emit `RuntimeError::CanonicalDivergence`. Then call decode for the normal path. This catches CONF-07 before signature verification runs.
   - **Option B (post-decode heuristic):** Add a `verify_canonical` step after decode that re-canonicalizes and checks. Only emits `CanonicalDivergence` if the bytes pass parse but fail this check. Complex; ordering matters.
   - **Option C (extended `EnvelopeDecodeError`):** Add a `CanonicalDivergence` variant to `EnvelopeDecodeError` in `famp-envelope`. This changes a Phase 1 API. CONTEXT.md deferred.md says "DO NOT retroactively modify Phase 1 body schemas" — but error enums are not body schemas. Still, changing `famp-envelope` in Phase 3 is a risk.

   **Recommendation:** Option A. Pre-decode canonicalization check is ~5 lines, does not modify any Phase 1 or Phase 2 APIs, and correctly identifies the CONF-07 condition at the point where it is semantically meaningful (bytes-on-wire differ from canonical form). The CONF-07 fixture is designed to contain valid JSON bytes whose canonical form differs from what was signed — the check fires before decode.

2. **Receiver storage in `MemoryTransport` — separate maps vs per-principal mutex**

   **What we know:** `UnboundedReceiver` is not `Clone`. `recv(&self, as_principal)` needs exclusive access to the receiver.

   **Recommendation:** Separate `senders: Arc<Mutex<HashMap<Principal, Sender>>>` and `receivers: Arc<Mutex<HashMap<Principal, Receiver>>>`. `recv` locks the receiver map, calls `.get_mut()`, and calls `.recv().await` on the mutable reference. This avoids holding the lock across the await if using a try-receive + notify pattern, OR simply holds the `tokio::sync::Mutex` (not `std::sync::Mutex`) across the await which is fine for async. Use `tokio::sync::Mutex` for the receiver map to allow lock-across-await safely.

3. **`rand` and keypair generation in `personal_two_agents.rs`**

   **Recommendation:** Add `rand = "0.9"` to workspace deps (check compatibility with `ed25519-dalek 2.2.0` which uses `rand_core 0.6` — note potential version mismatch). Alternatively, use `FampSigningKey::from_bytes(OsRng.gen())` — but this requires enabling the `rand_core` feature on `ed25519-dalek`. The simplest approach for Phase 3: add `rand_core = { version = "0.6", features = ["getrandom"] }` to `crates/famp` dev-deps and generate keys via `OsRng`.

   [ASSUMED: `rand_core 0.6` with `getrandom` feature is the right path. Verify against `ed25519-dalek 2.2.0` feature requirements.]

---

## Environment Availability

Step 2.6: SKIPPED for core library development — no external services, databases, or CLI tools required beyond the Rust toolchain. The `cargo-nextest` test runner is already confirmed in the workspace Justfile. No new external dependencies.

---

## Validation Architecture

**Framework:** `cargo nextest` (0.9.132, already in workspace)
**Config:** `rust-version = "1.87"` pinned in `Cargo.toml`
**Quick run:** `cargo nextest run -p famp-transport -p famp-keyring`
**Full suite:** `cargo nextest run --workspace` + `cargo test --workspace --doc`

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Location |
|--------|----------|-----------|-------------------|--------------|
| TRANS-01 | `Transport` trait compiles; `MemoryTransport` implements it | smoke + compile | `cargo nextest run -p famp-transport` | `crates/famp-transport/tests/` |
| TRANS-02 | `MemoryTransport` send+recv round-trip; ~50 LoC implementation | integration | `cargo nextest run -p famp-transport` | `crates/famp-transport/tests/memory_transport.rs` |
| KEY-01 | `Keyring::new()` + `with_peer` + `get` semantics; Principal is key, `TrustedVerifyingKey` is value | unit | `cargo nextest run -p famp-keyring` | `crates/famp-keyring/tests/keyring.rs` |
| KEY-02 | Load → save → byte-compare round-trip on `two_peers.keyring` fixture | integration | `cargo nextest run -p famp-keyring` | `crates/famp-keyring/tests/roundtrip.rs` |
| KEY-03 | `with_peer` accepts valid `--peer`-style entries; conflict rejects | unit | `cargo nextest run -p famp-keyring` | `crates/famp-keyring/tests/keyring.rs` |
| EX-01 | Example binary exits 0, prints expected trace lines | integration | `cargo nextest run -p famp` (example_happy_path.rs subprocess test) | `crates/famp/tests/example_happy_path.rs` |
| CONF-03 | `request → commit → deliver → ack` completes over `MemoryTransport` | integration | `cargo nextest run -p famp` | Covered by EX-01 test |
| CONF-05 | Unsigned message → `RuntimeError::Decode(MissingSignature)` | adversarial | `cargo nextest run -p famp` | `crates/famp/tests/adversarial.rs` |
| CONF-06 | Wrong-key message → `RuntimeError::Decode(SignatureInvalid)` | adversarial | `cargo nextest run -p famp` | `crates/famp/tests/adversarial.rs` |
| CONF-07 | Canonical-divergence message → `RuntimeError::CanonicalDivergence` | adversarial | `cargo nextest run -p famp` | `crates/famp/tests/adversarial.rs` |

**Additional property-based tests:**

| Property | Test Type | Automated Command | Notes |
|----------|-----------|-------------------|-------|
| Keyring round-trip: `proptest` on arbitrary principal+key pairs | property | `cargo nextest run -p famp-keyring` | Generates random principal strings + valid keys, save+load, assert equal |
| `MemoryTransport` multi-principal: register N principals, all send/recv correctly | integration | `cargo nextest run -p famp-transport` | ≥3 principals to catch address routing bugs |
| `pin_tofu` conflict semantics: second pin with same principal rejects | unit | `cargo nextest run -p famp-keyring` | Assert `KeyringError::KeyConflict` |
| Keyring duplicate principal in file: load rejects | unit | `cargo nextest run -p famp-keyring` | Fixture with duplicate principal |
| Keyring duplicate pubkey in file: load rejects | unit | `cargo nextest run -p famp-keyring` | Fixture with duplicate pubkey under different principal |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p famp-transport` or `-p famp-keyring` (whichever crate was changed)
- **Per wave merge:** `cargo nextest run --workspace`
- **Phase gate:** `just ci` green (fmt-check + lint + build + test-canonical-strict + test-crypto + test + test-doc + spec-lint)

### Wave 0 Gaps (new files needed before implementation)

- [ ] `crates/famp-keyring/` — new crate scaffold (Cargo.toml, src/lib.rs, src/error.rs, src/file_format.rs)
- [ ] `crates/famp-keyring/tests/fixtures/two_peers.keyring` — committed round-trip fixture (KEY-02)
- [ ] `crates/famp-keyring/tests/roundtrip.rs` — round-trip test
- [ ] `crates/famp-keyring/tests/keyring.rs` — unit tests for API semantics
- [ ] `crates/famp-transport/src/memory.rs` — `MemoryTransport` implementation (replaces stub)
- [ ] `crates/famp-transport/src/error.rs` — `MemoryTransportError`
- [ ] `crates/famp-transport/tests/memory_transport.rs` — integration tests
- [ ] `crates/famp/src/runtime/` — runtime glue module (mod.rs or similar)
- [ ] `crates/famp/src/runtime/error.rs` — `RuntimeError` enum
- [ ] `crates/famp/tests/adversarial.rs` — CONF-05/06/07 tests
- [ ] `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` — pre-generated CONF-07 fixture bytes
- [ ] `crates/famp/tests/example_happy_path.rs` — subprocess test for EX-01
- [ ] `crates/famp/examples/personal_two_agents.rs` — the example binary

---

## Security Domain

`security_enforcement` is not explicitly set to false in `.planning/config.json`, so this section is required.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Yes (key pinning) | TOFU pinning via `Keyring`; `TrustedVerifyingKey` ingress-vetted newtype; weak-key rejection in `famp_crypto` |
| V3 Session Management | Partial | No session tokens; each message is individually signed. No replay cache in v0.7 (deferred to v0.9) |
| V4 Access Control | Yes | `RuntimeError::UnknownSender` rejects unregistered principals; `RecipientMismatch` rejects envelope addressed to wrong party |
| V5 Input Validation | Yes | `from_slice_strict` for JSON (rejects duplicates); `TrustedVerifyingKey::from_b64url` for pubkeys (strict decode); `Principal::from_str` for identities |
| V6 Cryptography | Yes | `ed25519-dalek` with `verify_strict` (rejects small-order points, non-canonical signatures); domain separation prefix |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Message forgery (sending without valid private key) | Spoofing | `verify_strict` in `AnySignedEnvelope::decode`; TOFU keyring rejects unknown principals |
| Key substitution (pinning wrong key) | Tampering | `pin_tofu` first-pin-wins; `KeyConflict` on subsequent pin attempts; one-to-one pubkey constraint |
| Replay attack | Repudiation | Partially mitigated by `MessageId` uniqueness (UUIDv7); full replay cache deferred to v0.9 |
| Canonical-form ambiguity (CONF-07) | Tampering | Pre-decode canonicalization check in runtime glue (Option A from Open Question #1) |
| Recipient spoofing | Tampering | Envelope `to_principal()` cross-checked against transport `msg.recipient` (D-D5) |
| Unsigned message injection (CONF-05) | Tampering | INV-10 enforced at type level in `famp-envelope`; `MissingSignature` error before any processing |
| Weak-key injection (small-order Edwards points) | Spoofing | `TrustedVerifyingKey::from_bytes` calls `vk.is_weak()` — rejected at keyring ingress |
| Test-util escape hatch in production | Elevation of Privilege | `send_raw_for_test` behind `#[cfg(feature = "test-util")]`; feature NOT in production `[dependencies]` |

---

## Sources

### Primary (HIGH confidence)
- `crates/famp-envelope/src/envelope.rs` — `SignedEnvelope` accessors including `to_principal()`, `from_principal()`, decode pipeline [VERIFIED: read directly 2026-04-13]
- `crates/famp-envelope/src/dispatch.rs` — `AnySignedEnvelope::decode` signature takes `&TrustedVerifyingKey` [VERIFIED: read directly 2026-04-13]
- `crates/famp-envelope/src/error.rs` — `EnvelopeDecodeError` full variant list [VERIFIED: read directly 2026-04-13]
- `crates/famp-crypto/src/keys.rs` — `TrustedVerifyingKey::from_b64url`, `TrustedVerifyingKey::from_bytes`, weak-key check [VERIFIED: read directly 2026-04-13]
- `crates/famp-crypto/src/error.rs` — `CryptoError` variant list [VERIFIED: read directly 2026-04-13]
- `crates/famp-crypto/src/verify.rs` — `verify_value` canonicalizes then calls `verify_strict` [VERIFIED: read directly 2026-04-13]
- `crates/famp-fsm/src/engine.rs` — `TaskFsm::step` signature, legal arrows [VERIFIED: read directly 2026-04-13]
- `crates/famp-fsm/src/input.rs` — `TaskTransitionInput { class, terminal_status }` (no `relation` field in shipped code) [VERIFIED: read directly 2026-04-13]
- `crates/famp-core/src/identity.rs` — `Principal::from_str`, `Display` impl [VERIFIED: read directly 2026-04-13]
- `crates/famp-envelope/src/wire.rs` — `WireEnvelope` has `from: Principal` and `to: Principal` fields, confirming D-D5 [VERIFIED: read directly 2026-04-13]
- `Cargo.toml` (workspace) — dependency versions, `rust-version = "1.87"`, workspace members [VERIFIED: read directly 2026-04-13]
- `crates/famp-transport/src/lib.rs` — Phase 0 stub body, confirms crate exists but is empty [VERIFIED: read directly 2026-04-13]
- `.planning/phases/03-memorytransport-tofu-keyring-same-process-example/03-CONTEXT.md` — all implementation decisions [VERIFIED: read directly 2026-04-13]

### Secondary (MEDIUM confidence)
- Rust 1.75 stable AFIT (async fn in trait) — documented in Rust reference; workspace pins 1.87 [CITED: blog.rust-lang.org — async fn in traits stable Nov 2023]
- `tokio::sync::mpsc::unbounded_channel` behavior (non-Clone receiver, `Arc<Mutex<>>` patterns) — standard tokio docs [ASSUMED: consistent with tokio 1.51.1 behavior; no version regression expected]

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all library APIs verified from actual codebase source files
- Architecture: HIGH — transport trait shape, keyring API, and runtime glue loop all verified against CONTEXT.md decisions and existing Phase 1/2 API surface
- Pitfalls: HIGH — CONF-06/07 distinction (Pitfall 2) and receiver ownership (Pitfall 1) identified from direct code inspection of verify_value and mpsc documentation
- Open Questions: MEDIUM — two questions have recommended resolutions (Option A for CONF-07, separate sender/receiver maps) but require planner confirmation

**Research date:** 2026-04-13
**Valid until:** 2026-07-13 (stable ecosystem; FAMP substrate crates are locked at workspace versions)
