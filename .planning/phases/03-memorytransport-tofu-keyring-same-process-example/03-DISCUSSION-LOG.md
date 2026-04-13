# Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-13
**Phase:** 03-memorytransport-tofu-keyring-same-process-example
**Areas discussed:** Principal ↔ pubkey binding, Transport trait + MemoryTransport shape, Runtime glue home + adversarial injection, Keyring file format + TOFU semantics, Test hook exposure, Envelope `to` field, Ack handling, Key wrapper type, Keyring crate layout, Channel semantics, CONF-07 construction

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Principal ↔ pubkey binding | KEY-01 says principal = raw 32-byte Ed25519 pubkey, but v0.6 shipped Principal as `agent:<authority>/<name>`. Real contract conflict. | ✓ |
| Transport trait + MemoryTransport shape | async-trait vs AFIT; address type; hub vs per-peer; sig verification location. | ✓ |
| Runtime glue home + adversarial injection | New famp-runtime crate, module in famp-transport, or top crate. Injection mechanism for CONF-05/06/07. | ✓ |
| Keyring file format + TOFU semantics | Line syntax, duplicate handling, TOFU rules, CLI flag merge. | ✓ |

**User's choice:** All four selected.

---

## Principal ↔ Pubkey Binding

**User's directive:** Do NOT redefine `Principal`. Rolling back v0.6 is wrong. KEY-01 wording is the problem, not the type. Correct reading: *"each principal is authenticated by exactly one pinned Ed25519 public key in personal v0.7."* Keyring is `HashMap<Principal, TrustedVerifyingKey>`. Update the requirement wording before implementation.

**Keyring file example proposed by user:**
```
agent:local/alice MCowBQ...   # actual key: raw-key b64url
agent:local/bob   11qYAY...
```

**Notes:** User explicitly flagged this as "not a minor interpretation issue — a real contract conflict."

---

## Transport Trait + MemoryTransport Shape

**User's directive:**
- **Do NOT put signature verification inside the transport trait.** It belongs one layer up in runtime glue, shared across Phase 3 and Phase 4.
- Transport trait shape:
  ```rust
  pub trait Transport {
      type Error;
      type Incoming;
      async fn send(&self, recipient: &Principal, bytes: Vec<u8>) -> Result<(), Self::Error>;
      async fn recv(&self) -> Result<Self::Incoming, Self::Error>;
  }
  ```
  Or with explicit `TransportMessage { sender, recipient, bytes }`.
- Use `Principal` as the route key; no separate address type for personal v0.7.
- MemoryTransport: shared inbox hub keyed by `Principal`, one async channel receiver per principal.
- async-trait vs AFIT: "choose the simplest thing that compiles cleanly with your lint profile." Rust 1.87+ native AFIT likely fine.

**Locked in CONTEXT.md D-C1..D-C7.** Native AFIT chosen (D-C5) given 1.87+ toolchain pin.

---

## Runtime Glue Home + Adversarial Injection

**User's directive:**
- **Do NOT create `famp-runtime` crate.** Another abstraction layer before knowing what wants to stabilize.
- Keep `famp-transport`, `famp-envelope`, `famp-fsm` pure.
- Add orchestration module in `crates/famp/src/` used by examples and tests.
- Runtime glue receives raw bytes from transport, decodes/verifies, looks up sender key in TOFU keyring, extracts FSM input, applies transition, emits next message.
- Adversarial tests: explicit test-only raw injection on `MemoryTransport` via `send_raw_for_test`. Rejected wrapper transport (too much machinery) and envelope constructor bypass (weakens the more important boundary).

**User note on #5 (unscoped by me, added by user):**

> **Sender identity check:** When a message is received, verify both:
> - signature is valid under the pinned key for the claimed sender principal
> - transport recipient matches envelope `to` principal if that field is present in the v0.7 envelope shape
>
> That keeps the runtime honest and prevents the keyring from becoming a weak "just any valid key" check.

**Locked in CONTEXT.md D-D1..D-D8.** D-D5 carries the sender cross-check with a researcher flag to verify whether Phase 1 envelope carries the `to` field.

---

## Keyring File Format + TOFU Semantics

**User's directive:**
```
# principal SP base64url-unpadded-ed25519-pubkey
agent:local/alice 11qYAY7...
agent:local/bob   nWGxne_...
```
- One entry per line, separator = one or more ASCII spaces/tabs.
- `#` comments only at line start; blank lines allowed.
- No inline trailing comments for v0.7.
- Duplicate principal entries → reject.
- Same pubkey under two different principals → reject.
- Invalid principal parse / bad base64url / wrong key length → reject.

**TOFU semantics:**
- First sight of unknown principal + valid key: pin if caller explicitly persists.
- Known principal seen with different key: ALWAYS reject. Never auto-overwrite in v0.7.

**CLI flag merge:**
- File is base; `--peer <principal>=<b64url>` may add entries.
- CLI entry conflicting with file → reject (no precedence games).

**Locked in CONTEXT.md D-B1..D-B6.**

---

## Test Hook Exposure

| Option | Description | Selected |
|--------|-------------|----------|
| test-util feature flag | `famp-transport` exposes `send_raw_for_test` under `#[cfg(feature = "test-util")]`; dev-deps enable it in `crates/famp`. | ✓ |
| pub(crate) + tests in same crate | Keep the hook `#[cfg(test)]` inside `famp-transport`; put adversarial tests inside `crates/famp-transport/tests/`. | |
| Always-pub unsafe-looking name | Expose as `send_raw_unchecked()` permanently with doc warning. | |

**User's choice:** test-util feature flag. Locked in D-D6.

---

## Envelope `to` Field (Decision #5 follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Flag for researcher to verify | Write D-5 conditionally; researcher checks Phase 1 output for a recipient field before planner locks in the cross-check. | ✓ |
| Lock as required, add to envelope if missing | Mandate cross-check; retroactively modify Phase 1 body schemas. | |
| Defer cross-check to v0.8 | Drop D-5 for v0.7; keyring-pinned signature alone suffices. | |

**User's choice:** Flag for researcher to verify. Locked in D-D5 with a three-branch outcome tree for researcher to resolve.

---

## Ack Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Transport-only, no FSM touch | `ack` is signed/verified like any other message but does NOT call `TaskFsm::step`. Runtime glue logs/traces only. Consistent with Phase 2's 4 legal FSM arrows. | ✓ |
| ack is FSM no-op with explicit arm | Add a TaskTransitionInput for ack that returns current state unchanged; requires reopening Phase 2. | |
| Drop ack from EX-01 | Example runs request→commit→deliver only; diverges from roadmap SC#3. | |

**User's choice:** Transport-only, no FSM touch. Locked in D-D4.

---

## Key Wrapper Type

| Option | Description | Selected |
|--------|-------------|----------|
| Newtype `TrustedVerifyingKey` | `pub struct TrustedVerifyingKey(VerifyingKey)` — only the keyring boundary constructs it; production code cannot reach around it to store untrusted keys. | ✓ |
| Raw `VerifyingKey` | `HashMap<Principal, VerifyingKey>`; trust level lives in the keyring container, not the value. | |

**User's choice:** Newtype. Locked in D-A2.

---

## Keyring Crate Layout

| Option | Description | Selected |
|--------|-------------|----------|
| New `famp-keyring` crate | Dedicated crate depending on `famp-core` + `famp-crypto`. Clean layering; top crate composes. | ✓ |
| Module inside `crates/famp/src/` | Fewer crates, but Phase 4 HTTP middleware (separate crate) inverts layering to reach the keyring. | |
| Module inside `famp-transport` | Couples transport to trust store — explicitly rejected earlier in the discussion. | |

**User's choice:** New `famp-keyring` crate. Locked in D-D2.

---

## MemoryTransport Channel Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Unbounded `tokio::sync::mpsc` | No backpressure; same-process happy path; matches ~50 LoC budget. | ✓ |
| Bounded with fixed capacity | Adds backpressure, more realistic for HTTP, but complicates the trace and blows the LoC budget. | |

**User's choice:** Unbounded. Locked in D-C4.

---

## CONF-07 Canonical Divergence Construction

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-crafted JSON with non-canonical ordering + valid signature over a DIFFERENT canonical form | Committed fixture bytes; exercises the real re-canonicalize-then-verify code path; distinguishable from CONF-06. | ✓ |
| Re-sign after whitespace mutation | Signature becomes invalid — indistinguishable from CONF-06 (wrong key). | |
| Defer to Phase 4 HTTP | Contradicts roadmap SC#4, which lists CONF-07 on MemoryTransport. | |

**User's choice:** Hand-crafted fixture. Locked in D-D7.

---

## Claude's Discretion

- Exact internal module layouts for `famp-keyring/src/`, `crates/famp/src/runtime/`.
- Exact function names for the envelope→FSM adapter.
- Trace string layout in the example binary (as long as integration test can assert on it).
- Exact `Cargo.toml` `[features]` wording for `test-util`.
- Whether `Keyring::load_from_file` collects warnings or hard-errors on first issue.

## Deferred Ideas

- Key rotation / multi-key principals (v0.9+)
- Agent Card distribution (v0.8)
- Pluggable `TrustStore` trait (v0.8+)
- `dyn Transport` support (add when a concrete caller needs it)
- Inline trailing comments in keyring file format
- Retroactive envelope `to` field addition (belongs in v0.8 if researcher finds it missing)
- `stateright` model check over runtime glue (v0.14)
- `famp keygen` / CLI subcommands (v0.8+)
